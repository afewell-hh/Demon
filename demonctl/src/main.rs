use anyhow::Result;
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::{fmt, EnvFilter};

mod k8s_bootstrap;

const MANIFEST_FILES: [&str; 5] = [
    "namespace.yaml",
    "nats.yaml",
    "runtime.yaml",
    "engine.yaml",
    "operate-ui.yaml",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BootstrapExecutionMode {
    Full,
    ApplyOnly,
}

impl BootstrapExecutionMode {
    fn from_env() -> Self {
        match std::env::var("DEMONCTL_K8S_BOOTSTRAP_EXECUTION")
            .unwrap_or_default()
            .as_str()
        {
            "apply-only" => Self::ApplyOnly,
            _ => Self::Full,
        }
    }

    fn is_apply_only(self) -> bool {
        matches!(self, Self::ApplyOnly)
    }
}

fn resolve_command_executor() -> Box<dyn k8s_bootstrap::CommandExecutor> {
    let mode = std::env::var("DEMONCTL_K8S_EXECUTOR").unwrap_or_default();
    match mode.as_str() {
        "simulate-success" => {
            let stdout = std::env::var("DEMONCTL_K8S_EXECUTOR_STDOUT")
                .unwrap_or_else(|_| "kubectl apply - simulated success".to_string());
            Box::new(k8s_bootstrap::SimulatedCommandExecutor::success(stdout))
        }
        "simulate-failure" => {
            let stderr = std::env::var("DEMONCTL_K8S_EXECUTOR_STDERR")
                .unwrap_or_else(|_| "kubectl apply failed - simulated".to_string());
            Box::new(k8s_bootstrap::SimulatedCommandExecutor::failure(stderr))
        }
        _ => Box::new(k8s_bootstrap::SystemCommandExecutor),
    }
}

#[derive(ValueEnum, Clone, Debug)]
enum ProviderType {
    /// Environment file provider (default)
    Envfile,
    /// Vault stub provider
    Vault,
}

#[derive(Parser)]
#[command(name = "demonctl", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a ritual from a YAML file
    Run {
        /// Path to ritual YAML
        #[arg(value_name = "FILE")]
        file: String,
        #[arg(long, default_value = "false")]
        replay: bool,
        /// Save result envelope to result.json
        #[arg(long)]
        save: bool,
        /// Output directory for saved files (default: current directory)
        #[arg(long, value_name = "DIR")]
        output_dir: Option<PathBuf>,
    },
    /// Contract management commands
    Contracts {
        #[command(subcommand)]
        cmd: ContractsCommands,
    },
    /// Manage secrets for capsules
    Secrets {
        #[command(subcommand)]
        cmd: SecretsCommands,
    },
    /// Bootstrap Demon prerequisites (self-host v0)
    Bootstrap {
        /// Profile: local-dev (default) or remote-nats
        #[arg(long, value_enum, default_value_t = ProfileArg::LocalDev)]
        profile: ProfileArg,

        /// Ensure stream exists
        #[arg(long, action = ArgAction::SetTrue)]
        ensure_stream: bool,
        /// Seed minimal preview events
        #[arg(long, action = ArgAction::SetTrue)]
        seed: bool,
        /// Verify Operate UI readiness
        #[arg(long, action = ArgAction::SetTrue)]
        verify: bool,

        /// Ritual id used for seeding (default: preview)
        #[arg(long, default_value = "preview")]
        ritual_id: String,

        /// Optional bundle file (YAML)
        #[arg(long)]
        bundle: Option<String>,

        /// Optional overrides (flags > bundle > env)
        #[arg(long)]
        nats_url: Option<String>,
        #[arg(long)]
        stream_name: Option<String>,
        #[arg(long)]
        ui_base_url: Option<String>,

        /// Verify only (resolve + provenance check; no NATS/seed/verify-UI phases)
        #[arg(long, action = ArgAction::SetTrue)]
        verify_only: bool,
    },
    /// Kubernetes bootstrapper commands
    K8sBootstrap {
        #[command(subcommand)]
        cmd: K8sBootstrapCommands,
    },
    /// Print version and exit
    Version,
}

#[derive(Subcommand)]
enum K8sBootstrapCommands {
    /// Bootstrap a Kubernetes cluster with Demon
    Bootstrap {
        /// Path to bootstrap configuration YAML file
        #[arg(long, short, value_name = "FILE")]
        config: String,
        /// Perform validation only, don't execute
        #[arg(long)]
        dry_run: bool,
        /// Enable verbose output
        #[arg(long, short)]
        verbose: bool,
    },
}

#[derive(Subcommand)]
enum SecretsCommands {
    /// Set a secret value
    Set {
        /// Scope and key in format: scope/key
        #[arg(value_name = "SCOPE/KEY")]
        key_path: String,
        /// Secret value (use --from-env to read from environment variable)
        #[arg(
            value_name = "VALUE",
            conflicts_with = "from_env",
            required_unless_present_any = ["from_env", "stdin"]
        )]
        value: Option<String>,
        /// Read value from environment variable
        #[arg(long, conflicts_with = "value")]
        from_env: Option<String>,
        /// Read value from stdin
        #[arg(long, conflicts_with_all = &["value", "from_env"])]
        stdin: bool,
        /// Path to secrets file (defaults to CONFIG_SECRETS_FILE or .demon/secrets.json)
        #[arg(long)]
        secrets_file: Option<String>,
        /// Secret provider to use (envfile or vault)
        #[arg(long, value_enum, default_value_t = ProviderType::Envfile)]
        provider: ProviderType,
    },
    /// Get a secret value
    Get {
        /// Scope and key in format: scope/key
        #[arg(value_name = "SCOPE/KEY")]
        key_path: String,
        /// Output raw value without redaction
        #[arg(long)]
        raw: bool,
        /// Path to secrets file (defaults to CONFIG_SECRETS_FILE or .demon/secrets.json)
        #[arg(long)]
        secrets_file: Option<String>,
        /// Secret provider to use (envfile or vault)
        #[arg(long, value_enum, default_value_t = ProviderType::Envfile)]
        provider: ProviderType,
    },
    /// List secrets
    List {
        /// Filter by scope
        #[arg(long)]
        scope: Option<String>,
        /// Path to secrets file (defaults to CONFIG_SECRETS_FILE or .demon/secrets.json)
        #[arg(long)]
        secrets_file: Option<String>,
        /// Secret provider to use (envfile or vault)
        #[arg(long, value_enum, default_value_t = ProviderType::Envfile)]
        provider: ProviderType,
    },
    /// Delete a secret
    Delete {
        /// Scope and key in format: scope/key
        #[arg(value_name = "SCOPE/KEY")]
        key_path: String,
        /// Path to secrets file (defaults to CONFIG_SECRETS_FILE or .demon/secrets.json)
        #[arg(long)]
        secrets_file: Option<String>,
        /// Secret provider to use (envfile or vault)
        #[arg(long, value_enum, default_value_t = ProviderType::Envfile)]
        provider: ProviderType,
    },
}

#[derive(Subcommand)]
enum ContractsCommands {
    /// Validate an envelope against the result envelope schema
    ValidateEnvelope {
        /// Path to envelope JSON file (use --stdin to read from stdin)
        #[arg(value_name = "FILE", conflicts_with = "stdin")]
        file: Option<String>,
        /// Read envelope from stdin
        #[arg(long, conflicts_with = "file")]
        stdin: bool,
        /// Use remote registry endpoint instead of local validation
        #[arg(long)]
        remote: bool,
        /// Registry endpoint URL (defaults to http://localhost:8090)
        #[arg(long, default_value = "http://localhost:8090")]
        registry_endpoint: String,
        /// Validate all result.json files in directory (bulk mode)
        #[arg(long, value_name = "DIR", conflicts_with_all = &["file", "stdin"])]
        bulk: Option<PathBuf>,
    },
    /// Validate a config file against a capsule's config schema
    ValidateConfig {
        /// Path to config JSON file (use --stdin to read from stdin)
        #[arg(value_name = "FILE", conflicts_with = "stdin")]
        file: Option<String>,
        /// Read config from stdin
        #[arg(long, conflicts_with = "file")]
        stdin: bool,
        /// Capsule name for schema selection (auto-detected from filename if not provided)
        #[arg(long)]
        schema: Option<String>,
        /// Path to secrets file for resolving secret:// URIs
        #[arg(long)]
        secrets_file: Option<String>,
    },
    /// Export contracts bundle with schemas and WIT definitions
    Bundle {
        /// Output format (json, summary)
        #[arg(long, default_value = "summary")]
        format: String,
        /// Include WIT definitions in the bundle
        #[arg(long)]
        include_wit: bool,
    },
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ProfileArg {
    #[value(name = "local-dev")]
    LocalDev,
    #[value(name = "remote-nats")]
    RemoteNats,
}

impl From<ProfileArg> for bootstrapper_demonctl::Profile {
    fn from(p: ProfileArg) -> Self {
        match p {
            ProfileArg::LocalDev => bootstrapper_demonctl::Profile::LocalDev,
            ProfileArg::RemoteNats => bootstrapper_demonctl::Profile::RemoteNats,
        }
    }
}

fn init_tracing() {
    let _ = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .try_init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();

    match cli.cmd {
        Commands::Run {
            file,
            replay: _,
            save,
            output_dir,
        } => {
            let mut engine = engine::rituals::Engine::new();

            if save {
                // Use the new method that returns the result for saving
                match engine.run_from_file_with_result(&file).await {
                    Ok(result_event) => {
                        // Still print to stdout for visibility
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&result_event)
                                .unwrap_or_else(|_| "Failed to serialize result".to_string())
                        );

                        // Save the envelope
                        if let Err(e) = save_result_envelope(&result_event, &output_dir) {
                            eprintln!("Error saving result envelope: {:?}", e);
                            std::process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error running ritual: {:?}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Use the original method that prints directly
                if let Err(e) = engine.run_from_file(&file).await {
                    eprintln!("Error running ritual: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Bootstrap {
            profile,
            ensure_stream,
            seed,
            verify,
            ritual_id,
            bundle,
            nats_url,
            stream_name,
            ui_base_url,
            verify_only,
        } => {
            run_bootstrap(
                profile,
                ensure_stream,
                seed,
                verify,
                ritual_id,
                bundle,
                nats_url,
                stream_name,
                ui_base_url,
                verify_only,
            )
            .await?;
        }
        Commands::Contracts { cmd } => {
            handle_contracts_command(cmd).await?;
        }
        Commands::K8sBootstrap { cmd } => {
            handle_k8s_bootstrap_command(cmd).await?;
        }
        Commands::Secrets { cmd } => {
            handle_secrets_command(cmd)?;
        }
        Commands::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_bootstrap(
    profile: ProfileArg,
    ensure_stream: bool,
    seed: bool,
    verify: bool,
    ritual_id: String,
    bundle: Option<String>,
    nats_url: Option<String>,
    stream_name: Option<String>,
    ui_base_url: Option<String>,
    verify_only: bool,
) -> Result<()> {
    // Only initialize tracing if not already initialized
    let _ = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .try_init();

    let _cfg = bootstrapper_demonctl::BootstrapConfig {
        profile: profile.into(),
        ..Default::default()
    };

    // Log deprecation if DEMON_RITUAL_EVENTS is set and RITUAL_STREAM_NAME is not
    if std::env::var("RITUAL_STREAM_NAME").is_err() && std::env::var("DEMON_RITUAL_EVENTS").is_ok()
    {
        tracing::warn!("[deprecation] using DEMON_RITUAL_EVENTS; set RITUAL_STREAM_NAME instead");
    }

    // Resolve bundle: explicit bundle arg > profile default > none
    let effective_bundle = bundle
        .clone()
        .or_else(|| bootstrapper_demonctl::get_default_bundle_for_profile(&profile.into()));

    // Don't pass lib:// URIs to compute_effective_config - they need resolution first
    let bundle_for_config = effective_bundle
        .as_deref()
        .filter(|uri| !uri.starts_with("lib://"));
    let (cfg, provenance) = bootstrapper_demonctl::compute_effective_config(
        bundle_for_config.map(std::path::Path::new),
        nats_url.as_deref(),
        stream_name.as_deref(),
        None, // subjects - not in CLI yet
        ui_base_url.as_deref(),
    )?;

    println!(
        "{}",
        serde_json::json!({
            "phase":"config",
            "effective":{
                "nats_url": cfg.nats_url,
                "stream_name": cfg.stream_name,
                "subjects": cfg.subjects,
                "dedupe": cfg.dedupe_window_secs,
                "ui_url": cfg.ui_url,
            },
            "provenance": provenance
        })
    );

    if verify_only {
        if let Some(uri) = effective_bundle.as_deref() {
            if uri.starts_with("lib://local/") {
                let mut idx_path = std::path::PathBuf::from("bootstrapper/library/index.json");
                if !idx_path.exists() {
                    for prefix in ["..", "../..", "../../.."].iter() {
                        let p =
                            std::path::Path::new(prefix).join("bootstrapper/library/index.json");
                        if p.exists() {
                            idx_path = p;
                            break;
                        }
                    }
                }
                let resolved = bootstrapper_demonctl::libindex::resolve_local(uri, &idx_path)?;
                println!(
                    "{}",
                    serde_json::json!({
                        "phase":"resolve",
                        "uri": uri,
                        "provider": resolved.provider,
                        "name": resolved.name,
                        "version": resolved.version,
                        "path": resolved.path
                    })
                );
                let vr = bootstrapper_demonctl::provenance::verify_provenance(
                    &resolved.path,
                    &resolved.pub_key_id,
                    &resolved.digest_sha256,
                    &resolved.sig_ed25519,
                )?;
                if vr.signature_ok {
                    println!(
                        "{}",
                        serde_json::json!({
                            "phase":"verify",
                            "bundle": {"name": resolved.name, "version": resolved.version},
                            "digest": vr.digest_hex,
                            "signature": "ok",
                            "pubKeyId": resolved.pub_key_id
                        })
                    );
                } else {
                    println!(
                        "{}",
                        serde_json::json!({
                            "phase":"verify",
                            "bundle": {"name": resolved.name, "version": resolved.version},
                            "digest": vr.digest_hex,
                            "signature": "failed",
                            "reason": vr.reason.unwrap_or_else(|| "unknown".to_string()),
                            "pubKeyId": resolved.pub_key_id
                        })
                    );
                    anyhow::bail!("signature verification failed");
                }
                return Ok(());
            }
        }
        anyhow::bail!("--verify-only requires --bundle lib://local/... URI");
    }

    if !(ensure_stream || seed || verify) {
        // default: run all
        run_all(&cfg, &ritual_id, effective_bundle.as_deref()).await
    } else {
        run_some(&cfg, ensure_stream, seed, verify, &ritual_id).await
    }
}

async fn run_all(
    cfg: &bootstrapper_demonctl::BootstrapConfig,
    ritual: &str,
    bundle_uri: Option<&str>,
) -> Result<()> {
    let stream = bootstrapper_demonctl::ensure_stream(cfg).await?;
    info!(name=%stream.cached_info().config.name, "ensure_stream: ok");
    let client = async_nats::connect(&cfg.nats_url).await?;
    let js = async_nats::jetstream::new(client);
    if let Some(uri) = bundle_uri {
        // Resolve the URI if it's a lib:// URI
        let bundle_path = if uri.starts_with("lib://local/") {
            let mut idx_path = std::path::PathBuf::from("bootstrapper/library/index.json");
            if !idx_path.exists() {
                for prefix in ["..", "../..", "../../.."].iter() {
                    let p = std::path::Path::new(prefix).join("bootstrapper/library/index.json");
                    if p.exists() {
                        idx_path = p;
                        break;
                    }
                }
            }
            let resolved = bootstrapper_demonctl::libindex::resolve_local(uri, &idx_path)?;
            resolved.path
        } else {
            std::path::PathBuf::from(uri)
        };
        let b = bootstrapper_demonctl::bundle::load_bundle(&bundle_path)?;
        let b_json = serde_json::to_value(&b)?;
        bootstrapper_demonctl::seed_from_bundle(&js, &b_json, &cfg.stream_name, &cfg.ui_url)
            .await?;
        let token = b
            .operate_ui
            .admin_token
            .or_else(|| std::env::var("ADMIN_TOKEN").ok());
        bootstrapper_demonctl::verify_ui_with_token(&cfg.ui_url, token.as_deref()).await?;
    } else {
        bootstrapper_demonctl::seed_preview_min(&js, ritual, &cfg.ui_url).await?;
        bootstrapper_demonctl::verify_ui_with_token(
            &cfg.ui_url,
            std::env::var("ADMIN_TOKEN").ok().as_deref(),
        )
        .await?;
    }
    info!("seed: ok");
    info!("verify: ok");
    info!("done: all checks passed");
    Ok(())
}

async fn run_some(
    cfg: &bootstrapper_demonctl::BootstrapConfig,
    ensure_stream: bool,
    seed: bool,
    verify: bool,
    ritual_id: &str,
) -> Result<()> {
    if ensure_stream {
        let stream = bootstrapper_demonctl::ensure_stream(cfg).await?;
        info!(name=%stream.cached_info().config.name, "ensure_stream: ok");
    }
    if seed {
        let client = async_nats::connect(&cfg.nats_url).await?;
        let js = async_nats::jetstream::new(client);
        bootstrapper_demonctl::seed_preview_min(&js, ritual_id, &cfg.ui_url).await?;
        info!("seed: ok");
    }
    if verify {
        bootstrapper_demonctl::verify_ui_with_token(
            &cfg.ui_url,
            std::env::var("ADMIN_TOKEN").ok().as_deref(),
        )
        .await?;
        info!("verify: ok");
    }
    info!("done");
    Ok(())
}

/// Save the result envelope from a ritual completion event to result.json
fn save_result_envelope(
    result_event: &serde_json::Value,
    output_dir: &Option<PathBuf>,
) -> Result<()> {
    use envelope::EnvelopeValidator;

    // Extract the envelope from the "outputs" field
    let envelope_value = result_event
        .get("outputs")
        .ok_or_else(|| anyhow::anyhow!("No outputs field found in result event"))?;

    // Validate the envelope against the schema
    let validator = EnvelopeValidator::new()?;
    if let Err(e) = validator.validate_json(envelope_value) {
        eprintln!("Warning: Result envelope validation failed: {}", e);
        eprintln!("Continuing to save the result, but it may not conform to the expected schema.");
    }

    // Determine output directory
    let dir = output_dir
        .as_ref()
        .map(|p| p.as_path())
        .unwrap_or_else(|| std::path::Path::new("."));

    // Create directory if it doesn't exist
    std::fs::create_dir_all(dir)?;

    // Write the envelope to result.json
    let result_path = dir.join("result.json");
    let envelope_json = serde_json::to_string_pretty(envelope_value)?;
    std::fs::write(&result_path, envelope_json)?;

    info!("Result envelope saved to: {}", result_path.display());

    Ok(())
}

async fn handle_contracts_command(cmd: ContractsCommands) -> Result<()> {
    match cmd {
        ContractsCommands::ValidateEnvelope {
            file,
            stdin,
            remote,
            registry_endpoint,
            bulk,
        } => {
            if let Some(bulk_dir) = bulk {
                validate_bulk_envelopes(&bulk_dir, remote, &registry_endpoint).await?;
            } else if stdin {
                validate_envelope_stdin(remote, &registry_endpoint).await?;
            } else if let Some(file_path) = file {
                validate_envelope_file(&file_path, remote, &registry_endpoint).await?;
            } else {
                anyhow::bail!("Must specify either a file path, --stdin, or --bulk");
            }
        }
        ContractsCommands::ValidateConfig {
            file,
            stdin,
            schema,
            secrets_file,
        } => {
            if stdin {
                validate_config_stdin(schema, secrets_file).await?;
            } else if let Some(file_path) = file {
                validate_config_file(&file_path, schema, secrets_file).await?;
            } else {
                anyhow::bail!("Must specify either a file path or --stdin");
            }
        }
        ContractsCommands::Bundle {
            format,
            include_wit,
        } => {
            export_contracts_bundle(&format, include_wit).await?;
        }
    }
    Ok(())
}

async fn validate_envelope_file(
    file_path: &str,
    remote: bool,
    registry_endpoint: &str,
) -> Result<()> {
    use envelope::EnvelopeValidator;

    let content = std::fs::read_to_string(file_path)?;
    let envelope: serde_json::Value = serde_json::from_str(&content)?;

    if remote {
        validate_envelope_remote(&envelope, registry_endpoint).await
    } else {
        let validator = EnvelopeValidator::new()?;
        match validator.validate_json(&envelope) {
            Ok(_) => {
                println!("✓ Valid envelope");
                Ok(())
            }
            Err(e) => {
                eprintln!("✗ Invalid envelope:");
                eprintln!("  {}", e);
                std::process::exit(1);
            }
        }
    }
}

async fn validate_envelope_stdin(remote: bool, registry_endpoint: &str) -> Result<()> {
    use envelope::EnvelopeValidator;
    use std::io::Read;

    let mut buffer = String::new();
    std::io::stdin().read_to_string(&mut buffer)?;
    let envelope: serde_json::Value = serde_json::from_str(&buffer)?;

    if remote {
        validate_envelope_remote(&envelope, registry_endpoint).await
    } else {
        let validator = EnvelopeValidator::new()?;
        match validator.validate_json(&envelope) {
            Ok(_) => {
                println!("✓ Valid envelope");
                Ok(())
            }
            Err(e) => {
                eprintln!("✗ Invalid envelope:");
                eprintln!("  {}", e);
                std::process::exit(1);
            }
        }
    }
}

async fn validate_bulk_envelopes(
    dir: &PathBuf,
    remote: bool,
    registry_endpoint: &str,
) -> Result<()> {
    use envelope::EnvelopeValidator;
    use std::fs;

    let entries = fs::read_dir(dir)?;
    let mut results = Vec::new();

    let validator = if !remote {
        Some(EnvelopeValidator::new()?)
    } else {
        None
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.file_name() == Some(std::ffi::OsStr::new("result.json")) {
            let content = fs::read_to_string(&path)?;
            let envelope: serde_json::Value = match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(e) => {
                    results.push((path, false, format!("JSON parse error: {}", e)));
                    continue;
                }
            };

            if remote {
                match validate_envelope_remote_raw(&envelope, registry_endpoint).await {
                    Ok(valid) => results.push((path, valid, String::new())),
                    Err(e) => results.push((path, false, e.to_string())),
                }
            } else if let Some(ref v) = validator {
                match v.validate_json(&envelope) {
                    Ok(_) => results.push((path, true, String::new())),
                    Err(e) => results.push((path, false, e.to_string())),
                }
            }
        }
    }

    let valid_count = results.iter().filter(|(_, valid, _)| *valid).count();
    let invalid_count = results.len() - valid_count;

    println!("Validation Results:");
    println!("  Valid: {}", valid_count);
    println!("  Invalid: {}", invalid_count);

    if invalid_count > 0 {
        println!("\nInvalid envelopes:");
        for (path, valid, error) in &results {
            if !valid {
                println!("  ✗ {}: {}", path.display(), error);
            }
        }
        std::process::exit(1);
    } else {
        println!("\n✓ All envelopes valid");
    }

    Ok(())
}

async fn validate_envelope_remote(
    envelope: &serde_json::Value,
    registry_endpoint: &str,
) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/contracts/validate/envelope", registry_endpoint);

    let response = client.post(&url).json(envelope).send().await?;

    if !response.status().is_success() {
        anyhow::bail!("Registry returned error: {}", response.status());
    }

    let result: serde_json::Value = response.json().await?;

    if result["valid"].as_bool().unwrap_or(false) {
        println!("✓ Valid envelope");
        Ok(())
    } else {
        eprintln!("✗ Invalid envelope:");
        if let Some(errors) = result["errors"].as_array() {
            for error in errors {
                let path = error["path"].as_str().unwrap_or("");
                let message = error["message"].as_str().unwrap_or("Unknown error");
                eprintln!("  {} at {}", message, path);
            }
        }
        std::process::exit(1);
    }
}

async fn validate_envelope_remote_raw(
    envelope: &serde_json::Value,
    registry_endpoint: &str,
) -> Result<bool> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/contracts/validate/envelope", registry_endpoint);

    let response = client.post(&url).json(envelope).send().await?;

    if !response.status().is_success() {
        anyhow::bail!("Registry returned error: {}", response.status());
    }

    let result: serde_json::Value = response.json().await?;
    Ok(result["valid"].as_bool().unwrap_or(false))
}

async fn validate_config_file(
    file_path: &str,
    schema: Option<String>,
    secrets_file: Option<String>,
) -> Result<()> {
    use config_loader::{ConfigManager, EnvFileSecretProvider};
    use std::path::Path;

    let config_manager = ConfigManager::new();
    let path = Path::new(file_path);

    // Determine capsule name from schema arg or filename
    let capsule_name = if let Some(schema_name) = schema {
        schema_name
    } else {
        // Try to extract capsule name from filename
        match path.file_stem().and_then(|s| s.to_str()) {
            Some(name) => {
                // Handle patterns like "echo_config.json" -> "echo"
                if name.ends_with("_config") {
                    name.strip_suffix("_config").unwrap_or(name).to_string()
                } else {
                    name.to_string()
                }
            }
            None => anyhow::bail!(
                "Could not determine capsule name from filename. Use --schema to specify."
            ),
        }
    };

    // Create secret provider if secrets file is specified
    let result = if let Some(secrets_path) = secrets_file {
        let secret_provider = EnvFileSecretProvider::with_secrets_file(secrets_path);
        config_manager.validate_config_file_with_secrets(&capsule_name, path, &secret_provider)
    } else {
        config_manager.validate_config_file(&capsule_name, path)
    };

    match result {
        Ok(_) => {
            println!("✓ Valid config for capsule: {}", capsule_name);
            Ok(())
        }
        Err(config_loader::ConfigError::ValidationFailed { errors }) => {
            eprintln!("✗ Invalid config for capsule '{}':", capsule_name);
            for error in errors {
                eprintln!("  Path {}: {}", error.json_pointer, error.message);
                eprintln!("    Schema: {}", error.schema_path);
            }
            std::process::exit(1);
        }
        Err(config_loader::ConfigError::SecretResolutionFailed { error }) => {
            eprintln!(
                "✗ Secret resolution failed for capsule '{}': {}",
                capsule_name, error
            );
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("✗ Config validation failed: {}", e);
            std::process::exit(1);
        }
    }
}

async fn validate_config_stdin(schema: Option<String>, secrets_file: Option<String>) -> Result<()> {
    use config_loader::{ConfigManager, EnvFileSecretProvider};
    use std::io::Read;

    let capsule_name =
        schema.ok_or_else(|| anyhow::anyhow!("--schema is required when reading from stdin"))?;

    let mut buffer = String::new();
    std::io::stdin().read_to_string(&mut buffer)?;
    let config_value: serde_json::Value = serde_json::from_str(&buffer)?;

    let config_manager = ConfigManager::new();

    // Create secret provider if secrets file is specified
    let result = if let Some(secrets_path) = secrets_file {
        let secret_provider = EnvFileSecretProvider::with_secrets_file(secrets_path);
        config_manager.validate_config_value_with_secrets(
            &capsule_name,
            &config_value,
            &secret_provider,
        )
    } else {
        config_manager.validate_config_value(&capsule_name, &config_value)
    };

    match result {
        Ok(_) => {
            println!("✓ Valid config for capsule: {}", capsule_name);
            Ok(())
        }
        Err(config_loader::ConfigError::ValidationFailed { errors }) => {
            eprintln!("✗ Invalid config for capsule '{}':", capsule_name);
            for error in errors {
                eprintln!("  Path {}: {}", error.json_pointer, error.message);
                eprintln!("    Schema: {}", error.schema_path);
            }
            std::process::exit(1);
        }
        Err(config_loader::ConfigError::SecretResolutionFailed { error }) => {
            eprintln!(
                "✗ Secret resolution failed for capsule '{}': {}",
                capsule_name, error
            );
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("✗ Config validation failed: {}", e);
            std::process::exit(1);
        }
    }
}

async fn export_contracts_bundle(format: &str, include_wit: bool) -> Result<()> {
    use std::collections::BTreeMap;
    use std::fs;

    let contracts_dir = PathBuf::from("contracts");

    // Collect all schemas
    let schemas_dir = contracts_dir.join("schemas");
    let mut schemas = BTreeMap::new();

    if schemas_dir.exists() {
        for entry in fs::read_dir(&schemas_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                let content = fs::read_to_string(&path)?;
                schemas.insert(name.to_string(), content);
            }
        }
    }

    // Collect the envelope schema
    let envelope_schema_path = contracts_dir.join("envelopes/result.json");
    if envelope_schema_path.exists() {
        let content = fs::read_to_string(&envelope_schema_path)?;
        schemas.insert("result-envelope.json".to_string(), content);
    }

    // Collect WIT definitions if requested
    let mut wit_definitions = BTreeMap::new();
    if include_wit {
        let wit_dir = contracts_dir.join("wit");
        if wit_dir.exists() {
            for entry in fs::read_dir(&wit_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("wit") {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown");
                    let content = fs::read_to_string(&path)?;
                    wit_definitions.insert(name.to_string(), content);
                }
            }
        }
    }

    match format {
        "json" => {
            let bundle = serde_json::json!({
                "version": "1.0.0",
                "schemas": schemas,
                "wit": wit_definitions,
            });
            println!("{}", serde_json::to_string_pretty(&bundle)?);
        }
        _ => {
            println!("Contract Bundle Summary");
            println!("=======================");
            println!();
            println!("Schemas ({}):", schemas.len());
            for (name, content) in &schemas {
                let lines = content.lines().count();
                let bytes = content.len();
                println!("  - {} ({} lines, {} bytes)", name, lines, bytes);
            }

            if include_wit && !wit_definitions.is_empty() {
                println!();
                println!("WIT Definitions ({}):", wit_definitions.len());
                for (name, content) in &wit_definitions {
                    let lines = content.lines().count();
                    let bytes = content.len();
                    println!("  - {} ({} lines, {} bytes)", name, lines, bytes);
                }
            }

            println!();
            println!("Total contracts: {}", schemas.len() + wit_definitions.len());
        }
    }

    Ok(())
}

fn handle_secrets_command(cmd: SecretsCommands) -> Result<()> {
    use config_loader::{
        secrets_store, SecretProvider, SecretsStore, VaultHttpSecretProvider, VaultStubProvider,
    };
    use std::env;
    use std::io::Read;

    match cmd {
        SecretsCommands::Set {
            key_path,
            value,
            from_env,
            stdin,
            secrets_file,
            provider,
        } => {
            let (scope, key) = SecretsStore::parse_scope_key(&key_path)?;

            let secret_value = if stdin {
                let mut buffer = String::new();
                std::io::stdin().read_to_string(&mut buffer)?;
                buffer.trim().to_string()
            } else if let Some(env_var) = from_env {
                std::env::var(&env_var)
                    .map_err(|_| anyhow::anyhow!("Environment variable {} not found", env_var))?
            } else {
                value.ok_or_else(|| anyhow::anyhow!("No value provided"))?
            };

            match provider {
                ProviderType::Envfile => {
                    let store = if let Some(path) = secrets_file {
                        SecretsStore::new(path)
                    } else {
                        SecretsStore::default_location()
                    };

                    store.set(&scope, &key, &secret_value)?;

                    #[cfg(unix)]
                    store.check_permissions()?;

                    println!("✓ Secret {}/{} set successfully", scope, key);
                    println!("  Stored in: {}", store.path().display());
                }
                ProviderType::Vault => {
                    if secrets_file.is_some() {
                        eprintln!("⚠ Warning: --secrets-file is ignored when using vault provider");
                    }

                    // Determine which Vault provider to use based on VAULT_ADDR
                    let vault_addr =
                        env::var("VAULT_ADDR").unwrap_or_else(|_| "file://vault_stub".to_string());

                    if vault_addr.starts_with("http://") || vault_addr.starts_with("https://") {
                        // Use HTTP provider for real Vault
                        let vault_provider = VaultHttpSecretProvider::from_env().map_err(|e| {
                            anyhow::anyhow!("Failed to initialize Vault HTTP provider: {}", e)
                        })?;

                        vault_provider
                            .put(&scope, &key, &secret_value)
                            .map_err(|e| {
                                anyhow::anyhow!("Failed to store secret in Vault: {}", e)
                            })?;

                        println!(
                            "✓ Secret {}/{} set successfully in Vault (HTTP)",
                            scope, key
                        );
                    } else {
                        // Use stub provider for file:// URLs
                        let vault_provider = VaultStubProvider::from_env().map_err(|e| {
                            anyhow::anyhow!("Failed to initialize Vault stub provider: {}", e)
                        })?;

                        vault_provider
                            .put(&scope, &key, &secret_value)
                            .map_err(|e| {
                                anyhow::anyhow!("Failed to store secret in vault stub: {}", e)
                            })?;

                        println!("✓ Secret {}/{} set successfully in vault stub", scope, key);
                    }
                }
            }
        }
        SecretsCommands::Get {
            key_path,
            raw,
            secrets_file,
            provider,
        } => {
            let (scope, key) = SecretsStore::parse_scope_key(&key_path)?;

            match provider {
                ProviderType::Envfile => {
                    let store = if let Some(path) = secrets_file {
                        SecretsStore::new(path)
                    } else {
                        SecretsStore::default_location()
                    };

                    let value = store.get(&scope, &key)?;

                    if raw {
                        println!("{}", value);
                    } else {
                        println!("{}/{}: {}", scope, key, secrets_store::redact_value(&value));
                    }
                }
                ProviderType::Vault => {
                    if secrets_file.is_some() {
                        eprintln!("⚠ Warning: --secrets-file is ignored when using vault provider");
                    }

                    // Determine which Vault provider to use based on VAULT_ADDR
                    let vault_addr =
                        env::var("VAULT_ADDR").unwrap_or_else(|_| "file://vault_stub".to_string());

                    let value = if vault_addr.starts_with("http://")
                        || vault_addr.starts_with("https://")
                    {
                        // Use HTTP provider for real Vault
                        let vault_provider = VaultHttpSecretProvider::from_env().map_err(|e| {
                            anyhow::anyhow!("Failed to initialize Vault HTTP provider: {}", e)
                        })?;

                        vault_provider.resolve(&scope, &key).map_err(|e| {
                            anyhow::anyhow!("Failed to get secret from Vault: {}", e)
                        })?
                    } else {
                        // Use stub provider for file:// URLs
                        let vault_provider = VaultStubProvider::from_env().map_err(|e| {
                            anyhow::anyhow!("Failed to initialize Vault stub provider: {}", e)
                        })?;

                        vault_provider.resolve(&scope, &key).map_err(|e| {
                            anyhow::anyhow!("Failed to get secret from vault stub: {}", e)
                        })?
                    };

                    if raw {
                        println!("{}", value);
                    } else {
                        println!("{}/{}: {}", scope, key, secrets_store::redact_value(&value));
                    }
                }
            }
        }
        SecretsCommands::List {
            scope,
            secrets_file,
            provider,
        } => match provider {
            ProviderType::Envfile => {
                let store = if let Some(path) = secrets_file {
                    SecretsStore::new(path)
                } else {
                    SecretsStore::default_location()
                };

                if let Some(scope_filter) = scope {
                    let secrets = store.list_scope(&scope_filter)?;
                    if secrets.is_empty() {
                        println!("No secrets found for scope: {}", scope_filter);
                    } else {
                        println!("Secrets in scope '{}':", scope_filter);
                        for (key, value) in secrets {
                            println!("  {}: {}", key, value);
                        }
                    }
                } else {
                    let all_secrets = store.list()?;
                    if all_secrets.is_empty() {
                        println!("No secrets found");
                    } else {
                        println!("Secrets:");
                        for (scope, secrets) in all_secrets {
                            println!("  {}:", scope);
                            for (key, value) in secrets {
                                println!("    {}: {}", key, value);
                            }
                        }
                    }
                }
            }
            ProviderType::Vault => {
                if secrets_file.is_some() {
                    eprintln!("⚠ Warning: --secrets-file is ignored when using vault provider");
                }

                // Determine which Vault provider to use based on VAULT_ADDR
                let vault_addr =
                    env::var("VAULT_ADDR").unwrap_or_else(|_| "file://vault_stub".to_string());

                if vault_addr.starts_with("http://") || vault_addr.starts_with("https://") {
                    // HTTP provider doesn't support list operation yet
                    eprintln!("List operation is not yet supported for Vault HTTP provider");
                    eprintln!("Use vault CLI directly: vault kv list secret/");
                    std::process::exit(1);
                } else {
                    // Use stub provider for file:// URLs
                    let vault_provider = VaultStubProvider::from_env().map_err(|e| {
                        anyhow::anyhow!("Failed to initialize vault stub provider: {}", e)
                    })?;

                    let all_secrets = vault_provider.list(scope.as_deref()).map_err(|e| {
                        anyhow::anyhow!("Failed to list secrets from vault stub: {}", e)
                    })?;

                    if all_secrets.is_empty() {
                        if let Some(scope_filter) = scope {
                            println!("No secrets found for scope: {}", scope_filter);
                        } else {
                            println!("No secrets found");
                        }
                    } else if let Some(scope_filter) = scope {
                        if let Some(secrets) = all_secrets.get(&scope_filter) {
                            println!("Secrets in scope '{}' (vault stub):", scope_filter);
                            for (key, value) in secrets {
                                println!("  {}: {}", key, value);
                            }
                        } else {
                            println!("No secrets found for scope: {}", scope_filter);
                        }
                    } else {
                        println!("Secrets (vault stub):");
                        for (scope, secrets) in all_secrets {
                            println!("  {}:", scope);
                            for (key, value) in secrets {
                                println!("    {}: {}", key, value);
                            }
                        }
                    }
                }
            }
        },
        SecretsCommands::Delete {
            key_path,
            secrets_file,
            provider,
        } => {
            let (scope, key) = SecretsStore::parse_scope_key(&key_path)?;

            match provider {
                ProviderType::Envfile => {
                    let store = if let Some(path) = secrets_file {
                        SecretsStore::new(path)
                    } else {
                        SecretsStore::default_location()
                    };

                    store.delete(&scope, &key)?;
                    println!("✓ Secret {}/{} deleted", scope, key);
                }
                ProviderType::Vault => {
                    if secrets_file.is_some() {
                        eprintln!("⚠ Warning: --secrets-file is ignored when using vault provider");
                    }

                    // Determine which Vault provider to use based on VAULT_ADDR
                    let vault_addr =
                        env::var("VAULT_ADDR").unwrap_or_else(|_| "file://vault_stub".to_string());

                    if vault_addr.starts_with("http://") || vault_addr.starts_with("https://") {
                        // Use HTTP provider for real Vault
                        let vault_provider = VaultHttpSecretProvider::from_env().map_err(|e| {
                            anyhow::anyhow!("Failed to initialize Vault HTTP provider: {}", e)
                        })?;

                        vault_provider.delete(&scope, &key).map_err(|e| {
                            anyhow::anyhow!("Failed to delete secret from Vault: {}", e)
                        })?;

                        println!("✓ Secret {}/{} deleted from Vault (HTTP)", scope, key);
                    } else {
                        // Use stub provider for file:// URLs
                        let vault_provider = VaultStubProvider::from_env().map_err(|e| {
                            anyhow::anyhow!("Failed to initialize Vault stub provider: {}", e)
                        })?;

                        vault_provider.delete(&scope, &key).map_err(|e| {
                            anyhow::anyhow!("Failed to delete secret from vault stub: {}", e)
                        })?;

                        println!("✓ Secret {}/{} deleted from vault stub", scope, key);
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_k8s_bootstrap_command(cmd: K8sBootstrapCommands) -> Result<()> {
    match cmd {
        K8sBootstrapCommands::Bootstrap {
            config,
            dry_run,
            verbose,
        } => {
            if verbose {
                println!("Loading K8s bootstrap configuration from: {}", config);
            }

            let bootstrap_config = k8s_bootstrap::load_config(&config)?;

            if verbose {
                println!("Configuration loaded successfully");
                println!("Cluster: {}", bootstrap_config.cluster.name);
                println!("Namespace: {}", bootstrap_config.demon.namespace);
            }

            k8s_bootstrap::validate_config(&bootstrap_config)?;

            if verbose {
                println!("Configuration validation passed");
            }

            // Collect secrets
            let secret_material =
                k8s_bootstrap::secrets::collect_secrets(&bootstrap_config.secrets, dry_run)?;

            // Render secret manifest
            let secret_manifest = k8s_bootstrap::secrets::render_secret_manifest(
                &bootstrap_config.demon.namespace,
                None, // Use default name "demon-secrets"
                &secret_material,
            )?;

            // Create image pull secrets for registries
            let registry_secrets = if let Some(registries) = &bootstrap_config.registries {
                k8s_bootstrap::secrets::create_image_pull_secrets(
                    registries,
                    &bootstrap_config.demon.namespace,
                    dry_run,
                )?
            } else {
                Vec::new()
            };

            // Initialize template renderer
            let templates_dir = format!("{}/resources/k8s", env!("CARGO_MANIFEST_DIR"));
            let template_renderer = k8s_bootstrap::templates::TemplateRenderer::new(&templates_dir);

            // Render manifests
            let mut manifests = template_renderer.render_manifests(&bootstrap_config)?;

            // Prepend secret manifest if we have secrets
            if !secret_manifest.is_empty() {
                manifests = format!("{}\n---\n{}", secret_manifest, manifests);
            }

            // Add registry secrets if we have any
            if !registry_secrets.is_empty() {
                let registry_manifests = registry_secrets.join("\n---\n");
                manifests = format!("{}\n---\n{}", manifests, registry_manifests);
            }

            // Process add-ons
            let addon_manifests =
                k8s_bootstrap::addons::process_addons(&bootstrap_config, dry_run, verbose)?;
            if !addon_manifests.is_empty() {
                manifests = format!("{}\n---\n{}", manifests, addon_manifests.join("\n---\n"));
            }

            if dry_run {
                println!("✓ Configuration is valid");
                println!("Dry run mode - no changes will be made");
                println!(
                    "Cluster: {} (namespace: {})",
                    bootstrap_config.cluster.name, bootstrap_config.demon.namespace
                );
                let addon_manifest_count = addon_manifests.len();
                let manifest_count = MANIFEST_FILES.len()
                    + if secret_manifest.is_empty() { 0 } else { 1 }
                    + if bootstrap_config.networking.ingress.enabled {
                        1
                    } else {
                        0
                    }
                    + addon_manifest_count;
                println!(
                    "{} manifest{} will be generated.",
                    manifest_count,
                    if manifest_count == 1 { "" } else { "s" }
                );

                if verbose {
                    println!();
                    println!("Configuration summary:");
                    println!(
                        "  Cluster: {} ({})",
                        bootstrap_config.cluster.name, bootstrap_config.cluster.k3s.version
                    );
                    println!("  Runtime: {}", bootstrap_config.cluster.runtime);
                    println!("  Namespace: {}", bootstrap_config.demon.namespace);
                    println!("  NATS URL: {}", bootstrap_config.demon.nats_url);
                    println!("  Stream: {}", bootstrap_config.demon.stream_name);
                    println!("  UI URL: {}", bootstrap_config.demon.ui_url);
                    if !bootstrap_config.addons.is_empty() {
                        println!("  Add-ons: {}", bootstrap_config.addons.len());
                        for addon in &bootstrap_config.addons {
                            println!("    - {} (enabled: {})", addon.name, addon.enabled);
                        }
                    }

                    // Show secrets information from collected material
                    if !secret_material.provider_used.is_empty() {
                        println!("  Secrets: {}", secret_material.provider_used);
                        if !secret_material.data.is_empty() {
                            println!(
                                "    - Keys to be configured: {:?}",
                                secret_material.data.keys().cloned().collect::<Vec<_>>()
                            );
                        }
                    } else {
                        println!("  Secrets: none configured");
                    }

                    // Show networking configuration
                    println!("  Networking:");
                    let ingress = &bootstrap_config.networking.ingress;
                    if ingress.enabled {
                        let hostname_str = ingress.hostname.as_deref().unwrap_or("no hostname");
                        let tls_str = if ingress.tls.enabled {
                            match &ingress.tls.secret_name {
                                Some(secret) => format!("TLS: {}", secret),
                                None => "TLS: enabled (no secret)".to_string(),
                            }
                        } else {
                            "TLS: disabled".to_string()
                        };
                        println!("    Ingress: enabled (host: {}, {})", hostname_str, tls_str);
                    } else {
                        println!("    Ingress: disabled");
                    }
                    let mesh = &bootstrap_config.networking.service_mesh;
                    if mesh.enabled {
                        println!("    Service mesh: enabled");
                    } else {
                        println!("    Service mesh: disabled");
                    }

                    println!();
                    let k3s_installer = k8s_bootstrap::k3s::K3sInstaller::new(
                        bootstrap_config.cluster.k3s.clone(),
                        dry_run,
                    );
                    k3s_installer.install_k3s()?;

                    println!("Manifests to be applied:");
                    if !secret_manifest.is_empty() {
                        println!("  - demon-secrets (Secret)");
                    }
                    for file in MANIFEST_FILES.iter() {
                        println!("  - {}", file);
                    }
                    if !addon_manifests.is_empty() {
                        println!("  - {} add-on manifest(s)", addon_manifests.len());
                    }

                    println!();
                    println!("Generated manifests:");
                    println!("---");
                    println!("{}", manifests);
                } else {
                    println!(
                        "Run with --verbose to view the k3s installation plan and manifest preview."
                    );
                    println!("Note: Health checks will run after deployment to verify runtime API and Operate UI.");
                }

                return Ok(());
            }

            let execution_mode = BootstrapExecutionMode::from_env();

            if execution_mode.is_apply_only() {
                println!("🚀 Starting K8s bootstrap process (manifests only)...");
                if verbose {
                    println!("Phase 3: Deploying Demon components");
                }

                let command_executor = resolve_command_executor();
                apply_manifests(&manifests, command_executor.as_ref(), verbose)?;

                if verbose {
                    println!("✓ Demon components deployed");
                }

                println!("🎯 Manifest application simulation complete");
                return Ok(());
            }

            println!("🚀 Starting K8s bootstrap process...");

            if verbose {
                println!("Phase 1: Installing k3s cluster");
            }

            let k3s_installer = k8s_bootstrap::k3s::K3sInstaller::new(
                bootstrap_config.cluster.k3s.clone(),
                dry_run,
            );

            k3s_installer.install_k3s()?;

            if verbose {
                println!("✓ k3s cluster installation completed");
            }

            // Wait for k3s to be ready
            if verbose {
                println!("Phase 2: Waiting for k3s cluster to be ready");
            }

            let k3s_ready = k3s_installer.is_k3s_ready()?;
            if !k3s_ready {
                anyhow::bail!("k3s cluster is not ready after installation");
            }

            if verbose {
                println!("✓ k3s cluster is ready");
                println!("Phase 3: Deploying Demon components");
            }

            // Apply manifests to cluster
            let command_executor = resolve_command_executor();
            apply_manifests(&manifests, command_executor.as_ref(), verbose)?;

            if verbose {
                println!("✓ Demon components deployed");
                println!("Phase 4: Waiting for pods to be ready");
            }

            // Wait for Demon pods to be ready
            wait_for_demon_pods(
                &bootstrap_config.demon.namespace,
                command_executor.as_ref(),
                verbose,
            )?;

            if verbose {
                println!("Phase 5: Running health checks");
            }

            // Run health checks
            run_health_checks(
                &bootstrap_config.demon.namespace,
                command_executor.as_ref(),
                verbose,
            )?;

            println!("🎉 Demon deployment completed successfully!");
            println!("You can now use kubectl to interact with your cluster:");
            println!("  sudo k3s kubectl get nodes");
            println!(
                "  sudo k3s kubectl get pods -n {}",
                bootstrap_config.demon.namespace
            );
            println!(
                "  sudo k3s kubectl get services -n {}",
                bootstrap_config.demon.namespace
            );

            Ok(())
        }
    }
}

fn apply_manifests(
    manifests: &str,
    executor: &dyn k8s_bootstrap::CommandExecutor,
    verbose: bool,
) -> Result<()> {
    apply_manifests_with_namespace_wait(manifests, executor, verbose)
}

fn apply_manifests_with_namespace_wait(
    manifests: &str,
    executor: &dyn k8s_bootstrap::CommandExecutor,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("Applying manifests to cluster with namespace readiness checks...");
    }

    // Split manifests into individual YAML documents
    let manifest_docs: Vec<&str> = manifests
        .split("---")
        .filter(|doc| !doc.trim().is_empty())
        .collect();

    // Separate namespace manifests from other manifests
    let mut namespace_manifests = Vec::new();
    let mut other_manifests = Vec::new();

    for doc in manifest_docs {
        if doc.contains("kind: Namespace") {
            namespace_manifests.push(doc);
        } else {
            other_manifests.push(doc);
        }
    }

    // Apply namespaces first
    if !namespace_manifests.is_empty() {
        if verbose {
            println!("Applying namespace manifests...");
        }
        let namespace_yaml = namespace_manifests.join("\n---\n");
        let output = executor.execute(
            "k3s",
            &["kubectl", "apply", "-f", "-"],
            Some(&namespace_yaml),
        )?;
        if output.status != 0 {
            eprintln!("Failed to apply namespace manifests:");
            eprintln!("stdout: {}", output.stdout);
            eprintln!("stderr: {}", output.stderr);
            anyhow::bail!("kubectl apply failed with exit code {}", output.status);
        }

        if verbose && !output.stdout.trim().is_empty() {
            println!("{}", output.stdout.trim());
        }

        // Wait for namespaces to be ready
        for manifest in &namespace_manifests {
            if let Some(namespace_name) = extract_namespace_name(manifest) {
                wait_for_namespace_ready(&namespace_name, executor, verbose)?;
            }
        }

        if verbose {
            println!("✓ Namespaces applied and ready");
        }
    }

    // Apply remaining manifests
    if !other_manifests.is_empty() {
        if verbose {
            println!("Applying remaining manifests...");
        }
        let remaining_yaml = other_manifests.join("\n---\n");
        let output = executor.execute(
            "k3s",
            &["kubectl", "apply", "-f", "-"],
            Some(&remaining_yaml),
        )?;
        if output.status != 0 {
            eprintln!("Failed to apply manifests:");
            eprintln!("stdout: {}", output.stdout);
            eprintln!("stderr: {}", output.stderr);
            anyhow::bail!("kubectl apply failed with exit code {}", output.status);
        }

        if verbose && !output.stdout.trim().is_empty() {
            println!("{}", output.stdout.trim());
        }

        if verbose {
            println!("✓ All manifests applied successfully");
        }
    }

    Ok(())
}

fn extract_namespace_name(manifest: &str) -> Option<String> {
    // Parse YAML properly to extract namespace name from a Namespace manifest
    match serde_yaml::from_str::<serde_yaml::Value>(manifest) {
        Ok(value) => {
            // Check if this is a Namespace manifest
            if value
                .get("kind")
                .and_then(|v| v.as_str())
                .map(|s| s == "Namespace")
                .unwrap_or(false)
            {
                // Extract metadata.name
                value
                    .get("metadata")
                    .and_then(|m| m.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        }
        Err(_) => {
            // Fallback to simple parsing if YAML parsing fails
            // This is more conservative: only matches if we see kind: Namespace first
            let mut is_namespace = false;
            for line in manifest.lines() {
                let trimmed = line.trim();

                // Check if this is a Namespace kind
                if trimmed == "kind: Namespace" {
                    is_namespace = true;
                } else if is_namespace && trimmed.starts_with("name:") {
                    // Only extract name if we've confirmed this is a Namespace
                    if let Some(name) = trimmed.strip_prefix("name:") {
                        return Some(name.trim().to_string());
                    }
                }
            }
            None
        }
    }
}

fn wait_for_namespace_ready(
    namespace: &str,
    executor: &dyn k8s_bootstrap::CommandExecutor,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("Waiting for namespace '{}' to be ready...", namespace);
    }

    // For testing with simulated executors, we can skip the actual wait
    // since simulation doesn't model real Kubernetes behavior
    if std::env::var("DEMONCTL_K8S_EXECUTOR").is_ok() {
        if verbose {
            println!("✓ Namespace '{}' is ready (simulated)", namespace);
        }
        return Ok(());
    }

    let timeout_secs = 30;
    let check_interval_secs = 2;
    let mut elapsed = 0;

    while elapsed < timeout_secs {
        let output = executor.execute(
            "k3s",
            &["kubectl", "get", "namespace", namespace, "--no-headers"],
            None,
        )?;

        if output.status == 0 && output.stdout.contains("Active") {
            if verbose {
                println!("✓ Namespace '{}' is ready", namespace);
            }
            return Ok(());
        }

        std::thread::sleep(std::time::Duration::from_secs(check_interval_secs));
        elapsed += check_interval_secs;
    }

    anyhow::bail!("Timeout waiting for namespace '{}' to be ready", namespace)
}

fn wait_for_demon_pods(
    namespace: &str,
    executor: &dyn k8s_bootstrap::CommandExecutor,
    verbose: bool,
) -> Result<()> {
    let timeout_secs = std::env::var("K8S_POD_TIMEOUT")
        .unwrap_or_else(|_| "240".to_string())
        .parse::<u64>()
        .unwrap_or(240);
    let check_interval_secs = 5;
    let mut elapsed = 0;

    if verbose {
        println!(
            "Waiting for Demon pods to be ready (timeout: {}s)...",
            timeout_secs
        );
    }

    while elapsed < timeout_secs {
        let output = executor.execute(
            "k3s",
            &["kubectl", "get", "pods", "-n", namespace, "--no-headers"],
            None,
        )?;

        if output.status == 0 {
            let lines: Vec<&str> = output
                .stdout
                .lines()
                .filter(|line| !line.trim().is_empty())
                .collect();
            if lines.is_empty() {
                if verbose {
                    println!("No pods found yet, waiting...");
                }
            } else {
                let total_pods = lines.len();
                let ready_pods = lines
                    .iter()
                    .filter(|line| {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 3 {
                            let ready_status = parts[1];
                            let status = parts[2];
                            status == "Running" && ready_status.contains("/") && {
                                let ready_parts: Vec<&str> = ready_status.split('/').collect();
                                ready_parts.len() == 2 && ready_parts[0] == ready_parts[1]
                            }
                        } else {
                            false
                        }
                    })
                    .count();

                if verbose {
                    println!("Pods ready: {}/{}", ready_pods, total_pods);
                }

                if ready_pods == total_pods && total_pods > 0 {
                    if verbose {
                        println!("All Demon pods are ready!");
                    }
                    return Ok(());
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(check_interval_secs));
        elapsed += check_interval_secs;
    }

    // Capture diagnostics before failing
    let mut error_message = format!(
        "Timeout waiting for Demon pods to be ready after {}s",
        timeout_secs
    );

    // Get pod status details
    if let Ok(pod_output) = executor.execute(
        "k3s",
        &["kubectl", "get", "pods", "-n", namespace, "-o", "wide"],
        None,
    ) {
        if pod_output.status == 0 {
            error_message.push_str("\n\nPod status details:");
            error_message.push_str(&format!("\n{}", pod_output.stdout));
        }
    }

    // Get recent events
    if let Ok(events_output) = executor.execute(
        "k3s",
        &[
            "kubectl",
            "get",
            "events",
            "-n",
            namespace,
            "--sort-by=.lastTimestamp",
        ],
        None,
    ) {
        if events_output.status == 0 && !events_output.stdout.trim().is_empty() {
            error_message.push_str("\n\nRecent events:");
            error_message.push_str(&format!("\n{}", events_output.stdout));
        }
    }

    anyhow::bail!("{}", error_message)
}

fn run_health_checks(
    namespace: &str,
    executor: &dyn k8s_bootstrap::CommandExecutor,
    verbose: bool,
) -> Result<()> {
    if verbose {
        println!("Running post-deployment health checks...");
    }

    // Check runtime health endpoint
    let runtime_pod = get_pod_by_label(namespace, "app=demon-runtime", executor)?;
    if let Some(pod_name) = runtime_pod {
        if verbose {
            println!("Checking runtime health endpoint for pod: {}", pod_name);
        }

        match check_runtime_health(namespace, &pod_name, executor, verbose) {
            Ok(_) => {
                if verbose {
                    println!("✓ Runtime health check passed");
                }
            }
            Err(e) => {
                eprintln!("✗ Runtime health check failed: {}", e);
                eprintln!("  To investigate, check runtime logs and port-forward to the pod:");
                eprintln!("  kubectl logs -n {} {}", namespace, pod_name);
                eprintln!(
                    "  kubectl port-forward -n {} pod/{} 8080:8080",
                    namespace, pod_name
                );
                return Err(e);
            }
        }
    } else {
        anyhow::bail!("No demon-runtime pod found for health checking");
    }

    // Check Operate UI health
    let ui_pod = get_pod_by_label(namespace, "app=demon-operate-ui", executor)?;
    if let Some(pod_name) = ui_pod {
        if verbose {
            println!("Checking Operate UI health endpoint for pod: {}", pod_name);
        }

        match check_ui_health(namespace, &pod_name, executor, verbose) {
            Ok(_) => {
                if verbose {
                    println!("✓ Operate UI health check passed");
                }
            }
            Err(e) => {
                eprintln!("✗ Operate UI health check failed: {}", e);
                eprintln!("  To investigate, check UI logs and port-forward to the pod:");
                eprintln!("  kubectl logs -n {} {}", namespace, pod_name);
                eprintln!(
                    "  kubectl port-forward -n {} pod/{} 3000:3000",
                    namespace, pod_name
                );
                return Err(e);
            }
        }
    } else {
        anyhow::bail!("No demon-operate-ui pod found for health checking");
    }

    if verbose {
        println!("All health checks passed!");
    }

    Ok(())
}

fn get_pod_by_label(
    namespace: &str,
    label_selector: &str,
    executor: &dyn k8s_bootstrap::CommandExecutor,
) -> Result<Option<String>> {
    let output = executor.execute(
        "k3s",
        &[
            "kubectl",
            "get",
            "pods",
            "-n",
            namespace,
            "-l",
            label_selector,
            "-o",
            "jsonpath={.items[0].metadata.name}",
        ],
        None,
    )?;

    if output.status == 0 && !output.stdout.trim().is_empty() {
        Ok(Some(output.stdout.trim().to_string()))
    } else {
        Ok(None)
    }
}

fn check_runtime_health(
    namespace: &str,
    pod_name: &str,
    executor: &dyn k8s_bootstrap::CommandExecutor,
    verbose: bool,
) -> Result<()> {
    // Try to curl the health endpoint from within the pod
    let output = executor.execute(
        "k3s",
        &[
            "kubectl",
            "exec",
            "-n",
            namespace,
            pod_name,
            "--",
            "curl",
            "-f",
            "-s",
            "http://localhost:8080/health",
        ],
        None,
    )?;

    if output.status == 0 {
        if verbose {
            println!("Runtime health response: {}", output.stdout.trim());
        }
        Ok(())
    } else {
        anyhow::bail!(
            "Runtime health check failed - status: {}, stderr: {}",
            output.status,
            output.stderr
        );
    }
}

fn check_ui_health(
    namespace: &str,
    pod_name: &str,
    executor: &dyn k8s_bootstrap::CommandExecutor,
    verbose: bool,
) -> Result<()> {
    // Try to curl the health/readiness endpoint from within the pod
    let output = executor.execute(
        "k3s",
        &[
            "kubectl",
            "exec",
            "-n",
            namespace,
            pod_name,
            "--",
            "curl",
            "-f",
            "-s",
            "http://localhost:3000/api/runs",
        ],
        None,
    )?;

    if output.status == 0 {
        if verbose {
            println!("Operate UI health response: {}", output.stdout.trim());
        }
        Ok(())
    } else {
        anyhow::bail!(
            "Operate UI health check failed - status: {}, stderr: {}",
            output.status,
            output.stderr
        );
    }
}

// no-op: exercise replies guard

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_namespace_manifest_when_extract_namespace_name_then_returns_correct_name() {
        let manifest = r#"
apiVersion: v1
kind: Namespace
metadata:
  name: demon-system
  labels:
    app.kubernetes.io/name: demon
"#;
        let name = extract_namespace_name(manifest);
        assert_eq!(name, Some("demon-system".to_string()));
    }

    #[test]
    fn given_non_namespace_manifest_when_extract_namespace_name_then_returns_none() {
        let manifest = r#"
apiVersion: v1
kind: Service
metadata:
  name: nats
  namespace: demon-system
"#;
        let name = extract_namespace_name(manifest);
        assert_eq!(name, None);
    }

    #[test]
    fn given_manifest_without_name_when_extract_namespace_name_then_returns_none() {
        let manifest = r#"
apiVersion: v1
kind: Service
metadata:
  namespace: demon-system
"#;
        let name = extract_namespace_name(manifest);
        assert_eq!(name, None);
    }

    #[test]
    fn given_namespace_with_comments_when_extract_namespace_name_then_returns_correct_name() {
        let manifest = r#"
# This is a comment
---
apiVersion: v1
kind: Namespace  # Another comment
metadata:
  # Name of the namespace
  name: test-namespace
  labels:
    app: test
"#;
        let name = extract_namespace_name(manifest);
        assert_eq!(name, Some("test-namespace".to_string()));
    }

    #[test]
    fn given_multiple_name_fields_when_extract_namespace_name_then_returns_only_namespace_name() {
        let manifest = r#"
apiVersion: v1
kind: Namespace
metadata:
  name: actual-namespace
  labels:
    name: label-name
    app.kubernetes.io/name: app-name
"#;
        let name = extract_namespace_name(manifest);
        assert_eq!(name, Some("actual-namespace".to_string()));
    }

    #[test]
    fn given_malformed_yaml_but_valid_namespace_when_extract_namespace_name_then_uses_fallback() {
        // Intentionally malformed YAML that would fail parsing but has clear structure
        let manifest = r#"
kind: Namespace
metadata:
  name: fallback-namespace
  labels: {invalid yaml here
"#;
        let name = extract_namespace_name(manifest);
        assert_eq!(name, Some("fallback-namespace".to_string()));
    }

    #[test]
    fn given_deployment_with_name_field_when_extract_namespace_name_then_returns_none() {
        let manifest = r#"
apiVersion: apps/v1
kind: Deployment
metadata:
  name: my-deployment
  namespace: demon-system
spec:
  replicas: 1
"#;
        let name = extract_namespace_name(manifest);
        assert_eq!(name, None);
    }

    #[test]
    fn given_manifests_with_namespace_when_apply_manifests_with_namespace_wait_then_separates_correctly(
    ) {
        let manifests = r#"---
apiVersion: v1
kind: Namespace
metadata:
  name: test-namespace
---
apiVersion: v1
kind: Service
metadata:
  name: test-service
  namespace: test-namespace
"#;

        // For testing, we need to create a custom executor that simulates namespace readiness
        let executor = MockNamespaceWaitExecutor::new();
        let result = apply_manifests_with_namespace_wait(manifests, &executor, false);

        assert!(result.is_ok());
    }

    #[cfg(test)]
    struct MockNamespaceWaitExecutor {
        call_count: std::cell::RefCell<usize>,
    }

    #[cfg(test)]
    impl MockNamespaceWaitExecutor {
        fn new() -> Self {
            Self {
                call_count: std::cell::RefCell::new(0),
            }
        }
    }

    #[cfg(test)]
    impl k8s_bootstrap::CommandExecutor for MockNamespaceWaitExecutor {
        fn execute(
            &self,
            _program: &str,
            args: &[&str],
            _input: Option<&str>,
        ) -> anyhow::Result<k8s_bootstrap::CommandOutput> {
            let mut count = self.call_count.borrow_mut();
            *count += 1;

            // If this is a namespace status check, return Active status
            if args.len() >= 4 && args[1] == "get" && args[2] == "namespace" {
                return Ok(k8s_bootstrap::CommandOutput {
                    status: 0,
                    stdout: "test-namespace   Active   1m".to_string(),
                    stderr: String::new(),
                });
            }

            // Otherwise return success for kubectl apply commands
            Ok(k8s_bootstrap::CommandOutput {
                status: 0,
                stdout: "namespace/test-namespace created\nservice/test-service created"
                    .to_string(),
                stderr: String::new(),
            })
        }
    }

    #[test]
    fn given_manifests_without_namespace_when_apply_manifests_with_namespace_wait_then_applies_all()
    {
        use k8s_bootstrap::SimulatedCommandExecutor;

        let manifests = r#"---
apiVersion: v1
kind: Service
metadata:
  name: test-service
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: test-config
"#;

        let executor = SimulatedCommandExecutor::success(
            "service/test-service created\nconfigmap/test-config created",
        );
        let result = apply_manifests_with_namespace_wait(manifests, &executor, false);

        assert!(result.is_ok());
    }
}
