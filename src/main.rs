mod benchmark;
mod config;
mod logging;
mod rust_setup;
mod s3;
mod secrets;
mod workingmd;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::PluginConfig;

/// Where key/health notifications are written so the user sees them after an
/// automatic run (gitignored, machine-local).
const NOTIFICATION_FILE: &str = "key-bitcher-notifications.md";

#[derive(Parser)]
#[command(
    name = "key-bitcher",
    version,
    about = "Key-Bitcher: AI env + secrets manager (S3/Backblaze -> .env + Wave)"
)]
struct Cli {
    /// Path to key-bitcher.toml
    #[arg(long, default_value = "key-bitcher.toml")]
    config: String,

    /// If set, automatically sync secrets on startup before any command
    #[arg(long)]
    auto_sync: bool,

    /// Enable verbose debug logging (always recorded in the log file)
    #[arg(long, short = 'd', global = true)]
    debug: bool,

    /// Append-only log file path (never truncated between runs)
    #[arg(long, global = true, default_value = "key-bitcher.log")]
    log_file: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Fetch secrets from S3 and update the env file
    SyncSecrets {
        /// Also print xport commands to stdout
        #[arg(long)]
        print_exports: bool,

        /// Show what a sync would change without writing anything
        #[arg(long)]
        dry_run: bool,
    },

    /// Validate secret values against provider key formats
    Validate {
        /// Source of secrets: env (default) or s3
        #[arg(long, default_value = "env")]
        source: String,
    },

    /// Check the Wave integration (wsh, JWT, secret store)
    WaveTest {},

    /// Enable (or report) S3 object versioning on the bucket
    S3Versioning {
        /// Only report the current versioning state
        #[arg(long)]
        status: bool,
    },

    /// Upload a local secrets JSON file to S3
    Upload {
        /// Local JSON file to upload (default: example_s3_secrets.json)
        #[arg(long, default_value = "example_s3_secrets.json")]
        file: String,
    },

    /// Parse a working.md-style file, save as secrets.json and upload to S3
    ImportMd {
        /// Path to the markdown secrets file
        #[arg(long, default_value = "working.md")]
        file: String,
    },

    /// List secret names from S3 (values are never printed)
    List {},

    /// Restrict file permissions on .env and secrets.json to the current user
    Secure {
        /// Paths to restrict (default: .env, secrets.json)
        #[arg(long)]
        paths: Vec<String>,
    },

    /// Import secrets into the Wave secret store (via wsh secret set)
    WaveImport {
        /// Only import the working key for each provider (requires prior benchmark)
        #[arg(long)]
        working_only: bool,
    },

    /// Run model benchmarks
    Benchmark {},

    /// Check / help set up the Rust toolchain
    RustSetup {},

    /// Diagnose S3 connectivity (list buckets, head configured bucket)
    S3Test {},

    /// Create the configured S3 bucket if it does not exist
    CreateBucket {},
}

async fn do_sync(cfg: &PluginConfig, print_exports: bool) -> Result<()> {
    log_info!(
        "sync-secrets: fetching s3://{}/{}",
        cfg.s3.bucket,
        cfg.s3.object_key
    );
    let json = s3::fetch_object_to_string(&cfg.s3).await?;
    let pairs = secrets::parse_secrets(&json)?;
    secrets::update_env_file(&cfg.secrets, &pairs)?;
    if let Err(e) = secrets::restrict_file_permissions(&cfg.secrets.output_env_file) {
        log_warn!(
            "sync-secrets: could not restrict {}: {:#}",
            cfg.secrets.output_env_file,
            e
        );
    }
    log_info!(
        "sync-secrets: wrote {} secrets to {}",
        pairs.len(),
        cfg.secrets.output_env_file
    );
    println!(
        "Synced {} secrets from s3://{}/{} to {}",
        pairs.len(),
        cfg.s3.bucket,
        cfg.s3.object_key,
        cfg.secrets.output_env_file
    );
    for (k, v) in &pairs {
        log_debug!("sync-secrets: {} len={}", k, v.len());
    }
    if print_exports {
        secrets::print_shell_exports(&pairs);
    }
    Ok(())
}

fn resolve_wsh(cfg: &PluginConfig) -> Option<String> {
    if let Some(p) = cfg.wave.as_ref().and_then(|w| w.wsh_path.clone()) {
        if !p.is_empty() {
            return Some(p);
        }
    }
    if let Ok(p) = which::which("wsh") {
        return Some(p.to_string_lossy().to_string());
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let known = std::path::Path::new(&local)
                .join("waveterm")
                .join("Data")
                .join("bin")
                .join("wsh.exe");
            if known.exists() {
                return Some(known.to_string_lossy().to_string());
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let known = "~/.waveterm/bin/wsh";
        let expanded = known.replace("~", &std::env::var("HOME").unwrap_or_default());
        if std::path::Path::new(&expanded).exists() {
            return Some(expanded);
        }
    }
    None
}

async fn do_wave_import(
    cfg: &PluginConfig,
    pairs: &[(String, String)],
    working_keys: Option<&std::collections::HashSet<String>>,
) -> Result<()> {
    if std::env::var("WAVETERM_JWT").is_err() {
        println!(
            "[wave] not running inside Wave (WAVETERM_JWT missing) - skipping Wave secret import"
        );
        log_warn!("wave-import: skipped - WAVETERM_JWT not set (not running inside Wave)");
        return Ok(());
    }
    let Some(wsh) = resolve_wsh(cfg) else {
        println!("[wave] wsh binary not found - skipping Wave secret import");
        log_warn!("wave-import: skipped - wsh binary not found");
        return Ok(());
    };

    log_info!("wave-import: using wsh at {}", wsh);

    let mut names: Vec<(String, String)> = Vec::new();

    // Filter pairs based on working_keys if provided
    let filtered_pairs: Vec<_> = if let Some(working) = working_keys {
        pairs
            .iter()
            .filter(|(k, _)| working.contains(k))
            .cloned()
            .collect()
    } else {
        pairs.to_vec()
    };

    for (name, value) in &filtered_pairs {
        if secrets::is_placeholder(value) {
            log_debug!("wave-import: skip {} (placeholder value)", name);
            continue;
        }
        if !name
            .chars()
            .next()
            .map(|c| c.is_ascii_alphabetic())
            .unwrap_or(false)
            || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            log_debug!("wave-import: skip {} (invalid name)", name);
            continue;
        }
        names.push((name.clone(), value.clone()));
    }

    // Add Wave aliases for the working keys
    for (a, b) in secrets::wave_aliases(&filtered_pairs) {
        if !names.iter().any(|(k, _)| k == &a) {
            names.push((a, b));
        }
    }

    let mut set = 0usize;
    let skipped = 0usize;
    let mut tasks = tokio::task::JoinSet::new();
    let max_concurrent = 8usize;

    for (name, value) in &names {
        let wsh = wsh.clone();
        let name = name.clone();
        let value = value.clone();
        tasks.spawn(async move {
            let out = tokio::process::Command::new(&wsh)
                .args(["secret", "set", &format!("{}={}", name, value)])
                .output()
                .await;
            (name, out)
        });
        if tasks.len() >= max_concurrent {
            if let Some(res) = tasks.join_next().await {
                handle_wave_set(res, &mut set);
            }
        }
    }
    while let Some(res) = tasks.join_next().await {
        handle_wave_set(res, &mut set);
    }

    log_info!("wave-import: set {} secret(s), {} skipped", set, skipped);
    println!(
        "[wave] imported {} secret(s) into Wave ({} placeholder(s) skipped)",
        set, skipped
    );
    Ok(())
}

fn handle_wave_set(
    res: Result<(String, std::io::Result<std::process::Output>), tokio::task::JoinError>,
    set: &mut usize,
) {
    match res {
        Ok((name, Ok(out))) if out.status.success() => {
            *set += 1;
            println!("[wave] set secret {}", name);
            log_debug!("wave-import: set {}", name);
        }
        Ok((name, Ok(out))) => {
            println!(
                "[wave] FAILED to set {}: {}",
                name,
                String::from_utf8_lossy(&out.stderr).trim()
            );
            log_error!(
                "wave-import: failed to set {}: {}",
                name,
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok((name, Err(e))) => {
            println!("[wave] FAILED to spawn wsh secret set {}: {}", name, e);
            log_error!("wave-import: spawn error for {}: {}", name, e);
        }
        Err(e) => {
            println!("[wave] FAILED to join wsh secret set: {}", e);
            log_error!("wave-import: join error: {}", e);
        }
    }
}

/// Sends a Wave desktop notification via wsh notify. Best-effort: silently
/// skipped when not running inside Wave or wsh is unavailable.
async fn wave_notify(cfg: &PluginConfig, title: &str, body: &str) {
    if std::env::var("WAVETERM_JWT").is_err() {
        log_debug!("wave-notify: skipped (not inside Wave)");
        return;
    }
    let Some(wsh) = resolve_wsh(cfg) else {
        log_debug!("wave-notify: skipped (wsh not found)");
        return;
    };
    match tokio::process::Command::new(&wsh)
        .args(["notify", body, "-t", title])
        .output()
        .await
    {
        Ok(o) if o.status.success() => log_info!("wave-notify: sent '{title}'"),
        Ok(o) => log_warn!(
            "wave-notify: failed: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ),
        Err(e) => log_warn!("wave-notify: error: {e}"),
    }
}

async fn run_benchmark(cfg: &PluginConfig) -> Result<Vec<benchmark::ModelBenchmarkResult>> {
    match &cfg.benchmark {
        Some(bcfg) => {
            log_info!("benchmark: starting {} model(s)", bcfg.models.len());
            let results = benchmark::run_benchmarks(bcfg).await?;
            println!("{}", benchmark::format_results(&results));
            for r in &results {
                if r.ok {
                    log_info!(
                        "benchmark: {} OK ({}ms) via {}",
                        r.model_id,
                        r.latency_ms,
                        r.worked_key.as_deref().unwrap_or("?")
                    );
                } else {
                    log_error!("benchmark: {} FAILED: {:?}", r.model_id, r.error);
                }
            }
            Ok(results)
        }
        None => {
            log_warn!("benchmark: no [benchmark] section in config");
            println!("No [benchmark] section in config.");
            Ok(Vec::new())
        }
    }
}

/// Self-healing: if an alternate key works but the primary does not, swap the
/// values in S3 so the working key becomes primary, then re-sync .env.
async fn promote_working_keys(
    cfg: &PluginConfig,
    results: &[benchmark::ModelBenchmarkResult],
) -> Result<Vec<String>> {
    use std::collections::BTreeMap;
    let mut by_provider: BTreeMap<&str, &benchmark::ModelBenchmarkResult> = BTreeMap::new();
    for r in results {
        let p = benchmark::provider_of(&r.model_id);
        if r.ok && !by_provider.contains_key(p) {
            by_provider.insert(p, r);
        }
    }

    let mut messages = Vec::new();
    for (provider, r) in &by_provider {
        let Some(names) = benchmark::candidate_env_names(provider) else {
            continue;
        };
        let primary = names[0];
        let Some(worked) = &r.worked_key else {
            continue;
        };
        if worked == primary {
            continue;
        }
        let json = s3::fetch_object_to_string(&cfg.s3).await?;
        let mut pairs = secrets::parse_secrets(&json)?;
        let primary_val = pairs
            .iter()
            .find(|(k, _)| k == primary)
            .map(|(_, v)| v.clone());
        let worked_val = pairs
            .iter()
            .find(|(k, _)| k == worked)
            .map(|(_, v)| v.clone());
        match (primary_val, worked_val) {
            (Some(pv), Some(wv)) => {
                for (k, v) in pairs.iter_mut() {
                    if k == primary {
                        *v = wv.clone();
                    } else if k == worked {
                        *v = pv.clone();
                    }
                }
                let new_json = secrets::pairs_to_json(&pairs);
                s3::put_object_string(&cfg.s3, &new_json).await?;
                secrets::update_env_file(&cfg.secrets, &pairs)?;
                if let Err(e) = secrets::restrict_file_permissions(&cfg.secrets.output_env_file) {
                    log_warn!(
                        "promotion: could not restrict {}: {:#}",
                        cfg.secrets.output_env_file,
                        e
                    );
                }
                let msg = format!(
                    "{}: promoted working key {} -> primary {} (S3 updated + .env re-synced)",
                    provider, worked, primary
                );
                log_info!("promotion: {}", msg);
                messages.push(msg);
            }
            _ => log_warn!(
                "promotion: {} and {} not both found in S3 data",
                primary,
                worked
            ),
        }
    }
    Ok(messages)
}

/// Writes key-bitcher-notifications.md listing providers whose keys failed, with a
/// link to get a replacement, plus any promotions that were applied.
fn write_notifications(
    results: &[benchmark::ModelBenchmarkResult],
    swaps: &[String],
) -> Result<()> {
    use std::collections::BTreeMap;
    let mut failed: BTreeMap<&str, Vec<&benchmark::ModelBenchmarkResult>> = BTreeMap::new();
    for r in results {
        if !r.ok {
            failed
                .entry(benchmark::provider_of(&r.model_id))
                .or_default()
                .push(r);
        }
    }

    let mut lines = Vec::new();
    lines.push("# Key-Bitcher notifications".to_string());
    lines.push(String::new());
    lines.push(format!("Generated: {}", logging::now_ms()));
    lines.push(String::new());

    if failed.is_empty() && swaps.is_empty() {
        lines.push("All benchmarked provider keys are working. Nothing to do.".to_string());
    }

    for (provider, fails) in &failed {
        let link = benchmark::key_link_for(provider).unwrap_or("(no link known)");
        lines.push(format!("## {} - keys rejected", provider));
        lines.push(String::new());
        for f in fails {
            lines.push(format!(
                "- {}: {}",
                f.model_id,
                f.error.as_deref().unwrap_or("unknown error")
            ));
        }
        if let Some(names) = benchmark::candidate_env_names(provider) {
            lines.push(format!("- Key env names: {}", names.join(", ")));
        }
        lines.push(format!("- Get a new key / regenerate: {}", link));
        lines.push(String::new());
    }

    for s in swaps {
        lines.push(format!("- {}", s));
    }
    if !swaps.is_empty() {
        lines.push(String::new());
    }

    lines.push(
        "To update S3: paste the new key into working.md and run key-bitcher import-md --file <path>."
            .to_string(),
    );
    lines.push(String::new());

    let content = lines.join("\n");
    std::fs::write(NOTIFICATION_FILE, &content)?;
    log_info!(
        "notifications: wrote {} ({} provider failure(s), {} swap(s))",
        NOTIFICATION_FILE,
        failed.len(),
        swaps.len()
    );
    println!(
        "Notified: see {} ({} provider(s) need new keys)",
        NOTIFICATION_FILE,
        failed.len()
    );
    for provider in failed.keys() {
        let link = benchmark::key_link_for(provider).unwrap_or("(no link known)");
        println!("  - {}: get a new key at {}", provider, link);
    }
    Ok(())
}

/// Re-reads the env file into the process environment. Used after a sync so
/// benchmark/import steps see the freshly written keys, not the env loaded at
/// startup.
fn reload_env_from_file(path: &str) {
    match secrets::read_env_file(path) {
        Ok(pairs) => {
            for (k, v) in pairs {
                std::env::set_var(k, v);
            }
            log_debug!("env: reloaded vars from {}", path);
        }
        Err(e) => log_warn!("env: could not reload {}: {:#}", path, e),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let started = logging::now_ms();
    let result = run().await;
    match &result {
        Ok(()) => log_info!("run: finished OK in {}ms", logging::now_ms() - started),
        Err(e) => log_error!("run: FAILED in {}ms: {:#}", logging::now_ms() - started, e),
    }
    result
}

async fn run() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    let config_path = if std::path::Path::new(&cli.config).exists() {
        cli.config.clone()
    } else if std::path::Path::new("plugin_config.toml").exists() {
        log_warn!(
            "startup: '{}' not found, falling back to legacy plugin_config.toml",
            cli.config
        );
        "plugin_config.toml".to_string()
    } else {
        cli.config.clone()
    };
    let cfg_result = PluginConfig::load(&config_path);
    let log_max = cfg_result
        .as_ref()
        .ok()
        .and_then(|c| c.logging.as_ref())
        .and_then(|l| l.max_size_bytes);
    logging::init(&cli.log_file, cli.debug, log_max)?;
    let cfg = cfg_result?;
    log_info!(
        "startup: cmd={:?} config={} region={} bucket={}",
        cli.command
            .as_ref()
            .map(|c| format!("{c:?}"))
            .unwrap_or_else(|| "auto".to_string()),
        config_path,
        cfg.s3.region,
        cfg.s3.bucket
    );

    match cli.command {
        Some(Commands::SyncSecrets {
            print_exports,
            dry_run,
        }) => {
            if cli.auto_sync {
                log_info!("auto-sync: performing sync first");
            }
            if dry_run {
                let json = s3::fetch_object_to_string(&cfg.s3).await?;
                let incoming = secrets::parse_secrets(&json)?;
                let current = secrets::read_env_file(&cfg.secrets.output_env_file)?;
                let changes = secrets::diff_env(&current, &incoming);
                if changes.is_empty() {
                    println!(
                        "dry-run: no changes ({} secret(s) already in sync)",
                        incoming.len()
                    );
                } else {
                    for (name, change) in &changes {
                        println!("  {} {}", change, name);
                    }
                    println!("dry-run: {} change(s) pending", changes.len());
                }
                log_info!(
                    "sync-secrets: dry-run: {} secret(s) from s3, {} change(s) pending",
                    incoming.len(),
                    changes.len()
                );
                return Ok(());
            }
            do_sync(&cfg, print_exports).await?;
            let pairs = secrets::read_env_file(&cfg.secrets.output_env_file)?;
            do_wave_import(&cfg, &pairs, None).await?;
        }

        Some(Commands::Validate { source }) => {
            let pairs = match source.as_str() {
                "s3" => {
                    let json = s3::fetch_object_to_string(&cfg.s3).await?;
                    secrets::parse_secrets(&json)?
                }
                "env" => secrets::read_env_file(&cfg.secrets.output_env_file)?,
                other => anyhow::bail!("unknown source '{other}' (use 'env' or 's3')"),
            };
            let warnings = secrets::validate_pairs(&pairs);
            if warnings.is_empty() {
                println!(
                    "✓ validated {} secret(s) from {source} - no issues",
                    pairs.len()
                );
                log_info!("validate: {} secret(s) from {source} - OK", pairs.len());
            } else {
                println!("{} warning(s) for {source}:", warnings.len());
                for w in &warnings {
                    println!("  ! {w}");
                    log_warn!("validate: {w}");
                }
                anyhow::bail!("{} validation warning(s) in {source}", warnings.len());
            }
        }

        Some(Commands::WaveTest {}) => {
            println!("Wave integration self-test");
            match std::env::var("WAVETERM_JWT") {
                Ok(_) => println!("  ✓ WAVETERM_JWT present (inside Wave)"),
                Err(_) => println!("  ! WAVETERM_JWT missing (not inside Wave)"),
            }
            let Some(wsh) = resolve_wsh(&cfg) else {
                println!("  ! wsh binary not found - nothing to test");
                log_warn!("wave-test: wsh binary not found");
                return Ok(());
            };
            println!("  ✓ wsh at {wsh}");
            log_info!("wave-test: using wsh at {}", wsh);
            let out = tokio::process::Command::new(&wsh)
                .args(["secret", "list"])
                .output()
                .await;
            match out {
                Ok(o) if o.status.success() => {
                    let text = String::from_utf8_lossy(&o.stdout);
                    let n = text.lines().filter(|l| !l.trim().is_empty()).count();
                    println!("  ✓ wsh secret list returned {n} secret(s)");
                    log_info!("wave-test: wsh secret list returned {} secret(s)", n);
                }
                Ok(o) => println!(
                    "  ! wsh secret list failed: {}",
                    String::from_utf8_lossy(&o.stderr).trim()
                ),
                Err(e) => println!("  ! could not run wsh: {e}"),
            }
        }

        Some(Commands::S3Versioning { status }) => {
            if status {
                match s3::versioning_status(&cfg.s3).await {
                    Ok(s) => println!("Versioning on {}: {}", cfg.s3.bucket, s),
                    Err(e) => {
                        log_error!("s3-versioning: could not read state: {:#}", e);
                        println!("Could not read versioning state: {e:?}");
                    }
                }
            } else {
                s3::enable_versioning(&cfg.s3).await?;
                log_info!("s3-versioning: enabled on {}", cfg.s3.bucket);
                println!("Versioning enabled on {}", cfg.s3.bucket);
            }
        }

        Some(Commands::Upload { file }) => {
            if cli.auto_sync {
                do_sync(&cfg, false).await?;
            }
            let content = std::fs::read_to_string(&file)?;
            s3::put_object_string(&cfg.s3, &content).await?;
            log_info!(
                "upload: {} -> s3://{}/{}",
                file,
                cfg.s3.bucket,
                cfg.s3.object_key
            );
            println!(
                "Uploaded {} -> s3://{}/{}",
                file, cfg.s3.bucket, cfg.s3.object_key
            );
        }

        Some(Commands::ImportMd { file }) => {
            let (pairs, skipped) =
                workingmd::parse_working_md_with_sections(&file, cfg.sections.as_ref())?;
            for warning in secrets::validate_pairs(&pairs) {
                log_warn!("import-md: {}", warning);
                println!("  WARN: {}", warning);
            }
            let json = secrets::pairs_to_json(&pairs);
            s3::put_object_string(&cfg.s3, &json).await?;
            log_info!(
                "import-md: {} secrets from {} ({} skipped) -> s3://{}/{}",
                pairs.len(),
                file,
                skipped,
                cfg.s3.bucket,
                cfg.s3.object_key
            );
            println!(
                "Imported {} secrets from {} ({} entries skipped) -> s3://{}/{}",
                pairs.len(),
                file,
                skipped,
                cfg.s3.bucket,
                cfg.s3.object_key
            );
            for (name, value) in &pairs {
                log_debug!("import-md: {} len={}", name, value.len());
                println!("  {}", name);
            }
        }

        Some(Commands::List {}) => {
            let json = s3::fetch_object_to_string(&cfg.s3).await?;
            let pairs = secrets::parse_secrets(&json)?;
            log_info!(
                "list: {} secret name(s) from s3://{}/{}",
                pairs.len(),
                cfg.s3.bucket,
                cfg.s3.object_key
            );
            for (name, value) in &pairs {
                println!("{} (len={})", name, value.len());
                log_debug!("list: {} len={}", name, value.len());
            }
            println!("{} secret(s)", pairs.len());
        }

        Some(Commands::Secure { paths }) => {
            let mut targets = paths;
            if targets.is_empty() {
                targets.push(cfg.secrets.output_env_file.clone());
                targets.push("secrets.json".to_string());
            }
            for path in &targets {
                if !std::path::Path::new(path).exists() {
                    log_warn!("secure: {} does not exist - skipping", path);
                    println!("[secure] {}: not present, skipping", path);
                    continue;
                }
                secrets::restrict_file_permissions(path)?;
                log_info!("secure: restricted {}", path);
                println!("[secure] restricted {}", path);
            }
        }

        Some(Commands::WaveImport { working_only }) => {
            let pairs = secrets::read_env_file(&cfg.secrets.output_env_file)?;
            log_info!(
                "wave-import: reading {} keys from {}",
                pairs.len(),
                cfg.secrets.output_env_file
            );

            let working_keys = if working_only {
                // Run benchmark first to determine working keys
                let results = run_benchmark(&cfg).await?;
                let mut working = std::collections::HashSet::new();
                for r in &results {
                    if r.ok {
                        if let Some(ref key) = r.worked_key {
                            working.insert(key.clone());
                        }
                    }
                }
                Some(working)
            } else {
                None
            };

            do_wave_import(&cfg, &pairs, working_keys.as_ref()).await?;
        }

        Some(Commands::Benchmark {}) => {
            if cli.auto_sync {
                do_sync(&cfg, false).await?;
                reload_env_from_file(&cfg.secrets.output_env_file);
            }
            let results = run_benchmark(&cfg).await?;
            let swaps = promote_working_keys(&cfg, &results).await?;
            write_notifications(&results, &swaps)?;
            let (ok, total) = benchmark::ok_count(&results);
            let title = format!("key-bitcher benchmark: {}/{} models OK", ok, total);
            let body = if swaps.is_empty() {
                "No key promotions were needed.".to_string()
            } else {
                format!("Promoted {} working key(s).", swaps.len())
            };
            wave_notify(&cfg, &title, &body).await;
        }

        Some(Commands::RustSetup {}) => {
            rust_setup::check_rust_env()?;
        }

        Some(Commands::S3Test {}) => {
            match s3::list_buckets(&cfg.s3).await {
                Ok(names) => {
                    log_info!("s3-test: listed {} bucket(s)", names.len());
                    println!("Buckets reachable at {}:", s3::endpoint_for(&cfg.s3));
                    for n in names {
                        println!("  {}", n);
                    }
                }
                Err(e) => {
                    log_error!("s3-test: list-buckets failed: {:?}", e);
                    println!("list-buckets failed: {e:?}");
                }
            }
            match s3::bucket_exists(&cfg.s3).await {
                Ok(true) => println!("Bucket '{}' exists", cfg.s3.bucket),
                Ok(false) => {
                    log_warn!("s3-test: bucket '{}' not accessible", cfg.s3.bucket);
                    println!(
                        "Bucket '{}' does not exist or is not accessible",
                        cfg.s3.bucket
                    );
                }
                Err(e) => println!("head-bucket error: {e:?}"),
            }
        }

        Some(Commands::CreateBucket {}) => {
            s3::create_bucket(&cfg.s3).await?;
            log_info!("create-bucket: verified at {}", s3::endpoint_for(&cfg.s3));
            println!(
                "Bucket '{}' created/verified at {}",
                cfg.s3.bucket,
                s3::endpoint_for(&cfg.s3)
            );
        }

        None => {
            // Full automatic flow: sync -> benchmark (validate keys) -> notify +
            // promote working keys -> wave import (only after keys are verified).
            do_sync(&cfg, false).await?;
            reload_env_from_file(&cfg.secrets.output_env_file);
            let results = run_benchmark(&cfg).await?;
            let swaps = promote_working_keys(&cfg, &results).await?;
            write_notifications(&results, &swaps)?;

            // Only import working keys to Wave
            let mut working = std::collections::HashSet::new();
            for r in &results {
                if r.ok {
                    if let Some(ref key) = r.worked_key {
                        working.insert(key.clone());
                    }
                }
            }
            let pairs = secrets::read_env_file(&cfg.secrets.output_env_file)?;
            do_wave_import(&cfg, &pairs, Some(&working)).await?;

            let (ok, total) = benchmark::ok_count(&results);
            let title = format!("key-bitcher auto flow: {}/{} models OK", ok, total);
            let body = if swaps.is_empty() {
                "Sync + benchmark + wave import done.".to_string()
            } else {
                format!(
                    "Sync done, promoted {} key(s), wave import done.",
                    swaps.len()
                )
            };
            wave_notify(&cfg, &title, &body).await;
        }
    }

    Ok(())
}
