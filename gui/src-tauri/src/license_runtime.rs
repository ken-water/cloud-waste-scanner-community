use crate::db;
use crate::license;
use crate::runtime_helpers::normalize_runtime_plan_type;
use chrono::Utc;
use serde::Serialize;
use sqlx::{Pool, Sqlite};
use std::path::Path;

const TRIAL_DURATION_SECS: i64 = 7 * 24 * 60 * 60;
const CLOCK_ROLLBACK_GRACE_SECS: i64 = 60 * 60;
const TRIAL_STARTED_AT_KEY: &str = "trial_started_at";
const TRIAL_EXPIRES_AT_KEY: &str = "trial_expires_at";
const TRIAL_LAST_SEEN_AT_KEY: &str = "trial_last_seen_at";
const PUBLIC_BETA_PLAN_TYPE: &str = "public_beta";

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuntimeLicensePolicy {
    pub(crate) plan_type: String,
    pub(crate) is_trial: bool,
    pub(crate) api_enabled: bool,
    pub(crate) resource_details_enabled: bool,
    pub(crate) trial_expires_at: Option<i64>,
    pub(crate) quota: Option<i64>,
    pub(crate) max_quota: Option<i64>,
}

pub(crate) async fn read_runtime_plan_type(db_path: &Path) -> Option<String> {
    let conn = db::init_db(db_path).await.ok()?;
    let value = ensure_runtime_trial_plan_type(&conn).await.ok()?;
    let trimmed = value.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

async fn read_i64_setting(conn: &Pool<Sqlite>, key: &str) -> i64 {
    db::get_setting(conn, key)
        .await
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or(0)
}

async fn save_i64_setting(conn: &Pool<Sqlite>, key: &str, value: i64) {
    let _ = db::save_setting(conn, key, &value.to_string()).await;
}

fn paid_plan_type(plan: &str) -> bool {
    matches!(
        normalize_runtime_plan_type(plan).as_str(),
        "monthly" | "yearly" | "lifetime" | "pro" | "team" | "enterprise" | "advanced" | "site"
    )
}

pub(crate) async fn ensure_runtime_trial_plan_type(conn: &Pool<Sqlite>) -> Result<String, String> {
    let current = db::get_setting(conn, "runtime_plan_type")
        .await
        .unwrap_or_default();
    let normalized = normalize_runtime_plan_type(&current);
    if paid_plan_type(&normalized) {
        return Ok(normalized);
    }
    if normalized == PUBLIC_BETA_PLAN_TYPE || normalized.is_empty() || normalized == "trial" || normalized == "trial_expired" {
        let plan = PUBLIC_BETA_PLAN_TYPE.to_string();
        let _ = db::save_setting(conn, "runtime_plan_type", &plan).await;
        return Ok(plan);
    }

    let now = Utc::now().timestamp();
    let last_seen_at = read_i64_setting(conn, TRIAL_LAST_SEEN_AT_KEY).await;
    if last_seen_at > 0 && now + CLOCK_ROLLBACK_GRACE_SECS < last_seen_at {
        let plan = "trial_expired".to_string();
        let _ = db::save_setting(conn, "runtime_plan_type", &plan).await;
        return Ok(plan);
    }

    let mut started_at = read_i64_setting(conn, TRIAL_STARTED_AT_KEY).await;
    let mut expires_at = read_i64_setting(conn, TRIAL_EXPIRES_AT_KEY).await;
    if started_at <= 0 || expires_at <= 0 {
        started_at = now;
        expires_at = now + TRIAL_DURATION_SECS;
        save_i64_setting(conn, TRIAL_STARTED_AT_KEY, started_at).await;
        save_i64_setting(conn, TRIAL_EXPIRES_AT_KEY, expires_at).await;
    }

    save_i64_setting(conn, TRIAL_LAST_SEEN_AT_KEY, now.max(last_seen_at)).await;

    let plan = if now <= expires_at {
        "trial".to_string()
    } else {
        "trial_expired".to_string()
    };
    let _ = db::save_setting(conn, "runtime_plan_type", &plan).await;
    Ok(plan)
}

pub(crate) async fn read_trial_expires_at(db_path: &Path) -> Option<i64> {
    let conn = db::init_db(db_path).await.ok()?;
    let expires_at = read_i64_setting(&conn, TRIAL_EXPIRES_AT_KEY).await;
    (expires_at > 0).then_some(expires_at)
}

pub(crate) async fn persist_runtime_plan_type_from_status(
    conn: &Pool<Sqlite>,
    status: &license::CheckResponse,
) {
    let plan_value = if status.valid {
        if status
            .is_trial
            .unwrap_or_else(|| matches!(status.plan_type.as_deref(), Some("trial")))
        {
            "trial".to_string()
        } else {
            status
                .plan_type
                .as_deref()
                .map(normalize_runtime_plan_type)
                .unwrap_or_else(|| "pro".to_string())
        }
    } else {
        "".to_string()
    };

    let _ = db::save_setting(conn, "runtime_plan_type", &plan_value).await;
}

pub(crate) async fn fetch_runtime_license_policy(
    db_path: &Path,
    key: &str,
    _machine_id: Option<&str>,
    _now: i64,
) -> Result<RuntimeLicensePolicy, String> {
    let conn = db::init_db(db_path).await.map_err(|e| e.to_string())?;
    let payload = license::verify_license(key.trim())?;
    let status = license::status_from_payload(&payload);
    let plan_type = status
        .plan_type
        .as_deref()
        .map(normalize_runtime_plan_type)
        .unwrap_or_else(|| "monthly".to_string());
    let is_trial = status
        .is_trial
        .unwrap_or_else(|| plan_type.eq_ignore_ascii_case("trial"));
    let policy = RuntimeLicensePolicy {
        plan_type: plan_type.clone(),
        is_trial,
        api_enabled: status.api_enabled.unwrap_or(!is_trial),
        resource_details_enabled: status.resource_details_enabled.unwrap_or(!is_trial),
        trial_expires_at: status.trial_expires_at.or(payload.expires_at),
        quota: status.quota,
        max_quota: status.max_quota.or(payload.max_hosts),
    };

    let _ = db::save_setting(&conn, "runtime_plan_type", &plan_type).await;

    Ok(policy)
}

pub(crate) fn resolve_effective_license_key_from_text(local_key: &str) -> Result<String, String> {
    let trimmed = local_key.trim().to_string();
    if trimmed.is_empty() {
        Ok(String::new())
    } else {
        Ok(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::{CheckResponse, LicensePayload, LicenseType};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cws-license-runtime-{}-{}.sqlite",
            name,
            Uuid::new_v4()
        ))
    }

    async fn fresh_db(name: &str) -> (PathBuf, Pool<Sqlite>) {
        let path = temp_db_path(name);
        let pool = db::init_db(&path).await.expect("init db");
        (path, pool)
    }

    #[tokio::test]
    async fn persist_runtime_plan_type_follows_license_status() {
        let (path, pool) = fresh_db("plan-type").await;
        persist_runtime_plan_type_from_status(
            &pool,
            &CheckResponse {
                valid: true,
                latest_version: "2.7.0".to_string(),
                download_url: None,
                download_urls: None,
                message: None,
                quota: Some(12),
                max_quota: Some(100),
                plan_type: Some("yearly".to_string()),
                is_trial: Some(false),
                trial_expires_at: None,
                api_enabled: Some(true),
                resource_details_enabled: Some(true),
                customer_email: None,
                license_started_at: None,
                first_purchase_at: None,
                latest_purchase_at: None,
                purchase_count: None,
                latest_order_ref: None,
                latest_order_amount: None,
                latest_order_plan: None,
                latest_order_status: None,
                order_history: None,
            },
        )
        .await;
        assert_eq!(
            db::get_setting(&pool, "runtime_plan_type")
                .await
                .expect("saved plan type"),
            "yearly"
        );

        persist_runtime_plan_type_from_status(
            &pool,
            &CheckResponse {
                valid: false,
                latest_version: "2.7.0".to_string(),
                download_url: None,
                download_urls: None,
                message: Some("expired".to_string()),
                quota: None,
                max_quota: None,
                plan_type: Some("trial".to_string()),
                is_trial: Some(true),
                trial_expires_at: None,
                api_enabled: Some(false),
                resource_details_enabled: Some(false),
                customer_email: None,
                license_started_at: None,
                first_purchase_at: None,
                latest_purchase_at: None,
                purchase_count: None,
                latest_order_ref: None,
                latest_order_amount: None,
                latest_order_plan: None,
                latest_order_status: None,
                order_history: None,
            },
        )
        .await;
        assert_eq!(
            db::get_setting(&pool, "runtime_plan_type")
                .await
                .expect("cleared plan type"),
            ""
        );

        drop(pool);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn ensure_runtime_trial_plan_type_defaults_to_public_beta() {
        let (path, pool) = fresh_db("trial-start").await;
        let plan = ensure_runtime_trial_plan_type(&pool)
            .await
            .expect("resolve public beta");

        assert_eq!(plan, "public_beta");
        assert_eq!(
            db::get_setting(&pool, "runtime_plan_type")
                .await
                .expect("runtime plan"),
            "public_beta"
        );
        let started_at = read_i64_setting(&pool, TRIAL_STARTED_AT_KEY).await;
        let expires_at = read_i64_setting(&pool, TRIAL_EXPIRES_AT_KEY).await;
        assert_eq!(started_at, 0);
        assert_eq!(expires_at, 0);

        drop(pool);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn ensure_runtime_trial_plan_type_migrates_expired_trial_to_public_beta() {
        let (path, pool) = fresh_db("trial-expired").await;
        let now = Utc::now().timestamp();
        db::save_setting(
            &pool,
            TRIAL_STARTED_AT_KEY,
            &(now - TRIAL_DURATION_SECS - 60).to_string(),
        )
        .await
        .expect("save start");
        db::save_setting(&pool, TRIAL_EXPIRES_AT_KEY, &(now - 60).to_string())
            .await
            .expect("save expiry");
        db::save_setting(&pool, TRIAL_LAST_SEEN_AT_KEY, &(now - 30).to_string())
            .await
            .expect("save last seen");

        let plan = ensure_runtime_trial_plan_type(&pool)
            .await
            .expect("resolve expired trial as beta");
        assert_eq!(plan, "public_beta");

        drop(pool);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn ensure_runtime_trial_plan_type_migrates_clock_rollback_to_public_beta() {
        let (path, pool) = fresh_db("trial-clock-rollback").await;
        let now = Utc::now().timestamp();
        db::save_setting(&pool, TRIAL_STARTED_AT_KEY, &now.to_string())
            .await
            .expect("save start");
        db::save_setting(
            &pool,
            TRIAL_EXPIRES_AT_KEY,
            &(now + TRIAL_DURATION_SECS).to_string(),
        )
        .await
        .expect("save expiry");
        db::save_setting(
            &pool,
            TRIAL_LAST_SEEN_AT_KEY,
            &(now + CLOCK_ROLLBACK_GRACE_SECS + 60).to_string(),
        )
        .await
        .expect("save future last seen");

        let plan = ensure_runtime_trial_plan_type(&pool)
            .await
            .expect("resolve rollback as beta");
        assert_eq!(plan, "public_beta");

        drop(pool);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn resolve_effective_license_key_from_text_trims_and_defaults_empty_values() {
        assert_eq!(
            resolve_effective_license_key_from_text("  signed-key  ").expect("trim key"),
            "signed-key"
        );
        assert!(resolve_effective_license_key_from_text("  ")
            .expect("default key")
            .is_empty());
    }

    #[test]
    fn runtime_trial_policy_logic_is_consistent() {
        let status = CheckResponse {
            valid: true,
            latest_version: "2.7.0".to_string(),
            download_url: None,
            download_urls: None,
            message: None,
            quota: Some(5),
            max_quota: Some(100),
            plan_type: Some("trial".to_string()),
            is_trial: Some(true),
            trial_expires_at: Some(2_000_000_000),
            api_enabled: Some(false),
            resource_details_enabled: Some(false),
            customer_email: None,
            license_started_at: None,
            first_purchase_at: None,
            latest_purchase_at: None,
            purchase_count: None,
            latest_order_ref: None,
            latest_order_amount: None,
            latest_order_plan: None,
            latest_order_status: None,
            order_history: None,
        };
        let payload = LicensePayload {
            id: "lic_1".to_string(),
            user: "ops@example.com".to_string(),
            l_type: LicenseType::Trial,
            expires_at: Some(2_000_000_001),
            max_hosts: Some(5),
        };

        let plan_type = status
            .plan_type
            .clone()
            .unwrap_or_else(|| format!("{:?}", payload.l_type).to_lowercase());
        let is_trial = status
            .is_trial
            .unwrap_or_else(|| plan_type.eq_ignore_ascii_case("trial"));
        let trial_expires_at = status.trial_expires_at.or(payload.expires_at);

        assert_eq!(plan_type, "trial");
        assert!(is_trial);
        assert_eq!(trial_expires_at, Some(2_000_000_000));
    }
}
