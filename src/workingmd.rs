use std::collections::{BTreeMap, HashMap};

use anyhow::Result;

use crate::log_warn;

/// Maps each `## Section` in working.md to the ordered env-var names that
/// should be assigned to the tokens listed under it (1:1 with
/// example_s3_secrets.json).
fn names_for_section(section: &str) -> Option<&'static [&'static str]> {
    let s = section.to_lowercase();
    let s = s.split_whitespace().collect::<String>();
    match s.as_str() {
        "nvidia" => Some(&[
            "NVIDIA_API_KEY",
            "NVIDIA_API_KEY_ALT",
            "NVIDIA_API_KEY_ALT2",
            "NVIDIA_API_KEY_ALT3",
        ]),
        "opencodezen" => Some(&["OPENCODEZEN_API_KEY"]),
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
        "openai/deepseek" => Some(&["OPENAI_API_KEY", "OPENAI_API_KEY_2", "DEEPSEEK_API_KEY"]),
        "openai/deepseek(unknownstatus)" => Some(&["DEEPSEEK_API_KEY_2", "DEEPSEEK_API_KEY_3"]),
        "google" => Some(&["GOOGLE_API_KEY", "GOOGLE_API_KEY_2", "GOOGLE_API_KEY_3"]),
        "xai/grok" => Some(&["XAI_API_KEY"]),
        "mistral" => Some(&["MISTRAL_API_KEY"]),
        "cohere" => Some(&["COHERE_API_KEY"]),
        "cerebras" => Some(&["CEREBRAS_API_KEY"]),
        "huggingface" => Some(&["HF_TOKEN", "HF_TOKEN_ALT"]),
        "stabilityai" => Some(&["STABILITY_API_KEY"]),
        "elevenlabs" => Some(&["ELEVENLABS_API_KEY"]),
        "deepgram" => Some(&["DEEPGRAM_API_KEY"]),
        "ollama(local)" => Some(&["OLLAMA_API_KEY"]),
        "cloudflare" => Some(&[
            "CLOUDFLARE_API_TOKEN",
            "CLOUDFLARE_API_TOKEN_2",
            "CLOUDFLARE_API_TOKEN_3",
        ]),
        "vercel" => Some(&["VERCEL_TOKEN", "VERCEL_TOKEN_WORKING"]),
        "aigateway" => Some(&["AI_GATEWAY_API_KEY", "AI_GATEWAY_API_KEY_2"]),
        "github" => Some(&["GITHUB_TOKEN", "GITHUB_TOKEN_FULL"]),
        "github(unknownstatus)" => Some(&["GITHUB_TOKEN_FULL"]),
        "jwt/opencode/qdrant" => Some(&["QDRANT_API_KEY"]),
        _ => None,
    }
}

/// Resolves the env-name list for a section, giving user-defined mappings in
/// `key-bitcher.toml` ([sections]) priority over the built-in names.
fn section_names(
    section: &str,
    custom: Option<&BTreeMap<String, Vec<String>>>,
) -> Option<Vec<String>> {
    if let Some(map) = custom {
        if let Some(names) = map.get(section.trim()) {
            return Some(names.clone());
        }
    }
    names_for_section(section).map(|names| names.iter().map(|s| s.to_string()).collect())
}

fn is_valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .next()
            .map(|c| c.is_ascii_alphabetic())
            .unwrap_or(false)
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Extracts the first backtick code span in a body, returning the token plus
/// everything after the closing backtick (used for annotation parsing).
fn extract_code_span(body: &str) -> Option<(String, String)> {
    let start = body.find('`')?;
    let rest = &body[start + 1..];
    let end = rest.find('`')?;
    Some((rest[..end].trim().to_string(), rest[end + 1..].to_string()))
}

/// Parses an `(annotation)` suffix into an env-name annotation when present.
fn parse_annotation(after: &str) -> Option<String> {
    let after = after.trim();
    if let Some(open) = after.rfind('(') {
        let tail = &after[open + 1..];
        if let Some(close) = tail.find(')') {
            let inside = tail[..close].trim();
            if let Some(comma) = inside.find(',') {
                let candidate = inside[comma + 1..].trim().to_string();
                if is_valid_env_name(&candidate) {
                    return Some(candidate);
                }
            } else if looks_like_env_name(inside) {
                return Some(inside.to_string());
            }
        }
    }
    None
}

/// Extracts a backtick token and an optional explicit env-name annotation.
/// Example: `- `sk-...` (unknown, DEEPSEEK_API_KEY_3)` yields the token plus
/// the env name `DEEPSEEK_API_KEY_3`.
fn extract_token_and_name(line: &str) -> Option<(String, Option<String>)> {
    let line = line.trim();
    if !line.starts_with('-') {
        return None;
    }
    let body = &line[1..].trim_start();
    let (token, after) = extract_code_span(body)?;
    Some((token, parse_annotation(&after)))
}

/// Extracts a backtick code span from a plain (non-dash) line, e.g. a key
/// mentioned inline in prose: `the key is `sk-abc123``.
fn extract_plain_token(line: &str) -> Option<(String, Option<String>)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('-') || line.starts_with('#') {
        return None;
    }
    let (token, after) = extract_code_span(line)?;
    if token.is_empty() {
        return None;
    }
    Some((token, parse_annotation(&after)))
}

/// Parses `NAME: value` / `NAME = value` forms (dash or plain lines). The
/// value may be backtick-wrapped and may carry a trailing `(annotation)`.
/// `allow_colon` gates the `NAME: value` form so plain prose lines (e.g.
/// `note: this is not a token`) are not misread as key/value pairs.
fn parse_name_value(s: &str, allow_colon: bool) -> Option<(String, String)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let sep = if allow_colon {
        s.find(['=', ':'])?
    } else {
        s.find('=')?
    };
    let name = s[..sep].trim().to_string();
    if !is_valid_env_name(&name) {
        return None;
    }
    let mut value = s[sep + 1..].trim().to_string();
    if value.starts_with('`') && value.len() >= 2 && value.ends_with('`') {
        value = value[1..value.len() - 1].to_string();
    }
    if let Some(open) = value.rfind('(') {
        if value.trim_end().ends_with(')') {
            value = value[..open].trim_end().to_string();
        }
    }
    if value.is_empty() {
        return None;
    }
    Some((name, value))
}

/// Records an explicitly-named pair, warning on duplicate values but always
/// keeping the entry.
fn record_named(
    pairs: &mut Vec<(String, String)>,
    seen: &mut HashMap<String, String>,
    name: String,
    token: String,
) {
    if let Some(first) = seen.get(&normalize_value(&token)) {
        log_warn!(
            "working.md: value of {} duplicates {} (same key under two names)",
            name,
            first
        );
    } else {
        seen.insert(normalize_value(&token), name.clone());
    }
    pairs.push((name, token));
}

/// Status words like `(working)` / `(alt)` are not env-name annotations. A bare
/// annotation must be an all-uppercase, underscore-capable name (e.g.
/// `DEEPSEEK_API_KEY_3`), matching the convention for env vars.
fn looks_like_env_name(s: &str) -> bool {
    is_valid_env_name(s) && (s == s.to_uppercase() || s.contains('_'))
}

/// Strips a key's provider prefix so two keys from the same account (but with
/// different prefixes) can be detected as duplicates.
fn normalize_value(value: &str) -> String {
    let s = value.trim().to_lowercase();
    for prefix in [
        "sk-proj-",
        "sk-ant-api03-",
        "sk-ant-",
        "sk-or-v1-",
        "sk_or",
        "sk_",
        "sk-",
        "nvapi-",
        "csk-",
        "hf_",
        "xai-",
        "gsk_",
        "ghp_",
        "gho_",
        "ghs_",
        "github_pat_",
        "sh_ollama_",
        "glc_",
        "pat_",
        "tb_",
        "aiag-",
    ] {
        if let Some(stripped) = s.strip_prefix(prefix) {
            return stripped.to_string();
        }
    }
    s
}

/// Like [`parse_working_md_with_sections`] with no custom section mappings.
/// Honors user-defined section mappings in `key-bitcher.toml` ([sections]).
pub fn parse_working_md_with_sections(
    path: &str,
    custom: Option<&BTreeMap<String, Vec<String>>>,
) -> Result<(Vec<(String, String)>, usize)> {
    let content = std::fs::read_to_string(path)?;
    let mut pairs = Vec::new();
    let mut skipped = 0usize;
    let mut parent_names: Option<Vec<String>> = None;
    let mut current_names: Option<Vec<String>> = None;
    let mut current_section = String::new();
    let mut index = 0usize;
    let mut seen: HashMap<String, String> = HashMap::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("### ") {
            // Subsection: its own mapping wins (custom or built-in); otherwise
            // inherit the enclosing `##` section's names.
            current_names = section_names(rest, custom).or_else(|| parent_names.clone());
            index = 0;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            parent_names = section_names(rest, custom);
            current_names = parent_names.clone();
            current_section = rest.to_string();
            index = 0;
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let is_dash = trimmed.starts_with('-');
        let body = if is_dash {
            trimmed[1..].trim_start()
        } else {
            trimmed
        };

        // 1) explicit `NAME: value` / `NAME = value` forms. The `NAME: value`
        // form is only accepted on dash lines to avoid misreading prose.
        if let Some((name, token)) = parse_name_value(body, is_dash) {
            record_named(&mut pairs, &mut seen, name, token);
            index += 1;
            continue;
        }

        // 2) backtick code spans on dash lines or plain lines.
        let extracted = if is_dash {
            extract_token_and_name(trimmed)
        } else {
            extract_plain_token(trimmed)
        };
        let Some((token, annotated)) = extracted else {
            continue;
        };
        let explicit_name = annotated.filter(|n| is_valid_env_name(n));

        match (explicit_name, &current_names) {
            // An explicit per-token env name always wins, even in sections
            // that are not otherwise mapped.
            (Some(name), _) => {
                record_named(&mut pairs, &mut seen, name, token);
                index += 1;
            }
            (None, Some(names)) => {
                if index >= names.len() {
                    log_warn!(
                        "working.md: section '{current_section}' has more tokens than mapped names ({}); token {} skipped",
                        names.len(),
                        index + 1
                    );
                    skipped += 1;
                } else {
                    record_named(&mut pairs, &mut seen, names[index].to_string(), token);
                }
                index += 1;
            }
            (None, None) => {
                skipped += 1;
            }
        }
    }

    Ok((pairs, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_fixture(name: &str, content: &str) -> (Vec<(String, String)>, usize) {
        let dir = std::env::temp_dir().join(format!("key-bitcher-wmd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.md"));
        std::fs::write(&path, content).unwrap();
        parse_working_md_with_sections(path.to_str().unwrap(), None).unwrap()
    }

    #[test]
    fn extracts_backtick_tokens_in_order() {
        let content = "# Working API Keys

## NVIDIA
- `nv-key-1` (working)
- `nv-key-2` (alt)

## Mistral
- `ms-key`
";
        let (pairs, skipped) = parse_fixture("extract", content);
        assert_eq!(skipped, 0);
        assert_eq!(
            pairs,
            vec![
                ("NVIDIA_API_KEY".to_string(), "nv-key-1".to_string()),
                ("NVIDIA_API_KEY_ALT".to_string(), "nv-key-2".to_string()),
                ("MISTRAL_API_KEY".to_string(), "ms-key".to_string()),
            ]
        );
    }

    #[test]
    fn assigns_sequential_names_per_section() {
        let content = "## NVIDIA
- `a`
- `b`
- `c`
- `d`
- `e`
";
        let (pairs, skipped) = parse_fixture("sequential", content);
        assert_eq!(
            pairs.len(),
            4,
            "only names defined for the section are used"
        );
        assert_eq!(pairs[0].0, "NVIDIA_API_KEY");
        assert_eq!(pairs[3].0, "NVIDIA_API_KEY_ALT3");
        assert_eq!(skipped, 1, "the 5th entry has no name left");
    }

    #[test]
    fn skips_unknown_sections_and_non_token_lines() {
        let content = "## Unknown / To Verify
- `secret-thing`

## GitHub
- `gh-token`
";
        let (pairs, skipped) = parse_fixture("unknown", content);
        assert_eq!(
            pairs,
            vec![("GITHUB_TOKEN".to_string(), "gh-token".to_string())]
        );
        assert_eq!(skipped, 1);
    }

    #[test]
    fn ignores_plain_text_lines() {
        let content = "## Anthropic
note: this is not a token
- `sk-ant-test`

## Cohere
- `cohere-key`
";
        let (pairs, _) = parse_fixture("plain", content);
        assert_eq!(
            pairs,
            vec![
                ("ANTHROPIC_API_KEY".to_string(), "sk-ant-test".to_string()),
                ("COHERE_API_KEY".to_string(), "cohere-key".to_string()),
            ]
        );
    }

    #[test]
    fn missing_file_is_an_error() {
        assert!(parse_working_md_with_sections("C:\\nonexistent-file.md", None).is_err());
    }

    #[test]
    fn honors_per_token_env_name_annotations() {
        let content = "## OpenAI / DeepSeek (unknown status)
- `sk-aaa` (unknown, DEEPSEEK_API_KEY_3)
- `sk-bbb` (unknown, DEEPSEEK_API_KEY_2)

## GitHub (unknown status)
- `ghp-ccc` (unknown, GITHUB_TOKEN_FULL)
";
        let (pairs, skipped) = parse_fixture("annotated", content);
        assert_eq!(skipped, 0, "annotated tokens must not be skipped");
        assert_eq!(
            pairs,
            vec![
                ("DEEPSEEK_API_KEY_3".to_string(), "sk-aaa".to_string()),
                ("DEEPSEEK_API_KEY_2".to_string(), "sk-bbb".to_string()),
                ("GITHUB_TOKEN_FULL".to_string(), "ghp-ccc".to_string()),
            ]
        );
    }

    #[test]
    fn positional_mapping_works_without_annotations() {
        let content = "## NVIDIA
- `a` (working)
- `b` (alt)
";
        let (pairs, skipped) = parse_fixture("positional", content);
        assert_eq!(skipped, 0);
        assert_eq!(
            pairs,
            vec![
                ("NVIDIA_API_KEY".to_string(), "a".to_string()),
                ("NVIDIA_API_KEY_ALT".to_string(), "b".to_string()),
            ]
        );
    }

    #[test]
    fn custom_sections_override_builtin_names() {
        let content = "## My Custom Section
- `c1`
- `c2`
";
        let dir = std::env::temp_dir().join(format!("key-bitcher-wmd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("custom.md");
        std::fs::write(&path, content).unwrap();
        let mut custom = BTreeMap::new();
        custom.insert(
            "My Custom Section".to_string(),
            vec!["MY_KEY".to_string(), "MY_KEY_2".to_string()],
        );
        let (pairs, skipped) =
            parse_working_md_with_sections(path.to_str().unwrap(), Some(&custom)).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(
            pairs,
            vec![
                ("MY_KEY".to_string(), "c1".to_string()),
                ("MY_KEY_2".to_string(), "c2".to_string()),
            ]
        );
    }

    #[test]
    fn parses_name_value_forms_on_dash_and_plain_lines() {
        let content = "## Custom
- OPENAI_API_KEY: `sk-a`
- GITHUB_TOKEN=ghp_x
MISTRAL_API_KEY=sk-b
OPENROUTER_API_KEY: sk-or-v1-zzz
";
        let (pairs, skipped) = parse_fixture("namevalue", content);
        // The plain-line `NAME: value` form is deliberately ignored (only `=`
        // is recognized on plain lines; `NAME: value` requires a dash). Prose
        // lines are not counted as skipped secrets.
        assert_eq!(skipped, 0);
        assert_eq!(
            pairs,
            vec![
                ("OPENAI_API_KEY".to_string(), "sk-a".to_string()),
                ("GITHUB_TOKEN".to_string(), "ghp_x".to_string()),
                ("MISTRAL_API_KEY".to_string(), "sk-b".to_string()),
            ]
        );
    }

    #[test]
    fn colon_form_on_plain_line_is_not_a_key_value_pair() {
        let content = "## Anthropic
note: this is not a token
- `sk-ant-test`
";
        let (pairs, skipped) = parse_fixture("plaincolon", content);
        assert_eq!(
            pairs,
            vec![("ANTHROPIC_API_KEY".to_string(), "sk-ant-test".to_string())]
        );
        assert_eq!(skipped, 0);
    }

    #[test]
    fn extracts_code_spans_on_plain_lines() {
        let content = "## OpenRouter
the current key is `or-1`
backup is `or-2`
- `or-3`
";
        let (pairs, skipped) = parse_fixture("plaintoken", content);
        assert_eq!(skipped, 0);
        assert_eq!(
            pairs,
            vec![
                ("OPENROUTER_API_KEY_CURRENT".to_string(), "or-1".to_string()),
                ("OPENROUTER_API_KEY".to_string(), "or-2".to_string()),
                ("OPENROUTER_API_KEY_ALT".to_string(), "or-3".to_string()),
            ]
        );
    }

    #[test]
    fn subsections_use_own_mapping_else_inherit_parent() {
        let content = "## NVIDIA
- `n1`
### Google
- `g1`
### Sub section
- `s1`
";
        let (pairs, skipped) = parse_fixture("subsections", content);
        assert_eq!(skipped, 0);
        assert_eq!(
            pairs,
            vec![
                ("NVIDIA_API_KEY".to_string(), "n1".to_string()),
                ("GOOGLE_API_KEY".to_string(), "g1".to_string()),
                ("NVIDIA_API_KEY".to_string(), "s1".to_string()),
            ]
        );
    }
}
