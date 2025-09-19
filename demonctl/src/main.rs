use anyhow::Result;
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::{fmt, EnvFilter};

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
    /// Print version and exit
    Version,
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
            required_unless_present = "from_env"
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
    },
    /// List secrets
    List {
        /// Filter by scope
        #[arg(long)]
        scope: Option<String>,
        /// Path to secrets file (defaults to CONFIG_SECRETS_FILE or .demon/secrets.json)
        #[arg(long)]
        secrets_file: Option<String>,
    },
    /// Delete a secret
    Delete {
        /// Scope and key in format: scope/key
        #[arg(value_name = "SCOPE/KEY")]
        key_path: String,
        /// Path to secrets file (defaults to CONFIG_SECRETS_FILE or .demon/secrets.json)
        #[arg(long)]
        secrets_file: Option<String>,
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

fn handle_secrets_command(cmd: SecretsCommands) -> Result<()> {
    use config_loader::{secrets_store, SecretsStore};
    use std::io::Read;

    match cmd {
        SecretsCommands::Set {
            key_path,
            value,
            from_env,
            stdin,
            secrets_file,
        } => {
            let store = if let Some(path) = secrets_file {
                SecretsStore::new(path)
            } else {
                SecretsStore::default_location()
            };

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

            store.set(&scope, &key, &secret_value)?;

            #[cfg(unix)]
            store.check_permissions()?;

            println!("✓ Secret {}/{} set successfully", scope, key);
            println!("  Stored in: {}", store.path().display());
        }
        SecretsCommands::Get {
            key_path,
            raw,
            secrets_file,
        } => {
            let store = if let Some(path) = secrets_file {
                SecretsStore::new(path)
            } else {
                SecretsStore::default_location()
            };

            let (scope, key) = SecretsStore::parse_scope_key(&key_path)?;
            let value = store.get(&scope, &key)?;

            if raw {
                println!("{}", value);
            } else {
                println!("{}/{}: {}", scope, key, secrets_store::redact_value(&value));
            }
        }
        SecretsCommands::List {
            scope,
            secrets_file,
        } => {
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
        SecretsCommands::Delete {
            key_path,
            secrets_file,
        } => {
            let store = if let Some(path) = secrets_file {
                SecretsStore::new(path)
            } else {
                SecretsStore::default_location()
            };

            let (scope, key) = SecretsStore::parse_scope_key(&key_path)?;
            store.delete(&scope, &key)?;
            println!("✓ Secret {}/{} deleted", scope, key);
        }
    }

    Ok(())
}

// no-op: exercise replies guard
