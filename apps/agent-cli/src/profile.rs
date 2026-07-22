use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub base_url: String,
    pub project_id: Uuid,
    pub session_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Credentials {
    token: String,
}

fn config_path() -> PathBuf {
    if let Some(path) = std::env::var_os("RIICHI_CONFIG_DIR") {
        return PathBuf::from(path).join("profiles.json");
    }
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(path).join("riichi/profiles.json");
    }
    if let Some(path) = std::env::var_os("HOME") {
        return PathBuf::from(path).join(".config/riichi/profiles.json");
    }
    PathBuf::from(".riichi/profiles.json")
}

fn credentials_path() -> PathBuf {
    let path = config_path();
    path.with_file_name("credentials.json")
}

fn human_credentials_path() -> PathBuf {
    config_path().with_file_name("human-credentials.json")
}

fn human_profiles_path() -> PathBuf {
    config_path().with_file_name("human-profiles.json")
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HumanProfile {
    pub base_url: String,
    pub organization_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
}

fn load_all() -> Result<std::collections::BTreeMap<String, Profile>, String> {
    let path = config_path();
    if !path.exists() {
        return Ok(Default::default());
    }
    let bytes =
        fs::read(&path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("could not parse {}: {error}", path.display()))
}

fn save_all(profiles: &std::collections::BTreeMap<String, Profile>) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(profiles).map_err(|error| error.to_string())?;
    fs::write(&path, bytes)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn load_credentials(name: &str) -> Result<String, String> {
    let path = credentials_path();
    let bytes =
        fs::read(&path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let credentials: std::collections::BTreeMap<String, Credentials> =
        serde_json::from_slice(&bytes)
            .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
    credentials
        .get(name)
        .map(|value| value.token.clone())
        .ok_or_else(|| format!("profile '{name}' has no stored token"))
}

fn save_credential(name: String, token: String) -> Result<(), String> {
    let path = credentials_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    let mut credentials: std::collections::BTreeMap<String, Credentials> = if path.exists() {
        serde_json::from_slice(&fs::read(&path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?
    } else {
        Default::default()
    };
    credentials.insert(name, Credentials { token });
    fs::write(
        &path,
        serde_json::to_vec_pretty(&credentials).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn get(name: &str) -> Result<(Profile, String), String> {
    let profile = load_all()?.remove(name).ok_or_else(|| {
        format!("profile '{name}' does not exist; run `riichi-agent profile set`")
    })?;
    Ok((profile, load_credentials(name)?))
}

pub fn list() -> Result<Vec<String>, String> {
    Ok(load_all()?.into_keys().collect())
}

pub fn set(name: String, profile: Profile, token: String) -> Result<(), String> {
    let mut profiles = load_all()?;
    profiles.insert(name.clone(), profile);
    save_all(&profiles)?;
    save_credential(name, token)
}

pub fn read_token_from_stdin() -> Result<String, String> {
    let mut token = String::new();
    io::stdin()
        .read_line(&mut token)
        .map_err(|error| error.to_string())?;
    let token = token.trim().to_owned();
    if token.is_empty() {
        return Err("token input was empty".to_owned());
    }
    Ok(token)
}

pub fn save_human(name: &str, profile: HumanProfile, token: String) -> Result<(), String> {
    let profiles_path = human_profiles_path();
    if let Some(parent) = profiles_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut profiles: std::collections::BTreeMap<String, HumanProfile> = if profiles_path.exists() {
        serde_json::from_slice(&fs::read(&profiles_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?
    } else {
        Default::default()
    };
    profiles.insert(name.to_owned(), profile);
    fs::write(
        &profiles_path,
        serde_json::to_vec_pretty(&profiles).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let credentials_path = human_credentials_path();
    let mut credentials: std::collections::BTreeMap<String, String> = if credentials_path.exists() {
        serde_json::from_slice(&fs::read(&credentials_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?
    } else {
        Default::default()
    };
    credentials.insert(name.to_owned(), token);
    fs::write(
        &credentials_path,
        serde_json::to_vec_pretty(&credentials).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&profiles_path, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
        fs::set_permissions(&credentials_path, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn load_human(name: &str) -> Result<(HumanProfile, String), String> {
    let profiles_path = human_profiles_path();
    let profiles: std::collections::BTreeMap<String, HumanProfile> =
        serde_json::from_slice(&fs::read(&profiles_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let credentials_path = human_credentials_path();
    let credentials: std::collections::BTreeMap<String, String> =
        serde_json::from_slice(&fs::read(&credentials_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    Ok((
        profiles
            .get(name)
            .cloned()
            .ok_or_else(|| format!("human profile '{name}' does not exist; run `riichi login`"))?,
        credentials.get(name).cloned().ok_or_else(|| {
            format!("human profile '{name}' is not logged in; run `riichi login`")
        })?,
    ))
}

pub fn update_human_context(
    name: &str,
    organization_id: Option<Uuid>,
    project_id: Option<Uuid>,
) -> Result<(), String> {
    let profiles_path = human_profiles_path();
    let mut profiles: std::collections::BTreeMap<String, HumanProfile> =
        serde_json::from_slice(&fs::read(&profiles_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let profile = profiles
        .get_mut(name)
        .ok_or_else(|| format!("human profile '{name}' does not exist"))?;
    if organization_id.is_some() {
        profile.organization_id = organization_id;
    }
    if project_id.is_some() {
        profile.project_id = project_id;
    }
    fs::write(
        &profiles_path,
        serde_json::to_vec_pretty(&profiles).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_metadata_serializes_without_a_token() {
        let profile = Profile {
            base_url: "http://localhost:3000".to_owned(),
            project_id: Uuid::from_u128(1),
            session_id: Uuid::from_u128(2),
        };
        let encoded = serde_json::to_string(&profile).expect("profile serializes");
        assert!(!encoded.contains("token"));
        let decoded: Profile = serde_json::from_str(&encoded).expect("profile deserializes");
        assert_eq!(decoded.project_id, profile.project_id);
        assert_eq!(decoded.session_id, profile.session_id);
    }
}
