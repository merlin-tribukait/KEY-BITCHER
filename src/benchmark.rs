use crate::config::BenchmarkConfig;
use crate::log_debug;
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelBenchmarkResult {
    pub model_id: String,
    pub latency_ms: u128,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub ok: bool,
    /// The env-var name whose key actually authenticated successfully.
    pub worked_key: Option<String>,
    pub error: Option<String>,
}

/// Per-provider URLs to get / regenerate an API key.
pub const PROVIDER_KEY_LINKS: &[(&str, &str)] = &[
    ("openai", "https://platform.openai.com/api-keys"),
    ("openrouter", "https://openrouter.ai/keys"),
    ("anthropic", "https://console.anthropic.com/settings/keys"),
    ("mistral", "https://console.mistral.ai/api-keys"),
    ("cohere", "https://dashboard.cohere.com/api-keys"),
    ("google", "https://aistudio.google.com/app/apikey"),
    ("xai", "https://console.x.ai/api-keys"),
    ("groq", "https://console.groq.com/keys"),
    ("deepseek", "https://platform.deepseek.com/api_keys"),
    ("cerebras", "https://console.cerebras.ai/account/api-keys"),
    ("huggingface", "https://huggingface.co/settings/tokens"),
    ("nvidia", "https://build.nvidia.com"),
    ("elevenlabs", "https://elevenlabs.io/app/settings/api-keys"),
    ("deepgram", "https://console.deepgram.com/api-keys"),
    (
        "cloudflare",
        "https://dash.cloudflare.com/profile/api-tokens",
    ),
    ("vercel", "https://vercel.com/account/tokens"),
    ("github", "https://github.com/settings/tokens"),
    ("qdrant", "https://cloud.qdrant.io/"),
];

/// Provider name from a model id like `openai:gpt-4o-mini`.
pub fn provider_of(model_id: &str) -> &str {
    model_id.split(':').next().unwrap_or(model_id)
}

/// Counts (ok, total) benchmark results.
pub fn ok_count(results: &[ModelBenchmarkResult]) -> (usize, usize) {
    (results.iter().filter(|r| r.ok).count(), results.len())
}

pub fn key_link_for(provider: &str) -> Option<&'static str> {
    PROVIDER_KEY_LINKS
        .iter()
        .find(|(p, _)| p == &provider)
        .map(|(_, url)| *url)
}

/// The candidate env-var names a provider's benchmark tries, in priority order.
pub fn candidate_env_names(provider: &str) -> Option<&'static [&'static str]> {
    match provider {
        "openai" => Some(&["OPENAI_API_KEY", "OPENAI_API_KEY_2"]),
        "openrouter" => Some(&[
            "OPENROUTER_API_KEY_CURRENT",
            "OPENROUTER_API_KEY",
            "OPENROUTER_API_KEY_ALT",
            "OPENROUTER_API_KEY_ALT2",
            "OPENROUTER_API_KEY_ALT3",
        ]),
        "anthropic" => Some(&[
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_API_KEY_2",
            "ANTHROPIC_API_KEY_3",
        ]),
        "mistral" => Some(&["MISTRAL_API_KEY"]),
        "groq" => Some(&["GROQ_API_KEY"]),
        _ => None,
    }
}

pub async fn run_benchmarks(cfg: &BenchmarkConfig) -> Result<Vec<ModelBenchmarkResult>> {
    let client = Client::new();
    let mut results = Vec::new();

    for model in &cfg.models {
        let start = Instant::now();
        let mut res = ModelBenchmarkResult {
            model_id: model.clone(),
            latency_ms: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            ok: false,
            worked_key: None,
            error: None,
        };

        let r = bench_single(&client, model, cfg).await;

        res.latency_ms = start.elapsed().as_millis();
        match r {
            Ok((key_name, pt, ct)) => {
                res.ok = true;
                res.worked_key = Some(key_name);
                res.prompt_tokens = pt;
                res.completion_tokens = ct;
            }
            Err(e) => {
                res.ok = false;
                res.error = Some(e.to_string());
            }
        }

        results.push(res);
    }

    Ok(results)
}

async fn bench_single(
    client: &Client,
    model: &str,
    cfg: &BenchmarkConfig,
) -> Result<(String, u32, u32)> {
    let provider = provider_of(model);
    if let Some(rest) = model.strip_prefix("openai:") {
        bench_openai(client, rest, cfg, provider).await
    } else if let Some(rest) = model.strip_prefix("openrouter:") {
        bench_openrouter(client, rest, cfg, provider).await
    } else if let Some(rest) = model.strip_prefix("anthropic:") {
        bench_anthropic(client, rest, cfg, provider).await
    } else if let Some(rest) = model.strip_prefix("mistral:") {
        bench_mistral(client, rest, cfg, provider).await
    } else if let Some(rest) = model.strip_prefix("groq:") {
        bench_openai_compatible(
            client,
            rest,
            cfg,
            provider,
            "https://api.groq.com/openai/v1/chat/completions",
        )
        .await
    } else {
        anyhow::bail!("Unknown provider prefix in model '{}'", model);
    }
}

fn timeout(cfg: &BenchmarkConfig) -> Duration {
    Duration::from_secs(cfg.request_timeout_secs.unwrap_or(30))
}

/// Returns (env_name, value) for every candidate env var that is set, non-empty
/// and not a placeholder. The benchmark tries each in order and reports which
/// key actually works.
fn candidate_keys(names: &[&str]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for n in names {
        if let Ok(v) = std::env::var(n) {
            if !v.trim().is_empty() && !crate::secrets::is_placeholder(&v) {
                out.push((n.to_string(), v));
            }
        }
    }
    out
}

fn reject_all_err(provider: &str, keys: &[(String, String)]) -> anyhow::Error {
    let tried = keys
        .iter()
        .map(|(n, _)| n.clone())
        .collect::<Vec<_>>()
        .join(", ");
    anyhow::anyhow!("all {} keys rejected (tried {})", provider, tried)
}

fn max_tokens(cfg: &BenchmarkConfig) -> u32 {
    cfg.max_output_tokens.unwrap_or(256)
}

fn chat_body(model: &str, mt: u32) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": [ { "role": "user", "content": "Say 'ok'." } ],
        "max_tokens": mt
    })
}

fn usage_tokens(usage: &serde_json::Value, prompt_key: &str, completion_key: &str) -> (u32, u32) {
    let pt = usage.get(prompt_key).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let ct = usage
        .get(completion_key)
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    (pt, ct)
}

async fn bench_openai(
    client: &Client,
    model: &str,
    cfg: &BenchmarkConfig,
    provider: &str,
) -> Result<(String, u32, u32)> {
    bench_openai_compatible(
        client,
        model,
        cfg,
        provider,
        "https://api.openai.com/v1/chat/completions",
    )
    .await
}

async fn bench_openrouter(
    client: &Client,
    model: &str,
    cfg: &BenchmarkConfig,
    provider: &str,
) -> Result<(String, u32, u32)> {
    bench_openai_compatible(
        client,
        model,
        cfg,
        provider,
        "https://openrouter.ai/api/v1/chat/completions",
    )
    .await
}

/// OpenAI-chat-compatible endpoint (OpenAI, OpenRouter, Mistral, Groq, ...).
/// Tries every candidate key for the provider and returns the first that
/// authenticates.
async fn bench_openai_compatible(
    client: &Client,
    model: &str,
    cfg: &BenchmarkConfig,
    provider: &str,
    endpoint: &str,
) -> Result<(String, u32, u32)> {
    let names = candidate_env_names(provider).unwrap_or(&[]);
    let keys = candidate_keys(names);
    if keys.is_empty() {
        anyhow::bail!("no usable {} key in env", provider);
    }
    for (name, key) in &keys {
        let outcome = async {
            let resp = client
                .post(endpoint)
                .timeout(timeout(cfg))
                .bearer_auth(key)
                .json(&chat_body(model, max_tokens(cfg)))
                .send()
                .await?;
            let resp = resp.error_for_status()?;
            resp.json().await
        }
        .await;
        let body: serde_json::Value = match outcome {
            Ok(b) => b,
            Err(e) => {
                log_debug!("benchmark: {}: {} rejected: {:#}", provider, name, e);
                continue;
            }
        };
        let usage = body.get("usage").cloned().unwrap_or_default();
        log_debug!("benchmark: {}: {} works", provider, name);
        let (pt, ct) = usage_tokens(&usage, "prompt_tokens", "completion_tokens");
        return Ok((name.clone(), pt, ct));
    }
    Err(reject_all_err(provider, &keys))
}

async fn bench_anthropic(
    client: &Client,
    model: &str,
    cfg: &BenchmarkConfig,
    provider: &str,
) -> Result<(String, u32, u32)> {
    let names = candidate_env_names(provider).unwrap_or(&[]);
    let keys = candidate_keys(names);
    if keys.is_empty() {
        anyhow::bail!("no usable {} key in env", provider);
    }
    let body = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens(cfg),
        "messages": [
            { "role": "user", "content": [ { "type": "text", "text": "Say 'ok'." } ] }
        ]
    });

    for (name, key) in &keys {
        let outcome = async {
            let resp = client
                .post("https://api.anthropic.com/v1/messages")
                .timeout(timeout(cfg))
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .json(&body)
                .send()
                .await?;
            let resp = resp.error_for_status()?;
            resp.json().await
        }
        .await;
        let resp: serde_json::Value = match outcome {
            Ok(b) => b,
            Err(e) => {
                log_debug!("benchmark: anthropic: {} rejected: {:#}", name, e);
                continue;
            }
        };
        let usage = resp.get("usage").cloned().unwrap_or_default();
        log_debug!("benchmark: anthropic: {} works", name);
        let (pt, ct) = usage_tokens(&usage, "input_tokens", "output_tokens");
        return Ok((name.clone(), pt, ct));
    }
    Err(reject_all_err("anthropic", &keys))
}

/// ANSI reset used by the fancy console rendering.
pub const RESET: &str = "\x1b[0m";

/// Brand metadata used to render benchmark results with color and a small
/// ASCII logo per provider.
pub struct ProviderBrand {
    pub id: &'static str,
    pub display: &'static str,
    /// ANSI foreground escape (e.g. "\x1b[38;5;42m") for logo + provider name.
    pub color: &'static str,
    pub logo: &'static [&'static str],
}

const OPENAI_LOGO: &[&str] = &[
    "   ╭────╮",
    "  ╭╯ ╭╮ ╰╮",
    "  │  ╰╯  │",
    "  ╰╮    ╭╯",
    "   ╰────╯",
];
const OPENROUTER_LOGO: &[&str] = &[
    "   ╭────╮",
    "  ╭╯ ◉ ╰──╮",
    "  ╰────╯  │",
    "       ╭──╯",
    "       ╰──╯",
];
const ANTHROPIC_LOGO: &[&str] = &["     ▲", "    ▲ ▲", "   ▲   ▲", "  ▲     ▲", " ▲───────▲"];
const MISTRAL_LOGO: &[&str] = &[" ▲   ▲", "▲ ▲ ▲ ▲", "▲  ▲  ▲"];
const GOOGLE_LOGO: &[&str] = &["  ╭────╮", "  │  ╭─╯", "  │  │", "  ╰──╯"];
const XAI_LOGO: &[&str] = &[" ◇   ◇", "  ◇ ◇", "   ◇", "  ◇ ◇", " ◇   ◇"];
const DEEPSEEK_LOGO: &[&str] = &[" ╭───╮", " │   │", " │   │", " ╰───╯"];
const CEREBRAS_LOGO: &[&str] = &[" ██████", " ██████", " ██████"];
const NVIDIA_LOGO: &[&str] = &["   ◇", "  ◇ ◇", " ◇   ◇", "  ◇ ◇", "   ◇"];
const GROQ_LOGO: &[&str] = &[" ▄▄▄▄▄▄", " █ ▀▀ █", " ▀▄▄▄▄▀", " ▀▀▀▀▀▀"];
const GENERIC_LOGO: &[&str] = &["   ◈", "  ◈ ◈", " ◈ ◈ ◈", "  ◈ ◈", "   ◈"];

pub const BRANDS: &[ProviderBrand] = &[
    ProviderBrand {
        id: "openai",
        display: "OPENAI",
        color: "\x1b[38;5;42m",
        logo: OPENAI_LOGO,
    },
    ProviderBrand {
        id: "openrouter",
        display: "OPENROUTER",
        color: "\x1b[38;5;69m",
        logo: OPENROUTER_LOGO,
    },
    ProviderBrand {
        id: "anthropic",
        display: "ANTHROPIC",
        color: "\x1b[38;5;208m",
        logo: ANTHROPIC_LOGO,
    },
    ProviderBrand {
        id: "mistral",
        display: "MISTRAL",
        color: "\x1b[38;5;214m",
        logo: MISTRAL_LOGO,
    },
    ProviderBrand {
        id: "google",
        display: "GOOGLE",
        color: "\x1b[38;5;27m",
        logo: GOOGLE_LOGO,
    },
    ProviderBrand {
        id: "xai",
        display: "XAI",
        color: "\x1b[38;5;231m",
        logo: XAI_LOGO,
    },
    ProviderBrand {
        id: "deepseek",
        display: "DEEPSEEK",
        color: "\x1b[38;5;39m",
        logo: DEEPSEEK_LOGO,
    },
    ProviderBrand {
        id: "cerebras",
        display: "CEREBRAS",
        color: "\x1b[38;5;45m",
        logo: CEREBRAS_LOGO,
    },
    ProviderBrand {
        id: "nvidia",
        display: "NVIDIA",
        color: "\x1b[38;5;76m",
        logo: NVIDIA_LOGO,
    },
    ProviderBrand {
        id: "groq",
        display: "GROQ",
        color: "\x1b[38;5;203m",
        logo: GROQ_LOGO,
    },
    ProviderBrand {
        id: "__generic__",
        display: "AI",
        color: "\x1b[38;5;99m",
        logo: GENERIC_LOGO,
    },
];

pub fn brand_of(provider: &str) -> &'static ProviderBrand {
    BRANDS
        .iter()
        .find(|b| b.id == provider)
        .unwrap_or_else(|| &BRANDS[BRANDS.len() - 1])
}

fn title_box(title: &str) -> String {
    let width = title.chars().count().max(24) + 4;
    format!(
        "┌{}┐\n│  {}  │\n└{}┘\n",
        "─".repeat(width),
        title,
        "─".repeat(width)
    )
}

/// Renders all benchmark results: a title banner, one colored ASCII-logo block
/// per provider, and a summary box with links for the failing providers.
pub fn format_results(results: &[ModelBenchmarkResult]) -> String {
    let mut out = String::new();
    out.push_str("\x1b[1m");
    out.push_str(&title_box("KEY-BITCHER · KEY BENCHMARK"));
    out.push_str(RESET);
    out.push('\n');
    if results.is_empty() {
        out.push_str("  (no models configured)\n");
        return out;
    }
    for r in results {
        out.push_str(&format_result(r));
        out.push('\n');
    }
    out.push_str(&format_summary(results));
    out
}

fn detail_lines(r: &ModelBenchmarkResult, brand: &ProviderBrand) -> Vec<String> {
    let status = if r.ok {
        "\x1b[1;32m✓\x1b[0m".to_string()
    } else {
        "\x1b[1;31m✗\x1b[0m".to_string()
    };
    let mut v = Vec::new();
    v.push(format!(
        "  {status}  {}{}{}",
        brand.color, brand.display, RESET
    ));
    v.push(format!("  {}", r.model_id));
    if r.ok {
        v.push(format!(
            "  OK · {}ms · {}→{} tok",
            r.latency_ms, r.prompt_tokens, r.completion_tokens
        ));
        v.push(format!(
            "  via \x1b[1m{}\x1b[0m",
            r.worked_key.as_deref().unwrap_or("?")
        ));
    } else {
        v.push(format!("  REJECTED · {}ms", r.latency_ms));
        v.push(format!("  tried: {}", tried_names(r)));
    }
    v
}

fn tried_names(r: &ModelBenchmarkResult) -> String {
    match &r.error {
        Some(e) => match e.find("tried ") {
            Some(idx) => e[idx + 6..].trim_end_matches(')').to_string(),
            None => e.clone(),
        },
        None => "?".to_string(),
    }
}

fn format_result(r: &ModelBenchmarkResult) -> String {
    let brand = brand_of(provider_of(&r.model_id));
    let mut logo: Vec<String> = brand.logo.iter().map(|s| s.to_string()).collect();
    let mut details = detail_lines(r, brand);
    let h = logo.len().max(details.len());
    logo.resize(h, String::new());
    details.resize(h, String::new());

    let mut out = String::new();
    for i in 0..h {
        out.push_str(&format!(
            "{}{}{} {}\n",
            brand.color, logo[i], RESET, details[i]
        ));
    }
    out
}

fn format_summary(results: &[ModelBenchmarkResult]) -> String {
    let ok = results.iter().filter(|r| r.ok).count();
    let fail = results.len() - ok;
    let mut out = String::new();
    out.push_str(&title_box(&format!(
        "RESULT: {ok} OK · {fail} need new keys"
    )));
    for r in results.iter().filter(|r| !r.ok) {
        let p = provider_of(&r.model_id);
        if let Some(link) = key_link_for(p) {
            out.push_str(&format!("\x1b[1;31m  ✗\x1b[0m {:<12} {}\n", p, link));
        }
    }
    out
}

async fn bench_mistral(
    client: &Client,
    model: &str,
    cfg: &BenchmarkConfig,
    provider: &str,
) -> Result<(String, u32, u32)> {
    bench_openai_compatible(
        client,
        model,
        cfg,
        provider,
        "https://api.mistral.ai/v1/chat/completions",
    )
    .await
}
