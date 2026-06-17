//! MCP (Model Context Protocol) JSON-RPC 2.0 server over stdin/stdout.
//!
//! Provides 7 tools: `compress_schema`, `compress_response`, `rewrite_command`,
//! `compress_toon`, `decompress_toon`, `env_check`, `stats_summary`.
//!
//! Protocol: <https://spec.modelcontextprotocol.io/specification/2024-11-05/>

use std::{
    collections::HashMap,
    io::{self, BufRead, Write},
};

use rtk_registry::{Classification, classify_command, rewrite_command};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokenless_schema::{ResponseCompressor, SchemaCompressor};
use tokenless_stats::{StatsRecorder, StatsSummary, TokenlessConfig, estimate_tokens_from_bytes};

const SECURE_DEFAULT_MAX_SCHEMA_DESC: u64 = 1024;
const SECURE_DEFAULT_MAX_PARAM_DESC: u64 = 512;
const SECURE_DEFAULT_MAX_ENUM_ITEMS: u64 = 256;
const SECURE_DEFAULT_MAX_STRING_TRUNCATE: u64 = 4096;
const SECURE_DEFAULT_MAX_ARRAY_TRUNCATE: u64 = 256;
const SECURE_DEFAULT_MAX_STATS_LIMIT: u64 = 1000;

/// A single JSON-RPC 2.0 request from the MCP client.
#[derive(Debug, Deserialize)]
struct McpRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

/// A JSON-RPC 2.0 response back to the MCP client.
#[derive(Debug, Serialize)]
struct McpResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpError>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Serialize)]
struct McpError {
    code: i32,
    message: String,
}

// ── Helpers ────────────────────────────────────────────────────────────

/// Build a JSON-RPC error response.
fn error_response(id: Option<Value>, code: i32, message: &str) -> McpResponse {
    McpResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(McpError {
            code,
            message: message.to_string(),
        }),
    }
}

/// Look up the stats database path from env or default.
fn get_db_path() -> String {
    let home = crate::shared::get_home_dir();
    std::env::var("TOKENLESS_STATS_DB")
        .unwrap_or_else(|_| format!("{home}/.tokenfleet-ai/tokenless/stats.db"))
}

/// Ensure the stats database directory exists.
fn ensure_db_dir() -> Result<(), String> {
    let db_path = get_db_path();
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create database directory: {e}"))?;
    }
    Ok(())
}

/// Open the stats recorder, creating the DB directory if needed.
fn open_recorder() -> Result<StatsRecorder, String> {
    ensure_db_dir()?;
    StatsRecorder::new(get_db_path()).map_err(|e| format!("Failed to open database: {e}"))
}

/// Check whether a command is available on `$PATH`.
fn check_cmd(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .is_ok_and(|o| o.status.success())
}

// ── Main loop ──────────────────────────────────────────────────────────

/// Run the MCP JSON-RPC 2.0 server on stdin/stdout.
///
/// Reads one JSON line per request, processes it, and writes one JSON line
/// per response. The server exits when stdin is closed (EOF).
pub fn run_mcp() {
    let stdin = io::stdin();
    let stdout = io::stdout();

    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    let mut buf = String::new();
    let mut agent_id = String::from("mcp-client");

    loop {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) | Err(_) => break, // EOF or I/O error
            Ok(_) => {
                let trimmed = buf.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let req: McpRequest = match serde_json::from_str(trimmed) {
                    Ok(r) => r,
                    Err(e) => {
                        let resp = error_response(None, -32700, &format!("Parse error: {e}"));
                        writeln!(
                            writer,
                            "{}",
                            serde_json::to_string(&resp).unwrap_or_default()
                        )
                        .ok();
                        writer.flush().ok();
                        continue;
                    }
                };

                // Validate jsonrpc version.
                if req.jsonrpc != "2.0" {
                    let resp = error_response(
                        req.id.clone(),
                        -32600,
                        "Invalid Request: jsonrpc must be \"2.0\"",
                    );
                    writeln!(
                        writer,
                        "{}",
                        serde_json::to_string(&resp).unwrap_or_default()
                    )
                    .ok();
                    writer.flush().ok();
                    continue;
                }

                // Ignore notifications (id is null).
                if req.id.is_none() {
                    continue;
                }

                let resp = handle_message(&req, &mut agent_id);
                writeln!(
                    writer,
                    "{}",
                    serde_json::to_string(&resp).unwrap_or_default()
                )
                .ok();
                writer.flush().ok();
            }
        }
    }
}

// ── Message dispatch ───────────────────────────────────────────────────

fn handle_message(req: &McpRequest, agent_id: &mut String) -> McpResponse {
    match req.method.as_str() {
        "initialize" => handle_initialize(req, agent_id),
        "tools/list" => handle_tools_list(req),
        "tools/call" => handle_tools_call(req, agent_id),
        _ => error_response(
            req.id.clone(),
            -32601,
            &format!("Unknown method: {}", req.method),
        ),
    }
}

// ── initialize ─────────────────────────────────────────────────────────

fn handle_initialize(req: &McpRequest, agent_id: &mut String) -> McpResponse {
    if let Some(params) = &req.params
        && let Some(name) = params
            .get("clientInfo")
            .and_then(|c| c.get("name"))
            .and_then(|v| v.as_str())
    {
        *agent_id = match name {
            "claude-desktop" | "claude" | "claude-code" => name.to_string(),
            "cursor" => "cursor".to_string(),
            "continue" => "continue".to_string(),
            other => format!("mcp-{other}"),
        };
    }

    McpResponse {
        jsonrpc: "2.0",
        id: req.id.clone(),
        result: Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "tokenless",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        error: None,
    }
}

// ── tools/list ─────────────────────────────────────────────────────────

fn handle_tools_list(req: &McpRequest) -> McpResponse {
    McpResponse {
        jsonrpc: "2.0",
        id: req.id.clone(),
        result: Some(serde_json::json!({
            "tools": [
                {
                    "name": "compress_schema",
                    "description": "Compress an OpenAI Function Calling tool schema to reduce token usage (~57% savings).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "schema": { "type": "object", "description": "The tool schema to compress" },
                            "func_desc_max_len": { "type": "integer", "description": "Max chars for function description (default 256)" },
                            "param_desc_max_len": { "type": "integer", "description": "Max chars for parameter description (default 160)" },
                            "max_enum_items": { "type": "integer", "description": "Max enum items to keep (default unlimited)" }
                        },
                        "required": ["schema"]
                    }
                },
                {
                    "name": "compress_response",
                    "description": "Compress a JSON API/tool response to reduce token usage (~26-78% savings).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "response": { "type": "object", "description": "The JSON response to compress" },
                            "truncate_strings_at": { "type": "integer", "description": "Max string length (default 512)" },
                            "truncate_arrays_at": { "type": "integer", "description": "Max array length (default 16)" },
                            "drop_nulls": { "type": "boolean", "description": "Drop null values (default true)" }
                        },
                        "required": ["response"]
                    }
                },
                {
                    "name": "rewrite_command",
                    "description": "Rewrite a shell command to its RTK equivalent for token savings (60-90%).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "command": { "type": "string", "description": "The shell command to rewrite" }
                        },
                        "required": ["command"]
                    }
                },
                {
                    "name": "compress_toon",
                    "description": "Encode JSON to TOON format for 15-40% additional token savings.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "json": { "type": "object", "description": "The JSON to encode to TOON" }
                        },
                        "required": ["json"]
                    }
                },
                {
                    "name": "decompress_toon",
                    "description": "Decode TOON format back to JSON.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "toon": { "type": "string", "description": "The TOON string to decode" }
                        },
                        "required": ["toon"]
                    }
                },
                {
                    "name": "env_check",
                    "description": "Check tool execution environment readiness.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "tool": { "type": "string", "description": "Tool name to check (omit for all)" }
                        }
                    }
                },
                {
                    "name": "stats_summary",
                    "description": "View compression statistics summary.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "project": { "type": "string", "description": "Filter by project name" },
                            "namespace": { "type": "string", "description": "Filter by namespace" },
                            "limit": { "type": "integer", "description": "Max records to include" }
                        }
                    }
                }
            ]
        })),
        error: None,
    }
}

// ── tools/call ─────────────────────────────────────────────────────────

fn handle_tools_call(req: &McpRequest, agent_id: &str) -> McpResponse {
    let Some(params) = &req.params else {
        return error_response(req.id.clone(), -32602, "Missing params");
    };

    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let mut args = params.get("arguments").cloned().unwrap_or(Value::Null);

    if TokenlessConfig::load().is_secure_default_enabled()
        && let Err(error) = validate_secure_default_args(tool_name, &mut args)
    {
        return error_response(req.id.clone(), -32602, &error);
    }

    match execute_tool(tool_name, &args, agent_id) {
        Ok(result) => McpResponse {
            jsonrpc: "2.0",
            id: req.id.clone(),
            result: Some(serde_json::json!({
                "content": [{"type": "text", "text": serde_json::to_string(&result).unwrap_or_default()}]
            })),
            error: None,
        },
        Err(e) => error_response(req.id.clone(), -32603, &e),
    }
}

// ── Tool dispatch ──────────────────────────────────────────────────────

fn execute_tool(tool_name: &str, args: &Value, agent_id: &str) -> Result<Value, String> {
    match tool_name {
        "compress_schema" => exec_compress_schema(args),
        "compress_response" => exec_compress_response(args),
        "rewrite_command" => exec_rewrite_command(args, agent_id),
        "compress_toon" => exec_compress_toon(args),
        "decompress_toon" => exec_decompress_toon(args),
        "env_check" => Ok(exec_env_check(args)),
        "stats_summary" => exec_stats_summary(args),
        other => Err(format!("Unknown tool: {other}")),
    }
}

fn validate_secure_default_args(tool_name: &str, args: &mut Value) -> Result<(), String> {
    match tool_name {
        "compress_schema" => {
            clamp_u64_arg(args, "func_desc_max_len", SECURE_DEFAULT_MAX_SCHEMA_DESC)?;
            clamp_u64_arg(args, "param_desc_max_len", SECURE_DEFAULT_MAX_PARAM_DESC)?;
            clamp_u64_arg(args, "max_enum_items", SECURE_DEFAULT_MAX_ENUM_ITEMS)?;
        }
        "compress_response" => {
            clamp_u64_arg(
                args,
                "truncate_strings_at",
                SECURE_DEFAULT_MAX_STRING_TRUNCATE,
            )?;
            clamp_u64_arg(
                args,
                "truncate_arrays_at",
                SECURE_DEFAULT_MAX_ARRAY_TRUNCATE,
            )?;
        }
        "stats_summary" => {
            clamp_u64_arg(args, "limit", SECURE_DEFAULT_MAX_STATS_LIMIT)?;
        }
        _ => {}
    }
    Ok(())
}

fn clamp_u64_arg(args: &mut Value, key: &str, max_value: u64) -> Result<(), String> {
    let Some(value) = args.get_mut(key) else {
        return Ok(());
    };
    let Some(current) = value.as_u64() else {
        return Err(format!(
            "Invalid '{key}' parameter: expected non-negative integer"
        ));
    };
    if current > max_value {
        *value = Value::Number(max_value.into());
    }
    Ok(())
}

// ── Tool: compress_schema ──────────────────────────────────────────────

#[allow(clippy::cast_possible_truncation)]
fn exec_compress_schema(args: &Value) -> Result<Value, String> {
    let schema = args
        .get("schema")
        .ok_or_else(|| "Missing 'schema' parameter".to_string())?;

    let before = serde_json::to_string(schema).unwrap_or_default();

    let mut compressor = SchemaCompressor::new();
    if let Some(n) = args.get("func_desc_max_len").and_then(Value::as_u64) {
        compressor = compressor.with_func_desc_max_len(n as usize);
    }
    if let Some(n) = args.get("param_desc_max_len").and_then(Value::as_u64) {
        compressor = compressor.with_param_desc_max_len(n as usize);
    }
    if let Some(n) = args.get("max_enum_items").and_then(Value::as_u64) {
        compressor = compressor.with_max_enum_items(n as usize);
    }

    let compressed = compressor.compress(schema);
    let after = serde_json::to_string(&compressed).unwrap_or_default();

    Ok(serde_json::json!({
        "compressed": compressed,
        "savings": {
            "chars_before": before.len(),
            "chars_after": after.len(),
            "tokens_before": estimate_tokens_from_bytes(before.len()),
            "tokens_after": estimate_tokens_from_bytes(after.len()),
        }
    }))
}

// ── Tool: compress_response ────────────────────────────────────────────

#[allow(clippy::cast_possible_truncation)]
fn exec_compress_response(args: &Value) -> Result<Value, String> {
    let response = args
        .get("response")
        .ok_or_else(|| "Missing 'response' parameter".to_string())?;

    let before = serde_json::to_string(response).unwrap_or_default();

    let mut compressor = ResponseCompressor::new();
    if let Some(n) = args.get("truncate_strings_at").and_then(Value::as_u64) {
        compressor = compressor.with_truncate_strings_at(n as usize);
    }
    if let Some(n) = args.get("truncate_arrays_at").and_then(Value::as_u64) {
        compressor = compressor.with_truncate_arrays_at(n as usize);
    }
    if let Some(drop) = args.get("drop_nulls").and_then(Value::as_bool) {
        compressor = compressor.with_drop_nulls(drop);
    }

    let compressed = compressor.compress(response);
    let after = serde_json::to_string(&compressed).unwrap_or_default();

    Ok(serde_json::json!({
        "compressed": compressed,
        "savings": {
            "chars_before": before.len(),
            "chars_after": after.len(),
            "tokens_before": estimate_tokens_from_bytes(before.len()),
            "tokens_after": estimate_tokens_from_bytes(after.len()),
        }
    }))
}

// ── Tool: rewrite_command ──────────────────────────────────────────────

fn exec_rewrite_command(args: &Value, agent_id: &str) -> Result<Value, String> {
    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'command' parameter".to_string())?;

    if rtk_registry::contains_unattestable_construct(command) {
        return Ok(serde_json::json!({
            "rewritten": command,
            "savings_pct": 0.0_f64,
            "agent_id": agent_id,
            "skipped": true,
            "skipped_reason": "unattestable_construct",
        }));
    }

    let rewritten = rewrite_command(command, &[], &[]);

    match rewritten {
        Some(rw) if rw != command => {
            let savings_pct = match classify_command(command) {
                Classification::Supported {
                    estimated_savings_pct,
                    ..
                } => estimated_savings_pct,
                _ => 0.0,
            };

            Ok(serde_json::json!({
                "rewritten": rw,
                "savings_pct": savings_pct,
                "agent_id": agent_id,
            }))
        }
        _ => {
            // No rewrite available — return original command as pass-through.
            let savings_pct = 0.0_f64;
            Ok(serde_json::json!({
                "rewritten": command,
                "savings_pct": savings_pct,
                "agent_id": agent_id,
            }))
        }
    }
}

// ── Tool: compress_toon ────────────────────────────────────────────────

fn exec_compress_toon(args: &Value) -> Result<Value, String> {
    let json = args
        .get("json")
        .ok_or_else(|| "Missing 'json' parameter".to_string())?;

    let before = serde_json::to_string(json).unwrap_or_default();

    let toon = toon_format::encode_default(json).map_err(|e| format!("TOON encode failed: {e}"))?;

    Ok(serde_json::json!({
        "toon": toon,
        "savings": {
            "chars_before": before.len(),
            "chars_after": toon.len(),
            "tokens_before": estimate_tokens_from_bytes(before.len()),
            "tokens_after": estimate_tokens_from_bytes(toon.len()),
        }
    }))
}

// ── Tool: decompress_toon ──────────────────────────────────────────────

fn exec_decompress_toon(args: &Value) -> Result<Value, String> {
    let toon = args
        .get("toon")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'toon' parameter".to_string())?;

    let json: Value =
        toon_format::decode_default(toon).map_err(|e| format!("TOON decode failed: {e}"))?;

    Ok(serde_json::json!({
        "json": json,
    }))
}

// ── Tool: env_check ────────────────────────────────────────────────────

fn exec_env_check(args: &Value) -> Value {
    let tool = args.get("tool").and_then(|v| v.as_str());

    // Known tool binaries that tokenless depends on.
    let known_tools: HashMap<&str, &str> = HashMap::from([
        ("rtk", "Rust Token Killer (RTK) for command rewriting"),
        ("jq", "JSON processor for schema handling"),
        ("curl", "HTTP client for network checks"),
        ("git", "Version control system"),
        ("bash", "Unix shell"),
    ]);

    if let Some(tool_name) = tool {
        let available = check_cmd(tool_name);
        let desc = known_tools
            .get(tool_name)
            .copied()
            .unwrap_or("Unknown tool");
        let status = if available { "available" } else { "not_found" };

        let diagnostics = if available {
            None
        } else {
            Some(format!(
                "Tool '{tool_name}' ({desc}) is not available on PATH."
            ))
        };

        serde_json::json!({
            "status": status,
            "tool_name": tool_name,
            "diagnostics": diagnostics,
            "deps": [{"name": tool_name, "available": available}],
        })
    } else {
        let mut deps = Vec::new();
        let mut all_available = true;

        for (name, desc) in &known_tools {
            let available = check_cmd(name);
            if !available {
                all_available = false;
            }
            deps.push(serde_json::json!({
                "name": name,
                "description": desc,
                "available": available,
            }));
        }

        let status = if all_available { "ready" } else { "partial" };

        serde_json::json!({
            "status": status,
            "tool_name": "all",
            "diagnostics": if all_available { None } else { Some("Some dependencies are missing. Install them for full functionality.".to_string()) },
            "deps": deps,
        })
    }
}

// ── Tool: stats_summary ────────────────────────────────────────────────

fn exec_stats_summary(args: &Value) -> Result<Value, String> {
    let limit = args.get("limit").and_then(Value::as_u64);
    #[allow(clippy::cast_possible_truncation)]
    let limit = limit.map(|n| n as usize);
    let project = args.get("project").and_then(|v| v.as_str());
    let namespace = args.get("namespace").and_then(|v| v.as_str());

    if !TokenlessConfig::load().is_stats_enabled() {
        return Ok(serde_json::json!({
            "summary": {
                "total_records": 0,
                "total_saved_chars": 0,
                "total_saved_tokens": 0,
                "chars_percent": 0.0,
                "tokens_percent": 0.0,
                "note": "Stats recording is disabled.",
            }
        }));
    }

    let recorder = open_recorder()?;
    let records = recorder
        .records_filtered(None, None, project, namespace, limit)
        .map_err(|e| format!("Failed to query records: {e}"))?;

    let summary = StatsSummary::from_records(&records);

    Ok(serde_json::json!({
        "summary": {
            "total_records": summary.total_records,
            "total_saved_chars": summary.chars_saved(),
            "total_saved_tokens": summary.tokens_saved(),
            "total_before_chars": summary.total_before_chars,
            "total_after_chars": summary.total_after_chars,
            "total_before_tokens": summary.total_before_tokens,
            "total_after_tokens": summary.total_after_tokens,
            "chars_percent": summary.chars_percent(),
            "tokens_percent": summary.tokens_percent(),
            "project": project,
            "namespace": namespace,
        }
    }))
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_error_response() {
        let resp = error_response(Some(Value::Number(1.into())), -32601, "Unknown method");
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, Some(Value::Number(1.into())));
        assert!(resp.result.is_none());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("Unknown method"));
    }

    #[test]
    fn test_error_response_no_id() {
        let resp = error_response(None, -32700, "Parse error");
        assert!(resp.id.is_none());
        assert!(resp.error.is_some());
    }

    #[test]
    fn test_handle_initialize() {
        let req = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(1.into())),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "clientInfo": { "name": "claude-code", "version": "1.0" },
                "protocolVersion": "2024-11-05"
            })),
        };
        let mut agent_id = String::from("mcp-client");
        let resp = handle_message(&req, &mut agent_id);
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.error.is_none());
        assert_eq!(agent_id, "claude-code");

        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["capabilities"]["tools"], serde_json::json!({}));
        assert_eq!(result["serverInfo"]["name"], "tokenless");
        assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_handle_initialize_unknown_client() {
        let req = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(2.into())),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "clientInfo": { "name": "custom-agent", "version": "2.0" }
            })),
        };
        let mut agent_id = String::from("mcp-client");
        let resp = handle_message(&req, &mut agent_id);
        assert!(resp.error.is_none());
        assert_eq!(agent_id, "mcp-custom-agent");
    }

    #[test]
    fn test_handle_tools_list() {
        let req = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(3.into())),
            method: "tools/list".to_string(),
            params: None,
        };
        let mut agent_id = String::from("test");
        let resp = handle_message(&req, &mut agent_id);
        assert!(resp.error.is_none());

        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 7, "should return exactly 7 tools");

        let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(tool_names.contains(&"compress_schema"));
        assert!(tool_names.contains(&"compress_response"));
        assert!(tool_names.contains(&"rewrite_command"));
        assert!(tool_names.contains(&"compress_toon"));
        assert!(tool_names.contains(&"decompress_toon"));
        assert!(tool_names.contains(&"env_check"));
        assert!(tool_names.contains(&"stats_summary"));
    }

    #[test]
    fn test_handle_unknown_method() {
        let req = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(4.into())),
            method: "bogus/method".to_string(),
            params: None,
        };
        let mut agent_id = String::from("test");
        let resp = handle_message(&req, &mut agent_id);
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("bogus/method"));
    }

    #[test]
    fn test_exec_compress_schema_basic() {
        let args = serde_json::json!({
            "schema": {
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "description": "Get the current weather for a given city. Returns temperature, humidity, and conditions.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "city": {
                                "type": "string",
                                "description": "The city name, e.g. San Francisco, CA. Use the full city name."
                            },
                            "unit": {
                                "type": "string",
                                "enum": ["celsius", "fahrenheit"],
                                "description": "Temperature unit"
                            }
                        },
                        "required": ["city"]
                    }
                }
            }
        });
        let result = exec_compress_schema(&args).unwrap();
        assert!(
            result.get("compressed").is_some(),
            "should have compressed field"
        );
        let savings = &result["savings"];
        assert!(savings["chars_before"].as_u64().unwrap() > 0);
        assert!(savings["tokens_before"].as_u64().unwrap() > 0);
    }

    #[test]
    fn test_exec_compress_schema_missing_schema() {
        let args = serde_json::json!({});
        let result = exec_compress_schema(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'schema'"));
    }

    #[test]
    fn test_exec_compress_schema_with_options() {
        let args = serde_json::json!({
            "schema": {
                "type": "function",
                "function": {
                    "name": "test",
                    "description": "A".repeat(300),
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "x": {
                                "type": "string",
                                "description": "B".repeat(200)
                            }
                        }
                    }
                }
            },
            "func_desc_max_len": 128,
            "param_desc_max_len": 64,
            "max_enum_items": 10
        });
        let result = exec_compress_schema(&args).unwrap();
        let compressed = &result["compressed"];
        let func_desc = compressed["function"]["description"].as_str().unwrap();
        // Description should be truncated to at most func_desc_max_len
        assert!(func_desc.len() <= 128);
    }

    #[test]
    fn test_exec_compress_response_basic() {
        let args = serde_json::json!({
            "response": {
                "debug": "some debug info",
                "result": { "data": [1, 2, 3], "total": 3 },
                "trace": "stack trace here"
            }
        });
        let result = exec_compress_response(&args).unwrap();
        let compressed = &result["compressed"];
        // debug and trace fields should be dropped
        assert!(compressed.get("debug").is_none());
        assert!(compressed.get("trace").is_none());
        assert!(compressed.get("result").is_some());
    }

    #[test]
    fn test_exec_compress_response_missing_response() {
        let args = serde_json::json!({});
        let result = exec_compress_response(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_exec_compress_response_with_options() {
        let long_string = "x".repeat(1000);
        let args = serde_json::json!({
            "response": {
                "data": long_string,
                "items": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20],
                "nullable": null
            },
            "truncate_strings_at": 500,
            "truncate_arrays_at": 10,
            "drop_nulls": true
        });
        let result = exec_compress_response(&args).unwrap();
        let compressed = &result["compressed"];
        // Null field should be dropped
        assert!(compressed.get("nullable").is_none());
    }

    #[test]
    fn test_exec_rewrite_command_rtk() {
        let args = serde_json::json!({
            "command": "git status"
        });
        let result = exec_rewrite_command(&args, "test-agent").unwrap();
        assert!(result.get("rewritten").is_some());
        // With RTK available: should rewrite to "rtk git status"
        // Without RTK: may still be rewritten by rtk-registry since it does
        // pure text matching. Just verify the structure is valid.
        let rewritten = result["rewritten"].as_str().unwrap();
        assert!(!rewritten.is_empty());
        assert!(result.get("savings_pct").is_some());
        assert_eq!(result["agent_id"], "test-agent");
    }

    #[test]
    fn test_exec_rewrite_command_missing_command() {
        let args = serde_json::json!({});
        let result = exec_rewrite_command(&args, "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_exec_rewrite_command_skips_backtick_substitution() {
        let args = serde_json::json!({
            "command": "git log --pretty=`whoami`"
        });
        let result = exec_rewrite_command(&args, "test-agent").unwrap();
        assert_eq!(result["skipped"], true);
        assert_eq!(result["skipped_reason"], "unattestable_construct");
        assert_eq!(result["rewritten"], "git log --pretty=`whoami`");
        assert_eq!(result["savings_pct"], 0.0);
    }

    #[test]
    fn test_exec_rewrite_command_skips_dollar_paren_substitution() {
        let args = serde_json::json!({
            "command": "git status $(rm -rf /)"
        });
        let result = exec_rewrite_command(&args, "test-agent").unwrap();
        assert_eq!(result["skipped"], true);
        assert_eq!(result["skipped_reason"], "unattestable_construct");
    }

    #[test]
    fn test_exec_rewrite_command_allows_simple_variable_expansion() {
        let args = serde_json::json!({
            "command": "echo $HOME"
        });
        let result = exec_rewrite_command(&args, "test-agent").unwrap();
        // echo is not rewritable, but the command should not be flagged as unsafe
        assert!(result.get("skipped").is_none() || result["skipped"] == false);
    }

    #[test]
    fn test_exec_compress_toon_basic() {
        let args = serde_json::json!({
            "json": { "name": "Alice", "age": 30 }
        });
        let result = exec_compress_toon(&args).unwrap();
        assert!(result.get("toon").is_some());
        let toon = result["toon"].as_str().unwrap();
        assert!(!toon.is_empty());
        // Should contain TOON-format key:value pairs
        assert!(toon.contains("name:"));
        assert!(toon.contains("Alice"));
    }

    #[test]
    fn test_exec_compress_toon_missing_json() {
        let args = serde_json::json!({});
        let result = exec_compress_toon(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_exec_decompress_toon_roundtrip() {
        let original = serde_json::json!({
            "name": "Test",
            "count": 42,
            "tags": ["a", "b", "c"]
        });
        let args = serde_json::json!({ "json": original.clone() });
        let toon_result = exec_compress_toon(&args).unwrap();
        let toon_str = toon_result["toon"].as_str().unwrap().to_string();

        let decode_args = serde_json::json!({ "toon": toon_str });
        let decoded = exec_decompress_toon(&decode_args).unwrap();
        assert!(decoded.get("json").is_some());
    }

    #[test]
    fn test_exec_decompress_toon_missing_toon() {
        let args = serde_json::json!({});
        let result = exec_decompress_toon(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_exec_decompress_toon_invalid() {
        let args = serde_json::json!({ "toon": "{{{{invalid toon!!!!" });
        let result = exec_decompress_toon(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_exec_env_check_single() {
        let args = serde_json::json!({ "tool": "bash" });
        let result = exec_env_check(&args);
        assert_eq!(result["tool_name"], "bash");
        // bash should be available on any reasonable system
        assert!(result["status"].as_str().is_some());
    }

    #[test]
    fn test_exec_env_check_unknown_tool() {
        let args = serde_json::json!({ "tool": "nonexistent-tool-xyzzy" });
        let result = exec_env_check(&args);
        assert_eq!(result["status"], "not_found");
        assert!(result["diagnostics"].as_str().is_some());
    }

    #[test]
    fn test_exec_env_check_all() {
        let args = serde_json::json!({});
        let result = exec_env_check(&args);
        assert_eq!(result["tool_name"], "all");
        let deps = result["deps"].as_array().unwrap();
        assert!(!deps.is_empty(), "should have at least known tools");
        // status should be "ready" or "partial"
        let status = result["status"].as_str().unwrap();
        assert!(status == "ready" || status == "partial");
    }

    #[test]
    fn test_exec_stats_summary_without_limit() {
        let args = serde_json::json!({});
        let result = exec_stats_summary(&args);
        // May fail if DB is not accessible (no home dir in test env),
        // but should not panic.
        if let Ok(result) = result {
            assert!(result.get("summary").is_some());
            let summary = &result["summary"];
            assert!(summary["total_records"].as_u64().is_some());
        }
    }

    #[test]
    fn test_exec_stats_summary_with_limit() {
        let args = serde_json::json!({ "limit": 5 });
        let result = exec_stats_summary(&args);
        if let Ok(result) = result {
            assert!(result.get("summary").is_some());
        }
    }

    #[test]
    fn test_execute_tool_unknown() {
        let args = serde_json::json!({});
        let result = execute_tool("nonexistent", &args, "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown tool"));
    }

    // ── McpResponse / McpError serialization ────────────────────────────

    #[test]
    fn test_mcp_response_serialization_success() {
        let resp = McpResponse {
            jsonrpc: "2.0",
            id: Some(Value::Number(1.into())),
            result: Some(serde_json::json!({"key": "val"})),
            error: None,
        };
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(json_str.contains("\"jsonrpc\":\"2.0\""));
        assert!(json_str.contains("\"id\":1"));
        assert!(json_str.contains("\"result\":{"));
        assert!(!json_str.contains("\"error\""));
    }

    #[test]
    fn test_mcp_response_serialization_error() {
        let resp = McpResponse {
            jsonrpc: "2.0",
            id: Some(Value::Number(2.into())),
            result: None,
            error: Some(McpError {
                code: -32603,
                message: "Internal error".to_string(),
            }),
        };
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(json_str.contains("\"error\":{"));
        assert!(!json_str.contains("\"result\""));
    }

    #[test]
    fn test_mcp_response_no_id() {
        let resp = McpResponse {
            jsonrpc: "2.0",
            id: None,
            result: Some(serde_json::json!({"ok": true})),
            error: None,
        };
        let json_str = serde_json::to_string(&resp).unwrap();
        assert!(!json_str.contains("\"id\""));
    }

    #[test]
    fn test_handle_message_notification_is_ignored() {
        // Notifications have no id field – should be ignored in run_mcp loop.
        let req = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };
        let mut agent_id = String::from("test");
        let resp = handle_message(&req, &mut agent_id);
        // handle_message doesn't check for id; the run_mcp loop skips
        // notifications. But handle_message itself still processes the method.
        // "notifications/initialized" is an unknown method, so:
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32601);
    }

    // ── execute_tool integration ────────────────────────────────────────

    #[test]
    fn test_execute_tool_compress_schema_via_dispatch() {
        let args = serde_json::json!({
            "schema": {"type": "object", "properties": {}}
        });
        let result = execute_tool("compress_schema", &args, "test");
        assert!(result.is_ok());
        let val = result.unwrap();
        assert!(val.get("compressed").is_some());
    }

    #[test]
    fn test_execute_tool_rewrite_via_dispatch() {
        let args = serde_json::json!({"command": "ls -la"});
        let result = execute_tool("rewrite_command", &args, "test-agent-2");
        assert!(result.is_ok());
        let val = result.unwrap();
        assert!(val.get("rewritten").is_some());
    }

    #[test]
    fn test_tools_call_missing_params() {
        let req = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(10.into())),
            method: "tools/call".to_string(),
            params: None,
        };
        let resp = handle_tools_call(&req, "test");
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32602);
    }

    #[test]
    fn test_tools_call_unknown_tool() {
        let req = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(11.into())),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "bogus_tool",
                "arguments": {}
            })),
        };
        let resp = handle_tools_call(&req, "test");
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32603);
    }
}
