use crate::models::WastedResource;
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Pool, Row, Sqlite,
};
use std::path::PathBuf;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct ScanRecord {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub profile: String,
    pub region: String,
    pub total_cost: f64,
}

pub struct Db {
    pool: Pool<Sqlite>,
}

impl Db {
    /// Initialize the database at the given path.
    /// Creates the file and tables if they don't exist.
    pub async fn new(db_path: PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Configure connection options to create the DB file if missing
        let options =
            SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.to_string_lossy()))?
                .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        let db = Self { pool };
        db.init_schema().await?;
        Ok(db)
    }

    async fn init_schema(&self) -> Result<()> {
        // Enable WAL mode for better concurrency
        sqlx::query("PRAGMA journal_mode=WAL;")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS scan_history (
                id TEXT PRIMARY KEY,
                timestamp DATETIME NOT NULL,
                profile TEXT NOT NULL,
                region TEXT NOT NULL,
                total_cost REAL NOT NULL
            );
            CREATE TABLE IF NOT EXISTS scan_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                scan_id TEXT NOT NULL,
                resource_id TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                details TEXT,
                cost REAL NOT NULL,
                FOREIGN KEY(scan_id) REFERENCES scan_history(id)
            );
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn save_scan(
        &self,
        profile: &str,
        region: &str,
        items: &[WastedResource],
    ) -> Result<String> {
        let scan_id = Uuid::new_v4().to_string();
        let total_cost: f64 = items.iter().map(|i| i.estimated_monthly_cost).sum();
        let now = Utc::now();

        let mut tx = self.pool.begin().await?;

        // 1. Save Header
        sqlx::query(
            "INSERT INTO scan_history (id, timestamp, profile, region, total_cost) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&scan_id)
        .bind(now)
        .bind(profile)
        .bind(region)
        .bind(total_cost)
        .execute(&mut *tx)
        .await?;

        // 2. Save Items
        for item in items {
            sqlx::query(
                "INSERT INTO scan_items (scan_id, resource_id, resource_type, details, cost) VALUES (?, ?, ?, ?, ?)"
            )
            .bind(&scan_id)
            .bind(&item.id)
            .bind(&item.resource_type)
            .bind(&item.details)
            .bind(item.estimated_monthly_cost)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(scan_id)
    }

    pub async fn get_recent_scans(&self, limit: i64) -> Result<Vec<ScanRecord>> {
        let rows = sqlx::query_as::<_, ScanRecord>(
            r#"
            SELECT id, timestamp, profile, region, total_cost
            FROM scan_history
            ORDER BY timestamp DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_total_stats(&self) -> Result<(i64, f64)> {
        let rec = sqlx::query(
            "SELECT COUNT(*) as count, COALESCE(SUM(total_cost), 0.0) as cost FROM scan_history",
        )
        .fetch_one(&self.pool)
        .await?;

        let count: i64 = rec.try_get("count")?;
        let cost: f64 = rec.try_get("cost")?;

        Ok((count, cost))
    }
}
