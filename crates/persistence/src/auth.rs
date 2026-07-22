use chrono::{DateTime, Duration, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct OidcLoginState {
    pub issuer: String,
    pub nonce: String,
    pub pkce_verifier: String,
    pub return_to: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct HumanSession {
    pub id: Uuid,
    pub account_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct HumanAccount {
    pub id: Uuid,
    pub issuer: String,
    pub subject: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub last_completed_nux_version: Option<String>,
    pub last_completed_nux_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct HumanMembership {
    pub project_id: Uuid,
    pub project_name: String,
    pub role: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct TeamMembership {
    pub team_id: Uuid,
    pub team_name: String,
    pub team_key: String,
    pub role: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct AcceptedInvite {
    pub project_id: Uuid,
    pub role: String,
}

#[derive(Debug, Clone)]
pub struct ProjectInviteSeed {
    pub id: Uuid,
    pub project_id: Uuid,
    pub invited_by: Uuid,
    pub role: String,
    pub email_hint: Option<String>,
    pub token_hash: Vec<u8>,
    pub lifetime: Duration,
}
