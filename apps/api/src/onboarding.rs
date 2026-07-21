use super::*;

pub(super) async fn create_onboarding_sample(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<riichi_persistence::OnboardingSampleRecord>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    if let Some(sample) = state
        .application
        .database()
        .onboarding_sample(project_id)
        .await
        .map_err(ApiError::from)?
    {
        return Ok(Json(sample));
    }

    let application = &state.application;
    if !application
        .database()
        .claim_onboarding_sample(project_id)
        .await
        .map_err(ApiError::from)?
    {
        return Ok(Json(
            application
                .database()
                .onboarding_sample(project_id)
                .await
                .map_err(ApiError::from)?
                .ok_or(ApiError::NotFound)?,
        ));
    }
    let role_id = application
        .create_agent_role_with_id(
            project_id,
            "Onboarding agent",
            principal.account.id,
            vec![
                "comment".to_owned(),
                "complete".to_owned(),
                "release".to_owned(),
                "request_spec".to_owned(),
            ],
        )
        .await
        .map_err(ApiError::from)?;
    let agent_token = format!("{}{}", Uuid::now_v7().simple(), Uuid::now_v7().simple());
    let session_id = application
        .create_agent_session(project_id, role_id, Duration::hours(2), &agent_token)
        .await
        .map_err(ApiError::from)?;

    let create_sample_issue =
        |title: &str, agent_eligible: bool, spec_complete: bool, rank: i64| {
            application.create_issue(
                project_id,
                riichi_persistence::IssueCreate {
                    id: Uuid::now_v7(),
                    display_key: String::new(),
                    title: title.to_owned(),
                    body: "This issue is part of the guided Riichi workflow.".to_owned(),
                    status: "todo".to_owned(),
                    agent_eligible,
                    spec_complete,
                    rank,
                    labels: vec!["onboarding-sample".to_owned()],
                    assignee_account_id: None,
                    parent_issue_id: None,
                },
                principal.account.id,
            )
        };
    let triage_issue = create_sample_issue("Sample: triage a human issue", false, false, 10)
        .await
        .map_err(ApiError::from)?;
    let agent_issue = create_sample_issue("Sample: agent claim and report", true, true, 20)
        .await
        .map_err(ApiError::from)?;
    let recovery_issue = create_sample_issue("Sample: recover an agent lease", true, true, 30)
        .await
        .map_err(ApiError::from)?;

    let claim = application
        .claim(
            project_id,
            session_id,
            agent_issue.id,
            Duration::minutes(30),
            "onboarding-agent-claim",
        )
        .await
        .map_err(ApiError::from)?;
    application
        .report_batch(
            project_id,
            session_id,
            claim.lease_id,
            claim.fencing_token,
            riichi_persistence::ReportBatch {
                idempotency_key: "onboarding-agent-report".to_owned(),
                operations: vec![
                    riichi_persistence::ReportOperation::Comment {
                        body: "The onboarding agent inspected this issue.".to_owned(),
                    },
                    riichi_persistence::ReportOperation::Release,
                ],
            },
        )
        .await
        .map_err(ApiError::from)?;
    let _recovery_claim = application
        .claim(
            project_id,
            session_id,
            recovery_issue.id,
            Duration::minutes(30),
            "onboarding-recovery-claim",
        )
        .await
        .map_err(ApiError::from)?;
    let checklist = application
        .takeover_issue(
            project_id,
            recovery_issue.id,
            principal.account.id,
            "Review the guided recovery workflow",
        )
        .await
        .map_err(ApiError::from)?;
    let approval = application
        .create_approval_request(
            project_id,
            triage_issue.id,
            principal.account.id,
            triage_issue.version,
            riichi_persistence::ApprovalOperation::SetRank { rank: 5 },
            Duration::days(1),
        )
        .await
        .map_err(ApiError::from)?;
    let sample = riichi_persistence::OnboardingSampleRecord {
        project_id,
        role_id,
        session_id,
        triage_issue_id: triage_issue.id,
        agent_issue_id: agent_issue.id,
        recovery_issue_id: recovery_issue.id,
        approval_id: approval.id,
        recovery_checklist_id: checklist.id,
        created_at: chrono::Utc::now(),
    };
    application
        .database()
        .record_onboarding_sample(&sample)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(sample))
}
