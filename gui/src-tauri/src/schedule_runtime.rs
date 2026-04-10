use crate::db;
use crate::ApiSchedule;
use std::collections::HashMap;
use std::path::Path;

pub(crate) async fn persist_schedules_to_db(
    db_path: &Path,
    schedules: &[ApiSchedule],
) -> Result<(), String> {
    let conn = db::init_db(db_path).await?;
    let payload = serde_json::to_string(schedules).map_err(|e| e.to_string())?;
    db::save_setting(&conn, "api_schedules_json", &payload)
        .await
        .map_err(|e| e.to_string())
}

pub(crate) async fn load_schedules_from_db(
    db_path: &Path,
) -> Result<HashMap<String, ApiSchedule>, String> {
    let conn = db::init_db(db_path).await?;
    let raw = db::get_setting(&conn, "api_schedules_json")
        .await
        .map_err(|e| e.to_string())?;
    let parsed: Vec<ApiSchedule> = serde_json::from_str(&raw).unwrap_or_default();
    Ok(parsed
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApiScanRequest;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cws-schedule-runtime-{}-{}.sqlite",
            name,
            Uuid::new_v4()
        ))
    }

    fn sample_schedule(id: &str, run_at: i64) -> ApiSchedule {
        ApiSchedule {
            id: id.to_string(),
            name: format!("Schedule {}", id),
            enabled: true,
            run_at,
            interval_minutes: Some(60),
            timezone: Some("UTC".to_string()),
            next_run_at: Some(run_at + 3600),
            last_run_at: None,
            last_scan_id: None,
            last_error: None,
            created_at: run_at - 60,
            updated_at: run_at,
            scan: ApiScanRequest::default(),
        }
    }

    #[tokio::test]
    async fn schedules_round_trip_from_db_setting() {
        let path = temp_db_path("roundtrip");
        let schedules = vec![
            sample_schedule("sched_1", 1000),
            sample_schedule("sched_2", 2000),
        ];

        persist_schedules_to_db(&path, &schedules)
            .await
            .expect("persist schedules");
        let loaded = load_schedules_from_db(&path).await.expect("load schedules");

        assert_eq!(loaded.len(), 2);
        assert_eq!(
            loaded.get("sched_1").map(|s| s.name.as_str()),
            Some("Schedule sched_1")
        );
        assert_eq!(
            loaded.get("sched_2").and_then(|s| s.next_run_at),
            Some(5600)
        );

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn invalid_schedule_payload_falls_back_to_empty_map() {
        let path = temp_db_path("invalid");
        let conn = db::init_db(&path).await.expect("init db");
        db::save_setting(&conn, "api_schedules_json", "{\"not\":\"a list\"}")
            .await
            .expect("save invalid payload");

        let loaded = load_schedules_from_db(&path).await.expect("load schedules");
        assert!(loaded.is_empty());

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn duplicate_schedule_ids_keep_last_payload_entry() {
        let path = temp_db_path("duplicate-id");
        let conn = db::init_db(&path).await.expect("init db");
        let duplicate_payload = serde_json::json!([
            sample_schedule("sched_1", 1000),
            sample_schedule("sched_1", 2000)
        ]);
        db::save_setting(&conn, "api_schedules_json", &duplicate_payload.to_string())
            .await
            .expect("save duplicate payload");

        let loaded = load_schedules_from_db(&path).await.expect("load schedules");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.get("sched_1").map(|s| s.run_at), Some(2000));

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn persist_overwrites_previous_schedule_payload() {
        let path = temp_db_path("overwrite");
        persist_schedules_to_db(&path, &[sample_schedule("sched_1", 1000)])
            .await
            .expect("persist first");
        persist_schedules_to_db(&path, &[sample_schedule("sched_2", 2000)])
            .await
            .expect("persist second");

        let loaded = load_schedules_from_db(&path).await.expect("load schedules");
        assert_eq!(loaded.len(), 1);
        assert!(loaded.get("sched_1").is_none());
        assert!(loaded.get("sched_2").is_some());

        let _ = std::fs::remove_file(path);
    }
}
