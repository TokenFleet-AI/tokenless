//! Rule-based field classification using compiled-in TOML profiles.

use serde::Deserialize;
use std::collections::HashMap;

/// Action to take for a JSON field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldAction {
    /// Keep the field regardless of standard truncation rules.
    Keep,
    /// Drop the field entirely from the output.
    Drop,
    /// Apply default truncation (neither explicitly kept nor dropped).
    Truncate,
}

/// Compiled rules for a single context category.
#[derive(Debug, Deserialize)]
struct CategoryRules {
    #[serde(default)]
    keep: Vec<String>,
    #[serde(default)]
    drop: Vec<String>,
}

/// All compiled context rules.
#[derive(Debug, Default, Deserialize)]
struct RulesDoc {
    #[serde(flatten)]
    categories: HashMap<String, CategoryRules>,
}

/// Static rules loaded at compile time from `context_rules.toml`.
#[allow(
    clippy::expect_used,
    reason = "bundled TOML is validated at compile time"
)]
static DEFAULT_RULES: std::sync::LazyLock<RulesDoc> = std::sync::LazyLock::new(|| {
    toml::from_str(include_str!("context_rules.toml")).expect("valid bundled TOML rules")
});

/// Detect the best-matching context category from a user-provided string.
///
/// Matching is keyword-based: each category has a hardcoded set of trigger
/// keywords for English and Chinese.  Returns a `&'static str` because
/// all category names are compile-time constants.
#[must_use]
pub(crate) fn detect_category(context: &str) -> &'static str {
    let ctx = context.to_lowercase();

    if ctx.contains("weather")
        || ctx.contains("temperature")
        || ctx.contains("forecast")
        || ctx.contains("天气")
        || ctx.contains("温度")
        || ctx.contains("气候")
    {
        return "weather";
    }
    if ctx.contains("deploy")
        || ctx.contains("pod")
        || ctx.contains("k8s")
        || ctx.contains("kubernetes")
        || ctx.contains("集群")
        || ctx.contains("部署")
    {
        return "devops";
    }
    if ctx.contains("query")
        || ctx.contains("sql")
        || ctx.contains("table")
        || ctx.contains("查询")
        || ctx.contains("数据库")
    {
        return "database";
    }
    if ctx.contains("git")
        || ctx.contains("commit")
        || ctx.contains("branch")
        || ctx.contains("仓库")
    {
        return "git";
    }

    "default"
}

/// Classify a single field name according to the rules of the given category.
pub(crate) fn classify_field(field_name: &str, category: &str) -> FieldAction {
    let rules = DEFAULT_RULES.categories.get(category);

    // Drop rules are checked first (security-sensitive fields).
    if let Some(rules) = rules {
        for pattern in &rules.drop {
            if glob_match(pattern, field_name) {
                return FieldAction::Drop;
            }
        }
    }

    // Keep rules override default truncation.
    if let Some(rules) = rules {
        for pattern in &rules.keep {
            if glob_match(pattern, field_name) {
                return FieldAction::Keep;
            }
        }
    }

    FieldAction::Truncate
}

/// Simple glob match: `*` matches any sequence, `?` matches a single character.
fn glob_match(pattern: &str, text: &str) -> bool {
    let pat = pattern.as_bytes();
    let txt = text.as_bytes();
    glob_match_bytes(pat, 0, txt, 0)
}

fn glob_match_bytes(pat: &[u8], pi: usize, txt: &[u8], ti: usize) -> bool {
    let mut pi = pi;
    let mut ti = ti;
    let mut star_pi = None;
    let mut star_ti = 0;

    while ti < txt.len() || pi < pat.len() {
        if pi < pat.len() && pat[pi] == b'*' {
            star_pi = Some(pi);
            star_ti = ti;
            pi += 1;
        } else if pi < pat.len() && ti < txt.len() && (pat[pi] == txt[ti] || pat[pi] == b'?') {
            pi += 1;
            ti += 1;
        } else if let Some(sp) = star_pi {
            pi = sp + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match() {
        assert!(glob_match("temp*", "temperature"));
        assert!(glob_match("temp*", "temp"));
        assert!(glob_match("temp*", "temp_celsius"));
        assert!(!glob_match("temp*", "system_status"));
        assert!(glob_match("*status*", "deployment_status"));
        assert!(glob_match("*status*", "status"));
        assert!(glob_match("sensor_*", "sensor_version"));
        assert!(glob_match("sensor_*", "sensor_123"));
        assert!(!glob_match("sensor_*", "sensor"));
    }

    #[test]
    fn test_detect_weather() {
        assert_eq!(detect_category("今天天气怎么样"), "weather");
        assert_eq!(detect_category("get weather forecast"), "weather");
        assert_eq!(detect_category("温度是多少"), "weather");
        assert_eq!(detect_category("what is the temperature"), "weather");
    }

    #[test]
    fn test_detect_devops() {
        assert_eq!(detect_category("deploy the app"), "devops");
        assert_eq!(detect_category("check kubernetes pods"), "devops");
        assert_eq!(detect_category("查看集群状态"), "devops");
    }

    #[test]
    fn test_detect_database() {
        assert_eq!(detect_category("run a sql query"), "database");
        assert_eq!(detect_category("查询数据库"), "database");
    }

    #[test]
    fn test_detect_git() {
        assert_eq!(detect_category("git status"), "git");
        assert_eq!(detect_category("show me the commits"), "git");
    }

    #[test]
    fn test_detect_default() {
        assert_eq!(detect_category("hello world"), "default");
        assert_eq!(detect_category(""), "default");
    }

    #[test]
    fn test_classify_weather() {
        assert_eq!(classify_field("temperature", "weather"), FieldAction::Keep);
        assert_eq!(classify_field("wind_speed", "weather"), FieldAction::Keep);
        assert_eq!(classify_field("station_id", "weather"), FieldAction::Drop);
        assert_eq!(
            classify_field("sensor_version", "weather"),
            FieldAction::Drop
        );
        assert_eq!(
            classify_field("unknown_field", "weather"),
            FieldAction::Truncate
        );
    }

    #[test]
    fn test_classify_devops() {
        assert_eq!(classify_field("pod_status", "devops"), FieldAction::Keep);
        assert_eq!(classify_field("cpu_usage", "devops"), FieldAction::Keep);
        assert_eq!(classify_field("uid", "devops"), FieldAction::Drop);
        assert_eq!(
            classify_field("owner_references", "devops"),
            FieldAction::Drop
        );
    }

    #[test]
    fn test_classify_default_drops_debug() {
        assert_eq!(classify_field("debug", "default"), FieldAction::Drop);
        assert_eq!(classify_field("trace", "default"), FieldAction::Drop);
        assert_eq!(classify_field("stacktrace", "default"), FieldAction::Drop);
    }
}
