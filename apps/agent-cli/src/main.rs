use clap::{Parser, Subcommand};
use reqwest::{Client, RequestBuilder, StatusCode};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(
    name = "riichi-agent",
    about = "thin CLI and MCP adapters for Riichi agent intentions"
)]
struct Cli {
    #[arg(
        long,
        env = "RIICHI_AGENT_URL",
        default_value = "http://127.0.0.1:3000"
    )]
    base_url: String,
    #[arg(long, env = "RIICHI_PROJECT_ID")]
    project_id: Uuid,
    #[arg(long, env = "RIICHI_SESSION_ID")]
    session_id: Uuid,
    #[arg(long, env = "RIICHI_AGENT_TOKEN")]
    token: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Ready {
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    Claim {
        issue_id: Uuid,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long, default_value_t = 1800)]
        ttl_seconds: i64,
    },
    Context {
        issue_id: Uuid,
        #[arg(long)]
        max_bytes: Option<usize>,
    },
    Report {
        lease_id: Uuid,
        fencing_token: i64,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long, help = "JSON array of report operations")]
        operations: String,
    },
    Mcp,
}

#[derive(Clone)]
struct AgentClient {
    http: Client,
    base_url: String,
    project_id: Uuid,
    session_id: Uuid,
    token: String,
}

impl AgentClient {
    fn request(&self, path: &str) -> RequestBuilder {
        self.http
            .post(format!("{}{}", self.base_url.trim_end_matches('/'), path))
            .header("x-riichi-project-id", self.project_id.to_string())
            .header("x-riichi-session-id", self.session_id.to_string())
            .bearer_auth(&self.token)
            .header("content-type", "application/json")
    }

    async fn call(&self, path: &str, body: Value) -> Result<Value, String> {
        let response = self
            .request(path)
            .json(&body)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status();
        let body = response
            .json::<Value>()
            .await
            .map_err(|error| error.to_string())?;
        if status != StatusCode::OK {
            return Err(format!("Riichi returned {status}: {body}"));
        }
        Ok(body)
    }
}

async fn call_tool(client: &AgentClient, name: &str, arguments: Value) -> Result<Value, String> {
    match name {
        "ready" => client.call("/api/v1/ready", arguments).await,
        "claim" => client.call("/api/v1/claim", arguments).await,
        "context" => client.call("/api/v1/context", arguments).await,
        "report" => client.call("/api/v1/report/batch", arguments).await,
        _ => Err(format!("unknown Riichi tool: {name}")),
    }
}

async fn run_mcp(client: AgentClient) -> Result<(), String> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = lines.next_line().await.map_err(|error| error.to_string())? {
        let request: Value = serde_json::from_str(&line).map_err(|error| error.to_string())?;
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let response = match request.get("method").and_then(Value::as_str) {
            Some("initialize") => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {"protocolVersion": "2024-11-05", "serverInfo": {"name": "riichi-agent", "version": "0.1.0"}}
            }),
            Some("notifications/initialized") => continue,
            Some("tools/list") => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {"tools": [
                    {"name": "ready", "description": "List authoritative eligible work", "inputSchema": {"type": "object"}},
                    {"name": "claim", "description": "Claim one issue with a lease", "inputSchema": {"type": "object"}},
                    {"name": "context", "description": "Fetch bounded issue context", "inputSchema": {"type": "object"}},
                    {"name": "report", "description": "Submit an idempotent report batch", "inputSchema": {"type": "object"}}
                ]}
            }),
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
        println!("{}", response);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let cli = Cli::parse();
    let client = AgentClient {
        http: Client::new(),
        base_url: cli.base_url,
        project_id: cli.project_id,
        session_id: cli.session_id,
        token: cli.token,
    };
    let result = match cli.command {
        Command::Ready { limit } => client.call("/api/v1/ready", json!({"limit": limit})).await,
        Command::Claim { issue_id, idempotency_key, ttl_seconds } => client.call(
            "/api/v1/claim",
            json!({"issue_id": issue_id, "idempotency_key": idempotency_key, "requested_ttl_seconds": ttl_seconds}),
        ).await,
        Command::Context { issue_id, max_bytes } => client.call(
            "/api/v1/context",
            json!({"issue_id": issue_id, "max_bytes": max_bytes}),
        ).await,
        Command::Report { lease_id, fencing_token, idempotency_key, operations } => {
            let operations: Value = serde_json::from_str(&operations).map_err(|error| error.to_string())?;
            client.call(
                "/api/v1/report/batch",
                json!({"lease_id": lease_id, "fencing_token": fencing_token, "idempotency_key": idempotency_key, "operations": operations}),
            ).await
        }
        Command::Mcp => return run_mcp(client).await,
    }?;
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(|error| error.to_string())?
    );
    Ok(())
}
