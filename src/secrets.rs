use crate::config::SecretsConfig;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;

pub fn parse_secrets(json_str: &str) -> Result<Vec<(String, String)>> {
    let v: Value = serde_json::from_str(json_str)?;
    let obj = v
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Expected JSON object"))?;

    let mut pairs = Vec::new();
    for (k, v) in obj {
        if let Some(s) = v.as_str() {
            pairs.push((k.clone(), s.to_string()));
        }
    }
    Ok(pairs)
}

pub fn pairs_to_json(pairs: &[(String, String)]) -> String {
    let mut obj = serde_json::Map::new();
    for (k, v) in pairs {
        obj.insert(k.clone(), json!(v));
    }
    serde_json::to_string_pretty(&Value::Object(obj)).unwrap_or_default()
}

pub fn update_env_file(cfg: &SecretsConfig, pairs: &[(String, String)]) -> Result<()> {
    let path = &cfg.output_env_file;

    if !cfg.overwrite_existing && std::path::Path::new(path).exists() {
        bail!("Env file {} exists and overwrite_existing = false", path);
    }

    // Preserve existing AWS_* variables (plugin credentials live in .env and
    // must survive each sync).
    let mut existing = if std::path::Path::new(path).exists() {
        read_env_file(path)?
    } else {
        Vec::new()
    };
    existing.retain(|(k, _)| k.starts_with("AWS_"));
    existing.retain(|(k, _)| !pairs.iter().any(|(pk, _)| pk == k));

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;

    for (k, v) in existing {
        writeln!(file, "{}={}", k, v)?;
    }
    for (k, v) in pairs {
        // Do NOT log values; only write to file.
        writeln!(file, "{}={}", k, v)?;
    }

    Ok(())
}

pub fn read_env_file(path: &str) -> Result<Vec<(String, String)>> {
    let content = fs::read_to_string(path)?;
    let mut pairs = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(idx) = line.find('=') {
            let k = line[..idx].trim().to_string();
            let v = line[idx + 1..].trim().to_string();
            if !k.is_empty() {
                pairs.push((k, v));
            }
        }
    }
    Ok(pairs)
}

pub fn print_shell_exports(pairs: &[(String, String)]) {
    for (k, v) in pairs {
        let escaped = v.replace('\'', "'\\''");
        println!("export {}='{}'", k, escaped);
    }
}

/// Returns true when the value still looks like a template placeholder.
pub fn is_placeholder(v: &str) -> bool {
    let up = v.to_uppercase();
    up.contains("PASTE") || up.contains("HERE") || up.contains("XXXX") || v.trim().is_empty()
}

/// Canonical Wave AI secret names for provider presets (auto-referenced).
/// Maps plugin env keys to the exact secret names Wave's `ai:provider`
/// presets look up.
pub fn wave_aliases(pairs: &[(String, String)]) -> Vec<(String, String)> {
    let map: &[(&str, &str)] = &[
        ("OPENAI_API_KEY", "OPENAI_KEY"),
        ("OPENROUTER_API_KEY", "OPENROUTER_KEY"),
        ("GOOGLE_API_KEY", "GOOGLE_AI_KEY"),
        ("MISTRAL_API_KEY", "MISTRAL_KEY"),
        ("DEEPSEEK_API_KEY", "DEEPSEEK_KEY"),
        ("XAI_API_KEY", "XAI_KEY"),
        ("GROQ_API_KEY", "GROQ_KEY"),
        ("NANOGPT_API_KEY", "NANOGPT_KEY"),
        ("NVIDIA_API_KEY", "NVIDIA_KEY"),
        ("CEREBRAS_API_KEY", "CEREBRAS_KEY"),
    ];
    let mut aliases = Vec::new();
    for (src, dst) in map {
        if let Some((_, v)) = pairs.iter().find(|(k, _)| k == src) {
            aliases.push((dst.to_string(), v.clone()));
        }
    }
    aliases
}

/// Restricts a file to the current user only.
/// Windows: `icacls` (remove inheritance, grant current user full control).
/// Unix: chmod 0600.
pub fn restrict_file_permissions(path: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let Some(user) = std::env::var("USERNAME").ok().filter(|u| !u.is_empty()) else {
            return Ok(());
        };
        let out = std::process::Command::new("icacls")
            .args([path, "/inheritance:r", "/grant:r", &format!("{user}:F")])
            .output();
        match out {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => bail!(
                "icacls failed on {}: {}",
                path,
                String::from_utf8_lossy(&o.stderr).trim()
            ),
            Err(e) => bail!("icacls error on {}: {}", path, e),
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(path)?;
        let mut perms = meta.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(path, perms)?;
        Ok(())
    }
}

/// Cheap sanity checks that a key's prefix matches the provider it is named
/// after. Returns human-readable warnings (not errors).
pub fn validate_pairs(pairs: &[(String, String)]) -> Vec<String> {
    let checks: &[(&str, &[&str])] = &[
        ("OPENROUTER_", &["sk-or-v1-"]),
        ("ANTHROPIC_", &["sk-ant-api03-", "sk-ant-"]),
        ("CEREBRAS_", &["csk-"]),
        ("NVIDIA_", &["nvapi-"]),
        ("HF_", &["hf_"]),
        ("GOOGLE_", &["AIza", "AQ."]),
        ("XAI_", &["xai-"]),
        ("ELEVENLABS_", &["sk_"]),
        ("CLOUDFLARE_", &["cfut_", "2a", "O-", "0f"]),
        ("VERCEL_", &["vck_"]),
        ("GITHUB_", &["ghp_", "gho_", "ghs_", "github_pat_"]),
        ("DEEPSEEK_", &["sk-"]),
        ("OPENAI_", &["sk-"]),
        ("GROQ_", &["gsk_"]),
        ("STABILITY_", &["sk-"]),
        ("OLLAMA_", &["ollama", "sk-", "sh_ollama_"]),
        ("QDRANT_", &["eyJ"]),
    ];
    let mut warnings = Vec::new();
    for (name, value) in pairs {
        if is_placeholder(value) {
            warnings.push(format!(
                "{}: placeholder value - will be skipped by wave-import",
                name
            ));
            continue;
        }
        if let Some((prefix, expected)) = checks.iter().find(|(p, _)| name.starts_with(p)) {
            let ok = expected.iter().any(|e| value.starts_with(e));
            if !ok {
                warnings.push(format!(
                    "{}: value does not look like a {} key (expected prefix {:?})",
                    name, prefix, expected
                ));
            }
        }
    }
    warnings
}

/// Kinds of change a sync would apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Added,
    Changed,
    Removed,
}

impl std::fmt::Display for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Change::Added => write!(f, "+ added"),
            Change::Changed => write!(f, "~ changed"),
            Change::Removed => write!(f, "- removed"),
        }
    }
}

/// Computes what `update_env_file` would do when applying `incoming` over the
/// current file pairs, without touching the file. AWS_* vars are treated as
/// preserved by the sync and are never reported as removed.
pub fn diff_env(
    current: &[(String, String)],
    incoming: &[(String, String)],
) -> Vec<(String, Change)> {
    let mut out = Vec::new();
    for (name, new_val) in incoming {
        match current.iter().find(|(k, _)| k == name) {
            Some((_, old_val)) if old_val != new_val => {
                out.push((name.clone(), Change::Changed));
            }
            None => out.push((name.clone(), Change::Added)),
            _ => {}
        }
    }
    for (name, _) in current {
        if name.starts_with("AWS_") {
            continue;
        }
        if !incoming.iter().any(|(k, _)| k == name) {
            out.push((name.clone(), Change::Removed));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SecretsConfig;

    fn tmp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("key-bitcher-secrets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn pairs_json_round_trip() {
        let pairs = vec![
            ("OPENAI_API_KEY".to_string(), "sk-abc".to_string()),
            (
                "MISTRAL_API_KEY".to_string(),
                "val-with\nnewline".to_string(),
            ),
        ];
        let json = pairs_to_json(&pairs);
        let back = parse_secrets(&json).unwrap();
        assert_eq!(back.len(), 2);
        let openai = back.iter().find(|(k, _)| k == "OPENAI_API_KEY").unwrap();
        assert_eq!(openai.1, "sk-abc");
        let mistral = back.iter().find(|(k, _)| k == "MISTRAL_API_KEY").unwrap();
        assert_eq!(mistral.1, "val-with\nnewline");
    }

    #[test]
    fn parse_secrets_ignores_non_strings() {
        let json = r#"{"A": "x", "B": 5, "C": {"nested": true}}"#;
        let pairs = parse_secrets(json).unwrap();
        assert_eq!(pairs, vec![("A".to_string(), "x".to_string())]);
    }

    #[test]
    fn placeholder_detection() {
        assert!(is_placeholder(""));
        assert!(is_placeholder("PASTE_OPENAI_KEY_HERE"));
        assert!(is_placeholder("xxxx-YYYY-XXXX"));
        assert!(is_placeholder("sk-PASTE_REAL_OPENAI_KEY_2_HERE"));
        assert!(!is_placeholder("sk-real-key-123"));
    }

    #[test]
    fn validate_pairs_flags_mismatches_and_placeholders() {
        let ok = vec![
            ("OPENAI_API_KEY".to_string(), "sk-real".to_string()),
            (
                "ANTHROPIC_API_KEY".to_string(),
                "sk-ant-api03-real".to_string(),
            ),
            ("GOOGLE_API_KEY".to_string(), "AIza-real".to_string()),
        ];
        assert!(validate_pairs(&ok).is_empty());

        let bad = vec![
            ("OPENAI_API_KEY".to_string(), "garbage".to_string()),
            ("OPENROUTER_API_KEY".to_string(), "sk-".to_string()),
            (
                "OPENAI_API_KEY_2".to_string(),
                "sk-PASTE_REAL_OPENAI_KEY_2_HERE".to_string(),
            ),
        ];
        let warnings = validate_pairs(&bad);
        assert_eq!(warnings.len(), 3, "warnings: {:?}", warnings);
        assert!(warnings
            .iter()
            .any(|w| w.contains("does not look like a OPENAI_")));
        assert!(warnings
            .iter()
            .any(|w| w.contains("does not look like a OPENROUTER_")));
        assert!(warnings.iter().any(|w| w.contains("placeholder")));
    }

    #[test]
    fn wave_aliases_map_known_providers() {
        let pairs = vec![
            ("OPENAI_API_KEY".to_string(), "sk-o".to_string()),
            ("OPENROUTER_API_KEY".to_string(), "or-key".to_string()),
            ("GOOGLE_API_KEY".to_string(), "gg-key".to_string()),
        ];
        let aliases = wave_aliases(&pairs);
        assert!(aliases.contains(&("OPENAI_KEY".to_string(), "sk-o".to_string())));
        assert!(aliases.contains(&("OPENROUTER_KEY".to_string(), "or-key".to_string())));
        assert!(aliases.contains(&("GOOGLE_AI_KEY".to_string(), "gg-key".to_string())));
        assert!(!aliases.iter().any(|(k, _)| k == "MISTRAL_KEY"));
    }

    #[test]
    fn read_env_file_skips_comments_and_blanks() {
        let path = tmp_dir().join("in.env");
        std::fs::write(&path, "# comment\n\nA=1\nB=two=three\n").unwrap();
        let pairs = read_env_file(path.to_str().unwrap()).unwrap();
        assert_eq!(
            pairs,
            vec![
                ("A".to_string(), "1".to_string()),
                ("B".to_string(), "two=three".to_string()),
            ]
        );
    }

    #[test]
    fn update_env_file_preserves_aws_and_overwrites_managed() {
        let dir = tmp_dir();
        let path = dir.join("out.env");
        let cfg = SecretsConfig {
            output_env_file: path.to_str().unwrap().to_string(),
            overwrite_existing: true,
        };

        std::fs::write(&path, "AWS_ACCESS_KEY_ID=old\nOPENAI_API_KEY=old-key\n").unwrap();

        let pairs = vec![
            ("OPENAI_API_KEY".to_string(), "new-key".to_string()),
            ("MISTRAL_API_KEY".to_string(), "mk".to_string()),
        ];
        update_env_file(&cfg, &pairs).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            content.contains("AWS_ACCESS_KEY_ID=old"),
            "AWS_* must survive sync"
        );
        assert!(
            content.contains("OPENAI_API_KEY=new-key"),
            "managed key must be overwritten"
        );
        assert!(content.contains("MISTRAL_API_KEY=mk"));
        assert!(!content.contains("old-key"));
    }

    #[test]
    fn update_env_file_refuses_when_overwrite_existing_is_false() {
        let dir = tmp_dir();
        let path = dir.join("refuse.env");
        std::fs::write(&path, "EXISTS=1\n").unwrap();
        let cfg = SecretsConfig {
            output_env_file: path.to_str().unwrap().to_string(),
            overwrite_existing: false,
        };
        assert!(update_env_file(&cfg, &[]).is_err());
    }

    #[test]
    fn diff_env_reports_added_changed_removed_but_not_aws() {
        let current = vec![
            ("AWS_ACCESS_KEY_ID".to_string(), "keep".to_string()),
            ("A".to_string(), "1".to_string()),
            ("B".to_string(), "old".to_string()),
            ("GONE".to_string(), "x".to_string()),
        ];
        let incoming = vec![
            ("A".to_string(), "1".to_string()),
            ("B".to_string(), "new".to_string()),
            ("C".to_string(), "fresh".to_string()),
        ];
        let diff = diff_env(&current, &incoming);
        assert!(diff.contains(&("B".to_string(), Change::Changed)));
        assert!(diff.contains(&("C".to_string(), Change::Added)));
        assert!(diff.contains(&("GONE".to_string(), Change::Removed)));
        assert!(!diff.iter().any(|(k, _)| k == "AWS_ACCESS_KEY_ID"));
        assert!(
            !diff.iter().any(|(k, _)| k == "A"),
            "unchanged keys are omitted"
        );
    }
}
