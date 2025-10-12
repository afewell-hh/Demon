use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use envelope::{
    Diagnostic, DiagnosticLevel, DurationMetrics, Metrics, Provenance, ResultEnvelope, SourceInfo,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::time::Instant;
use tempfile::TempDir;
use thiserror::Error;

type Envelope = ResultEnvelope<JsonValue>;

/// Configuration for executing a containerized capsule invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerExecConfig {
    pub image_digest: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub working_dir: Option<String>,
    pub envelope_path: String,
    #[serde(default)]
    pub capsule_name: Option<String>,
    #[serde(default)]
    pub app_pack_dir: Option<PathBuf>,
    #[serde(default)]
    pub artifacts_dir: Option<PathBuf>,
}

impl ContainerExecConfig {
    pub fn validate(&self) -> Result<()> {
        if !self.image_digest.contains("@sha256:") {
            anyhow::bail!(
                "Container image must be digest-pinned (expected '@sha256:' in reference)"
            );
        }

        if self.command.is_empty() {
            anyhow::bail!("Container command cannot be empty");
        }

        if self.envelope_path.trim().is_empty() {
            anyhow::bail!("Envelope path cannot be empty");
        }

        if !self.envelope_path.starts_with('/') {
            anyhow::bail!("Envelope path '{}' must be absolute", self.envelope_path);
        }

        // Enforce workspace boundary: result envelope must live under /workspace/.artifacts
        if !self.envelope_path.starts_with("/workspace/.artifacts/")
            && self.envelope_path != "/workspace/.artifacts"
        {
            anyhow::bail!(
                "Envelope path '{}' must live under /workspace/.artifacts",
                self.envelope_path
            );
        }

        if let Some(dir) = &self.app_pack_dir {
            if !dir.is_absolute() {
                anyhow::bail!(
                    "App Pack directory '{}' must be an absolute path",
                    dir.display()
                );
            }

            if !dir.exists() {
                anyhow::bail!("App Pack directory '{}' does not exist", dir.display());
            }
        }

        if let Some(dir) = &self.artifacts_dir {
            if !dir.is_absolute() {
                anyhow::bail!(
                    "Artifacts directory '{}' must be an absolute path",
                    dir.display()
                );
            }
            if !dir.exists() {
                fs::create_dir_all(dir).with_context(|| {
                    format!("Failed to create artifacts directory '{}'", dir.display())
                })?;
            }

            #[cfg(unix)]
            fs::set_permissions(dir, fs::Permissions::from_mode(0o777)).with_context(|| {
                format!(
                    "Failed to set permissions on artifacts directory '{}'",
                    dir.display()
                )
            })?;
        }

        Ok(())
    }
}

/// Result of running the container execution capsule.
#[derive(Debug, Clone)]
pub struct ContainerExecResult {
    pub envelope: Envelope,
    pub duration_ms: f64,
    pub exit_status: Option<i32>,
}

/// Execute a containerized capsule and return its result envelope.
///
/// This function enforces sandboxing guards, captures stdout/stderr for diagnostics,
/// and validates the emitted envelope. If any step fails, a canonical error envelope
/// is produced with diagnostic context.
pub fn execute(config: &ContainerExecConfig) -> Envelope {
    match execute_internal(config) {
        Ok(mut result) => {
            annotate_success(&mut result, config);
            result.envelope
        }
        Err(error) => build_error_envelope(error, config),
    }
}

fn execute_internal(config: &ContainerExecConfig) -> Result<ContainerExecResult, ExecError> {
    config.validate().map_err(|err| ExecError::InvalidConfig {
        message: err.to_string(),
    })?;

    match detect_runtime_kind() {
        RuntimeKind::Stub => execute_stub(config),
        RuntimeKind::Binary(runtime_bin) => execute_with_runtime(config, runtime_bin),
    }
}

fn execute_stub(config: &ContainerExecConfig) -> Result<ContainerExecResult, ExecError> {
    let stub_path = env::var("DEMON_CONTAINER_EXEC_STUB_ENVELOPE")
        .map(PathBuf::from)
        .map_err(|_| ExecError::Stub {
            message:
                "stub runtime requires DEMON_CONTAINER_EXEC_STUB_ENVELOPE to point to an envelope"
                    .to_string(),
        })?;

    let raw = fs::read(&stub_path).map_err(|err| ExecError::Stub {
        message: format!(
            "Failed to read stub envelope at {}: {}",
            stub_path.display(),
            err
        ),
    })?;

    let mut envelope: Envelope = serde_json::from_slice(&raw).map_err(|err| ExecError::Stub {
        message: format!("Failed to parse stub envelope JSON: {}", err),
    })?;

    if let Err(err) = envelope.validate() {
        return Err(ExecError::Stub {
            message: format!("Stub envelope validation failed: {}", err),
        });
    }

    envelope.diagnostics.push(
        Diagnostic::info(format!(
            "container-exec stub envelope loaded from {}",
            stub_path.display()
        ))
        .with_source("container-exec")
        .with_context(serde_json::json!({
            "mode": "stub",
            "image": config.image_digest,
        })),
    );

    Ok(ContainerExecResult {
        envelope,
        duration_ms: 0.0,
        exit_status: Some(0),
    })
}

fn execute_with_runtime(
    config: &ContainerExecConfig,
    runtime_bin: String,
) -> Result<ContainerExecResult, ExecError> {
    if let Some(artifacts_dir) = &config.artifacts_dir {
        fs::create_dir_all(artifacts_dir).map_err(|err| ExecError::Io {
            message: format!(
                "Failed to create artifacts directory {}: {}",
                artifacts_dir.display(),
                err
            ),
        })?;

        #[cfg(unix)]
        {
            let permissions = fs::Permissions::from_mode(0o777);
            fs::set_permissions(artifacts_dir, permissions).map_err(|err| ExecError::Io {
                message: format!(
                    "Failed to set permissions on artifacts directory {}: {}",
                    artifacts_dir.display(),
                    err
                ),
            })?;
        }
    }

    // Ensure the workspace mount point and, if possible, the container-visible envelope
    // path exist under the App Pack directory. This helps Docker file-level binds succeed
    // even when the parent `/workspace` is bound read-only.
    if let Some(app_pack_dir) = &config.app_pack_dir {
        let artifacts_mp = app_pack_dir.join(".artifacts");
        fs::create_dir_all(&artifacts_mp).map_err(|err| ExecError::Io {
            message: format!(
                "Failed to ensure App Pack artifacts mount point {}: {}",
                artifacts_mp.display(),
                err
            ),
        })?;

        #[cfg(unix)]
        {
            // Make the mount point permissive so any container UID can traverse it
            fs::set_permissions(&artifacts_mp, fs::Permissions::from_mode(0o777)).map_err(
                |err| ExecError::Io {
                    message: format!(
                        "Failed to set permissions on artifacts mount point {}: {}",
                        artifacts_mp.display(),
                        err
                    ),
                },
            )?;
        }

        // If the envelope path is under /workspace/.artifacts, create a placeholder under
        // the App Pack so the container-side target exists before mount wiring.
        if let Some(rel) = config
            .envelope_path
            .strip_prefix("/workspace/.artifacts/")
            .filter(|s| !s.is_empty())
        {
            let app_side_path = artifacts_mp.join(rel);
            if let Some(parent) = app_side_path.parent() {
                fs::create_dir_all(parent).map_err(|err| ExecError::Io {
                    message: format!(
                        "Failed to create App Pack envelope parent {}: {}",
                        parent.display(),
                        err
                    ),
                })?;
                #[cfg(unix)]
                fs::set_permissions(parent, fs::Permissions::from_mode(0o777)).map_err(|err| {
                    ExecError::Io {
                        message: format!(
                            "Failed to set permissions on App Pack envelope parent {}: {}",
                            parent.display(),
                            err
                        ),
                    }
                })?;
            }
            // Best-effort placeholder; ignore errors here since the file-level bind
            // below will still point the container at the host-side placeholder.
            let _ = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&app_side_path);
            #[cfg(unix)]
            let _ = fs::set_permissions(&app_side_path, fs::Permissions::from_mode(0o666));
        }
    }

    let temp_dir = TempDir::new().map_err(|err| ExecError::Io {
        message: format!("Failed to create temp directory: {}", err),
    })?;

    let artifacts_dir = config.artifacts_dir.as_ref();
    let mount = EnvelopeMount::prepare(
        &config.envelope_path,
        temp_dir.path(),
        artifacts_dir.map(Path::new),
    )?;

    let host_target = mount.container_root.clone();
    if let Some(root) = mount.host_root() {
        fs::create_dir_all(root).map_err(|err| ExecError::Io {
            message: format!(
                "Failed to create host mount directory {}: {}",
                root.display(),
                err
            ),
        })?;

        #[cfg(unix)]
        {
            fs::set_permissions(root, fs::Permissions::from_mode(0o777)).map_err(|err| {
                ExecError::Io {
                    message: format!(
                        "Failed to set permissions on mount directory {}: {}",
                        root.display(),
                        err
                    ),
                }
            })?;
        }
    }

    if let Some(parent) = mount.host_envelope_path.parent() {
        fs::create_dir_all(parent).map_err(|err| ExecError::Io {
            message: format!(
                "Failed to create envelope parent directory {}: {}",
                parent.display(),
                err
            ),
        })?;

        #[cfg(unix)]
        {
            fs::set_permissions(parent, fs::Permissions::from_mode(0o777)).map_err(|err| {
                ExecError::Io {
                    message: format!(
                        "Failed to set permissions on envelope parent directory {}: {}",
                        parent.display(),
                        err
                    ),
                }
            })?;
        }
    }

    ensure_envelope_placeholder(&mount.host_envelope_path)?;

    let mut command = Command::new(&runtime_bin);
    configure_command(&mut command, config, &mount)?;
    let runtime_cmdline = command_line_string(&command);

    let start = Instant::now();
    let output = command.output().map_err(|err| ExecError::RuntimeSpawn {
        runtime: runtime_bin.clone(),
        source: err,
    })?;
    let duration = start.elapsed();

    let logs = CommandLogs::from_output(&output);

    let envelope_bytes =
        fs::read(&mount.host_envelope_path).map_err(|err| ExecError::EnvelopeMissing {
            path: mount.host_envelope_path.clone(),
            status: output.status,
            logs: logs.clone(),
            source: err,
        })?;

    let envelope: Envelope =
        serde_json::from_slice(&envelope_bytes).map_err(|err| ExecError::EnvelopeInvalid {
            path: mount.host_envelope_path.clone(),
            status: output.status,
            logs: logs.clone(),
            source: anyhow!(err),
        })?;

    if let Err(err) = envelope.validate() {
        return Err(ExecError::EnvelopeInvalid {
            path: mount.host_envelope_path.clone(),
            status: output.status,
            logs,
            source: err,
        });
    }

    Ok(ContainerExecResult {
        envelope,
        duration_ms: duration.as_secs_f64() * 1000.0,
        exit_status: exit_code(&output.status),
    }
    .tap(|result| {
        annotate_logs(&mut result.envelope, &logs, &host_target, config);
        if debug_enabled() {
            annotate_host_postrun(
                &mut result.envelope,
                &mount.host_envelope_path,
                &runtime_cmdline,
            );
        }
    }))
}

fn annotate_logs(
    envelope: &mut Envelope,
    logs: &CommandLogs,
    container_target: &str,
    config: &ContainerExecConfig,
) {
    if let Some(code) = logs.exit_status {
        let message = format!("container runtime exited with code {}", code);
        let level = if code == 0 {
            DiagnosticLevel::Info
        } else {
            DiagnosticLevel::Warning
        };
        envelope.diagnostics.push(
            Diagnostic::new(level, message)
                .with_source("container-exec")
                .with_context(serde_json::json!({
                    "image": config.image_digest,
                    "mount": container_target,
                })),
        );
    }

    if !logs.stdout.trim().is_empty() {
        envelope.diagnostics.push(
            Diagnostic::info(format!("stdout: {}", truncate(&logs.stdout, 2048)))
                .with_source("container-exec"),
        );
    }

    if !logs.stderr.trim().is_empty() {
        envelope.diagnostics.push(
            Diagnostic::warning(format!("stderr: {}", truncate(&logs.stderr, 2048)))
                .with_source("container-exec"),
        );
    }
}

fn annotate_success(result: &mut ContainerExecResult, config: &ContainerExecConfig) {
    if result.envelope.metrics.is_none() {
        result.envelope.metrics = Some(Metrics {
            duration: Some(DurationMetrics {
                total_ms: Some(result.duration_ms),
                phases: Default::default(),
            }),
            resources: None,
            counters: Default::default(),
            custom: None,
        });
    } else if let Some(metrics) = &mut result.envelope.metrics {
        metrics.duration.get_or_insert(DurationMetrics {
            total_ms: Some(result.duration_ms),
            phases: Default::default(),
        });
    }

    result
        .envelope
        .provenance
        .get_or_insert_with(|| Provenance {
            source: Some(SourceInfo {
                system: "container-exec".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
                instance: config.capsule_name.clone(),
            }),
            timestamp: Some(Utc::now()),
            trace_id: None,
            span_id: None,
            parent_span_id: None,
            chain: vec![],
        });
}

fn configure_command(
    command: &mut Command,
    config: &ContainerExecConfig,
    mount: &EnvelopeMount,
) -> Result<(), ExecError> {
    command.arg("run");
    command.arg("--rm");
    command.arg("--pull").arg("never");
    command.arg("--network").arg("none");
    command.arg("--read-only");
    command.arg("--security-opt").arg("no-new-privileges");
    command.arg("--user").arg(container_user());
    command
        .arg("--tmpfs")
        .arg("/tmp:rw,noexec,nosuid,nodev,size=67108864");
    if let Some(app_dir) = &config.app_pack_dir {
        let app_dir = fs::canonicalize(app_dir).map_err(|err| ExecError::Io {
            message: format!(
                "Failed to canonicalize App Pack directory {}: {}",
                app_dir.display(),
                err
            ),
        })?;
        command.arg("--mount").arg(format!(
            "type=bind,source={},target=/workspace,readonly=true",
            app_dir.display()
        ));
    }

    if let Some(artifacts_dir) = &config.artifacts_dir {
        let artifacts_dir = fs::canonicalize(artifacts_dir).map_err(|err| ExecError::Io {
            message: format!(
                "Failed to canonicalize artifacts directory {}: {}",
                artifacts_dir.display(),
                err
            ),
        })?;
        command.arg("--mount").arg(format!(
            "type=bind,source={},target=/workspace/.artifacts,readonly=false",
            artifacts_dir.display()
        ));
    }

    if let Some(host_root) = mount.host_root() {
        command.arg("--mount").arg(format!(
            "type=bind,source={},target={},readonly=false",
            host_root.display(),
            mount.container_root
        ));
    }

    let mut workdir_set = false;
    if let Some(dir) = &config.working_dir {
        command.arg("--workdir").arg(dir);
        workdir_set = true;
    }

    if !workdir_set && config.app_pack_dir.is_some() {
        command.arg("--workdir").arg("/workspace");
    }

    // Additionally, bind the host envelope file directly to the container target to
    // guarantee writability regardless of parent mount semantics or UID.
    command.arg("--mount").arg(format!(
        "type=bind,source={},target={},readonly=false",
        mount.host_envelope_path.display(),
        config.envelope_path
    ));

    // Ensure capsule receives the enforced envelope path regardless of manifest env.
    command
        .arg("--env")
        .arg(format!("ENVELOPE_PATH={}", config.envelope_path));

    // Pass through DEMON_DEBUG to the container if enabled
    if let Ok(val) = env::var("DEMON_DEBUG") {
        if !val.is_empty() && val != "0" {
            command.arg("--env").arg(format!("DEMON_DEBUG={}", val));
        }
    }

    for (key, value) in &config.env {
        command.arg("--env").arg(format!("{}={}", key, value));
    }

    command.arg("--entrypoint").arg("");
    command.arg(&config.image_digest);

    if debug_enabled() {
        let original = shell_join(&config.command);
        let script = format!(
            "set -e; echo '=== DEMON_DEBUG pre-run ==='; echo uid: $(id -u); echo gid: $(id -g); \
             echo \"ENVELOPE_PATH=${{ENVELOPE_PATH}}\"; p=\"$(dirname \"$ENVELOPE_PATH\")\"; \
             ls -l \"$p\" || true; test -w \"$ENVELOPE_PATH\" || echo 'NOT WRITABLE'; \
             (mount 2>/dev/null || cat /proc/mounts 2>/dev/null) | sed -n '1,120p'; \
             echo '=== DEMON_DEBUG run ==='; {} ; echo '=== DEMON_DEBUG done ===';",
            original
        );
        command.arg("/bin/sh").arg("-c").arg(script);
    } else {
        for part in &config.command {
            command.arg(part);
        }
    }

    // Optional resource limits (Issue #270): cpus, memory, pids-limit
    if let Ok(cpus) = env::var("DEMON_CONTAINER_CPUS") {
        let cpus = cpus.trim();
        if !cpus.is_empty() {
            command.arg("--cpus").arg(cpus);
        }
    }
    if let Ok(mem) = env::var("DEMON_CONTAINER_MEMORY") {
        let mem = mem.trim();
        if !mem.is_empty() {
            command.arg("--memory").arg(mem);
        }
    }
    if let Ok(pids) = env::var("DEMON_CONTAINER_PIDS_LIMIT") {
        let pids = pids.trim();
        if !pids.is_empty() {
            command.arg("--pids-limit").arg(pids);
        }
    }

    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    Ok(())
}

fn container_user() -> String {
    if let Ok(value) = env::var("DEMON_CONTAINER_USER") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    #[cfg(unix)]
    {
        let uid = unsafe { libc::geteuid() };
        let gid = unsafe { libc::getegid() };
        format!("{}:{}", uid, gid)
    }
    #[cfg(not(unix))]
    {
        // Fallback to nobody on non-unix targets when we cannot infer a host UID/GID
        "65534:65534".to_string()
    }
}

fn ensure_envelope_placeholder(path: &Path) -> Result<(), ExecError> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|err| ExecError::Io {
            message: format!(
                "Failed to prepare envelope file {}: {}",
                path.display(),
                err
            ),
        })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, fs::Permissions::from_mode(0o666)).map_err(|err| {
            ExecError::Io {
                message: format!(
                    "Failed to set permissions on envelope file {}: {}",
                    path.display(),
                    err
                ),
            }
        })?;

        // Ensure file is empty for the container to overwrite cleanly.
        file.set_len(0).map_err(|err| ExecError::Io {
            message: format!(
                "Failed to truncate envelope placeholder {}: {}",
                path.display(),
                err
            ),
        })?;
    }

    #[cfg(not(unix))]
    {
        let _ = file; // suppress unused warning
    }

    Ok(())
}

fn exit_code(status: &ExitStatus) -> Option<i32> {
    status.code()
}

fn truncate(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        text.to_string()
    } else {
        let mut truncated = text[..limit].to_string();
        truncated.push_str("… (truncated)");
        truncated
    }
}

fn debug_enabled() -> bool {
    matches!(env::var("DEMON_DEBUG"), Ok(val) if !val.is_empty() && val != "0")
}

fn shell_escape(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    let escaped = arg.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

fn shell_join(args: &[String]) -> String {
    let mut out = String::new();
    let mut first = true;
    for a in args {
        if !first {
            out.push(' ');
        }
        first = false;
        out.push_str(&shell_escape(a));
    }
    out
}

fn command_line_string(cmd: &Command) -> String {
    let mut s = String::new();
    s.push_str(&cmd.get_program().to_string_lossy());
    for a in cmd.get_args() {
        s.push(' ');
        let a = a.to_string_lossy();
        if a.contains(' ') || a.contains('"') || a.contains('\'') {
            s.push_str(&shell_escape(&a));
        } else {
            s.push_str(&a);
        }
    }
    s
}

fn annotate_host_postrun(envelope: &mut Envelope, host_path: &Path, cmdline: &str) {
    // Always include the runtime command line when debugging
    envelope.diagnostics.push(
        Diagnostic::info(format!("runtime command: {}", truncate(cmdline, 1024)))
            .with_source("container-exec"),
    );

    let ls_output = Command::new("/bin/ls")
        .arg("-l")
        .arg(host_path)
        .output()
        .ok()
        .map(|o| CommandLogs::from_output(&o));
    let stat_output = Command::new("/usr/bin/stat")
        .arg(host_path)
        .output()
        .or_else(|_| Command::new("/bin/stat").arg(host_path).output())
        .ok()
        .map(|o| CommandLogs::from_output(&o));

    if let Some(logs) = ls_output {
        envelope.diagnostics.push(
            Diagnostic::info(format!(
                "host ls -l {}:\n{}",
                host_path.display(),
                truncate(&logs.stdout, 1024)
            ))
            .with_source("container-exec"),
        );
    }
    if let Some(logs) = stat_output {
        envelope.diagnostics.push(
            Diagnostic::info(format!(
                "host stat {}:\n{}",
                host_path.display(),
                truncate(&logs.stdout, 1024)
            ))
            .with_source("container-exec"),
        );
    }

    if let Ok(meta) = fs::metadata(host_path) {
        if meta.len() == 0 {
            envelope.diagnostics.push(
                Diagnostic::warning(format!(
                    "envelope size is 0 bytes; runtime command: {}",
                    truncate(cmdline, 1024)
                ))
                .with_source("container-exec"),
            );
        }
    }
}

fn build_error_envelope(error: ExecError, config: &ContainerExecConfig) -> Envelope {
    let message = error.message();
    let mut builder = Envelope::builder()
        .error_with_code(&message, error.code())
        .add_diagnostic(Diagnostic::error(&message).with_source("container-exec"));

    if let Some(logs) = error.logs() {
        if !logs.stdout.trim().is_empty() {
            builder = builder.add_diagnostic(
                Diagnostic::debug(format!("stdout: {}", truncate(&logs.stdout, 2048)))
                    .with_source("container-exec"),
            );
        }

        if !logs.stderr.trim().is_empty() {
            builder = builder.add_diagnostic(
                Diagnostic::warning(format!("stderr: {}", truncate(&logs.stderr, 2048)))
                    .with_source("container-exec"),
            );
        }
    }

    builder
        .with_source_info(
            "container-exec",
            Some(env!("CARGO_PKG_VERSION")),
            config.capsule_name.clone(),
        )
        .add_diagnostic(
            Diagnostic::info("container execution failed")
                .with_source("container-exec")
                .with_context(serde_json::json!({
                    "image": config.image_digest,
                    "command": config.command,
                    "envelopePath": config.envelope_path,
                })),
        )
        .build()
        .unwrap_or_else(|_| Envelope {
            result: envelope::OperationResult::error("Container execution failed"),
            diagnostics: vec![],
            suggestions: vec![],
            metrics: None,
            provenance: None,
        })
}

fn detect_runtime_kind() -> RuntimeKind {
    match env::var("DEMON_CONTAINER_RUNTIME") {
        Ok(val) if val.trim().eq_ignore_ascii_case("stub") => RuntimeKind::Stub,
        Ok(val) if !val.trim().is_empty() => RuntimeKind::Binary(val),
        _ => RuntimeKind::Binary("docker".to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeKind {
    Binary(String),
    Stub,
}

#[derive(Debug, Clone)]
struct CommandLogs {
    stdout: String,
    stderr: String,
    exit_status: Option<i32>,
}

impl CommandLogs {
    fn from_output(output: &Output) -> Self {
        Self {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_status: exit_code(&output.status),
        }
    }
}

#[derive(Debug, Clone)]
struct EnvelopeMount {
    container_root: String,
    host_mount_root: Option<PathBuf>,
    host_envelope_path: PathBuf,
}

impl EnvelopeMount {
    fn prepare(
        envelope_path: &str,
        temp_root: &Path,
        artifacts_dir: Option<&Path>,
    ) -> Result<Self, ExecError> {
        if !envelope_path.starts_with('/') {
            return Err(ExecError::InvalidConfig {
                message: format!("Envelope path '{}' must be absolute", envelope_path),
            });
        }

        let container_path = Path::new(envelope_path);
        let container_parent = container_path
            .parent()
            .ok_or_else(|| ExecError::InvalidConfig {
                message: "Envelope path must include parent directory".to_string(),
            })?;

        if let Some(dir) = artifacts_dir {
            let rel = container_path
                .strip_prefix(Path::new("/workspace/.artifacts"))
                .map_err(|_| ExecError::InvalidConfig {
                    message: format!(
                        "Envelope path '{}' must live under /workspace/.artifacts when artifactsDir is provided",
                        envelope_path
                    ),
                })?;

            let mut sanitized = PathBuf::new();
            for comp in rel.components() {
                match comp {
                    Component::Normal(part) => sanitized.push(part),
                    Component::CurDir => {}
                    Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                        return Err(ExecError::InvalidConfig {
                            message: "Envelope path cannot traverse outside /workspace/.artifacts"
                                .to_string(),
                        })
                    }
                }
            }

            if sanitized.as_os_str().is_empty() {
                return Err(ExecError::InvalidConfig {
                    message: "Envelope path must reference a file under /workspace/.artifacts"
                        .to_string(),
                });
            }

            let host_envelope_path = dir.join(sanitized);

            return Ok(Self {
                container_root: container_parent
                    .to_str()
                    .ok_or_else(|| ExecError::InvalidConfig {
                        message: "Container envelope path contains invalid UTF-8".to_string(),
                    })?
                    .to_string(),
                host_mount_root: None,
                host_envelope_path,
            });
        }

        let trimmed = container_path
            .strip_prefix(Path::new("/"))
            .unwrap_or(container_path);
        let mut components = trimmed.components();

        let first_component = loop {
            match components.next() {
                Some(Component::Normal(part)) => break part.to_os_string(),
                Some(Component::CurDir) => continue,
                Some(Component::ParentDir) => {
                    return Err(ExecError::InvalidConfig {
                        message: "Envelope path cannot traverse outside its root".to_string(),
                    })
                }
                Some(Component::RootDir) | Some(Component::Prefix(_)) => {
                    return Err(ExecError::InvalidConfig {
                        message: "Envelope path must be relative to workspace".to_string(),
                    })
                }
                None => {
                    return Err(ExecError::InvalidConfig {
                        message: "Envelope path missing components".to_string(),
                    })
                }
            }
        };

        let host_mount_root = temp_root.join("mount").join(&first_component);
        let mut host_envelope_path = host_mount_root.clone();
        for comp in components {
            match comp {
                Component::Normal(part) => {
                    let part_str = part.to_string_lossy();
                    host_envelope_path.push(part_str.as_ref());
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    return Err(ExecError::InvalidConfig {
                        message: "Envelope path cannot contain '..' segments".to_string(),
                    })
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(ExecError::InvalidConfig {
                        message: "Envelope path must remain under the workspace".to_string(),
                    })
                }
            }
        }

        let container_root = format!(
            "/{}",
            first_component.to_string_lossy().trim_end_matches('/')
        );

        Ok(Self {
            container_root,
            host_mount_root: Some(host_mount_root),
            host_envelope_path,
        })
    }

    fn host_root(&self) -> Option<&Path> {
        self.host_mount_root.as_deref()
    }
}

#[derive(Debug, Error)]
enum ExecError {
    #[error("Invalid container execution config: {message}")]
    InvalidConfig { message: String },
    #[error("Failed to spawn container runtime {runtime}: {source}")]
    RuntimeSpawn {
        runtime: String,
        source: std::io::Error,
    },
    #[error("Failed to prepare envelope: {message}")]
    Io { message: String },
    #[error("Envelope missing at {path}: {source}")]
    EnvelopeMissing {
        path: PathBuf,
        status: ExitStatus,
        logs: CommandLogs,
        source: std::io::Error,
    },
    #[error("Envelope invalid at {path}: {source}")]
    EnvelopeInvalid {
        path: PathBuf,
        status: ExitStatus,
        logs: CommandLogs,
        source: anyhow::Error,
    },
    #[error("Stub mode error: {message}")]
    Stub { message: String },
}

impl ExecError {
    fn message(&self) -> String {
        match self {
            ExecError::InvalidConfig { message } => message.clone(),
            ExecError::RuntimeSpawn { runtime, source } => {
                format!(
                    "Failed to spawn container runtime '{}': {}",
                    runtime, source
                )
            }
            ExecError::Io { message } => message.clone(),
            ExecError::EnvelopeMissing { path, .. } => {
                format!("Container envelope not found at {}", path.display())
            }
            ExecError::EnvelopeInvalid { path, source, .. } => {
                format!(
                    "Invalid container envelope at {}: {}",
                    path.display(),
                    source
                )
            }
            ExecError::Stub { message } => message.clone(),
        }
    }

    fn code(&self) -> &'static str {
        match self {
            ExecError::InvalidConfig { .. } => "CONTAINER_EXEC_INVALID_CONFIG",
            ExecError::RuntimeSpawn { .. } => "CONTAINER_EXEC_RUNTIME_ERROR",
            ExecError::Io { .. } => "CONTAINER_EXEC_IO_ERROR",
            ExecError::EnvelopeMissing { .. } => "CONTAINER_EXEC_ENVELOPE_MISSING",
            ExecError::EnvelopeInvalid { .. } => "CONTAINER_EXEC_ENVELOPE_INVALID",
            ExecError::Stub { .. } => "CONTAINER_EXEC_STUB_ERROR",
        }
    }

    fn logs(&self) -> Option<&CommandLogs> {
        match self {
            ExecError::EnvelopeMissing { logs, .. } | ExecError::EnvelopeInvalid { logs, .. } => {
                Some(logs)
            }
            _ => None,
        }
    }
}

trait Tap<T> {
    fn tap<F: FnOnce(&mut T)>(self, func: F) -> Self;
}

impl<T> Tap<T> for T {
    fn tap<F: FnOnce(&mut T)>(mut self, func: F) -> Self {
        func(&mut self);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use envelope::OperationResult;
    use std::fs::File;
    use std::io::Write;

    fn sample_envelope() -> Envelope {
        ResultEnvelope {
            result: OperationResult::success(serde_json::json!({"ok": true})),
            diagnostics: vec![],
            suggestions: vec![],
            metrics: None,
            provenance: None,
        }
    }

    #[test]
    fn truncate_limits_output() {
        let long = "a".repeat(3000);
        let truncated = truncate(&long, 1000);
        assert!(truncated.len() > 1000);
        assert!(truncated.ends_with("… (truncated)"));
    }

    #[test]
    fn envelope_mount_builds_host_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mount =
            EnvelopeMount::prepare("/workspace/.artifacts/result.json", temp_dir.path(), None)
                .unwrap();
        assert!(mount
            .host_envelope_path
            .display()
            .to_string()
            .contains("workspace"));
        assert_eq!(mount.container_root, "/workspace");
    }

    #[test]
    fn stub_mode_loads_envelope() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stub.json");
        let envelope = sample_envelope();
        let mut file = File::create(&path).unwrap();
        file.write_all(serde_json::to_vec(&envelope).unwrap().as_slice())
            .unwrap();

        env::set_var("DEMON_CONTAINER_RUNTIME", "stub");
        env::set_var(
            "DEMON_CONTAINER_EXEC_STUB_ENVELOPE",
            path.to_string_lossy().to_string(),
        );

        let config = ContainerExecConfig {
            image_digest: "ghcr.io/example/app@sha256:abcdef".to_string(),
            command: vec!["/bin/true".to_string()],
            env: BTreeMap::new(),
            working_dir: None,
            envelope_path: "/workspace/.artifacts/result.json".to_string(),
            capsule_name: Some("test".to_string()),
            app_pack_dir: None,
            artifacts_dir: None,
        };

        let result = execute(&config);
        assert!(matches!(result.result, OperationResult::Success { .. }));

        env::remove_var("DEMON_CONTAINER_RUNTIME");
        env::remove_var("DEMON_CONTAINER_EXEC_STUB_ENVELOPE");
    }

    #[test]
    fn invalid_config_builds_error_envelope() {
        env::remove_var("DEMON_CONTAINER_RUNTIME");
        let config = ContainerExecConfig {
            image_digest: "not-digest".to_string(),
            command: vec!["run".to_string()],
            env: BTreeMap::new(),
            working_dir: None,
            envelope_path: "/workspace/result.json".to_string(),
            capsule_name: None,
            app_pack_dir: None,
            artifacts_dir: None,
        };

        let result = execute(&config);
        assert!(matches!(result.result, OperationResult::Error { .. }));
    }

    #[test]
    fn configure_command_includes_workspace_and_artifacts_mounts() {
        let base = tempfile::tempdir().unwrap();
        let app_pack_dir = base.path().join("pack");
        let artifacts_dir = base.path().join("artifacts");
        fs::create_dir_all(&app_pack_dir).unwrap();
        fs::create_dir_all(&artifacts_dir).unwrap();

        let config = ContainerExecConfig {
            image_digest: "ghcr.io/example/app@sha256:abcdef".to_string(),
            command: vec!["/bin/true".to_string()],
            env: BTreeMap::new(),
            working_dir: None,
            envelope_path: "/workspace/.artifacts/result.json".to_string(),
            capsule_name: None,
            app_pack_dir: Some(app_pack_dir.clone()),
            artifacts_dir: Some(artifacts_dir.clone()),
        };

        config.validate().unwrap();

        let temp_root = tempfile::tempdir().unwrap();
        let mount = EnvelopeMount::prepare(
            &config.envelope_path,
            temp_root.path(),
            config.artifacts_dir.as_deref(),
        )
        .unwrap();

        let mut command = Command::new("docker");
        configure_command(&mut command, &config, &mount).unwrap();

        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        let workspace_mount = format!(
            "type=bind,source={},target=/workspace,readonly=true",
            fs::canonicalize(&app_pack_dir).unwrap().display()
        );
        let artifacts_mount = format!(
            "type=bind,source={},target=/workspace/.artifacts,readonly=false",
            fs::canonicalize(&artifacts_dir).unwrap().display()
        );
        let envelope_env = format!("ENVELOPE_PATH={}", config.envelope_path);

        assert!(args.contains(&"--mount".to_string()));
        assert!(args.iter().any(|arg| arg == &workspace_mount));
        assert!(args.iter().any(|arg| arg == &artifacts_mount));
        assert!(args.contains(&"--workdir".to_string()));
        assert!(args.contains(&"/workspace".to_string()));
        assert!(args.contains(&envelope_env));
    }

    #[test]
    fn configure_command_clears_entrypoint_and_preserves_command_order() {
        // Ensure debug mode is off so command parts are not wrapped
        env::remove_var("DEMON_DEBUG");

        let base = tempfile::tempdir().unwrap();
        let app_pack_dir = base.path().join("pack");
        let artifacts_dir = base.path().join("artifacts");
        fs::create_dir_all(&app_pack_dir).unwrap();
        fs::create_dir_all(&artifacts_dir).unwrap();

        let config = ContainerExecConfig {
            image_digest: "ghcr.io/example/app@sha256:abcdef".to_string(),
            command: vec![
                "/bin/run".to_string(),
                "-c".to_string(),
                "echo hi".to_string(),
            ],
            env: BTreeMap::new(),
            working_dir: None,
            envelope_path: "/workspace/.artifacts/result.json".to_string(),
            capsule_name: None,
            app_pack_dir: Some(app_pack_dir),
            artifacts_dir: Some(artifacts_dir),
        };

        let tmp = tempfile::tempdir().unwrap();
        let mount = EnvelopeMount::prepare(
            &config.envelope_path,
            tmp.path(),
            config.artifacts_dir.as_deref(),
        )
        .unwrap();

        let mut command = Command::new("docker");
        configure_command(&mut command, &config, &mount).unwrap();

        let args: Vec<String> = command
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();

        // Find --entrypoint "" then image, then our explicit command parts
        let idx_entry = args
            .iter()
            .position(|a| a == "--entrypoint")
            .expect("--entrypoint not present");
        assert_eq!(args.get(idx_entry + 1).map(|s| s.as_str()), Some(""));

        let idx_image = args
            .iter()
            .position(|a| a == &config.image_digest)
            .expect("image digest not present");
        assert!(idx_image > idx_entry, "image must come after --entrypoint");

        // Command parts must immediately follow the image unless debug wrapper is active
        assert_eq!(
            args.get(idx_image + 1).map(|s| s.as_str()),
            Some("/bin/run")
        );
        assert_eq!(args.get(idx_image + 2).map(|s| s.as_str()), Some("-c"));
        assert_eq!(args.get(idx_image + 3).map(|s| s.as_str()), Some("echo hi"));
    }

    #[test]
    fn resource_limits_flags_are_included_when_envs_set() {
        // Set resource limit envs
        env::set_var("DEMON_CONTAINER_CPUS", "0.5");
        env::set_var("DEMON_CONTAINER_MEMORY", "256m");
        env::set_var("DEMON_CONTAINER_PIDS_LIMIT", "128");

        let base = tempfile::tempdir().unwrap();
        let app_pack_dir = base.path().join("pack");
        let artifacts_dir = base.path().join("artifacts");
        fs::create_dir_all(&app_pack_dir).unwrap();
        fs::create_dir_all(&artifacts_dir).unwrap();

        let config = ContainerExecConfig {
            image_digest: "ghcr.io/example/app@sha256:abcdef".to_string(),
            command: vec!["/bin/true".to_string()],
            env: BTreeMap::new(),
            working_dir: None,
            envelope_path: "/workspace/.artifacts/result.json".to_string(),
            capsule_name: None,
            app_pack_dir: Some(app_pack_dir),
            artifacts_dir: Some(artifacts_dir),
        };

        let tmp = tempfile::tempdir().unwrap();
        let mount = EnvelopeMount::prepare(
            &config.envelope_path,
            tmp.path(),
            config.artifacts_dir.as_deref(),
        )
        .unwrap();

        let mut command = Command::new("docker");
        configure_command(&mut command, &config, &mount).unwrap();
        let args: Vec<String> = command
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();

        assert!(args.contains(&"--cpus".to_string()));
        assert!(args.contains(&"0.5".to_string()));
        assert!(args.contains(&"--memory".to_string()));
        assert!(args.contains(&"256m".to_string()));
        assert!(args.contains(&"--pids-limit".to_string()));
        assert!(args.contains(&"128".to_string()));

        // Cleanup env vars
        env::remove_var("DEMON_CONTAINER_CPUS");
        env::remove_var("DEMON_CONTAINER_MEMORY");
        env::remove_var("DEMON_CONTAINER_PIDS_LIMIT");
    }

    #[test]
    fn envelope_path_rejects_parent_dir_when_artifacts_dir_set() {
        let temp_root = tempfile::tempdir().unwrap();
        let artifacts_dir = tempfile::tempdir().unwrap();

        let result = EnvelopeMount::prepare(
            "/workspace/.artifacts/../evil.json",
            temp_root.path(),
            Some(artifacts_dir.path()),
        );

        assert!(matches!(result, Err(ExecError::InvalidConfig { .. })));
    }
}
