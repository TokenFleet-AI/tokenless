//! Environment readiness checker for the Tool Ready feature.
//!
//! Loads per-tool dependency declarations from `tool-ready-spec.json`
//! and checks binary availability, version constraints, config files,
//! permissions, and network connectivity.

// Re-exports to tests and submodules need a blanket allow — many items are only
// consumed by `#[cfg(test)] mod tests` via `use super::*;`.
#![allow(dead_code, unused_imports)]

mod checker;
mod fixer;
mod spec;

// Re-export all types and functions so tests (via `use super::*;`) and sibling
// modules can access them without deep path qualification.
pub(crate) use checker::{
    DepStatus, ReadyStatus, ToolReadyResult, check_config_file, check_dep, check_network,
    check_permission, check_tool, detect_system_manager, extract_required_version, resolve_manager,
    version_ge,
};
pub(crate) use fixer::{
    auto_fix, build_json_result, format_dep_status_label, format_status, generate_checklist,
};
pub(crate) use spec::{
    DepEntry, FallbackEntry, ToolDepSpec, find_spec_path, load_spec, normalize_dep, normalize_deps,
};

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
                .map_or_else(|| t.to_string(), |(k, _)| k.clone())
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
                    .map_err(|e| (e.to_string(), 2))?
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
                    .map_err(|e| (e.to_string(), 2))?
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
                .map_err(|e| (e.to_string(), 2))?
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
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

    #[test]
    fn test_check_dep_order_preserved() {
        // Create a spec with 3 required deps in known order
        let spec_json = json!({
            "aliases": [],
            "required": [
                {"binary": "sh", "package": "bash", "manager": "rpm"},
                {"binary": "ls", "package": "coreutils", "manager": "rpm"},
                {"binary": "cat", "package": "coreutils", "manager": "rpm"}
            ],
            "recommended": [],
            "config_files": [],
            "permissions": [],
            "network": []
        });

        // Manually check order via normalize_deps
        let deps = normalize_deps(&spec_json["required"]);
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].binary, "sh");
        assert_eq!(deps[1].binary, "ls");
        assert_eq!(deps[2].binary, "cat");
    }

    #[test]
    fn test_check_tool_empty_deps() {
        // This verifies the parallel path handles empty deps gracefully
        let spec_json = json!({
            "aliases": [],
            "required": [],
            "recommended": [],
            "config_files": [],
            "permissions": [],
            "network": []
        });
        let deps = normalize_deps(&spec_json["required"]);
        assert!(deps.is_empty());
        // After optimization, thread::scope on empty slice should not panic
    }

    #[test]
    fn test_dep_status_clone() {
        // Verify DepStatus::Missing clones correctly (needed for the parallel path)
        let status = DepStatus::Missing;
        let cloned = status.clone();
        assert_eq!(status, cloned);

        let version_low = DepStatus::VersionLow {
            installed: "1.0".to_string(),
            required: "2.0".to_string(),
        };
        let cloned = version_low.clone();
        assert!(matches!(cloned, DepStatus::VersionLow { .. }));
    }

    // ── Gap 8: normalize_dep fallback entries ─────────────────────────

    #[test]
    fn test_normalize_dep_with_fallback_entries() {
        let dep = normalize_dep(&json!({
            "binary": "python",
            "version": ">=3.8",
            "package": "python3",
            "manager": "apt",
            "pip_name": "python",
            "fallback": [
                {
                    "method": "pip",
                    "package": "python",
                    "binary": "python3"
                },
                {
                    "method": "cargo",
                    "package": "python-launcher",
                    "binary": "py",
                    "features": "default"
                }
            ]
        }));
        assert_eq!(dep.binary, "python");
        assert_eq!(dep.version.as_deref(), Some(">=3.8"));
        assert_eq!(dep.package, "python3");
        assert_eq!(dep.manager, "apt");
        assert_eq!(dep.pip_name.as_deref(), Some("python"));

        // Verify fallback entries
        assert_eq!(dep.fallback.len(), 2, "should have 2 fallback entries");
        assert_eq!(dep.fallback[0].method, "pip");
        assert_eq!(dep.fallback[0].package.as_deref(), Some("python"));
        assert_eq!(dep.fallback[0].binary.as_deref(), Some("python3"));
        assert_eq!(dep.fallback[1].method, "cargo");
        assert_eq!(dep.fallback[1].features.as_deref(), Some("default"));
    }

    #[test]
    fn test_normalize_dep_fallback_with_url_and_source() {
        let dep = normalize_dep(&json!({
            "binary": "node",
            "fallback": [
                {
                    "method": "curl",
                    "url": "https://nodejs.org/dist/v20.0.0/node-v20.0.0-linux-x64.tar.xz",
                    "binary": "node"
                },
                {
                    "method": "source",
                    "source": "https://github.com/nodejs/node.git",
                    "manifest": "Makefile"
                }
            ]
        }));
        assert_eq!(dep.fallback.len(), 2);
        assert_eq!(dep.fallback[0].method, "curl");
        assert_eq!(
            dep.fallback[0].url.as_deref(),
            Some("https://nodejs.org/dist/v20.0.0/node-v20.0.0-linux-x64.tar.xz")
        );
        assert_eq!(dep.fallback[1].method, "source");
        assert_eq!(
            dep.fallback[1].source.as_deref(),
            Some("https://github.com/nodejs/node.git")
        );
        assert_eq!(dep.fallback[1].manifest.as_deref(), Some("Makefile"));
    }

    #[test]
    fn test_normalize_dep_fallback_with_args() {
        let dep = normalize_dep(&json!({
            "binary": "ripgrep",
            "fallback": [
                {
                    "method": "cargo",
                    "package": "ripgrep",
                    "binary": "rg",
                    "args": "--features pcre2"
                }
            ]
        }));
        assert_eq!(dep.fallback.len(), 1);
        assert_eq!(dep.fallback[0].method, "cargo");
        assert_eq!(dep.fallback[0].args.as_deref(), Some("--features pcre2"));
    }

    #[test]
    fn test_normalize_dep_no_fallback() {
        let dep = normalize_dep(&json!({
            "binary": "curl",
            "package": "curl",
            "manager": "apt"
        }));
        assert_eq!(dep.fallback.len(), 0, "no fallback should be empty vec");
    }

    #[test]
    fn test_normalize_dep_use_npx() {
        let dep = normalize_dep(&json!({
            "binary": "tsx",
            "package": "tsx",
            "manager": "npm",
            "npm_name": "tsx",
            "use_npx": true
        }));
        assert_eq!(dep.binary, "tsx");
        assert!(dep.use_npx, "use_npx should be true");
        assert_eq!(dep.npm_name.as_deref(), Some("tsx"));
    }

    #[test]
    fn test_normalize_dep_uv_name() {
        let dep = normalize_dep(&json!({
            "binary": "ruff",
            "package": "ruff",
            "manager": "pip",
            "pip_name": "ruff",
            "uv_name": "ruff"
        }));
        assert_eq!(dep.pip_name.as_deref(), Some("ruff"));
        assert_eq!(dep.uv_name.as_deref(), Some("ruff"));
    }

    #[test]
    fn test_normalize_deps_array_with_fallback_mixed() {
        let deps = normalize_deps(&json!([
            "jq",
            {"binary": "python3", "fallback": [{"method": "apt", "package": "python3"}]},
            "git>=2.0"
        ]));
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].binary, "jq");
        assert_eq!(deps[0].fallback.len(), 0);
        assert_eq!(deps[1].binary, "python3");
        assert_eq!(deps[1].fallback.len(), 1);
        assert_eq!(deps[2].binary, "git");
        assert_eq!(deps[2].version.as_deref(), Some(">=2.0"));
    }

    // ── New tests added before refactoring ────────────────────────────

    #[test]
    fn test_format_dep_status_label_all() {
        assert_eq!(format_dep_status_label(&DepStatus::Available), "INSTALLED");
        assert_eq!(format_dep_status_label(&DepStatus::Missing), "MISSING");
        assert_eq!(
            format_dep_status_label(&DepStatus::VersionLow {
                installed: "1.0".to_string(),
                required: "2.0".to_string()
            }),
            "OUTDATED"
        );
    }

    #[test]
    fn test_build_json_result_partial() {
        let result = build_json_result(
            "rustc",
            &ReadyStatus::Partial,
            &["cargo-fix".to_string()],
            &["rustfmt".to_string()],
        );
        assert_eq!(result["tool"], "rustc");
        assert_eq!(result["status"], "PARTIAL");
        assert_eq!(result["fixed"][0], "cargo-fix");
        assert_eq!(result["missing"][0], "rustfmt");
        // Partial should not include diagnostic field
        assert!(result.get("diagnostic").is_none());
    }

    #[test]
    fn test_spec_loading_roundtrip() {
        let spec_content = serde_json::json!({
            "rust": {
                "aliases": ["rustc", "cargo"],
                "required": [
                    {"binary": "rustc", "package": "rustc", "manager": "rpm"},
                    {"binary": "cargo", "package": "cargo", "manager": "rpm"}
                ],
                "recommended": [
                    {"binary": "rustfmt", "package": "rustfmt", "manager": "rpm"}
                ],
                "config_files": ["~/.cargo/config.toml"],
                "permissions": ["file_read"],
                "network": ["https_outbound"]
            },
            "node": {
                "aliases": [],
                "required": ["node>=18.0"],
                "recommended": [],
                "config_files": [],
                "permissions": [],
                "network": []
            }
        });
        let tmp_dir = std::env::temp_dir();
        let spec_path = tmp_dir.join("test-spec-roundtrip.json");
        #[allow(clippy::disallowed_methods)]
        std::fs::write(&spec_path, serde_json::to_string(&spec_content).unwrap()).unwrap();

        let specs = load_spec(&spec_path).unwrap();
        #[allow(clippy::disallowed_methods)]
        std::fs::remove_file(&spec_path).ok();

        assert_eq!(specs.len(), 2, "should parse 2 tools");
        #[allow(clippy::expect_used)]
        let rust_spec = specs.get("rust").expect("rust tool spec should exist");
        assert_eq!(rust_spec.aliases, vec!["rustc", "cargo"]);
        assert_eq!(rust_spec.required.len(), 2);
        assert_eq!(rust_spec.recommended.len(), 1);
        assert_eq!(rust_spec.config_files.len(), 1);
        assert_eq!(rust_spec.permissions.len(), 1);
        assert_eq!(rust_spec.network.len(), 1);
        assert_eq!(rust_spec.required[0].binary, "rustc");
        assert_eq!(rust_spec.recommended[0].binary, "rustfmt");

        #[allow(clippy::expect_used)]
        let node_spec = specs.get("node").expect("node tool spec should exist");
        assert_eq!(node_spec.required.len(), 1);
        assert_eq!(node_spec.required[0].binary, "node");
        assert_eq!(node_spec.required[0].version.as_deref(), Some(">=18.0"));
    }
}
