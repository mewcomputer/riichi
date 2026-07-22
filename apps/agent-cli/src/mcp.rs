use super::{AgentClient, Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub(crate) async fn call_tool(
    client: &AgentClient,
    name: &str,
    arguments: Value,
) -> Result<Value, String> {
    match name {
        "ready" => client.call("/api/v1/ready", arguments).await,
        "claim" => client.call("/api/v1/claim", arguments).await,
        "context" => client.call("/api/v1/context", arguments).await,
        "report" => client.call("/api/v1/report/batch", arguments).await,
        _ => Err(format!("unknown Riichi tool: {name}")),
    }
}

pub(crate) async fn run(client: AgentClient) -> Result<(), String> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await.map_err(|error| error.to_string())? {
        let request: Value = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(error) => {
                let response = json!({"jsonrpc": "2.0", "id": Value::Null, "error": {"code": -32700, "message": error.to_string()}});
                stdout
                    .write_all(format!("{}\n", response).as_bytes())
                    .await
                    .map_err(|error| error.to_string())?;
                stdout.flush().await.map_err(|error| error.to_string())?;
                continue;
            }
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let response = match request.get("method").and_then(Value::as_str) {
            Some("initialize") => {
                json!({"jsonrpc": "2.0", "id": id, "result": {"protocolVersion": "2024-11-05", "serverInfo": {"name": "riichi-agent", "version": "0.1.0"}}})
            }
            Some("notifications/initialized") => continue,
            Some("tools/list") => json!({"jsonrpc": "2.0", "id": id, "result": {"tools": [
                {"name": "ready", "description": "List authoritative eligible work", "inputSchema": {"type": "object"}},
                {"name": "claim", "description": "Claim one issue with a lease", "inputSchema": {"type": "object"}},
                {"name": "context", "description": "Fetch bounded issue context", "inputSchema": {"type": "object"}},
                {"name": "report", "description": "Submit an idempotent report batch", "inputSchema": {"type": "object"}}
            ]}}),
            Some("tools/call") => {
                let params = request.get("params").cloned().unwrap_or_default();
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                match call_tool(&client, name, arguments).await {
                    Ok(result) => {
                        json!({"jsonrpc": "2.0", "id": id, "result": {"content": [{"type": "text", "text": result.to_string()}]}})
                    }
                    Err(error) => {
                        json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32000, "message": error}})
                    }
                }
            }
            Some(method) => {
                json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": format!("unknown method: {method}")}})
            }
            None => {
                json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32600, "message": "invalid JSON-RPC request"}})
            }
        };
        stdout
            .write_all(format!("{}\n", response).as_bytes())
            .await
            .map_err(|error| error.to_string())?;
        stdout.flush().await.map_err(|error| error.to_string())?;
    }
    Ok(())
}
