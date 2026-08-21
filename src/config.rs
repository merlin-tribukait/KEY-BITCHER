use anyhow::Result;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    pub object_key: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SecretsConfig {
    pub output_env_file: String,
    pub overwrite_existing: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BenchmarkConfig {
    pub models: Vec<String>,
    pub max_output_tokens: Option<u32>,
    pub request_timeout_secs: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WaveConfig {
    /// Path to the wsh binary used to store Wave secrets. If empty, resolved
    /// automatically (PATH lookup, then the default Windows install location).
    pub wsh_path: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    /// Rotate the log file when it exceeds this size in bytes.
    pub max_size_bytes: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PluginConfig {
    pub s3: S3Config,
    pub secrets: SecretsConfig,
    pub benchmark: Option<BenchmarkConfig>,
    pub wave: Option<WaveConfig>,
    pub logging: Option<LoggingConfig>,
    /// Optional user-defined `## Section` -> env-name list mapping for
    /// working.md import, overriding the built-in section names.
    pub sections: Option<BTreeMap<String, Vec<String>>>,
}

impl PluginConfig {
    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let cfg: PluginConfig = toml::from_str(&content)?;
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_file(name: &str, content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("key-bitcher-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn loads_full_config() {
        let path = tmp_file(
            "full.toml",
            r#"
            [s3]
            bucket = "wave-secrets-bucket"
            region = "eu-central-003"
            object_key = "secrets/ai-keys.json"

            [secrets]
            output_env_file = ".env"
            overwrite_existing = true

            [wave]
            wsh_path = "C:\\wsh.exe"

            [benchmark]
            models = ["openai:gpt-4o-mini"]
            max_output_tokens = 128
            request_timeout_secs = 10

            [logging]
            max_size_bytes = 1048576

            [sections]
            "My Custom Section" = ["MY_KEY", "MY_KEY_2"]
            "#,
        );
        let cfg = PluginConfig::load(path.to_str().unwrap()).unwrap();
        assert_eq!(cfg.s3.bucket, "wave-secrets-bucket");
        assert_eq!(cfg.s3.region, "eu-central-003");
        assert_eq!(cfg.s3.object_key, "secrets/ai-keys.json");
        assert_eq!(cfg.secrets.output_env_file, ".env");
        assert!(cfg.secrets.overwrite_existing);
        assert_eq!(cfg.wave.unwrap().wsh_path.unwrap(), "C:\\wsh.exe");
        let bench = cfg.benchmark.unwrap();
        assert_eq!(bench.models, vec!["openai:gpt-4o-mini"]);
        assert_eq!(bench.max_output_tokens, Some(128));
        assert_eq!(bench.request_timeout_secs, Some(10));
        assert_eq!(cfg.logging.unwrap().max_size_bytes, Some(1048576));
        let sections = cfg.sections.unwrap();
        assert_eq!(
            sections.get("My Custom Section").unwrap(),
            &vec!["MY_KEY".to_string(), "MY_KEY_2".to_string()]
        );
    }

    #[test]
    fn loads_minimal_config_without_optional_sections() {
        let path = tmp_file(
            "minimal.toml",
            r#"
            [s3]
            bucket = "b"
            region = "r"
            object_key = "k"

            [secrets]
            output_env_file = ".env"
            overwrite_existing = false
            "#,
        );
        let cfg = PluginConfig::load(path.to_str().unwrap()).unwrap();
        assert!(cfg.wave.is_none());
        assert!(cfg.benchmark.is_none());
        assert!(cfg.logging.is_none());
        assert!(cfg.sections.is_none());
        assert!(!cfg.secrets.overwrite_existing);
    }

    #[test]
    fn missing_file_errors() {
        assert!(PluginConfig::load("nonexistent-path.toml").is_err());
    }
}
