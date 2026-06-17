//! Spec loading: dependency declarations and spec file parsing.
//!
//! Provides the data types and functions for loading tool-ready
//! specifications from `tool-ready-spec.json` files.

use std::{collections::HashMap, path::PathBuf};

use serde_json::Value;

/// A single dependency entry, normalized from string or object format.
#[derive(Debug, Clone)]
pub(crate) struct DepEntry {
    pub(crate) binary: String,
    pub(crate) version: Option<String>,
    pub(crate) package: String,
    pub(crate) manager: String,
    pub(crate) pip_name: Option<String>,
    pub(crate) uv_name: Option<String>,
    pub(crate) npm_name: Option<String>,
    pub(crate) use_npx: bool,
    pub(crate) fallback: Vec<FallbackEntry>,
}

/// A fallback install strategy.
#[derive(Debug, Clone)]
pub(crate) struct FallbackEntry {
    pub(crate) method: String,
    pub(crate) package: Option<String>,
    pub(crate) binary: Option<String>,
    pub(crate) source: Option<String>,
    pub(crate) manifest: Option<String>,
    pub(crate) features: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) args: Option<String>,
}

/// Per-tool dependency specification.
#[derive(Debug, Clone)]
pub(crate) struct ToolDepSpec {
    pub(crate) aliases: Vec<String>,
    pub(crate) required: Vec<DepEntry>,
    pub(crate) recommended: Vec<DepEntry>,
    pub(crate) config_files: Vec<String>,
    pub(crate) permissions: Vec<String>,
    pub(crate) network: Vec<String>,
}

#[allow(clippy::too_many_lines)]
pub(crate) fn normalize_dep(value: &Value) -> DepEntry {
    match value {
        Value::String(s) => {
            if let Some(idx) = s.find(">=") {
                let binary = s[..idx].to_string();
                let version = Some(s[idx..].to_string());
                DepEntry {
                    binary,
                    version,
                    package: s[..idx].to_string(),
                    manager: "rpm".to_string(),
                    pip_name: None,
                    uv_name: None,
                    npm_name: None,
                    use_npx: false,
                    fallback: Vec::new(),
                }
            } else {
                DepEntry {
                    binary: s.clone(),
                    version: None,
                    package: s.clone(),
                    manager: "rpm".to_string(),
                    pip_name: None,
                    uv_name: None,
                    npm_name: None,
                    use_npx: false,
                    fallback: Vec::new(),
                }
            }
        }
        Value::Object(obj) => {
            let binary = obj
                .get("binary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let version = obj.get("version").and_then(Value::as_str).map(String::from);
            let package = obj
                .get("package")
                .and_then(Value::as_str)
                .unwrap_or(&binary)
                .to_string();
            let manager = obj
                .get("manager")
                .and_then(Value::as_str)
                .unwrap_or("rpm")
                .to_string();
            let pip_name = obj
                .get("pip_name")
                .and_then(Value::as_str)
                .map(String::from);
            let uv_name = obj.get("uv_name").and_then(Value::as_str).map(String::from);
            let npm_name = obj
                .get("npm_name")
                .and_then(Value::as_str)
                .map(String::from);
            let use_npx = obj.get("use_npx").and_then(Value::as_bool).unwrap_or(false);
            let fallback = obj
                .get("fallback")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|fb| {
                            let fb_obj = fb.as_object()?;
                            Some(FallbackEntry {
                                method: fb_obj
                                    .get("method")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string(),
                                package: fb_obj
                                    .get("package")
                                    .and_then(Value::as_str)
                                    .map(String::from),
                                binary: fb_obj
                                    .get("binary")
                                    .and_then(Value::as_str)
                                    .map(String::from),
                                source: fb_obj
                                    .get("source")
                                    .and_then(Value::as_str)
                                    .map(String::from),
                                manifest: fb_obj
                                    .get("manifest")
                                    .and_then(Value::as_str)
                                    .map(String::from),
                                features: fb_obj
                                    .get("features")
                                    .and_then(Value::as_str)
                                    .map(String::from),
                                url: fb_obj.get("url").and_then(Value::as_str).map(String::from),
                                args: fb_obj.get("args").and_then(Value::as_str).map(String::from),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            DepEntry {
                binary,
                version,
                package,
                manager,
                pip_name,
                uv_name,
                npm_name,
                use_npx,
                fallback,
            }
        }
        _ => DepEntry {
            binary: String::new(),
            version: None,
            package: String::new(),
            manager: "rpm".to_string(),
            pip_name: None,
            uv_name: None,
            npm_name: None,
            use_npx: false,
            fallback: Vec::new(),
        },
    }
}

pub(crate) fn normalize_deps(array: &Value) -> Vec<DepEntry> {
    array
        .as_array()
        .map(|arr| arr.iter().map(normalize_dep).collect())
        .unwrap_or_default()
}

pub(crate) fn load_spec(spec_path: &PathBuf) -> Result<HashMap<String, ToolDepSpec>, String> {
    let content =
        std::fs::read_to_string(spec_path).map_err(|e| format!("Failed to read spec file: {e}"))?;
    let value: Value =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse spec JSON: {e}"))?;

    let mut specs = HashMap::new();
    if let Value::Object(obj) = value {
        for (tool_name, tool_spec) in obj {
            if tool_name.starts_with('_') {
                continue;
            }
            if let Value::Object(spec_obj) = tool_spec {
                let aliases = spec_obj
                    .get("aliases")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                let required = normalize_deps(
                    spec_obj
                        .get("required")
                        .unwrap_or(&Value::Array(Vec::new())),
                );
                let recommended = normalize_deps(
                    spec_obj
                        .get("recommended")
                        .unwrap_or(&Value::Array(Vec::new())),
                );
                let config_files = spec_obj
                    .get("config_files")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                let permissions = spec_obj
                    .get("permissions")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                let network = spec_obj
                    .get("network")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                specs.insert(
                    tool_name,
                    ToolDepSpec {
                        aliases,
                        required,
                        recommended,
                        config_files,
                        permissions,
                        network,
                    },
                );
            }
        }
    }
    Ok(specs)
}

pub(crate) fn find_spec_path() -> Result<PathBuf, String> {
    let home = crate::shared::get_home_dir();
    let cwd = std::env::current_dir().ok();
    let candidates = [
        std::env::var("TOKENLESS_TOOL_READY_SPEC")
            .ok()
            .map(PathBuf::from),
        // Development path (relative to repository root)
        cwd.as_ref()
            .map(|d| d.join("adapters/tokenless/common/tool-ready-spec.json")),
        Some(PathBuf::from(format!(
            "{home}/.tokenfleet-ai/tokenless/tool-ready-spec.json"
        ))),
        Some(PathBuf::from(format!(
            "{home}/.local/share/anolisa/adapters/tokenless/common/tool-ready-spec.json"
        ))),
        Some(PathBuf::from(
            "/usr/share/anolisa/adapters/tokenless/common/tool-ready-spec.json",
        )),
    ];

    for candidate in candidates.iter().flatten() {
        #[allow(clippy::disallowed_methods)]
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }
    Err("No spec file found in any candidate path".to_string())
}
