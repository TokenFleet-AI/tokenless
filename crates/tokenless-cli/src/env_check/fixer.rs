//! Auto-fix and output formatting.
//!
//! Provides the auto-fix script runner and formatting helpers for
//! displaying tool-ready check results.

use std::{fmt::Write, os::unix::fs::MetadataExt, process::Command};

use serde_json::Value;

use crate::{
    env_check::{
        checker::{DepStatus, ReadyStatus, ToolReadyResult},
        spec::DepEntry,
    },
    shared::is_secure_default_enabled,
};

pub(crate) fn format_status(status: &ReadyStatus) -> &'static str {
    match status {
        ReadyStatus::Ready => "READY",
        ReadyStatus::Partial => "PARTIAL",
        ReadyStatus::NotReady => "NOT_READY",
        ReadyStatus::Unknown => "UNKNOWN",
    }
}

pub(crate) fn format_dep_status_label(status: &DepStatus) -> &'static str {
    match status {
        DepStatus::Available => "INSTALLED",
        DepStatus::Missing => "MISSING",
        DepStatus::VersionLow { .. } => "OUTDATED",
    }
}

pub(crate) fn generate_checklist(results: &[ToolReadyResult]) -> String {
    let mut output =
        String::from("Tool Environment Ready Checklist\n=================================\n\n");
    for result in results {
        let _ = writeln!(
            output,
            "{} [{}]",
            result.tool_name,
            format_status(&result.status)
        );
        for (dep, status) in &result.required_results {
            let _ = writeln!(
                output,
                "  required:   {:12} {}",
                dep.binary,
                format_dep_status_label(status)
            );
        }
        for (dep, status) in &result.recommended_results {
            let _ = writeln!(
                output,
                "  recommended:{:12} {}",
                dep.binary,
                format_dep_status_label(status)
            );
        }
        for (cfg, ok) in &result.config_results {
            let _ = writeln!(
                output,
                "  config:     {:12} {}",
                cfg,
                if *ok { "INSTALLED" } else { "MISSING" }
            );
        }
        for (perm, ok) in &result.permission_results {
            let _ = writeln!(
                output,
                "  permission: {:12} {}",
                perm,
                if *ok { "GRANTED" } else { "DENIED" }
            );
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
    let _ = writeln!(
        output,
        "Summary: {ready_count} ready, {partial_count} partial, {not_ready_count} not ready \
         (total: {})",
        results.len()
    );
    output
}

pub(crate) fn build_json_result(
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

pub(crate) fn auto_fix(missing_deps: &[DepEntry]) -> Result<String, String> {
    let home = crate::shared::get_home_dir();
    let cwd = std::env::current_dir().ok();
    let fix_script_candidates = [
        std::env::var("TOKENLESS_ENV_FIX_SCRIPT")
            .ok()
            .and_then(|p| {
                std::path::Path::new(&p).canonicalize().ok().and_then(|canon| {
                    let h = std::path::Path::new(&home);
                    if canon.starts_with(h) || canon.starts_with("/tmp") || canon.starts_with("/usr") {
                        Some(canon.display().to_string())
                    } else {
                        tracing::warn!(path = %p, "TOKENLESS_ENV_FIX_SCRIPT outside allowed dirs");
                        None
                    }
                })
            })
            .or_else(|| std::env::var("TOKENLESS_ENV_FIX_SCRIPT").ok()),
        cwd.as_ref().map(|d| {
            d.join("adapters/tokenless/common/tokenless-env-fix.sh")
                .to_string_lossy()
                .to_string()
        }),
        Some(format!(
            "{home}/.tokenfleet-ai/tokenless/tokenless-env-fix.sh"
        )),
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
        .unwrap_or_else(|| format!("{home}/.tokenfleet-ai/tokenless/tokenless-env-fix.sh"));

    if is_secure_default_enabled() {
        let script_path = std::path::Path::new(&fix_script);
        let metadata = std::fs::metadata(script_path)
            .map_err(|e| format!("Failed to stat fix script '{fix_script}': {e}"))?;
        if metadata.uid() != nix::unistd::Uid::effective().as_raw() {
            return Err(format!(
                "Secure-default rejected fix script not owned by current user: {fix_script}"
            ));
        }
        if metadata.mode() & 0o022 != 0 {
            return Err(format!(
                "Secure-default rejected fix script with group/world write bits: {fix_script}"
            ));
        }
    }

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
