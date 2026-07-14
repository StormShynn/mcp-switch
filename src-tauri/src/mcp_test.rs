use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::types::{ConnectionTestResult, McpServerEntry};

const INIT_REQ: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"mcp-switch","version":"0.4.3"}}}"#;

pub fn test_connection(entry: &McpServerEntry) -> ConnectionTestResult {
    match entry.transport.as_str() {
        "stdio" => test_stdio(entry),
        "http" => test_http(entry),
        "sse" => test_sse(entry),
        other => ConnectionTestResult {
            success: false,
            message: format!("Unknown transport: {other}"),
            server_info: None,
        },
    }
}

fn test_stdio(entry: &McpServerEntry) -> ConnectionTestResult {
    let command = match &entry.command {
        Some(c) => c,
        None => {
            return ConnectionTestResult {
                success: false,
                message: "No command configured".to_string(),
                server_info: None,
            }
        }
    };

    let args: Vec<&str> = entry
        .args
        .as_ref()
        .map(|a| a.iter().map(|s| s.as_str()).collect())
        .unwrap_or_default();

    let (cmd_path, cmd_args) = crate::winshim::build_test_command(command, &args);

    let mut process = Command::new(&cmd_path);
    process.args(&cmd_args);
    process.stdin(Stdio::piped());
    process.stdout(Stdio::piped());
    process.stderr(Stdio::null());

    if let Some(env) = &entry.env {
        for (k, v) in env {
            process.env(k, v);
        }
    }

    let mut child = match process.spawn() {
        Ok(c) => c,
        Err(e) => {
            return ConnectionTestResult {
                success: false,
                message: format!("Failed to launch \"{command}\": {e}"),
                server_info: None,
            }
        }
    };

    let mut stdin = match child.stdin.take() {
        Some(s) => s,
        None => {
            let _ = child.kill();
            return ConnectionTestResult {
                success: false,
                message: "Could not open stdin".to_string(),
                server_info: None,
            };
        }
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = child.kill();
            return ConnectionTestResult {
                success: false,
                message: "Could not open stdout".to_string(),
                server_info: None,
            };
        }
    };

    if writeln!(stdin, "{INIT_REQ}").is_err() {
        let _ = child.kill();
        return ConnectionTestResult {
            success: false,
            message: "Failed to send initialize request".to_string(),
            server_info: None,
        };
    }
    drop(stdin);

    let (tx, rx): (mpsc::Sender<Option<String>>, _) = mpsc::channel();
    let reader = BufReader::new(stdout);

    thread::spawn(move || {
        let mut collected = String::new();
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    collected.push_str(&l);
                    collected.push('\n');
                    if let Some(result) = try_parse_mcp_response(&collected) {
                        let _ = tx.send(Some(result));
                        return;
                    }
                }
                Err(_) => {
                    let _ = tx.send(None);
                    return;
                }
            }
        }
        let _ = tx.send(None);
    });

    let timeout_dur = Duration::from_secs(15);
    let result = match rx.recv_timeout(timeout_dur) {
        Ok(Some(info)) => ConnectionTestResult {
            success: true,
            message: format!("Server responded: {info}"),
            server_info: Some(info),
        },
        Ok(None) => ConnectionTestResult {
            success: false,
            message: "Process exited without sending a valid response".to_string(),
            server_info: None,
        },
        Err(_) => ConnectionTestResult {
            success: false,
            message: "Timed out after 15s — server may be starting slowly or hanging".to_string(),
            server_info: None,
        },
    };

    let _ = child.kill();
    let _ = child.wait();
    result
}

fn test_http(entry: &McpServerEntry) -> ConnectionTestResult {
    let url = match &entry.url {
        Some(u) => u,
        None => {
            return ConnectionTestResult {
                success: false,
                message: "No URL configured".to_string(),
                server_info: None,
            }
        }
    };

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "mcp-switch", "version": "0.4.3" }
        }
    });

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();

    let mut req = agent.post(url).set("content-type", "application/json");

    if let Some(headers) = &entry.headers {
        for (k, v) in headers {
            req = req.set(k, v);
        }
    }

    match req.send_json(body) {
        Ok(resp) => {
            let status = resp.status();
            let text = resp.into_string().unwrap_or_default();
            if let Some(ref info) = try_parse_mcp_response(&text) {
                ConnectionTestResult {
                    success: true,
                    message: format!("Server responded (HTTP {status}): {info}"),
                    server_info: Some(info.clone()),
                }
            } else {
                ConnectionTestResult {
                    success: true,
                    message: format!("Server reachable (HTTP {status}), but response was not a valid MCP initialize reply"),
                    server_info: None,
                }
            }
        }
        Err(ureq::Error::Status(status, resp)) => {
            let body_text = resp.into_string().unwrap_or_default();
            ConnectionTestResult {
                success: false,
                message: format!("Server returned HTTP {status}: {body_text}"),
                server_info: None,
            }
        }
        Err(ureq::Error::Transport(e)) => ConnectionTestResult {
            success: false,
            message: format!("Connection failed: {e}"),
            server_info: None,
        },
    }
}

fn test_sse(entry: &McpServerEntry) -> ConnectionTestResult {
    let url = match &entry.url {
        Some(u) => u,
        None => {
            return ConnectionTestResult {
                success: false,
                message: "No URL configured".to_string(),
                server_info: None,
            }
        }
    };

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();

    let mut req = agent.get(url).set("accept", "text/event-stream");

    if let Some(headers) = &entry.headers {
        for (k, v) in headers {
            req = req.set(k, v);
        }
    }

    match req.call() {
        Ok(resp) => {
            let status = resp.status();
            let content_type = resp
                .header("content-type")
                .unwrap_or("")
                .to_lowercase();

            let body = resp.into_string().unwrap_or_default();

            if let Some(endpoint_url) = extract_sse_endpoint(&body) {
                ConnectionTestResult {
                    success: true,
                    message: format!("SSE endpoint established (HTTP {status}). Message endpoint: {endpoint_url}"),
                    server_info: Some(endpoint_url),
                }
            } else if content_type.contains("text/event-stream") {
                ConnectionTestResult {
                    success: true,
                    message: format!("SSE connection established (HTTP {status})"),
                    server_info: None,
                }
            } else {
                try_post_initialize(agent, url, entry, status)
            }
        }
        Err(ureq::Error::Status(_status, _resp)) => {
            try_post_initialize_fallback(entry, url)
        }
        Err(ureq::Error::Transport(e)) => ConnectionTestResult {
            success: false,
            message: format!("SSE connection failed: {e}"),
            server_info: None,
        },
    }
}

fn try_post_initialize(
    agent: ureq::Agent,
    url: &str,
    entry: &McpServerEntry,
    get_status: u16,
) -> ConnectionTestResult {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "mcp-switch", "version": "0.4.3" }
        }
    });

    let mut req = agent.post(url).set("content-type", "application/json");
    if let Some(headers) = &entry.headers {
        for (k, v) in headers {
            req = req.set(k, v);
        }
    }

    match req.send_json(body) {
        Ok(resp) => {
            let post_status = resp.status();
            let text = resp.into_string().unwrap_or_default();
            if let Some(ref info) = try_parse_mcp_response(&text) {
                ConnectionTestResult {
                    success: true,
                    message: format!("Server responded to initialize (HTTP GET {get_status}, POST {post_status}): {info}"),
                    server_info: Some(info.clone()),
                }
            } else {
                ConnectionTestResult {
                    success: true,
                    message: format!("Server reachable (HTTP GET {get_status})"),
                    server_info: None,
                }
            }
        }
        Err(_) => ConnectionTestResult {
            success: true,
            message: format!("SSE endpoint reachable (HTTP {get_status}) — could not initialize via POST, but server is alive"),
            server_info: None,
        },
    }
}

fn try_post_initialize_fallback(entry: &McpServerEntry, url: &str) -> ConnectionTestResult {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "mcp-switch", "version": "0.4.3" }
        }
    });

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();

    let mut req = agent.post(url).set("content-type", "application/json");
    if let Some(headers) = &entry.headers {
        for (k, v) in headers {
            req = req.set(k, v);
        }
    }

    match req.send_json(body) {
        Ok(resp) => {
            let status = resp.status();
            let text = resp.into_string().unwrap_or_default();
            if let Some(ref info) = try_parse_mcp_response(&text) {
                ConnectionTestResult {
                    success: true,
                    message: format!("Server responded to initialize (HTTP POST {status}): {info}"),
                    server_info: Some(info.clone()),
                }
            } else {
                ConnectionTestResult {
                    success: true,
                    message: format!("Server reachable via POST (HTTP {status})"),
                    server_info: None,
                }
            }
        }
        Err(ureq::Error::Status(status, resp)) => {
            let body_text = resp.into_string().unwrap_or_default();
            ConnectionTestResult {
                success: false,
                message: format!("GET and POST both failed — HTTP {status}: {body_text}"),
                server_info: None,
            }
        }
        Err(ureq::Error::Transport(e)) => ConnectionTestResult {
            success: false,
            message: format!("GET and POST both failed — {e}"),
            server_info: None,
        },
    }
}

/// Extract SSE data from an SSE response body.
fn extract_sse_endpoint(body: &str) -> Option<String> {
    let mut found_endpoint = false;
    for line in body.lines() {
        if line.starts_with("event: endpoint") {
            found_endpoint = true;
            continue;
        }
        if found_endpoint && line.starts_with("data: ") {
            return Some(line["data: ".len()..].to_string());
        }
        if let Some(data) = line.strip_prefix("data: ") {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(endpoint) = val.get("endpoint").and_then(|v| v.as_str()) {
                    return Some(endpoint.to_string());
                }
            }
        }
    }
    None
}

/// Try to parse a JSON-RPC initialize response, returning server info on success.
fn try_parse_mcp_response(text: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(text).ok()?;

    if val.get("jsonrpc").and_then(|v| v.as_str()) != Some("2.0") {
        return None;
    }

    if let Some(result) = val.get("result") {
        if let Some(server_info) = result.get("serverInfo") {
            let name = server_info
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let version = server_info
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            return Some(format!("{name} v{version}"));
        }
        return Some("MCP server initialized".to_string());
    }

    if let Some(err) = val.get("error") {
        let msg = err
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        let code = err
            .get("code")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1);
        return Some(format!("MCP error ({code}): {msg}"));
    }

    None
}
