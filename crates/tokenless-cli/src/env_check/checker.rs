//! Checking logic: binary availability, version constraints, config files,
//! permissions, and network connectivity.

use std::{fs, process::Command};

use crate::env_check::spec::{DepEntry, ToolDepSpec};

/// Check whether a command is available on `$PATH`.
fn check_cmd(cmd: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {cmd}")])
        .output()
        .map_or(false, |o| o.status.success())
}

/// Status of a single dependency check.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DepStatus {
    Available,
    Missing,
    VersionLow { installed: String, required: String },
}

/// Overall readiness status for a tool.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ReadyStatus {
    Ready,
    Partial,
    NotReady,
    Unknown,
}

/// Combined result for a single tool's environment check.
pub(crate) struct ToolReadyResult {
    pub(crate) tool_name: String,
    pub(crate) status: ReadyStatus,
    pub(crate) required_results: Vec<(DepEntry, DepStatus)>,
    pub(crate) recommended_results: Vec<(DepEntry, DepStatus)>,
    pub(crate) config_results: Vec<(String, bool)>,
    pub(crate) permission_results: Vec<(String, bool)>,
    pub(crate) network_results: Vec<(String, bool)>,
}

pub(crate) fn detect_system_manager() -> String {
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

pub(crate) fn resolve_manager(manager: &str) -> String {
    if manager == "rpm" {
        detect_system_manager()
    } else {
        manager.to_string()
    }
}

pub(crate) fn extract_required_version(version: &str) -> &str {
    version
        .strip_prefix(">=")
        .or_else(|| version.strip_prefix('>'))
        .unwrap_or(version)
}

pub(crate) fn version_ge(installed: &str, required: &str) -> bool {
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

pub(crate) fn check_dep(dep: &DepEntry) -> DepStatus {
    let found = Command::new("sh")
        .args(["-c", &format!("command -v \"$1\""), "--", &dep.binary])
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

pub(crate) fn check_config_file(path: &str) -> bool {
    let expanded = if path == "~" || path.starts_with("~/") {
        let home = crate::shared::get_home_dir();
        path.replacen('~', &home, 1)
    } else {
        path.to_string()
    };
    fs::metadata(&expanded).is_ok()
}

pub(crate) fn check_permission(perm: &str) -> bool {
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
        _ => {
            eprintln!("[tokenless] env_check: unknown permission type: {perm}");
            true
        }
    }
}

pub(crate) fn check_network(net: &str) -> bool {
    #[allow(clippy::match_single_binding)]
    match net {
        "https_outbound" => Command::new("curl")
            .args(["-s", "--max-time", "2", "https://example.com"])
            .output()
            .map_or(false, |o| o.status.success()),
        _ => true,
    }
}

pub(crate) fn check_tool(tool_name: &str, spec: &ToolDepSpec) -> ToolReadyResult {
    let req_count = spec.required.len();

    // Collect all deps into one slice: required first, then recommended.
    // Order is preserved via index-based result collection.
    let all_deps: Vec<&DepEntry> = spec
        .required
        .iter()
        .chain(spec.recommended.iter())
        .collect();

    // Parallel dep checking via thread::scope.
    // One thread per dep — each is I/O-bound (subprocess spawn + wait).
    let mut dep_statuses: Vec<DepStatus> = vec![DepStatus::Missing; all_deps.len()];
    if !all_deps.is_empty() {
        std::thread::scope(|s| {
            let handles: Vec<_> = all_deps
                .iter()
                .enumerate()
                .map(|(i, dep)| {
                    let dep_ref: &DepEntry = dep;
                    s.spawn(move || {
                        let status = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            check_dep(dep_ref)
                        }))
                        .unwrap_or(DepStatus::Missing);
                        (i, status)
                    })
                })
                .collect();
            for handle in handles {
                if let Ok((i, status)) = handle.join() {
                    dep_statuses[i] = status;
                }
            }
        });
    }

    // Split results back into required / recommended in original order
    let required_results: Vec<_> = spec
        .required
        .iter()
        .enumerate()
        .map(|(i, d)| (d.clone(), dep_statuses[i].clone()))
        .collect();
    let recommended_results: Vec<_> = spec
        .recommended
        .iter()
        .enumerate()
        .map(|(i, d)| (d.clone(), dep_statuses[req_count + i].clone()))
        .collect();

    // Config/permission/network checks remain sequential (fast fs operations)
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
