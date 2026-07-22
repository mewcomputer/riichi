use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub base_url: String,
    pub project_id: Uuid,
    pub session_id: Uuid,
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

fn human_credential_entry(name: &str) -> Result<Entry, String> {
    let account = format!("{}:{name}", config_path().display());
    Entry::new("riichi-human", &account)
        .map_err(|error| format!("could not access OS credential store: {error}"))
}

fn agent_credential_entry(name: &str) -> Result<Entry, String> {
    let account = format!("{}:{name}", config_path().display());
    Entry::new("riichi-agent", &account)
        .map_err(|error| format!("could not access OS credential store: {error}"))
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
    atomic_write(&path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    fs::write(&temp, bytes)
        .map_err(|error| format!("could not write {}: {error}", temp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("could not replace {}: {error}", path.display()))?;
    }
    fs::rename(&temp, path)
        .map_err(|error| format!("could not replace {}: {error}", path.display()))
}

fn save_credential(name: String, token: String) -> Result<(), String> {
    agent_credential_entry(&name)?
        .set_password(&token)
        .map_err(|error| format!("could not save agent credential in OS credential store: {error}"))
}

pub fn get(name: &str) -> Result<(Profile, String), String> {
    let profile = load_all()?.remove(name).ok_or_else(|| {
        format!("profile '{name}' does not exist; run `riichi-agent profile set`")
    })?;
    let token = agent_credential_entry(name)?
        .get_password()
        .map_err(|error| {
            format!("could not read agent credential from OS credential store: {error}")
        })?;
    Ok((profile, token))
}

pub fn list() -> Result<Vec<String>, String> {
    Ok(load_all()?.into_keys().collect())
}

pub fn set(name: String, profile: Profile, token: String) -> Result<(), String> {
    let mut profiles = load_all()?;
    profiles.insert(name.clone(), profile);
    save_credential(name, token)?;
    save_all(&profiles)
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
    human_credential_entry(name)?
        .set_password(&token)
        .map_err(|error| {
            format!("could not save human credential in OS credential store: {error}")
        })?;
    atomic_write(
        &profiles_path,
        &serde_json::to_vec_pretty(&profiles).map_err(|error| error.to_string())?,
    )
}

pub fn load_human(name: &str) -> Result<(HumanProfile, String), String> {
    let profiles_path = human_profiles_path();
    let profiles: std::collections::BTreeMap<String, HumanProfile> =
        serde_json::from_slice(&fs::read(&profiles_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let token = human_credential_entry(name)?
        .get_password()
        .map_err(|error| {
            format!("could not read human credential from OS credential store: {error}")
        })?;
    Ok((
        profiles
            .get(name)
            .cloned()
            .ok_or_else(|| format!("human profile '{name}' does not exist; run `riichi login`"))?,
        token,
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
        profile.project_id = None;
    }
    if project_id.is_some() {
        profile.project_id = project_id;
    }
    atomic_write(
        &profiles_path,
        &serde_json::to_vec_pretty(&profiles).map_err(|error| error.to_string())?,
    )
}

pub fn clear_human(name: &str) -> Result<(), String> {
    match human_credential_entry(name)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "could not clear human credential from OS credential store: {error}"
        )),
    }?;
    let profiles_path = human_profiles_path();
    if profiles_path.exists() {
        let mut profiles: std::collections::BTreeMap<String, HumanProfile> =
            serde_json::from_slice(&fs::read(&profiles_path).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        profiles.remove(name);
        atomic_write(
            &profiles_path,
            &serde_json::to_vec_pretty(&profiles).map_err(|error| error.to_string())?,
        )?;
    }
    Ok(())
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
