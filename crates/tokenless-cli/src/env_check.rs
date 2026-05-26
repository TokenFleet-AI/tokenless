//! Environment readiness checker for the Tool Ready feature.
//!
//! Loads per-tool dependency declarations from `tool-ready-spec.json`
//! and checks binary availability, version constraints, config files,
//! permissions, and network connectivity.

#![allow(dead_code)]

use std::{collections::HashMap, fs, path::PathBuf, process::Command};

use serde_json::Value;

/// A single dependency entry, normalized from string or object format.
#[derive(Debug, Clone)]
struct DepEntry {
    binary: String,
    version: Option<String>,
    package: String,
    manager: String,
    pip_name: Option<String>,
    uv_name: Option<String>,
    npm_name: Option<String>,
    use_npx: bool,
    fallback: Vec<FallbackEntry>,
}

/// A fallback install strategy.
#[derive(Debug, Clone)]
struct FallbackEntry {
    method: String,
    package: Option<String>,
    binary: Option<String>,
    source: Option<String>,
    manifest: Option<String>,
    features: Option<String>,
    url: Option<String>,
    args: Option<String>,
}

/// Per-tool dependency specification.
#[derive(Debug, Clone)]
struct ToolDepSpec {
    aliases: Vec<String>,
    required: Vec<DepEntry>,
    recommended: Vec<DepEntry>,
    config_files: Vec<String>,
    permissions: Vec<String>,
    network: Vec<String>,
}

/// Status of a single dependency check.
#[derive(Debug, Clone, PartialEq)]
enum DepStatus {
    Available,
    Missing,
    VersionLow { installed: String, required: String },
}

/// Overall readiness status for a tool.
#[derive(Debug, Clone, PartialEq)]
enum ReadyStatus {
    Ready,
    Partial,
    NotReady,
    Unknown,
}

/// Combined result for a single tool's environment check.
struct ToolReadyResult {
    tool_name: String,
    status: ReadyStatus,
    required_results: Vec<(DepEntry, DepStatus)>,
    recommended_results: Vec<(DepEntry, DepStatus)>,
    config_results: Vec<(String, bool)>,
    permission_results: Vec<(String, bool)>,
    network_results: Vec<(String, bool)>,
}

fn normalize_dep(value: &Value) -> DepEntry {
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

fn normalize_deps(array: &Value) -> Vec<DepEntry> {
    array
        .as_array()
        .map(|arr| arr.iter().map(normalize_dep).collect())
        .unwrap_or_default()
}

fn detect_system_manager() -> String {
    if let Ok(mgr) = std::env::var("TOKENLESS_PACKAGE_MANAGER") {
        return mgr;
    }
    let has_rpm = check_cmd("rpm");
    let has_dpkg = check_cmd("dpkg");
    let has_apk = check_cmd("apk");

    if has_rpm {
        if check_cmd("dnf") {
            return "dnf".to_string();
        }
        return "yum".to_string();
    }
    if has_dpkg {
        return "apt".to_string();
    }
    if has_apk {
        return "apk".to_string();
    }
    "rpm".to_string()
}

fn resolve_manager(manager: &str) -> String {
    if manager == "rpm" {
        detect_system_manager()
    } else {
        manager.to_string()
    }
}

fn check_cmd(cmd: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {cmd}")])
        .output()
        .map_or(false, |o| o.status.success())
}

fn extract_required_version(version: &str) -> &str {
    version
        .strip_prefix(">=")
        .or_else(|| version.strip_prefix('>'))
        .unwrap_or(version)
}

fn version_ge(installed: &str, required: &str) -> bool {
    fn parse_ver(s: &str) -> Vec<u32> {
        let cleaned = s
            .trim()
            .strip_prefix('v')
            .or_else(|| s.trim().strip_prefix('V'))
            .unwrap_or(s.trim());
        cleaned
            .split('.')
            .filter_map(|seg| {
                let num_part: String = seg.chars().take_while(|c| c.is_ascii_digit()).collect();
                num_part.parse().ok()
            })
            .collect()
    }
    let i_parts = parse_ver(installed);
    let r_parts = parse_ver(required);
    for i in 0..3 {
        let iv = i_parts.get(i).copied().unwrap_or(0);
        let rv = r_parts.get(i).copied().unwrap_or(0);
        if iv > rv {
            return true;
        }
        if iv < rv {
            return false;
        }
    }
    true
}

fn check_dep(dep: &DepEntry) -> DepStatus {
    let found = Command::new("sh")
        .args(["-c", &format!("command -v \"$1\"",), "--", &dep.binary])
        .output();

    match found {
        Ok(output) if output.status.success() => {
            if let Some(ref version) = dep.version {
                let required_version = extract_required_version(version);
                let ver_output = Command::new(&dep.binary).arg("--version").output();
                let installed_version = ver_output
                    .ok()
                    .and_then(|out| {
                        String::from_utf8_lossy(&out.stdout)
                            .lines()
                            .next()
                            .map(|l| l.split_whitespace().last().unwrap_or("0.0.0").to_string())
                    })
                    .unwrap_or_else(|| "0.0.0".to_string());
                if version_ge(&installed_version, required_version) {
                    DepStatus::Available
                } else {
                    DepStatus::VersionLow {
                        installed: installed_version,
                        required: required_version.to_string(),
                    }
                }
            } else {
                DepStatus::Available
            }
        }
        _ => DepStatus::Missing,
    }
}

fn check_config_file(path: &str) -> bool {
    let expanded = if path == "~" || path.starts_with("~/") {
        let home = super::get_home_dir();
        path.replacen('~', &home, 1)
    } else {
        path.to_string()
    };
    fs::metadata(&expanded).is_ok()
}

fn check_permission(perm: &str) -> bool {
    match perm {
        "file_read" => fs::read_to_string("/etc/hostname").is_ok(),
        "file_write" => {
            let test_path =
                std::env::temp_dir().join(format!(".tokenless-ready-test-{}", std::process::id()));
            let can_write = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&test_path)
                .is_ok();
            if can_write {
                let _ = fs::remove_file(&test_path);
            }
            can_write
        }
        "exec_shell" => Command::new("which")
            .arg("bash")
            .output()
            .map_or(false, |o| o.status.success()),
        _ => true,
    }
}

fn check_network(net: &str) -> bool {
    #[allow(clippy::match_single_binding)]
    match net {
        "https_outbound" => Command::new("curl")
            .args(["-s", "--max-time", "2", "https://example.com"])
            .output()
            .map_or(false, |o| o.status.success()),
        _ => true,
    }
}

fn load_spec(spec_path: &PathBuf) -> Result<HashMap<String, ToolDepSpec>, String> {
    let content =
        fs::read_to_string(spec_path).map_err(|e| format!("Failed to read spec file: {e}"))?;
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

fn check_tool(tool_name: &str, spec: &ToolDepSpec) -> ToolReadyResult {
    let required_results: Vec<_> = spec
        .required
        .iter()
        .map(|d| (d.clone(), check_dep(d)))
        .collect();
    let recommended_results: Vec<_> = spec
        .recommended
        .iter()
        .map(|d| (d.clone(), check_dep(d)))
        .collect();
    let config_results: Vec<_> = spec
        .config_files
        .iter()
        .map(|f| (f.clone(), check_config_file(f)))
        .collect();
    let permission_results: Vec<_> = spec
        .permissions
        .iter()
        .map(|p| (p.clone(), check_permission(p)))
        .collect();
    let network_results: Vec<_> = spec
        .network
        .iter()
        .map(|n| (n.clone(), check_network(n)))
        .collect();

    let has_required_missing = required_results
        .iter()
        .any(|(_, s)| matches!(s, DepStatus::Missing | DepStatus::VersionLow { .. }));
    let has_perm_missing = permission_results.iter().any(|(_, ok)| !ok);
    let has_recommended_missing = recommended_results
        .iter()
        .any(|(_, s)| *s == DepStatus::Missing);
    let has_config_missing = config_results.iter().any(|(_, ok)| !ok);
    let has_net_missing = network_results.iter().any(|(_, ok)| !ok);

    let status = if has_required_missing || has_perm_missing {
        ReadyStatus::NotReady
    } else if has_recommended_missing || has_config_missing || has_net_missing {
        ReadyStatus::Partial
    } else {
        ReadyStatus::Ready
    };

    ToolReadyResult {
        tool_name: tool_name.to_string(),
        status,
        required_results,
        recommended_results,
        config_results,
        permission_results,
        network_results,
    }
}

fn format_status(status: &ReadyStatus) -> &'static str {
    match status {
        ReadyStatus::Ready => "READY",
        ReadyStatus::Partial => "PARTIAL",
        ReadyStatus::NotReady => "NOT_READY",
        ReadyStatus::Unknown => "UNKNOWN",
    }
}

fn format_dep_status_label(status: &DepStatus) -> &'static str {
    match status {
        DepStatus::Available => "INSTALLED",
        DepStatus::Missing => "MISSING",
        DepStatus::VersionLow { .. } => "OUTDATED",
    }
}

fn generate_checklist(results: &[ToolReadyResult]) -> String {
    let mut output =
        String::from("Tool Environment Ready Checklist\n=================================\n\n");
    for result in results {
        output.push_str(&format!(
            "{} [{}]\n",
            result.tool_name,
            format_status(&result.status)
        ));
        for (dep, status) in &result.required_results {
            output.push_str(&format!(
                "  required:   {:12} {}\n",
                dep.binary,
                format_dep_status_label(status)
            ));
        }
        for (dep, status) in &result.recommended_results {
            output.push_str(&format!(
                "  recommended:{:12} {}\n",
                dep.binary,
                format_dep_status_label(status)
            ));
        }
        for (cfg, ok) in &result.config_results {
            output.push_str(&format!(
                "  config:     {:12} {}\n",
                cfg,
                if *ok { "INSTALLED" } else { "MISSING" }
            ));
        }
        for (perm, ok) in &result.permission_results {
            output.push_str(&format!(
                "  permission: {:12} {}\n",
                perm,
                if *ok { "GRANTED" } else { "DENIED" }
            ));
        }
        output.push('\n');
    }

    let ready_count = results
        .iter()
        .filter(|r| r.status == ReadyStatus::Ready)
        .count();
    let partial_count = results
        .iter()
        .filter(|r| r.status == ReadyStatus::Partial)
        .count();
    let not_ready_count = results
        .iter()
        .filter(|r| r.status == ReadyStatus::NotReady)
        .count();
    output.push_str(&format!(
        "Summary: {ready_count} ready, {partial_count} partial, {not_ready_count} not ready \
         (total: {})\n",
        results.len()
    ));
    output
}

fn build_json_result(
    tool_name: &str,
    status: &ReadyStatus,
    fixed: &[String],
    missing: &[String],
) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("tool".into(), Value::String(tool_name.to_string()));
    obj.insert(
        "status".into(),
        Value::String(format_status(status).to_string()),
    );
    if !fixed.is_empty() {
        obj.insert(
            "fixed".into(),
            Value::Array(fixed.iter().map(|s| Value::String(s.clone())).collect()),
        );
    }
    if !missing.is_empty() {
        obj.insert(
            "missing".into(),
            Value::Array(missing.iter().map(|s| Value::String(s.clone())).collect()),
        );
    }
    if *status == ReadyStatus::NotReady {
        let diag = format!(
            "[tokenless tool-ready] {tool_name}: NOT_READY — {}. Skip retry — environment issue, \
             not logic error.",
            missing
                .iter()
                .map(|m| format!("required dependency missing: {m}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        obj.insert("diagnostic".into(), Value::String(diag));
    }
    Value::Object(obj)
}

fn find_spec_path() -> Result<PathBuf, String> {
    let home = super::get_home_dir();
    let cwd = std::env::current_dir().ok();
    let candidates = [
        std::env::var("TOKENLESS_TOOL_READY_SPEC")
            .ok()
            .map(PathBuf::from),
        // Development path (relative to repository root)
        cwd.as_ref()
            .map(|d| d.join("adapters/tokenless/common/tool-ready-spec.json")),
        Some(PathBuf::from(format!(
            "{home}/.tokenless/tool-ready-spec.json"
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

fn auto_fix(missing_deps: &[DepEntry]) -> Result<String, String> {
    let home = super::get_home_dir();
    let cwd = std::env::current_dir().ok();
    let fix_script_candidates = [
        std::env::var("TOKENLESS_ENV_FIX_SCRIPT").ok(),
        cwd.as_ref().map(|d| {
            d.join("adapters/tokenless/common/tokenless-env-fix.sh")
                .to_string_lossy()
                .to_string()
        }),
        Some(format!("{home}/.tokenless/tokenless-env-fix.sh")),
        Some(format!(
            "{home}/.local/share/anolisa/adapters/tokenless/common/tokenless-env-fix.sh"
        )),
        Some("/usr/share/anolisa/adapters/tokenless/common/tokenless-env-fix.sh".to_string()),
    ];
    let fix_script = fix_script_candidates
        .iter()
        .flatten()
        .find(|p| {
            #[allow(clippy::disallowed_methods)]
            std::path::Path::new(p).exists()
        })
        .cloned()
        .unwrap_or_else(|| format!("{home}/.tokenless/tokenless-env-fix.sh"));

    let deps_json: Vec<Value> = missing_deps
        .iter()
        .map(|dep| {
            let mut obj = serde_json::Map::new();
            obj.insert("binary".into(), Value::String(dep.binary.clone()));
            if let Some(ref v) = dep.version {
                obj.insert("version".into(), Value::String(v.clone()));
            }
            obj.insert("package".into(), Value::String(dep.package.clone()));
            obj.insert("manager".into(), Value::String(dep.manager.clone()));
            Value::Object(obj)
        })
        .collect();
    let _json_str =
        serde_json::to_string(&deps_json).map_err(|e| format!("Failed to serialize deps: {e}"))?;

    #[allow(clippy::disallowed_methods)]
    let output = Command::new("timeout")
        .arg("120")
        .arg("bash")
        .arg(&fix_script)
        .arg("fix-all")
        .stdin(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run env-fix: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

/// Run the env-check command.
#[allow(clippy::too_many_lines)]
pub fn run(
    tool: Option<&str>,
    all: bool,
    fix: bool,
    checklist: bool,
    json: bool,
) -> Result<(), (String, i32)> {
    let spec_path = find_spec_path().map_err(|e| (e, 1))?;
    let specs = load_spec(&spec_path).map_err(|e| (e, 1))?;

    if checklist {
        let results: Vec<_> = specs
            .keys()
            .map(|name| check_tool(name, &specs[name]))
            .collect();
        println!("{}", generate_checklist(&results));
        return Ok(());
    }

    let tool_names: Vec<String> = if all {
        specs.keys().cloned().collect()
    } else if let Some(t) = tool {
        let resolved = if specs.contains_key(t) {
            t.to_string()
        } else {
            specs
                .iter()
                .find(|(_, spec)| spec.aliases.iter().any(|a| a == t))
                .map(|(k, _)| k.clone())
                .unwrap_or_else(|| t.to_string())
        };
        if !specs.contains_key(&resolved) {
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&build_json_result(
                        &resolved,
                        &ReadyStatus::Unknown,
                        &[],
                        &[]
                    ))
                    .unwrap()
                );
                return Ok(());
            }
            println!("{}: {}", t, format_status(&ReadyStatus::Unknown));
            return Ok(());
        }
        vec![resolved]
    } else {
        return Err(("Specify --tool <name> or --all".to_string(), 1));
    };

    for tool_name in &tool_names {
        let spec = &specs[tool_name];
        let result = check_tool(tool_name, spec);

        let missing_deps: Vec<DepEntry> = result
            .required_results
            .iter()
            .chain(result.recommended_results.iter())
            .filter(|(_, s)| matches!(s, DepStatus::Missing | DepStatus::VersionLow { .. }))
            .map(|(d, _)| d.clone())
            .collect();
        let missing_names: Vec<String> = missing_deps.iter().map(|d| d.binary.clone()).collect();

        if fix && !missing_deps.is_empty() {
            if !json {
                println!(
                    "{}: {} (fixing: {})",
                    tool_name,
                    format_status(&result.status),
                    missing_names.join(", ")
                );
                println!("  Attempting auto-fix...");
            }
            let fix_output = auto_fix(&missing_deps).map_err(|e| (e, 1))?;
            if !json {
                for line in fix_output.lines() {
                    println!("  {line}");
                }
            }
            let post_result = check_tool(tool_name, spec);
            let post_missing: Vec<String> = post_result
                .required_results
                .iter()
                .chain(post_result.recommended_results.iter())
                .filter(|(_, s)| matches!(s, DepStatus::Missing | DepStatus::VersionLow { .. }))
                .map(|(d, _)| d.binary.clone())
                .collect();
            let fixed: Vec<String> = missing_names
                .iter()
                .filter(|n| !post_missing.contains(n))
                .cloned()
                .collect();
            if json {
                let post_status = if post_missing.is_empty() {
                    ReadyStatus::Ready
                } else {
                    ReadyStatus::Partial
                };
                println!(
                    "{}",
                    serde_json::to_string(&build_json_result(
                        tool_name,
                        &post_status,
                        &fixed,
                        &post_missing
                    ))
                    .unwrap()
                );
            } else {
                println!("{}: {}", tool_name, format_status(&post_result.status));
            }
        } else if json {
            println!(
                "{}",
                serde_json::to_string(&build_json_result(
                    tool_name,
                    &result.status,
                    &[],
                    &missing_names
                ))
                .unwrap()
            );
        } else {
            println!("{}: {}", tool_name, format_status(&result.status));
            for (dep, status) in &result.required_results {
                println!(
                    "  required: {} — {} [{}]",
                    dep.binary,
                    format_dep_status_label(status),
                    resolve_manager(&dep.manager)
                );
            }
            for (dep, status) in &result.recommended_results {
                println!(
                    "  recommended: {} — {} [{}]",
                    dep.binary,
                    format_dep_status_label(status),
                    resolve_manager(&dep.manager)
                );
            }
            println!();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn normalize_dep_simple_string() {
        let dep = normalize_dep(&json!("jq"));
        assert_eq!(dep.binary, "jq");
        assert_eq!(dep.package, "jq");
        assert_eq!(dep.manager, "rpm");
        assert!(dep.version.is_none());
    }

    #[test]
    fn normalize_dep_version_string() {
        let dep = normalize_dep(&json!("rtk>=0.35"));
        assert_eq!(dep.binary, "rtk");
        assert_eq!(dep.version.as_deref(), Some(">=0.35"));
    }

    #[test]
    fn normalize_dep_object() {
        let dep = normalize_dep(&json!({"binary": "curl", "package": "curl", "manager": "rpm"}));
        assert_eq!(dep.binary, "curl");
        assert_eq!(dep.manager, "rpm");
    }

    #[test]
    fn normalize_deps_mixed_array() {
        let deps = normalize_deps(
            &json!(["jq", "rtk>=0.35", {"binary": "curl", "package": "curl", "manager": "rpm"}]),
        );
        assert_eq!(deps.len(), 3);
    }

    #[test]
    fn normalize_deps_empty() {
        assert!(normalize_deps(&json!([])).is_empty());
    }

    #[test]
    fn extract_required_version_ge() {
        assert_eq!(extract_required_version(">=0.35"), "0.35");
    }

    #[test]
    fn version_ge_equal() {
        assert!(version_ge("0.35", "0.35"));
    }

    #[test]
    fn version_ge_greater() {
        assert!(version_ge("1.2.0", "1.0.0"));
    }

    #[test]
    fn version_ge_less() {
        assert!(!version_ge("0.34", "0.35"));
    }

    #[test]
    fn version_ge_prefixed_v() {
        assert!(version_ge("v22.1.0", "16.0.0"));
    }

    #[test]
    fn build_json_result_ready() {
        let result = build_json_result("Shell", &ReadyStatus::Ready, &[], &[]);
        assert_eq!(result["tool"], "Shell");
        assert_eq!(result["status"], "READY");
    }

    #[test]
    fn build_json_result_not_ready() {
        let result = build_json_result(
            "Shell",
            &ReadyStatus::NotReady,
            &[],
            &["fakebin99".to_string()],
        );
        assert_eq!(result["missing"][0], "fakebin99");
    }

    #[test]
    fn format_status_all() {
        assert_eq!(format_status(&ReadyStatus::Ready), "READY");
        assert_eq!(format_status(&ReadyStatus::Partial), "PARTIAL");
        assert_eq!(format_status(&ReadyStatus::NotReady), "NOT_READY");
        assert_eq!(format_status(&ReadyStatus::Unknown), "UNKNOWN");
    }

    #[test]
    fn load_spec_skips_meta_keys() {
        let tmp_dir = std::env::temp_dir();
        let spec_path = tmp_dir.join("test-tool-ready-spec.json");
        let spec_content = json!({
            "_comment": "skipped",
            "Shell": { "required": ["jq"], "recommended": [], "config_files": [], "permissions": [], "network": [] }
        });
        #[allow(clippy::disallowed_methods)]
        std::fs::write(&spec_path, serde_json::to_string(&spec_content).unwrap()).unwrap();
        let specs = load_spec(&spec_path).unwrap();
        assert!(!specs.contains_key("_comment"));
        assert!(specs.contains_key("Shell"));
        #[allow(clippy::disallowed_methods)]
        std::fs::remove_file(&spec_path).ok();
    }
}
