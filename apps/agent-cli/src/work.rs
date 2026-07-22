use super::{AgentClient, Value, json};
use uuid::Uuid;

pub(crate) async fn run(client: &AgentClient, limit: i64) -> Result<(), String> {
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
