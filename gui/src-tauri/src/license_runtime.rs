use crate::db;
use crate::license;
use crate::runtime_helpers::normalize_runtime_plan_type;
use serde::Serialize;
use sqlx::{Pool, Sqlite};
use std::path::Path;

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
    let value = db::get_setting(&conn, "runtime_plan_type").await.ok()?;
    let trimmed = value.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
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
    _key: &str,
    _machine_id: Option<&str>,
    _now: i64,
) -> Result<RuntimeLicensePolicy, String> {
    let conn = db::init_db(db_path).await.map_err(|e| e.to_string())?;
    let policy = RuntimeLicensePolicy {
        plan_type: "community".to_string(),
        is_trial: false,
        api_enabled: true,
        resource_details_enabled: true,
        trial_expires_at: None,
        quota: None,
        max_quota: None,
    };

    let _ = db::save_setting(&conn, "runtime_plan_type", "community").await;

    Ok(policy)
}

pub(crate) fn resolve_effective_license_key_from_text(local_key: &str) -> Result<String, String> {
    let trimmed = local_key.trim().to_string();
    if trimmed.is_empty() {
        Ok("community-local".to_string())
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

    #[test]
    fn resolve_effective_license_key_from_text_trims_and_defaults_empty_values() {
        assert_eq!(
            resolve_effective_license_key_from_text("  signed-key  ").expect("trim key"),
            "signed-key"
        );
        assert_eq!(
            resolve_effective_license_key_from_text("  ").expect("default key"),
            "community-local"
        );
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
