use crate::{
    ApiScanJob, ApiSchedule, API_MAX_SCAN_JOBS_ACTIVE, API_MAX_SCAN_JOBS_STORED,
    API_RATE_LIMIT_PREFIX,
};
use std::collections::HashMap;

pub(crate) fn prepare_job_queue_for_new_scan(
    jobs: &mut HashMap<String, ApiScanJob>,
) -> Result<(), String> {
    let active_jobs = jobs
        .values()
        .filter(|job| job.status == "queued" || job.status == "running")
        .count();
    if active_jobs >= API_MAX_SCAN_JOBS_ACTIVE {
        return Err(format!(
            "{}Too many scan jobs are in progress. Please retry in a moment.",
            API_RATE_LIMIT_PREFIX
        ));
    }

    if jobs.len() >= API_MAX_SCAN_JOBS_STORED {
        let mut evictable: Vec<(String, i64)> = jobs
            .iter()
            .filter_map(|(job_id, job)| {
                if job.status == "completed" || job.status == "failed" {
                    Some((job_id.clone(), job.finished_at.unwrap_or(job.created_at)))
                } else {
                    None
                }
            })
            .collect();
        evictable.sort_by_key(|(_, ts)| *ts);

        let keep_target = API_MAX_SCAN_JOBS_STORED.saturating_sub(1);
        for (job_id, _) in evictable {
            if jobs.len() <= keep_target {
                break;
            }
            jobs.remove(&job_id);
        }

        if jobs.len() > keep_target {
            return Err(format!(
                "{}Scan queue is temporarily full. Please retry shortly.",
                API_RATE_LIMIT_PREFIX
            ));
        }
    }

    Ok(())
}

pub(crate) fn list_schedules_sorted(schedules: &HashMap<String, ApiSchedule>) -> Vec<ApiSchedule> {
    let mut list: Vec<ApiSchedule> = schedules.values().cloned().collect();
    list.sort_by_key(|item| item.created_at);
    list
}

pub(crate) fn remove_schedule(
    schedules: &mut HashMap<String, ApiSchedule>,
    schedule_id: &str,
) -> bool {
    schedules.remove(schedule_id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ApiScanRequest;

    fn job(id: &str, status: &str, created_at: i64, finished_at: Option<i64>) -> ApiScanJob {
        ApiScanJob {
            id: id.to_string(),
            status: status.to_string(),
            trigger_source: None,
            created_at,
            started_at: None,
            finished_at,
            resources_found: None,
            estimated_monthly_savings: None,
            selected_accounts: None,
            error: None,
            report_email_status: None,
        }
    }

    fn schedule(id: &str, created_at: i64) -> ApiSchedule {
        ApiSchedule {
            id: id.to_string(),
            name: id.to_string(),
            enabled: true,
            run_at: created_at,
            interval_minutes: None,
            timezone: None,
            next_run_at: None,
            last_run_at: None,
            last_scan_id: None,
            last_error: None,
            created_at,
            updated_at: created_at,
            scan: ApiScanRequest::default(),
        }
    }

    #[test]
    fn job_queue_rejects_when_active_cap_is_hit() {
        let mut jobs = HashMap::new();
        for idx in 0..API_MAX_SCAN_JOBS_ACTIVE {
            jobs.insert(
                format!("job-{}", idx),
                job(&format!("job-{}", idx), "running", idx as i64, None),
            );
        }
        assert!(prepare_job_queue_for_new_scan(&mut jobs).is_err());
    }

    #[test]
    fn job_queue_evicts_old_finished_entries_before_insert() {
        let mut jobs = HashMap::new();
        for idx in 0..API_MAX_SCAN_JOBS_STORED {
            jobs.insert(
                format!("job-{}", idx),
                job(
                    &format!("job-{}", idx),
                    "completed",
                    idx as i64,
                    Some(idx as i64),
                ),
            );
        }
        prepare_job_queue_for_new_scan(&mut jobs).expect("queue should evict");
        assert_eq!(jobs.len(), API_MAX_SCAN_JOBS_STORED - 1);
        assert!(!jobs.contains_key("job-0"));
    }

    #[test]
    fn job_queue_rejects_when_all_entries_are_active_and_at_capacity() {
        let mut jobs = HashMap::new();
        for idx in 0..API_MAX_SCAN_JOBS_STORED {
            jobs.insert(
                format!("job-{}", idx),
                job(&format!("job-{}", idx), "running", idx as i64, None),
            );
        }
        let err = prepare_job_queue_for_new_scan(&mut jobs).expect_err("queue should reject");
        assert!(err.contains(API_RATE_LIMIT_PREFIX.trim()));
    }

    #[test]
    fn job_queue_uses_created_at_when_finished_at_is_missing_for_eviction() {
        let mut jobs = HashMap::new();
        for idx in 0..API_MAX_SCAN_JOBS_STORED {
            jobs.insert(
                format!("job-{}", idx),
                job(&format!("job-{}", idx), "completed", idx as i64, None),
            );
        }
        prepare_job_queue_for_new_scan(&mut jobs).expect("queue should evict oldest");
        assert_eq!(jobs.len(), API_MAX_SCAN_JOBS_STORED - 1);
        assert!(!jobs.contains_key("job-0"));
    }

    #[test]
    fn job_queue_can_evict_completed_even_when_active_jobs_exist() {
        let mut jobs = HashMap::new();
        for idx in 0..(API_MAX_SCAN_JOBS_STORED - 2) {
            jobs.insert(
                format!("done-{}", idx),
                job(
                    &format!("done-{}", idx),
                    "completed",
                    idx as i64,
                    Some(idx as i64),
                ),
            );
        }
        jobs.insert("run-1".to_string(), job("run-1", "running", 999, None));
        jobs.insert("run-2".to_string(), job("run-2", "running", 1000, None));

        prepare_job_queue_for_new_scan(&mut jobs).expect("queue should evict completed entry");
        assert_eq!(jobs.len(), API_MAX_SCAN_JOBS_STORED - 1);
        assert!(jobs.contains_key("run-1"));
        assert!(jobs.contains_key("run-2"));
    }

    #[test]
    fn schedule_helpers_sort_and_remove_entries() {
        let mut schedules = HashMap::from([
            ("b".to_string(), schedule("b", 200)),
            ("a".to_string(), schedule("a", 100)),
        ]);
        let sorted = list_schedules_sorted(&schedules);
        assert_eq!(sorted[0].id, "a");
        assert!(remove_schedule(&mut schedules, "a"));
        assert!(!remove_schedule(&mut schedules, "missing"));
    }
}
