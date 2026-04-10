use std::fs;
use std::path::PathBuf;

use ini::Ini;
use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct AwsProfile {
    pub name: String,
    pub region: String,
    pub key: String,
    pub secret: String,
    pub auth_type: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct AwsSsoProfileCandidate {
    pub name: String,
    pub region: String,
}

pub fn get_aws_dir() -> PathBuf {
    dirs::home_dir().unwrap().join(".aws")
}

pub fn list_profiles() -> Result<Vec<AwsProfile>, String> {
    let cred_path = get_aws_dir().join("credentials");
    let config_path = get_aws_dir().join("config");
    if !cred_path.exists() {
        return Ok(Vec::new());
    }

    let credentials_ini = Ini::load_from_file(&cred_path).map_err(|e| e.to_string())?;
    let config_ini = if config_path.exists() {
        Ini::load_from_file(&config_path).unwrap_or_else(|_| Ini::new())
    } else {
        Ini::new()
    };

    let mut profiles: Vec<AwsProfile> = Vec::new();

    for (sec, prop) in &credentials_ini {
        if let Some(name) = sec {
            let section_name = name.to_string();
            let config_section = if section_name == "default" {
                "default".to_string()
            } else {
                format!("profile {}", section_name)
            };
            let config_prop = config_ini.section(Some(config_section));
            let region = config_prop
                .and_then(|s| s.get("region"))
                .unwrap_or("us-east-1")
                .to_string();
            let key = prop.get("aws_access_key_id").unwrap_or("").to_string();
            let secret = prop.get("aws_secret_access_key").unwrap_or("").to_string();
            let is_sso = config_prop.map(is_sso_config_section).unwrap_or(false);
            let auth_type = if is_sso {
                "sso"
            } else if !key.trim().is_empty() && !secret.trim().is_empty() {
                "access_key"
            } else {
                "profile"
            };

            profiles.push(AwsProfile {
                name: section_name,
                region,
                key,
                secret,
                auth_type: auth_type.to_string(),
            });
        }
    }

    Ok(profiles)
}

fn normalize_config_profile_name(section: &str) -> Option<String> {
    let trimmed = section.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "default" {
        return Some("default".to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("profile ") {
        let name = rest.trim();
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    } else {
        Some(trimmed.to_string())
    }
}

fn is_sso_config_section(props: &ini::Properties) -> bool {
    props.get("sso_session").is_some()
        || props.get("sso_start_url").is_some()
        || props.get("sso_account_id").is_some()
        || props.get("sso_role_name").is_some()
}

pub fn list_sso_profile_candidates() -> Result<Vec<AwsSsoProfileCandidate>, String> {
    let config_path = get_aws_dir().join("config");
    if !config_path.exists() {
        return Ok(Vec::new());
    }

    let config_ini = Ini::load_from_file(&config_path).map_err(|e| e.to_string())?;
    let mut candidates: Vec<AwsSsoProfileCandidate> = Vec::new();

    for (sec, prop) in &config_ini {
        let Some(raw_name) = sec else {
            continue;
        };
        if raw_name.starts_with("sso-session ") {
            continue;
        }
        let Some(profile_name) = normalize_config_profile_name(raw_name) else {
            continue;
        };
        if !is_sso_config_section(prop) {
            continue;
        }
        let region = prop.get("region").unwrap_or("us-east-1").to_string();
        candidates.push(AwsSsoProfileCandidate {
            name: profile_name,
            region,
        });
    }

    Ok(candidates)
}

pub fn save_profile(
    name: &str,
    key: &str,
    secret: &str,
    region: Option<String>,
) -> Result<(), String> {
    let aws_dir = get_aws_dir();
    if !aws_dir.exists() {
        fs::create_dir_all(&aws_dir).map_err(|e| e.to_string())?;
    }

    // 1. Update Credentials
    let cred_path = aws_dir.join("credentials");
    let mut cred_ini = if cred_path.exists() {
        Ini::load_from_file(&cred_path).map_err(|e| e.to_string())?
    } else {
        Ini::new()
    };

    cred_ini
        .with_section(Some(name))
        .set("aws_access_key_id", key)
        .set("aws_secret_access_key", secret);

    cred_ini
        .write_to_file(&cred_path)
        .map_err(|e| e.to_string())?;

    // 2. Update Config (Region)
    if let Some(reg) = region {
        let config_path = aws_dir.join("config");
        let mut config_ini = if config_path.exists() {
            Ini::load_from_file(&config_path).unwrap_or_else(|_| Ini::new())
        } else {
            Ini::new()
        };

        let profile_section = if name == "default" {
            "default".to_string()
        } else {
            format!("profile {}", name)
        };

        config_ini
            .with_section(Some(profile_section))
            .set("region", reg);

        config_ini
            .write_to_file(&config_path)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn save_profile_reference(name: &str, region: Option<String>) -> Result<(), String> {
    let aws_dir = get_aws_dir();
    if !aws_dir.exists() {
        fs::create_dir_all(&aws_dir).map_err(|e| e.to_string())?;
    }

    let cred_path = aws_dir.join("credentials");
    let mut cred_ini = if cred_path.exists() {
        Ini::load_from_file(&cred_path).map_err(|e| e.to_string())?
    } else {
        Ini::new()
    };
    cred_ini.with_section(Some(name));
    cred_ini
        .write_to_file(&cred_path)
        .map_err(|e| e.to_string())?;

    if let Some(reg) = region {
        let config_path = aws_dir.join("config");
        let mut config_ini = if config_path.exists() {
            Ini::load_from_file(&config_path).unwrap_or_else(|_| Ini::new())
        } else {
            Ini::new()
        };
        let profile_section = if name == "default" {
            "default".to_string()
        } else {
            format!("profile {}", name)
        };

        config_ini
            .with_section(Some(profile_section))
            .set("region", reg);
        config_ini
            .write_to_file(&config_path)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn delete_profile(name: &str) -> Result<(), String> {
    let aws_dir = get_aws_dir();
    let cred_path = aws_dir.join("credentials");
    let config_path = aws_dir.join("config");

    // 1. Remove from credentials
    if cred_path.exists() {
        let mut ini = Ini::load_from_file(&cred_path).map_err(|e| e.to_string())?;
        ini.delete(Some(name));
        ini.write_to_file(&cred_path).map_err(|e| e.to_string())?;
    }

    // 2. Remove from config
    if config_path.exists() {
        if let Ok(mut ini) = Ini::load_from_file(&config_path) {
            let profile_section = if name == "default" {
                "default".to_string()
            } else {
                format!("profile {}", name)
            };
            ini.delete(Some(profile_section));
            let _ = ini.write_to_file(&config_path);
        }
    }

    Ok(())
}
