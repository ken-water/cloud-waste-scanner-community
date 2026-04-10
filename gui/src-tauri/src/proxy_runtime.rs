use crate::db;
use crate::proxy_helpers::{compose_proxy_url_from_parts, mask_proxy_url, normalize_proxy_mode};
use crate::runtime_helpers::normalize_account_notification_choices;
use crate::{
    ACCOUNT_NOTIFICATION_ASSIGNMENTS_SETTING_KEY, ACCOUNT_PROXY_ASSIGNMENTS_SETTING_KEY,
    PROXY_CHOICE_DIRECT, PROXY_CHOICE_GLOBAL,
};
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;

pub(crate) fn normalize_custom_proxy_url(url: &str) -> String {
    url.trim().to_string()
}

pub(crate) fn configure_proxy_env(mode: &str, url: &str) {
    std::env::remove_var("HTTP_PROXY");
    std::env::remove_var("HTTPS_PROXY");
    std::env::remove_var("ALL_PROXY");
    std::env::remove_var("NO_PROXY");
    std::env::remove_var("http_proxy");
    std::env::remove_var("https_proxy");
    std::env::remove_var("all_proxy");
    std::env::remove_var("no_proxy");

    if mode == "custom" && !url.is_empty() {
        let normalized_url = normalize_custom_proxy_url(url);
        std::env::set_var("HTTP_PROXY", &normalized_url);
        std::env::set_var("HTTPS_PROXY", &normalized_url);
        std::env::set_var("ALL_PROXY", &normalized_url);
        std::env::set_var("http_proxy", &normalized_url);
        std::env::set_var("https_proxy", &normalized_url);
        std::env::set_var("all_proxy", &normalized_url);
        println!("Proxy set to custom: {}", mask_proxy_url(&normalized_url));
    } else if mode == "none" {
        std::env::set_var("NO_PROXY", "*");
        std::env::set_var("no_proxy", "*");
        println!("Proxy set to NONE (Direct)");
    } else {
        println!("Proxy set to System Default");
    }
}

#[derive(Debug, Clone)]
struct ProxyEnvSnapshot {
    http_proxy: Option<String>,
    https_proxy: Option<String>,
    all_proxy: Option<String>,
    no_proxy: Option<String>,
    http_proxy_lower: Option<String>,
    https_proxy_lower: Option<String>,
    all_proxy_lower: Option<String>,
    no_proxy_lower: Option<String>,
}

pub(crate) struct ProxyEnvGuard {
    snapshot: ProxyEnvSnapshot,
    _lock_guard: tokio::sync::OwnedMutexGuard<()>,
}

impl Drop for ProxyEnvGuard {
    fn drop(&mut self) {
        restore_proxy_env(&self.snapshot);
    }
}

fn capture_proxy_env() -> ProxyEnvSnapshot {
    ProxyEnvSnapshot {
        http_proxy: std::env::var("HTTP_PROXY").ok(),
        https_proxy: std::env::var("HTTPS_PROXY").ok(),
        all_proxy: std::env::var("ALL_PROXY").ok(),
        no_proxy: std::env::var("NO_PROXY").ok(),
        http_proxy_lower: std::env::var("http_proxy").ok(),
        https_proxy_lower: std::env::var("https_proxy").ok(),
        all_proxy_lower: std::env::var("all_proxy").ok(),
        no_proxy_lower: std::env::var("no_proxy").ok(),
    }
}

fn set_or_remove_env(key: &str, value: Option<&str>) {
    if let Some(v) = value {
        std::env::set_var(key, v);
    } else {
        std::env::remove_var(key);
    }
}

fn restore_proxy_env(snapshot: &ProxyEnvSnapshot) {
    set_or_remove_env("HTTP_PROXY", snapshot.http_proxy.as_deref());
    set_or_remove_env("HTTPS_PROXY", snapshot.https_proxy.as_deref());
    set_or_remove_env("ALL_PROXY", snapshot.all_proxy.as_deref());
    set_or_remove_env("NO_PROXY", snapshot.no_proxy.as_deref());
    set_or_remove_env("http_proxy", snapshot.http_proxy_lower.as_deref());
    set_or_remove_env("https_proxy", snapshot.https_proxy_lower.as_deref());
    set_or_remove_env("all_proxy", snapshot.all_proxy_lower.as_deref());
    set_or_remove_env("no_proxy", snapshot.no_proxy_lower.as_deref());
}

pub(crate) async fn apply_proxy_env_with_guard(mode: &str, url: &str) -> ProxyEnvGuard {
    static PROXY_ENV_LOCK: std::sync::OnceLock<std::sync::Arc<tokio::sync::Mutex<()>>> =
        std::sync::OnceLock::new();
    let lock = PROXY_ENV_LOCK
        .get_or_init(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone();
    let lock_guard = lock.lock_owned().await;

    let snapshot = capture_proxy_env();
    configure_proxy_env(mode, url);
    ProxyEnvGuard {
        snapshot,
        _lock_guard: lock_guard,
    }
}

pub(crate) async fn resolve_proxy_runtime(
    conn: &Pool<Sqlite>,
    proxy_choice: Option<&str>,
) -> (String, String) {
    let choice = proxy_choice.unwrap_or(PROXY_CHOICE_GLOBAL).trim();
    if choice.is_empty() || choice == PROXY_CHOICE_GLOBAL {
        let mode_raw = db::get_setting(conn, "proxy_mode")
            .await
            .unwrap_or_else(|_| "none".to_string());
        let url_raw = db::get_setting(conn, "proxy_url").await.unwrap_or_default();
        return (normalize_proxy_mode(&mode_raw), url_raw.trim().to_string());
    }

    if choice == PROXY_CHOICE_DIRECT {
        return ("none".to_string(), String::new());
    }

    match db::get_proxy_profile(conn, choice).await {
        Ok(Some(profile)) => (
            "custom".to_string(),
            compose_proxy_url_from_parts(
                &profile.protocol,
                &profile.host,
                profile.port,
                profile.auth_username.as_deref(),
                profile.auth_password.as_deref(),
            ),
        ),
        _ => {
            let mode_raw = db::get_setting(conn, "proxy_mode")
                .await
                .unwrap_or_else(|_| "none".to_string());
            let url_raw = db::get_setting(conn, "proxy_url").await.unwrap_or_default();
            (normalize_proxy_mode(&mode_raw), url_raw.trim().to_string())
        }
    }
}

pub(crate) async fn apply_proxy_choice_with_guard(
    conn: &Pool<Sqlite>,
    proxy_choice: Option<&str>,
) -> ProxyEnvGuard {
    let (mode, url) = resolve_proxy_runtime(conn, proxy_choice).await;
    apply_proxy_env_with_guard(&mode, &url).await
}

pub(crate) async fn load_account_proxy_assignments(conn: &Pool<Sqlite>) -> HashMap<String, String> {
    let raw = db::get_setting(conn, ACCOUNT_PROXY_ASSIGNMENTS_SETTING_KEY)
        .await
        .unwrap_or_default();
    serde_json::from_str::<HashMap<String, String>>(&raw).unwrap_or_default()
}

pub(crate) async fn load_account_notification_assignments(
    conn: &Pool<Sqlite>,
) -> HashMap<String, Vec<String>> {
    let raw = db::get_setting(conn, ACCOUNT_NOTIFICATION_ASSIGNMENTS_SETTING_KEY)
        .await
        .unwrap_or_default();
    let mut assignments: HashMap<String, Vec<String>> = HashMap::new();
    let parsed = serde_json::from_str::<serde_json::Value>(&raw).ok();
    if let Some(map) = parsed.and_then(|value| value.as_object().cloned()) {
        for (account_id, raw_choice) in map {
            let choices = normalize_account_notification_choices(raw_choice);
            assignments.insert(account_id, choices);
        }
    }
    assignments
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cws-proxy-runtime-{}-{}.sqlite",
            name,
            Uuid::new_v4()
        ))
    }

    async fn fresh_db(name: &str) -> (PathBuf, Pool<Sqlite>) {
        let path = temp_db_path(name);
        let pool = db::init_db(&path).await.expect("init db");
        (path, pool)
    }

    fn clear_proxy_env() {
        for key in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "NO_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "no_proxy",
        ] {
            std::env::remove_var(key);
        }
    }

    #[tokio::test]
    async fn resolve_proxy_runtime_prefers_global_direct_and_profile_modes() {
        let (path, pool) = fresh_db("resolve").await;
        db::save_setting(&pool, "proxy_mode", "custom")
            .await
            .expect("save proxy mode");
        db::save_setting(&pool, "proxy_url", "http://global.proxy:9000")
            .await
            .expect("save proxy url");
        let profile_id = db::save_proxy_profile(
            &pool,
            Some("proxy-team".to_string()),
            "Team Proxy",
            "socks5h",
            "proxy.team.internal",
            1080,
            Some("ops"),
            Some("secret"),
        )
        .await
        .expect("save proxy profile");

        let global = resolve_proxy_runtime(&pool, None).await;
        assert_eq!(
            global,
            ("custom".to_string(), "http://global.proxy:9000".to_string())
        );

        let direct = resolve_proxy_runtime(&pool, Some(PROXY_CHOICE_DIRECT)).await;
        assert_eq!(direct, ("none".to_string(), String::new()));

        let profile = resolve_proxy_runtime(&pool, Some(profile_id.as_str())).await;
        assert_eq!(profile.0, "custom");
        assert_eq!(
            profile.1,
            "socks5h://ops:secret@proxy.team.internal:1080".to_string()
        );

        let missing = resolve_proxy_runtime(&pool, Some("missing-profile")).await;
        assert_eq!(missing, global);

        drop(pool);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn account_assignment_loaders_normalize_payloads() {
        let (path, pool) = fresh_db("assignments").await;
        db::save_setting(
            &pool,
            ACCOUNT_PROXY_ASSIGNMENTS_SETTING_KEY,
            r#"{"aws-prod":"proxy-a","azure-finance":"__direct__"}"#,
        )
        .await
        .expect("save proxy assignments");
        db::save_setting(
            &pool,
            ACCOUNT_NOTIFICATION_ASSIGNMENTS_SETTING_KEY,
            r#"{"aws-prod":["slack","email","slack"],"gcp-dev":"__all_channels__","empty":[]}"#,
        )
        .await
        .expect("save notification assignments");

        let proxies = load_account_proxy_assignments(&pool).await;
        assert_eq!(proxies.get("aws-prod").map(String::as_str), Some("proxy-a"));
        assert_eq!(
            proxies.get("azure-finance").map(String::as_str),
            Some(PROXY_CHOICE_DIRECT)
        );

        let notifications = load_account_notification_assignments(&pool).await;
        assert_eq!(
            notifications.get("aws-prod"),
            Some(&vec!["email".to_string(), "slack".to_string()])
        );
        assert_eq!(
            notifications.get("gcp-dev"),
            Some(&vec!["__all_channels__".to_string()])
        );
        assert_eq!(
            notifications.get("empty"),
            Some(&vec!["__all_channels__".to_string()])
        );

        drop(pool);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn proxy_env_guard_restores_previous_environment() {
        clear_proxy_env();
        std::env::set_var("HTTP_PROXY", "http://before.upper:8080");
        std::env::set_var("http_proxy", "http://before.lower:8080");
        std::env::set_var("NO_PROXY", "internal");

        {
            let _guard = apply_proxy_env_with_guard("custom", "http://after.proxy:9999").await;
            assert_eq!(
                std::env::var("HTTP_PROXY").ok().as_deref(),
                Some("http://after.proxy:9999")
            );
            assert_eq!(
                std::env::var("HTTPS_PROXY").ok().as_deref(),
                Some("http://after.proxy:9999")
            );
            assert_eq!(std::env::var("NO_PROXY").ok().as_deref(), None);
        }

        assert_eq!(
            std::env::var("HTTP_PROXY").ok().as_deref(),
            Some("http://before.upper:8080")
        );
        assert_eq!(
            std::env::var("http_proxy").ok().as_deref(),
            Some("http://before.lower:8080")
        );
        assert_eq!(std::env::var("NO_PROXY").ok().as_deref(), Some("internal"));
        clear_proxy_env();
    }

    #[tokio::test]
    async fn assignment_loaders_fallback_on_invalid_json_payloads() {
        let (path, pool) = fresh_db("assignments-invalid").await;
        db::save_setting(&pool, ACCOUNT_PROXY_ASSIGNMENTS_SETTING_KEY, "{bad json")
            .await
            .expect("save bad proxy payload");
        db::save_setting(
            &pool,
            ACCOUNT_NOTIFICATION_ASSIGNMENTS_SETTING_KEY,
            r#"{"aws-prod":42}"#,
        )
        .await
        .expect("save non-string notification choices");

        let proxies = load_account_proxy_assignments(&pool).await;
        assert!(proxies.is_empty());

        let notifications = load_account_notification_assignments(&pool).await;
        assert_eq!(
            notifications.get("aws-prod"),
            Some(&vec![crate::ACCOUNT_NOTIFICATION_CHOICE_ALL.to_string()])
        );

        drop(pool);
        let _ = std::fs::remove_file(path);
    }
}
