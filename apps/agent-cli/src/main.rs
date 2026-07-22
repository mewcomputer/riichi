use clap::{Parser, Subcommand};
use reqwest::{Client, RequestBuilder, StatusCode};
use serde_json::{Value, json};
use std::process::Command as ProcessCommand;
use std::time::{Duration, Instant};
use uuid::Uuid;

mod mcp;
mod profile;
mod work;

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
    Logout,
    Whoami,
    Organization {
        #[command(subcommand)]
        command: OrganizationCommand,
    },
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    Session {
        #[command(subcommand)]
        command: SessionCommand,
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
    Use { organization: String },
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    List,
    Use { project: String },
}

#[derive(Debug, Subcommand)]
enum SessionCommand {
    Use {
        session_id: Uuid,
        #[arg(long)]
        project_id: Option<Uuid>,
        #[arg(long)]
        token_stdin: bool,
    },
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
    profile: profile::HumanProfile,
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|error| format!("could not configure HTTP client: {error}"))
}

fn select_navigation_id<'a, I>(values: I, selector: &str, kind: &str) -> Result<Uuid, String>
where
    I: IntoIterator<Item = &'a Value>,
{
    let matches: Vec<&str> = values
        .into_iter()
        .filter(|value| {
            value.get("id").and_then(Value::as_str) == Some(selector)
                || value.get("name").and_then(Value::as_str) == Some(selector)
        })
        .filter_map(|value| value.get("id").and_then(Value::as_str))
        .collect();
    match matches.as_slice() {
        [id] => id
            .parse()
            .map_err(|_| format!("navigation returned an invalid {kind} ID")),
        [] => Err(format!(
            "{kind} '{selector}' is not accessible to this login"
        )),
        _ => Err(format!(
            "{kind} selector '{selector}' is ambiguous; use its UUID"
        )),
    }
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

    async fn post_empty(&self, path: &str) -> Result<(), String> {
        let response = self
            .http
            .post(format!("{}{}", self.base_url.trim_end_matches('/'), path))
            .header("cookie", format!("riichi_session={}", self.token))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Err(format!("Riichi returned {}", response.status()));
        }
        Ok(())
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

fn resolve_profile(cli: &Cli) -> Result<AgentClient, String> {
    let stored = match profile::get(&cli.profile) {
        Ok(value) => Some(value),
        Err(error) if error.contains("does not exist") => None,
        Err(error) => return Err(error),
    };
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
        http: http_client()?,
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
    let http = http_client()?;
    let response = http
        .post(format!(
            "{}/api/v1/auth/cli-login",
            base_url.trim_end_matches('/')
        ))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "Riichi returned {} while starting login",
            response.status()
        ));
    }
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
    let deadline = Instant::now() + Duration::from_secs(300);
    while Instant::now() < deadline {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let response = match http
            .post(format!(
                "{}/api/v1/auth/cli-login/{token}/exchange",
                base_url.trim_end_matches('/')
            ))
            .send()
            .await
        {
            Ok(response) => response,
            Err(_) => continue,
        };
        if !response.status().is_success() && response.status() != StatusCode::ACCEPTED {
            continue;
        }
        let body = match response.json::<Value>().await {
            Ok(body) => body,
            Err(_) => continue,
        };
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
        http: http_client()?,
        base_url: profile.base_url.clone(),
        token,
        profile,
    })
}

async fn run_human_command(cli: &Cli, command: &Command) -> Result<(), String> {
    let client = human_client(&cli.profile)?;
    match command {
        Command::Whoami => print_result(&client.get("/api/v1/auth/me").await?, cli.json),
        Command::Organization {
            command: OrganizationCommand::List,
        } => {
            let value = client.get("/api/v1/navigation").await?;
            let organizations: Vec<&Value> = value
                .get("organizations")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .collect();
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&organizations)
                        .map_err(|error| error.to_string())?
                );
            } else {
                for organization in organizations {
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
            }
            Ok(())
        }
        Command::Organization {
            command: OrganizationCommand::Use { organization },
        } => {
            let value = client.get("/api/v1/navigation").await?;
            let organization_id = select_navigation_id(
                value
                    .get("organizations")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten(),
                organization,
                "organization",
            )?;
            profile::update_human_context(&cli.profile, Some(organization_id), None)?;
            if cli.json {
                println!("{}", json!({"organization_id": organization_id}));
            } else {
                println!("selected organization {organization_id}");
            }
            Ok(())
        }
        Command::Project {
            command: ProjectCommand::List,
        } => {
            let value = client.get("/api/v1/navigation").await?;
            let organizations = value
                .get("organizations")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|organization| {
                    client.profile.organization_id.is_none_or(|id| {
                        organization.get("id").and_then(Value::as_str) == Some(&id.to_string())
                    })
                });
            let mut projects: Vec<(String, String, String)> = Vec::new();
            for organization in organizations {
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
                        projects.push((
                            project
                                .get("id")
                                .and_then(Value::as_str)
                                .unwrap_or("?")
                                .to_owned(),
                            team.get("key")
                                .and_then(Value::as_str)
                                .unwrap_or("?")
                                .to_owned(),
                            project
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("?")
                                .to_owned(),
                        ));
                    }
                }
            }
            if let Some(selected_project) = client.profile.project_id {
                let selected_project = selected_project.to_string();
                projects.sort_by_key(|(id, _, _)| if id == &selected_project { 0 } else { 1 });
            }
            if cli.json {
                let rows: Vec<Value> = projects.iter().map(|(id, team_key, name)| {
                    json!({"id": id, "team_key": team_key, "name": name})
                }).collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&rows).map_err(|error| error.to_string())?
                );
            } else {
                for (id, team_key, name) in projects {
                    println!("{id}\t{team_key}\t{name}");
                }
            }
            Ok(())
        }
        Command::Project {
            command: ProjectCommand::Use { project },
        } => {
            let value = client.get("/api/v1/navigation").await?;
            let project_id = select_navigation_id(
                value
                    .get("organizations")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|organization| {
                        client.profile.organization_id.is_none_or(|id| {
                            organization.get("id").and_then(Value::as_str) == Some(&id.to_string())
                        })
                    })
                    .flat_map(|organization| {
                        organization
                            .get("teams")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                    })
                    .flat_map(|team| {
                        team.get("projects")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                    }),
                project,
                "project",
            )?;
            profile::update_human_context(&cli.profile, None, Some(project_id))?;
            if cli.json {
                println!("{}", json!({"project_id": project_id}));
            } else {
                println!("selected project {project_id}");
            }
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
        Command::Logout => {
            let client = human_client(&cli.profile)?;
            client.post_empty("/auth/logout").await?;
            profile::clear_human(&cli.profile)?;
            println!("logged out of profile '{}'", cli.profile);
            return Ok(());
        }
        Command::Session {
            command:
                SessionCommand::Use {
                    session_id,
                    project_id,
                    token_stdin,
                },
        } => {
            let (stored, existing_token) = profile::get(&cli.profile)?;
            let token = if *token_stdin {
                profile::read_token_from_stdin()?
            } else {
                existing_token
            };
            profile::set(
                cli.profile.clone(),
                profile::Profile {
                    base_url: stored.base_url,
                    project_id: project_id.unwrap_or(stored.project_id),
                    session_id: *session_id,
                },
                token,
            )?;
            println!(
                "selected session {session_id} for profile '{}'",
                cli.profile
            );
            return Ok(());
        }
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
        Command::Work { limit } => return work::run(&client, limit).await,
        Command::Report { lease_id, fencing_token, idempotency_key, operations } => {
            let operations: Value = serde_json::from_str(&operations).map_err(|error| error.to_string())?;
            client.call(
                "/api/v1/report/batch",
                json!({"lease_id": lease_id, "fencing_token": fencing_token, "idempotency_key": idempotency_key, "operations": operations}),
            ).await
        }
        Command::Mcp => return mcp::run(client).await,
        Command::Profile { .. } => unreachable!("profile commands are handled before client resolution"),
        Command::Login | Command::Logout | Command::Whoami | Command::Organization { .. } | Command::Project { .. } | Command::Session { .. } => unreachable!("context commands are handled before agent client resolution"),
    }?;
    print_result(&result, cli.json)?;
    Ok(())
}
