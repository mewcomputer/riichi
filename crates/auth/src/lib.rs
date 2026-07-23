use std::{collections::HashMap, env, sync::Arc};

use chrono::{DateTime, Duration, Utc};
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet,
    EndpointNotSet, EndpointSet, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use openidconnect::{core::CoreAuthenticationFlow, core::CoreClient, core::CoreProviderMetadata};
use reqwest::redirect::Policy;
use riichi_persistence::{
    Database, Error as PersistenceError, HumanAccount, HumanMembership, HumanSession,
    ProjectInviteSeed, TeamMembership,
};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

type ConfiguredClient = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
    EndpointMaybeSet,
>;

const SESSION_COOKIE: &str = "riichi_session";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcConfig {
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub cookie_secure: bool,
    pub session_days: u32,
    pub login_state_minutes: u32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthConfigError {
    #[error("missing required configuration: {0}")]
    Missing(&'static str),

    #[error("invalid configuration for {key}: {value}")]
    Invalid { key: &'static str, value: String },
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("OIDC configuration error: {0}")]
    Config(#[from] AuthConfigError),

    #[error("database error: {0}")]
    Database(#[from] PersistenceError),

    #[error("OIDC provider request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("OIDC protocol error: {0}")]
    Oidc(String),

    #[error("login state was invalid or expired")]
    InvalidState,

    #[error("OIDC provider rejected the login: {0}")]
    ProviderRejected(String),

    #[error("OIDC provider did not return an ID token")]
    MissingIdToken,

    #[error("human session referenced a missing account")]
    SessionAccountMissing,

    #[error("the human principal does not have the required project role")]
    InsufficientRole,

    #[error("the project invite was invalid, expired, revoked, or already accepted")]
    InvalidInvite,

    #[error("project invites may only grant viewer, member, or admin")]
    InvalidInviteRole,

    #[error("project name cannot be empty")]
    ProjectNameRequired,

    #[error("project name is too long")]
    ProjectNameTooLong,
}

#[derive(Debug)]
pub struct LoginResult {
    pub session_token: String,
    pub return_to: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct HumanPrincipal {
    pub session_id: Uuid,
    pub account: HumanAccount,
    pub memberships: Vec<HumanMembership>,
    pub team_memberships: Vec<TeamMembership>,
    pub session_expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ProjectInvite {
    pub id: Uuid,
    pub project_id: Uuid,
    pub role: String,
    pub email_hint: Option<String>,
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct InviteAcceptance {
    pub project_id: Uuid,
    pub role: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HumanRole {
    Viewer,
    Member,
    Admin,
    Owner,
}

impl HumanRole {
    pub fn can_access(self, required: Self) -> bool {
        self >= required
    }
}

impl HumanPrincipal {
    pub fn role_for(&self, project_id: Uuid) -> Option<HumanRole> {
        self.memberships
            .iter()
            .find(|membership| membership.project_id == project_id)
            .and_then(|membership| match membership.role.as_str() {
                "viewer" => Some(HumanRole::Viewer),
                "member" => Some(HumanRole::Member),
                "admin" => Some(HumanRole::Admin),
                "owner" => Some(HumanRole::Owner),
                _ => None,
            })
    }

    pub fn can_access_project(&self, project_id: Uuid, required: HumanRole) -> bool {
        self.role_for(project_id)
            .map(|role| role.can_access(required))
            .unwrap_or(false)
    }

    pub fn role_for_team(&self, team_id: Uuid) -> Option<HumanRole> {
        self.team_memberships
            .iter()
            .find(|membership| membership.team_id == team_id)
            .and_then(|membership| match membership.role.as_str() {
                "viewer" => Some(HumanRole::Viewer),
                "member" => Some(HumanRole::Member),
                "admin" => Some(HumanRole::Admin),
                "owner" => Some(HumanRole::Owner),
                _ => None,
            })
    }

    pub fn can_access_team(&self, team_id: Uuid, required: HumanRole) -> bool {
        self.role_for_team(team_id)
            .map(|role| role.can_access(required))
            .unwrap_or(false)
    }
}

#[derive(Clone)]
pub struct AuthService {
    config: OidcConfig,
    client: Arc<ConfiguredClient>,
    http_client: reqwest::Client,
}

impl OidcConfig {
    pub fn from_env() -> Result<Self, AuthConfigError> {
        let values = env::vars().collect::<HashMap<_, _>>();
        Self::from_values(&values)
    }

    pub fn from_values(values: &HashMap<String, String>) -> Result<Self, AuthConfigError> {
        let issuer_url = required(values, "RIICHI_OIDC_ISSUER_URL")?;
        let client_id = required(values, "RIICHI_OIDC_CLIENT_ID")?;
        let client_secret = required(values, "RIICHI_OIDC_CLIENT_SECRET")?;
        let redirect_url = required(values, "RIICHI_OIDC_REDIRECT_URL")?;
        let cookie_secure = optional_bool(values, "RIICHI_AUTH_COOKIE_SECURE", false)?;
        let session_days = optional_positive_u32(values, "RIICHI_AUTH_SESSION_DAYS", 7)?;
        let login_state_minutes =
            optional_positive_u32(values, "RIICHI_AUTH_LOGIN_STATE_MINUTES", 10)?;

        for (key, value) in [
            ("RIICHI_OIDC_ISSUER_URL", &issuer_url),
            ("RIICHI_OIDC_REDIRECT_URL", &redirect_url),
        ] {
            if Url::parse(value).is_err() {
                return Err(AuthConfigError::Invalid {
                    key,
                    value: value.clone(),
                });
            }
        }
        if !cookie_secure
            && Url::parse(&redirect_url)
                .map(|url| url.scheme().eq_ignore_ascii_case("https"))
                .unwrap_or(false)
        {
            return Err(AuthConfigError::Invalid {
                key: "RIICHI_AUTH_COOKIE_SECURE",
                value: "false".to_owned(),
            });
        }

        Ok(Self {
            issuer_url,
            client_id,
            client_secret,
            redirect_url,
            cookie_secure,
            session_days,
            login_state_minutes,
        })
    }
}

impl AuthService {
    pub async fn discover(config: OidcConfig) -> Result<Self, AuthError> {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .redirect(Policy::none())
            .build()?;
        let issuer = IssuerUrl::new(config.issuer_url.clone())
            .map_err(|error| AuthError::Oidc(format!("invalid issuer URL: {error}")))?;
        let provider_metadata = CoreProviderMetadata::discover_async(issuer.clone(), &http_client)
            .await
            .map_err(|error| AuthError::Oidc(format!("provider discovery failed: {error}")))?;
        if provider_metadata.issuer().as_str() != issuer.as_str() {
            return Err(AuthError::Oidc(
                "provider discovery returned a different issuer".to_owned(),
            ));
        }
        let token_endpoint = provider_metadata
            .token_endpoint()
            .cloned()
            .ok_or_else(|| AuthError::Oidc("provider has no token endpoint".to_owned()))?;
        let redirect_url = RedirectUrl::new(config.redirect_url.clone())
            .map_err(|error| AuthError::Oidc(format!("invalid redirect URL: {error}")))?;
        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(config.client_id.clone()),
            Some(ClientSecret::new(config.client_secret.clone())),
        )
        .set_token_uri(token_endpoint)
        .set_redirect_uri(redirect_url);

        Ok(Self {
            config,
            client: Arc::new(client),
            http_client,
        })
    }

    pub fn cookie_name(&self) -> &'static str {
        SESSION_COOKIE
    }

    pub fn cookie_secure(&self) -> bool {
        self.config.cookie_secure
    }

    pub async fn begin_login(&self, database: &Database) -> Result<Url, AuthError> {
        self.begin_login_to(database, "/").await
    }

    pub async fn begin_login_to(
        &self,
        database: &Database,
        return_to: &str,
    ) -> Result<Url, AuthError> {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let (authorization_url, state, nonce) = self
            .client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("profile".to_owned()))
            .add_scope(Scope::new("email".to_owned()))
            .set_pkce_challenge(pkce_challenge)
            .url();

        database
            .create_oidc_login_state(
                &hash_secret(state.secret()),
                &self.config.issuer_url,
                nonce.secret(),
                pkce_verifier.secret(),
                return_to,
                Duration::minutes(i64::from(self.config.login_state_minutes)),
            )
            .await?;

        Ok(authorization_url)
    }

    pub async fn issue_session(
        &self,
        database: &Database,
        account_id: Uuid,
    ) -> Result<LoginResult, AuthError> {
        let session_token = CsrfToken::new_random().secret().to_owned();
        let expires_at = database
            .create_human_session(
                Uuid::new_v4(),
                account_id,
                &hash_secret(&session_token),
                Duration::days(i64::from(self.config.session_days)),
            )
            .await?;
        Ok(LoginResult {
            session_token,
            return_to: "/".to_owned(),
            expires_at,
        })
    }

    pub async fn finish_login(
        &self,
        database: &Database,
        code: &str,
        state: &str,
    ) -> Result<LoginResult, AuthError> {
        let login_state = database
            .consume_oidc_login_state(&hash_secret(state))
            .await?
            .filter(|stored| stored.issuer == self.config.issuer_url)
            .ok_or(AuthError::InvalidState)?;

        let token_response = self
            .client
            .exchange_code(AuthorizationCode::new(code.to_owned()))
            .set_pkce_verifier(PkceCodeVerifier::new(login_state.pkce_verifier))
            .request_async(&self.http_client)
            .await
            .map_err(|error| AuthError::Oidc(format!("token exchange failed: {error}")))?;

        let id_token = token_response.id_token().ok_or(AuthError::MissingIdToken)?;
        let verifier = self.client.id_token_verifier();
        let nonce = Nonce::new(login_state.nonce);
        let claims = id_token
            .claims(&verifier, &nonce)
            .map_err(|error| AuthError::Oidc(format!("ID token validation failed: {error}")))?;

        if let Some(expected_access_token_hash) = claims.access_token_hash() {
            let signing_alg = id_token
                .signing_alg()
                .map_err(|error| AuthError::Oidc(format!("invalid ID token algorithm: {error}")))?;
            let signing_key = id_token.signing_key(&verifier).map_err(|error| {
                AuthError::Oidc(format!("invalid ID token signing key: {error}"))
            })?;
            let actual_access_token_hash = AccessTokenHash::from_token(
                token_response.access_token(),
                signing_alg,
                signing_key,
            )
            .map_err(|error| {
                AuthError::Oidc(format!("access token hash validation failed: {error}"))
            })?;
            if actual_access_token_hash != *expected_access_token_hash {
                return Err(AuthError::Oidc(
                    "ID token access-token hash did not match".to_owned(),
                ));
            }
        }

        let email = claims.email().map(|value| value.as_str().to_owned());
        let display_name = claims
            .preferred_username()
            .map(|value| value.as_str().to_owned())
            .or_else(|| email.clone());
        let account_id = database
            .upsert_human_account(
                self.config.issuer_url.as_str(),
                claims.subject().as_str(),
                email.as_deref(),
                display_name.as_deref(),
            )
            .await?;
        let session_token = CsrfToken::new_random().secret().to_owned();
        let expires_at = database
            .create_human_session(
                Uuid::new_v4(),
                account_id,
                &hash_secret(&session_token),
                Duration::days(i64::from(self.config.session_days)),
            )
            .await?;

        Ok(LoginResult {
            session_token,
            return_to: login_state.return_to,
            expires_at,
        })
    }

    pub async fn cancel_login(&self, database: &Database, state: &str) -> Result<(), AuthError> {
        database
            .consume_oidc_login_state(&hash_secret(state))
            .await?
            .filter(|stored| stored.issuer == self.config.issuer_url)
            .ok_or(AuthError::InvalidState)?;
        Ok(())
    }

    pub async fn session(
        &self,
        database: &Database,
        session_token: &str,
    ) -> Result<Option<HumanSession>, AuthError> {
        Ok(database
            .active_human_session(&hash_secret(session_token))
            .await?)
    }

    pub async fn authenticate(
        &self,
        database: &Database,
        session_token: &str,
    ) -> Result<Option<HumanPrincipal>, AuthError> {
        let Some(session) = self.session(database, session_token).await? else {
            return Ok(None);
        };
        let account = database
            .human_account(session.account_id)
            .await?
            .ok_or(AuthError::SessionAccountMissing)?;
        let memberships = database.human_memberships(session.account_id).await?;
        let team_memberships = database.human_team_memberships(session.account_id).await?;
        Ok(Some(HumanPrincipal {
            session_id: session.id,
            account,
            memberships,
            team_memberships,
            session_expires_at: session.expires_at,
        }))
    }

    pub async fn create_project(
        &self,
        database: &Database,
        principal: &HumanPrincipal,
        name: &str,
    ) -> Result<Uuid, AuthError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AuthError::ProjectNameRequired);
        }
        if name.chars().count() > 200 {
            return Err(AuthError::ProjectNameTooLong);
        }
        let project_id = Uuid::new_v4();
        database
            .create_human_project(project_id, name, principal.account.id)
            .await?;
        Ok(project_id)
    }

    pub async fn create_invite(
        &self,
        database: &Database,
        principal: &HumanPrincipal,
        project_id: Uuid,
        role: &str,
        email_hint: Option<&str>,
        lifetime: Duration,
    ) -> Result<ProjectInvite, AuthError> {
        if !principal.can_access_project(project_id, HumanRole::Admin) {
            return Err(AuthError::InsufficientRole);
        }
        let role = role.trim().to_ascii_lowercase();
        if !matches!(role.as_str(), "viewer" | "member" | "admin") {
            return Err(AuthError::InvalidInviteRole);
        }
        let email_hint = email_hint
            .map(str::trim)
            .filter(|email| !email.is_empty())
            .map(str::to_owned);
        let token = CsrfToken::new_random().secret().to_owned();
        let invite_id = Uuid::new_v4();
        let expires_at = database
            .create_project_invite(ProjectInviteSeed {
                id: invite_id,
                project_id,
                invited_by: principal.account.id,
                role: role.clone(),
                email_hint: email_hint.clone(),
                token_hash: hash_secret(&token),
                lifetime: lifetime.max(Duration::minutes(5)).min(Duration::days(30)),
            })
            .await?;
        Ok(ProjectInvite {
            id: invite_id,
            project_id,
            role,
            email_hint,
            token,
            expires_at,
        })
    }

    pub async fn accept_invite(
        &self,
        database: &Database,
        principal: &HumanPrincipal,
        token: &str,
    ) -> Result<InviteAcceptance, AuthError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(AuthError::InvalidInvite);
        }
        let accepted = database
            .accept_project_invite(
                &hash_secret(token),
                principal.account.id,
                principal.account.email.as_deref(),
            )
            .await?
            .ok_or(AuthError::InvalidInvite)?;
        Ok(InviteAcceptance {
            project_id: accepted.project_id,
            role: accepted.role,
        })
    }

    pub async fn revoke_invite(
        &self,
        database: &Database,
        principal: &HumanPrincipal,
        project_id: Uuid,
        invite_id: Uuid,
    ) -> Result<(), AuthError> {
        if !principal.can_access_project(project_id, HumanRole::Admin) {
            return Err(AuthError::InsufficientRole);
        }
        database
            .revoke_project_invite(project_id, invite_id)
            .await?;
        Ok(())
    }

    pub async fn logout(&self, database: &Database, session_token: &str) -> Result<(), AuthError> {
        database
            .revoke_human_session(&hash_secret(session_token))
            .await?;
        Ok(())
    }

    pub async fn redrive_outbox(
        &self,
        database: &Database,
        principal: &HumanPrincipal,
        project_id: Uuid,
        message_id: Uuid,
    ) -> Result<bool, AuthError> {
        if !principal.can_access_project(project_id, HumanRole::Admin) {
            return Err(AuthError::InsufficientRole);
        }
        Ok(database
            .redrive_outbox(project_id, message_id, principal.account.id)
            .await?)
    }
}

fn hash_secret(secret: &str) -> Vec<u8> {
    Sha256::digest(secret.as_bytes()).to_vec()
}

fn required(
    values: &HashMap<String, String>,
    key: &'static str,
) -> Result<String, AuthConfigError> {
    values
        .get(key)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or(AuthConfigError::Missing(key))
}

fn optional_bool(
    values: &HashMap<String, String>,
    key: &'static str,
    default: bool,
) -> Result<bool, AuthConfigError> {
    values
        .get(key)
        .map(|value| {
            value.parse().map_err(|_| AuthConfigError::Invalid {
                key,
                value: value.clone(),
            })
        })
        .unwrap_or(Ok(default))
}

fn optional_positive_u32(
    values: &HashMap<String, String>,
    key: &'static str,
    default: u32,
) -> Result<u32, AuthConfigError> {
    let parsed = values
        .get(key)
        .map(|value| {
            value.parse().map_err(|_| AuthConfigError::Invalid {
                key,
                value: value.clone(),
            })
        })
        .transpose()?
        .unwrap_or(default);
    if parsed == 0 {
        return Err(AuthConfigError::Invalid {
            key,
            value: values
                .get(key)
                .cloned()
                .unwrap_or_else(|| default.to_string()),
        });
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> HashMap<String, String> {
        HashMap::from([
            (
                "RIICHI_OIDC_ISSUER_URL".to_owned(),
                "https://idp.example.test".to_owned(),
            ),
            ("RIICHI_OIDC_CLIENT_ID".to_owned(), "riichi".to_owned()),
            ("RIICHI_OIDC_CLIENT_SECRET".to_owned(), "secret".to_owned()),
            (
                "RIICHI_OIDC_REDIRECT_URL".to_owned(),
                "http://127.0.0.1:3000/auth/callback".to_owned(),
            ),
        ])
    }

    #[test]
    fn requires_all_provider_credentials() {
        let error = OidcConfig::from_values(&HashMap::new()).unwrap_err();

        assert_eq!(error, AuthConfigError::Missing("RIICHI_OIDC_ISSUER_URL"));
    }

    #[test]
    fn uses_development_safe_cookie_and_duration_defaults() {
        let config = OidcConfig::from_values(&values()).unwrap();

        assert!(!config.cookie_secure);
        assert_eq!(config.session_days, 7);
        assert_eq!(config.login_state_minutes, 10);
    }

    #[test]
    fn rejects_invalid_redirect_urls_and_zero_durations() {
        let mut invalid_redirect = values();
        invalid_redirect.insert(
            "RIICHI_OIDC_REDIRECT_URL".to_owned(),
            "/relative/callback".to_owned(),
        );
        assert!(matches!(
            OidcConfig::from_values(&invalid_redirect),
            Err(AuthConfigError::Invalid {
                key: "RIICHI_OIDC_REDIRECT_URL",
                ..
            })
        ));

        let mut zero_session = values();
        zero_session.insert("RIICHI_AUTH_SESSION_DAYS".to_owned(), "0".to_owned());
        assert!(matches!(
            OidcConfig::from_values(&zero_session),
            Err(AuthConfigError::Invalid {
                key: "RIICHI_AUTH_SESSION_DAYS",
                ..
            })
        ));

        let mut insecure_https_cookie = values();
        insecure_https_cookie.insert(
            "RIICHI_OIDC_REDIRECT_URL".to_owned(),
            "https://riichi.example.test/auth/callback".to_owned(),
        );
        assert!(matches!(
            OidcConfig::from_values(&insecure_https_cookie),
            Err(AuthConfigError::Invalid {
                key: "RIICHI_AUTH_COOKIE_SECURE",
                ..
            })
        ));
    }

    #[test]
    fn hashes_are_one_way_and_stable_for_storage_lookups() {
        assert_eq!(hash_secret("same"), hash_secret("same"));
        assert_ne!(hash_secret("same"), hash_secret("different"));
        assert_ne!(hash_secret("same"), b"same");
    }

    #[test]
    fn human_roles_are_hierarchical_and_unknown_roles_fail_closed() {
        assert!(HumanRole::Owner.can_access(HumanRole::Admin));
        assert!(HumanRole::Admin.can_access(HumanRole::Member));
        assert!(!HumanRole::Member.can_access(HumanRole::Admin));

        let principal = HumanPrincipal {
            session_id: Uuid::new_v4(),
            account: HumanAccount {
                id: Uuid::new_v4(),
                issuer: "https://idp.example.test".to_owned(),
                subject: "subject".to_owned(),
                email: None,
                display_name: None,
                last_completed_nux_version: None,
                last_completed_nux_at: None,
                theme_mode: "system".to_owned(),
                light_theme: "catppuccin-latte".to_owned(),
                dark_theme: "default".to_owned(),
            },
            memberships: vec![HumanMembership {
                project_id: Uuid::new_v4(),
                project_name: "test project".to_owned(),
                role: "unexpected".to_owned(),
            }],
            team_memberships: vec![],
            session_expires_at: Utc::now(),
        };
        assert!(
            !principal.can_access_project(principal.memberships[0].project_id, HumanRole::Viewer,)
        );
    }
}
