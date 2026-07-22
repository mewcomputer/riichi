use clap::{Parser, Subcommand};
use reqwest::{Client, RequestBuilder, StatusCode};
use serde_json::{Value, json};
use std::process::Command as ProcessCommand;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use uuid::Uuid;

mod profile;

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
    #[arg(long, env = "RIICHI_PROJECT_ID", global = true)]
    project_id: Option<Uuid>,
    #[arg(long, env = "RIICHI_SESSION_ID", global = true)]
    session_id: Option<Uuid>,
    #[arg(long, env = "RIICHI_AGENT_TOKEN", global = true)]
    token: Option<String>,
    #[arg(long, default_value = "default", global = true)]
    profile: String,
    #[arg(long, help = "read the agent token from stdin", global = true)]
    token_stdin: bool,
    #[arg(long, help = "emit machine-readable JSON", global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Login,
    Whoami,
    Organization {
        #[command(subcommand)]
        command: OrganizationCommand,
    },
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    Ready {
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    Claim {
        issue_id: String,
        #[arg(long)]
        idempotency_key: Option<String>,
        #[arg(long, default_value_t = 1800)]
        ttl_seconds: i64,
    },
    Context {
        issue_id: String,
        #[arg(long)]
        max_bytes: Option<usize>,
    },
    Resolve {
        display_key: String,
    },
    Renew {
        lease_id: Uuid,
        fencing_token: i64,
        #[arg(long, default_value_t = 1800)]
        ttl_seconds: i64,
    },
    Complete {
        lease_id: Uuid,
        fencing_token: i64,
        #[arg(long)]
        summary: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Release {
        lease_id: Uuid,
        fencing_token: i64,
        #[arg(long)]
        comment: Option<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Work {
        #[arg(long, default_value_t = 10)]
        limit: i64,
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

#[derive(Debug, Subcommand)]
enum OrganizationCommand {
    List,
    Use { organization_id: Uuid },
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    List,
    Use { project_id: Uuid },
}

#[derive(Debug, Subcommand)]
enum ProfileCommand {
    List,
    Set {
        name: String,
        #[arg(long)]
        project_id: Uuid,
        #[arg(long)]
        session_id: Uuid,
        #[arg(long, default_value = "http://127.0.0.1:3000")]
        base_url: String,
        #[arg(long)]
        token_stdin: bool,
    },
}

#[derive(Clone)]
struct AgentClient {
    http: Client,
    base_url: String,
    project_id: Uuid,
    session_id: Uuid,
    token: String,
}

#[derive(Clone)]
struct HumanClient {
    http: Client,
    base_url: String,
    token: String,
}

impl HumanClient {
    async fn get(&self, path: &str) -> Result<Value, String> {
        let response = self
            .http
            .get(format!("{}{}", self.base_url.trim_end_matches('/'), path))
            .header("cookie", format!("riichi_session={}", self.token))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status();
        let body = response
            .json::<Value>()
            .await
            .map_err(|error| error.to_string())?;
        if !status.is_success() {
            return Err(format!("Riichi returned {status}: {body}"));
        }
        Ok(body)
    }
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

async fn resolve_issue_id(client: &AgentClient, value: &str) -> Result<Uuid, String> {
    if let Ok(issue_id) = value.parse() {
        return Ok(issue_id);
    }
    let result = client
        .call("/api/v1/resolve", json!({"display_key": value}))
        .await?;
    result
        .get("issue_id")
        .and_then(Value::as_str)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| format!("issue '{value}' was not found in the selected project"))
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

async fn run_work(client: &AgentClient, limit: i64) -> Result<(), String> {
    let ready = client
        .call("/api/v1/ready", json!({"limit": limit}))
        .await?;
    let issues = ready
        .get("issues")
        .and_then(Value::as_array)
        .ok_or_else(|| "ready response did not contain issues".to_owned())?;
    if issues.is_empty() {
        println!("no eligible work");
        return Ok(());
    }
    println!("eligible work:");
    for (index, issue) in issues.iter().enumerate() {
        println!(
            "  {}. {:<12} {}",
            index + 1,
            issue
                .get("display_key")
                .and_then(Value::as_str)
                .unwrap_or("?"),
            issue
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("(untitled)")
        );
    }
    println!("choose an issue number, or press enter to cancel:");
    let mut selection = String::new();
    std::io::stdin()
        .read_line(&mut selection)
        .map_err(|error| error.to_string())?;
    let selection = selection.trim();
    if selection.is_empty() {
        return Ok(());
    }
    let index: usize = selection
        .parse()
        .map_err(|_| "please enter an issue number".to_owned())?;
    let issue = issues
        .get(
            index
                .checked_sub(1)
                .ok_or_else(|| "issue number must be positive".to_owned())?,
        )
        .ok_or_else(|| "that issue number is out of range".to_owned())?;
    let issue_id = issue
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| "ready issue did not contain an ID".to_owned())?;
    let claim = client.call("/api/v1/claim", json!({"issue_id": issue_id, "idempotency_key": format!("riichi-cli-work-{}", Uuid::new_v4()), "requested_ttl_seconds": 1800})).await?;
    println!(
        "claimed {}",
        claim
            .get("lease_id")
            .and_then(Value::as_str)
            .unwrap_or("(unknown lease)")
    );
    println!("report action: [c]omplete, [r]elease, or [s]top");
    let mut action = String::new();
    std::io::stdin()
        .read_line(&mut action)
        .map_err(|error| error.to_string())?;
    let lease_id = claim
        .get("lease_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "claim response did not contain a lease ID".to_owned())?;
    let fencing_token = claim
        .get("fencing_token")
        .and_then(Value::as_i64)
        .ok_or_else(|| "claim response did not contain a fencing token".to_owned())?;
    match action.trim().to_ascii_lowercase().as_str() {
        "c" | "complete" => {
            let mut summary = String::new();
            println!("resolution summary:");
            std::io::stdin()
                .read_line(&mut summary)
                .map_err(|error| error.to_string())?;
            client.call("/api/v1/report/batch", json!({"lease_id": lease_id, "fencing_token": fencing_token, "idempotency_key": format!("riichi-cli-work-report-{}", Uuid::new_v4()), "operations": [{"type": "complete", "resolution_summary": summary.trim()}]})).await?;
            println!("reported complete");
        }
        "r" | "release" => {
            client.call("/api/v1/report/batch", json!({"lease_id": lease_id, "fencing_token": fencing_token, "idempotency_key": format!("riichi-cli-work-report-{}", Uuid::new_v4()), "operations": [{"type": "release"}]})).await?;
            println!("released lease");
        }
        _ => println!("left lease active; renew or report it before expiry"),
    }
    Ok(())
}

fn resolve_profile(cli: &Cli) -> Result<AgentClient, String> {
    let stored = profile::get(&cli.profile).ok();
    let project_id = cli
        .project_id
        .or(stored.as_ref().map(|(value, _)| value.project_id))
        .ok_or_else(|| "missing project ID; pass --project-id or configure a profile".to_owned())?;
    let session_id = cli
        .session_id
        .or(stored.as_ref().map(|(value, _)| value.session_id))
        .ok_or_else(|| "missing session ID; pass --session-id or configure a profile".to_owned())?;
    let token = if cli.token_stdin {
        profile::read_token_from_stdin()?
    } else if let Some(token) = &cli.token {
        token.clone()
    } else {
        stored
            .as_ref()
            .map(|(_, token)| token.clone())
            .ok_or_else(|| {
                "missing agent token; pass --token, --token-stdin, or configure a profile"
                    .to_owned()
            })?
    };
    let base_url = if cli.base_url != "http://127.0.0.1:3000" {
        cli.base_url.clone()
    } else {
        stored
            .as_ref()
            .map(|(value, _)| value.base_url.clone())
            .unwrap_or_else(|| cli.base_url.clone())
    };
    Ok(AgentClient {
        http: Client::new(),
        base_url,
        project_id,
        session_id,
        token,
    })
}

fn print_result(value: &Value, json_output: bool) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(value).map_err(|error| error.to_string())?
        );
        return Ok(());
    }
    if let Some(issues) = value.get("issues").and_then(Value::as_array) {
        println!("{} eligible issue(s)", issues.len());
        for issue in issues {
            println!(
                "  {:<12} {}",
                issue
                    .get("display_key")
                    .and_then(Value::as_str)
                    .unwrap_or("?"),
                issue
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("(untitled)")
            );
        }
    } else if let Some(id) = value.get("lease_id").and_then(Value::as_str) {
        println!("claimed lease {id}");
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(value).map_err(|error| error.to_string())?
        );
    }
    Ok(())
}

async fn run_human_login(base_url: &str, profile_name: &str) -> Result<(), String> {
    let http = Client::new();
    let response = http
        .post(format!(
            "{}/api/v1/auth/cli-login",
            base_url.trim_end_matches('/')
        ))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let start = response
        .json::<Value>()
        .await
        .map_err(|error| error.to_string())?;
    let token = start
        .get("token")
        .and_then(Value::as_str)
        .ok_or_else(|| "login start response did not contain a token".to_owned())?;
    let login_url = start
        .get("login_url")
        .and_then(Value::as_str)
        .ok_or_else(|| "login start response did not contain a login URL".to_owned())?;
    let login_url = format!("{}{}", base_url.trim_end_matches('/'), login_url);
    println!("opening {login_url}");
    let opened = ProcessCommand::new("open")
        .arg(&login_url)
        .status()
        .is_ok_and(|status| status.success());
    if !opened {
        println!("open this URL in your browser to authenticate:\n{login_url}");
    }
    for _ in 0..150 {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let response = http
            .post(format!(
                "{}/api/v1/auth/cli-login/{token}/exchange",
                base_url.trim_end_matches('/')
            ))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        let body = response
            .json::<Value>()
            .await
            .map_err(|error| error.to_string())?;
        if body.get("status").and_then(Value::as_str) == Some("complete") {
            let session_token = body
                .get("session_token")
                .and_then(Value::as_str)
                .ok_or_else(|| "login exchange did not return a session token".to_owned())?;
            profile::save_human(
                profile_name,
                profile::HumanProfile {
                    base_url: base_url.to_owned(),
                    organization_id: None,
                    project_id: None,
                },
                session_token.to_owned(),
            )?;
            println!("logged in as profile '{profile_name}'");
            return Ok(());
        }
    }
    Err("login timed out after five minutes".to_owned())
}

fn human_client(profile_name: &str) -> Result<HumanClient, String> {
    let (profile, token) = profile::load_human(profile_name)?;
    Ok(HumanClient {
        http: Client::new(),
        base_url: profile.base_url,
        token,
    })
}

async fn run_human_command(cli: &Cli, command: &Command) -> Result<(), String> {
    let client = human_client(&cli.profile)?;
    let value = client.get("/api/v1/navigation").await?;
    match command {
        Command::Whoami => print_result(&client.get("/api/v1/auth/me").await?, cli.json),
        Command::Organization {
            command: OrganizationCommand::List,
        } => {
            for organization in value
                .get("organizations")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                println!(
                    "{}\t{}",
                    organization
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("?"),
                    organization
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("?")
                );
            }
            Ok(())
        }
        Command::Organization {
            command: OrganizationCommand::Use { organization_id },
        } => {
            profile::update_human_context(&cli.profile, Some(*organization_id), None)?;
            println!("selected organization {organization_id}");
            Ok(())
        }
        Command::Project {
            command: ProjectCommand::List,
        } => {
            for organization in value
                .get("organizations")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                for team in organization
                    .get("teams")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    for project in team
                        .get("projects")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        println!(
                            "{}\t{}\t{}",
                            project.get("id").and_then(Value::as_str).unwrap_or("?"),
                            team.get("key").and_then(Value::as_str).unwrap_or("?"),
                            project.get("name").and_then(Value::as_str).unwrap_or("?")
                        );
                    }
                }
            }
            Ok(())
        }
        Command::Project {
            command: ProjectCommand::Use { project_id },
        } => {
            profile::update_human_context(&cli.profile, None, Some(*project_id))?;
            println!("selected project {project_id}");
            Ok(())
        }
        _ => unreachable!("only human commands are routed here"),
    }
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let cli = Cli::parse();
    match &cli.command {
        Command::Login => return run_human_login(&cli.base_url, &cli.profile).await,
        Command::Whoami | Command::Organization { .. } | Command::Project { .. } => {
            return run_human_command(&cli, &cli.command).await;
        }
        _ => {}
    }
    if let Command::Profile { command } = &cli.command {
        match command {
            ProfileCommand::List => {
                for name in profile::list()? {
                    println!("{name}");
                }
                return Ok(());
            }
            ProfileCommand::Set {
                name,
                project_id,
                session_id,
                base_url,
                token_stdin,
            } => {
                let token = if *token_stdin {
                    profile::read_token_from_stdin()?
                } else {
                    return Err("profile set requires --token-stdin so tokens are not exposed in shell history".to_owned());
                };
                profile::set(
                    name.clone(),
                    profile::Profile {
                        base_url: base_url.clone(),
                        project_id: *project_id,
                        session_id: *session_id,
                    },
                    token,
                )?;
                println!("saved profile '{name}'");
                return Ok(());
            }
        }
    }
    let client = resolve_profile(&cli)?;
    let result = match cli.command {
        Command::Ready { limit } => client.call("/api/v1/ready", json!({"limit": limit})).await,
        Command::Claim { issue_id, idempotency_key, ttl_seconds } => client.call(
            "/api/v1/claim",
            json!({"issue_id": resolve_issue_id(&client, &issue_id).await?, "idempotency_key": idempotency_key.unwrap_or_else(|| format!("riichi-cli-{}", Uuid::new_v4())), "requested_ttl_seconds": ttl_seconds}),
        ).await,
        Command::Context { issue_id, max_bytes } => client.call(
            "/api/v1/context",
            json!({"issue_id": resolve_issue_id(&client, &issue_id).await?, "max_bytes": max_bytes}),
        ).await,
        Command::Resolve { display_key } => client.call("/api/v1/resolve", json!({"display_key": display_key})).await,
        Command::Renew { lease_id, fencing_token, ttl_seconds } => client.call("/api/v1/renew", json!({"lease_id": lease_id, "fencing_token": fencing_token, "requested_ttl_seconds": ttl_seconds})).await,
        Command::Complete { lease_id, fencing_token, summary, idempotency_key } => client.call("/api/v1/report/batch", json!({"lease_id": lease_id, "fencing_token": fencing_token, "idempotency_key": idempotency_key.unwrap_or_else(|| format!("riichi-cli-{}", Uuid::new_v4())), "operations": [{"type": "complete", "resolution_summary": summary}]})).await,
        Command::Release { lease_id, fencing_token, comment, idempotency_key } => {
            let mut operations = vec![json!({"type": "release"})];
            if let Some(comment) = comment { operations.insert(0, json!({"type": "comment", "body": comment})); }
            client.call("/api/v1/report/batch", json!({"lease_id": lease_id, "fencing_token": fencing_token, "idempotency_key": idempotency_key.unwrap_or_else(|| format!("riichi-cli-{}", Uuid::new_v4())), "operations": operations})).await
        }
        Command::Work { limit } => return run_work(&client, limit).await,
        Command::Report { lease_id, fencing_token, idempotency_key, operations } => {
            let operations: Value = serde_json::from_str(&operations).map_err(|error| error.to_string())?;
            client.call(
                "/api/v1/report/batch",
                json!({"lease_id": lease_id, "fencing_token": fencing_token, "idempotency_key": idempotency_key, "operations": operations}),
            ).await
        }
        Command::Mcp => return run_mcp(client).await,
        Command::Profile { .. } => unreachable!("profile commands are handled before client resolution"),
        Command::Login | Command::Whoami | Command::Organization { .. } | Command::Project { .. } => unreachable!("human commands are handled before agent client resolution"),
    }?;
    print_result(&result, cli.json)?;
    Ok(())
}
