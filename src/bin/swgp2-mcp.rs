//! swgp2-mcp - MCP (Model Context Protocol) server for swgp2-tool.
//!
//! stdio JSON-RPC 2.0, newline-delimited. Tools are registered in
//! `swgp2_tool::mcp` — the agent gets a broad control surface over the game
//! and the save editor (save read/write, process/memory access, file
//! analysis, DLL injection, wiki lookups, ...).
//!
//! Run it as an MCP server from an MCP-capable agent host, e.g.:
//!   { "command": "swgp2-mcp" }

use std::io::{BufRead, BufWriter, Write};
use std::process::exit;

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<serde_json::Value>(line) {
            Ok(req) => swgp2_tool::mcp::dispatch(&req),
            Err(e) => Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": serde_json::Value::Null,
                "error": { "code": -32700, "message": format!("parse error: {e}") }
            })),
        };
        if let Some(resp) = response {
            match serde_json::to_string(&resp) {
                Ok(text) => {
                    let _ = writeln!(out, "{text}");
                    let _ = out.flush();
                }
                Err(_) => exit(1),
            }
        }
    }
}
