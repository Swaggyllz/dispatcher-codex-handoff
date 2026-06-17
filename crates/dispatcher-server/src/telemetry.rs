use chrono::Datelike;
use dispatcher_engine::types::{ProviderHealthSnapshot, TelemetryRecord};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

type ProviderSummary = (String, i64, f64, i64, i64, i64, i64, f64);

#[derive(Debug, Clone)]
pub struct CodexTelemetryRecord {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub requested_model: String,
    pub model_id: String,
    pub reasoning_effort: String,
    pub speed: String,
    pub agent_tier: String,
    pub reason: String,
    pub success: bool,
    pub status_code: Option<u16>,
    pub latency_ms: u64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct QuotaEventRecord {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub provider_id: String,
    pub model_id: String,
    pub status_code: Option<u16>,
    pub retry_after_secs: Option<u64>,
    pub normalized_headroom: Option<f64>,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct HandoffContinuationRecord {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub package_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub success: bool,
    pub status_code: Option<u16>,
    pub latency_ms: u64,
    pub response_text: Option<String>,
    pub error_message: Option<String>,
}

pub struct TelemetryStore {
    db: Arc<Mutex<Connection>>,
}

impl TelemetryStore {
    pub async fn new(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS telemetry (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                request_tokens INTEGER NOT NULL DEFAULT 0,
                response_tokens INTEGER NOT NULL DEFAULT 0,
                latency_ms INTEGER NOT NULL DEFAULT 0,
                cost_usd REAL NOT NULL DEFAULT 0.0,
                success INTEGER NOT NULL DEFAULT 1,
                error_message TEXT,
                routing_strategy TEXT NOT NULL DEFAULT '',
                agent_tier TEXT NOT NULL DEFAULT '',
                is_fallback INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS codex_routes (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                requested_model TEXT NOT NULL,
                model_id TEXT NOT NULL,
                reasoning_effort TEXT NOT NULL,
                speed TEXT NOT NULL,
                agent_tier TEXT NOT NULL,
                reason TEXT NOT NULL,
                success INTEGER NOT NULL DEFAULT 0,
                status_code INTEGER,
                latency_ms INTEGER NOT NULL DEFAULT 0,
                error_message TEXT
            );

            CREATE TABLE IF NOT EXISTS quota_events (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                status_code INTEGER,
                retry_after_secs INTEGER,
                normalized_headroom REAL,
                source TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS handoff_packages (
                package_id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                schema_version TEXT NOT NULL,
                trigger TEXT NOT NULL,
                confidence TEXT NOT NULL,
                latest_user_request TEXT NOT NULL,
                package_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS handoff_continuations (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                package_id TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                success INTEGER NOT NULL DEFAULT 0,
                status_code INTEGER,
                latency_ms INTEGER NOT NULL DEFAULT 0,
                response_text TEXT,
                error_message TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_telemetry_timestamp ON telemetry(timestamp);
            CREATE INDEX IF NOT EXISTS idx_telemetry_provider ON telemetry(provider_id);
            CREATE INDEX IF NOT EXISTS idx_telemetry_success ON telemetry(success);
            CREATE INDEX IF NOT EXISTS idx_codex_routes_timestamp ON codex_routes(timestamp);
            CREATE INDEX IF NOT EXISTS idx_quota_events_timestamp ON quota_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_handoff_packages_created_at ON handoff_packages(created_at);
            CREATE INDEX IF NOT EXISTS idx_handoff_continuations_timestamp ON handoff_continuations(timestamp);
            CREATE INDEX IF NOT EXISTS idx_handoff_continuations_package ON handoff_continuations(package_id);",
        )?;
        ensure_telemetry_agent_tier_column(&conn)?;

        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn record(&self, record: &TelemetryRecord) -> anyhow::Result<()> {
        let db = self.db.lock().await;
        db.execute(
            "INSERT INTO telemetry (id, timestamp, provider_id, model_id, request_tokens,
             response_tokens, latency_ms, cost_usd, success, error_message, routing_strategy,
             agent_tier, is_fallback)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                record.id,
                record.timestamp.to_rfc3339(),
                record.provider_id,
                record.model_id,
                record.request_tokens,
                record.response_tokens,
                record.latency_ms,
                record.cost_usd,
                record.success as i32,
                record.error_message,
                record.routing_strategy,
                record.agent_tier,
                record.is_fallback as i32,
            ],
        )?;
        Ok(())
    }

    pub async fn record_codex_route(&self, record: &CodexTelemetryRecord) -> anyhow::Result<()> {
        let db = self.db.lock().await;
        db.execute(
            "INSERT INTO codex_routes (
                id, timestamp, requested_model, model_id, reasoning_effort, speed,
                agent_tier, reason, success, status_code, latency_ms, error_message
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                record.id,
                record.timestamp.to_rfc3339(),
                record.requested_model,
                record.model_id,
                record.reasoning_effort,
                record.speed,
                record.agent_tier,
                record.reason,
                record.success as i32,
                record.status_code,
                record.latency_ms,
                record.error_message,
            ],
        )?;
        Ok(())
    }

    pub async fn record_quota_event(&self, record: &QuotaEventRecord) -> anyhow::Result<()> {
        let db = self.db.lock().await;
        db.execute(
            "INSERT INTO quota_events (
                id, timestamp, provider_id, model_id, status_code, retry_after_secs,
                normalized_headroom, source
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                record.id,
                record.timestamp.to_rfc3339(),
                record.provider_id,
                record.model_id,
                record.status_code,
                record.retry_after_secs,
                record.normalized_headroom,
                record.source,
            ],
        )?;
        Ok(())
    }

    pub async fn record_handoff_package(
        &self,
        package: &crate::handoff::HandoffPackage,
    ) -> anyhow::Result<()> {
        let db = self.db.lock().await;
        db.execute(
            "INSERT OR REPLACE INTO handoff_packages (
                package_id, created_at, schema_version, trigger, confidence,
                latest_user_request, package_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                package.package_id,
                package.created_at.to_rfc3339(),
                package.schema_version,
                package.trigger,
                package.confidence,
                package.latest_user_request,
                serde_json::to_string(package)?,
            ],
        )?;
        Ok(())
    }

    pub async fn record_handoff_continuation(
        &self,
        record: &HandoffContinuationRecord,
    ) -> anyhow::Result<()> {
        let db = self.db.lock().await;
        db.execute(
            "INSERT INTO handoff_continuations (
                id, timestamp, package_id, provider_id, model_id, success, status_code,
                latency_ms, response_text, error_message
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                record.id,
                record.timestamp.to_rfc3339(),
                record.package_id,
                record.provider_id,
                record.model_id,
                record.success as i32,
                record.status_code,
                record.latency_ms,
                record.response_text,
                record.error_message,
            ],
        )?;
        Ok(())
    }

    pub async fn get_stats(&self) -> anyhow::Result<serde_json::Value> {
        self.get_stats_at(chrono::Local::now().fixed_offset()).await
    }

    async fn get_stats_at(
        &self,
        now: chrono::DateTime<chrono::FixedOffset>,
    ) -> anyhow::Result<serde_json::Value> {
        let db = self.db.lock().await;

        let total_requests: i64 = db
            .query_row("SELECT COUNT(*) FROM telemetry", [], |row| row.get(0))
            .unwrap_or(0);

        let total_success: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM telemetry WHERE success = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let total_tokens: i64 = db
            .query_row(
                "SELECT COALESCE(SUM(request_tokens + response_tokens), 0) FROM telemetry",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let avg_latency: f64 = db
            .query_row(
                "SELECT COALESCE(AVG(latency_ms), 0) FROM telemetry WHERE success = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        let total_cost: f64 = db
            .query_row(
                "SELECT COALESCE(SUM(cost_usd), 0) FROM telemetry",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0.0);
        let today_start = local_period_start_utc(now, now.year(), now.month(), now.day());
        let month_start = local_period_start_utc(now, now.year(), now.month(), 1);
        let today_cost = cost_since(&db, &today_start)?;
        let month_cost = cost_since(&db, &month_start)?;
        let cost_by_tier = cost_breakdown(&db, "agent_tier")?;
        let cost_by_strategy = cost_breakdown(&db, "routing_strategy")?;

        // Provider/model 级别统计
        let mut stmt = db.prepare(
            "SELECT provider_id, COUNT(*) as cnt, AVG(latency_ms) as avg_lat,
             SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) as success_cnt,
             COALESCE(SUM(request_tokens), 0) as request_tokens,
             COALESCE(SUM(response_tokens), 0) as response_tokens,
             COALESCE(SUM(request_tokens + response_tokens), 0) as total_tokens,
             COALESCE(SUM(cost_usd), 0) as total_cost_usd
             FROM telemetry GROUP BY provider_id",
        )?;

        let provider_summaries: Vec<ProviderSummary> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, f64>(7)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut provider_stats = Vec::new();
        for (
            provider_id,
            total_requests,
            avg_latency_ms,
            success_count,
            request_tokens,
            response_tokens,
            total_tokens,
            total_cost_usd,
        ) in provider_summaries
        {
            let mut model_stmt = db.prepare(
                "SELECT model_id, COUNT(*) as cnt, AVG(latency_ms) as avg_lat,
                 SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) as success_cnt,
                 COALESCE(SUM(request_tokens), 0) as request_tokens,
                 COALESCE(SUM(response_tokens), 0) as response_tokens,
                 COALESCE(SUM(request_tokens + response_tokens), 0) as total_tokens,
                 COALESCE(SUM(cost_usd), 0) as total_cost_usd
                 FROM telemetry
                 WHERE provider_id = ?1
                 GROUP BY model_id
                 ORDER BY total_cost_usd DESC, cnt DESC",
            )?;
            let model_stats: Vec<serde_json::Value> = model_stmt
                .query_map(params![provider_id], |row| {
                    Ok(serde_json::json!({
                        "model_id": row.get::<_, String>(0)?,
                        "total_requests": row.get::<_, i64>(1)?,
                        "avg_latency_ms": row.get::<_, f64>(2)?,
                        "success_count": row.get::<_, i64>(3)?,
                        "request_tokens": row.get::<_, i64>(4)?,
                        "response_tokens": row.get::<_, i64>(5)?,
                        "total_tokens": row.get::<_, i64>(6)?,
                        "total_cost_usd": row.get::<_, f64>(7)?,
                    }))
                })?
                .filter_map(|r| r.ok())
                .collect();

            provider_stats.push(serde_json::json!({
                "provider_id": provider_id,
                "total_requests": total_requests,
                "avg_latency_ms": avg_latency_ms,
                "success_count": success_count,
                "request_tokens": request_tokens,
                "response_tokens": response_tokens,
                "total_tokens": total_tokens,
                "total_cost_usd": total_cost_usd,
                "model_stats": model_stats,
            }));
        }

        let latest_codex_route = db
            .query_row(
                "SELECT timestamp, requested_model, model_id, reasoning_effort, speed,
                        agent_tier, reason, success, status_code, latency_ms, error_message
                 FROM codex_routes
                 ORDER BY timestamp DESC, rowid DESC
                 LIMIT 1",
                [],
                |row| {
                    Ok(serde_json::json!({
                        "timestamp": row.get::<_, String>(0)?,
                        "requested_model": row.get::<_, String>(1)?,
                        "model": row.get::<_, String>(2)?,
                        "reasoning_effort": row.get::<_, String>(3)?,
                        "speed": row.get::<_, String>(4)?,
                        "agent_tier": row.get::<_, String>(5)?,
                        "reason": row.get::<_, String>(6)?,
                        "success": row.get::<_, bool>(7)?,
                        "status_code": row.get::<_, Option<i64>>(8)?,
                        "latency_ms": row.get::<_, i64>(9)?,
                        "error_message": row.get::<_, Option<String>>(10)?,
                    }))
                },
            )
            .optional()?;

        let latest_quota_event = db
            .query_row(
                "SELECT timestamp, provider_id, model_id, status_code, retry_after_secs,
                        normalized_headroom, source
                 FROM quota_events
                 ORDER BY timestamp DESC, rowid DESC
                 LIMIT 1",
                [],
                |row| {
                    Ok(serde_json::json!({
                        "timestamp": row.get::<_, String>(0)?,
                        "provider_id": row.get::<_, String>(1)?,
                        "model_id": row.get::<_, String>(2)?,
                        "status_code": row.get::<_, Option<i64>>(3)?,
                        "retry_after_secs": row.get::<_, Option<i64>>(4)?,
                        "normalized_headroom": row.get::<_, Option<f64>>(5)?,
                        "source": row.get::<_, String>(6)?,
                    }))
                },
            )
            .optional()?;

        let latest_handoff = db
            .query_row(
                "SELECT package_json
                 FROM handoff_packages
                 ORDER BY created_at DESC, rowid DESC
                 LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok());

        let latest_handoff_continuation = db
            .query_row(
                "SELECT handoff_continuations.timestamp,
                        handoff_continuations.package_id,
                        handoff_continuations.provider_id,
                        handoff_continuations.model_id,
                        handoff_continuations.success,
                        handoff_continuations.status_code,
                        handoff_continuations.latency_ms,
                        handoff_continuations.response_text,
                        handoff_continuations.error_message,
                        handoff_packages.package_json
                 FROM handoff_continuations
                 LEFT JOIN handoff_packages ON handoff_packages.package_id = handoff_continuations.package_id
                 ORDER BY handoff_continuations.timestamp DESC, handoff_continuations.rowid DESC
                 LIMIT 1",
                [],
                |row| {
                    let package_id = row.get::<_, String>(1)?;
                    let provider_id = row.get::<_, String>(2)?;
                    let model_id = row.get::<_, String>(3)?;
                    let success = row.get::<_, bool>(4)?;
                    let status_code = row.get::<_, Option<i64>>(5)?;
                    let latency_ms = row.get::<_, i64>(6)?;
                    let response_text = row.get::<_, Option<String>>(7)?;
                    let error_message = row.get::<_, Option<String>>(8)?;
                    let package_json = row.get::<_, Option<String>>(9)?;
                    let review_prompt = build_handoff_review_prompt(HandoffReviewPromptInput {
                        package_json: package_json.as_deref(),
                        package_id: &package_id,
                        provider_id: &provider_id,
                        model_id: &model_id,
                        success,
                        status_code,
                        latency_ms,
                        response_text: response_text.as_deref(),
                        error_message: error_message.as_deref(),
                    });
                    Ok(serde_json::json!({
                        "timestamp": row.get::<_, String>(0)?,
                        "package_id": package_id,
                        "provider_id": provider_id,
                        "model_id": model_id,
                        "success": success,
                        "status_code": status_code,
                        "latency_ms": latency_ms,
                        "response_text": response_text,
                        "error_message": error_message,
                        "review_prompt": review_prompt,
                    }))
                },
            )
            .optional()?;

        Ok(serde_json::json!({
            "total_requests": total_requests,
            "total_success": total_success,
            "total_tokens": total_tokens,
            "total_cost_usd": total_cost,
            "cost_summary": {
                "today_usd": today_cost,
                "month_usd": month_cost,
                "total_usd": total_cost,
            },
            "cost_by_tier": cost_by_tier,
            "cost_by_strategy": cost_by_strategy,
            "avg_latency_ms": avg_latency,
            "success_rate": if total_requests > 0 {
                total_success as f64 / total_requests as f64
            } else {
                0.0
            },
            "provider_stats": provider_stats,
            "latest_codex_route": latest_codex_route,
            "latest_quota_event": latest_quota_event,
            "latest_handoff": latest_handoff,
            "latest_handoff_continuation": latest_handoff_continuation,
        }))
    }

    pub async fn get_provider_health(
        &self,
    ) -> anyhow::Result<HashMap<String, ProviderHealthSnapshot>> {
        let db = self.db.lock().await;
        let cutoff = (chrono::Utc::now() - chrono::Duration::hours(24)).to_rfc3339();
        let mut stmt = db.prepare(
            "SELECT provider_id,
                    COUNT(*) AS sample_count,
                    SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) AS success_count,
                    COALESCE(AVG(CASE WHEN success = 1 AND latency_ms > 0 THEN latency_ms END), 0)
             FROM telemetry
             WHERE timestamp >= ?1
             GROUP BY provider_id",
        )?;

        let rows = stmt.query_map(params![cutoff], |row| {
            let provider_id = row.get::<_, String>(0)?;
            let sample_count = row.get::<_, u64>(1)?;
            let success_count = row.get::<_, u64>(2)?;
            let avg_latency_ms = row.get::<_, f64>(3)?.round() as u64;
            Ok(ProviderHealthSnapshot {
                provider_id,
                sample_count,
                success_rate: if sample_count > 0 {
                    success_count as f64 / sample_count as f64
                } else {
                    0.0
                },
                avg_latency_ms,
            })
        })?;

        let mut health = HashMap::new();
        for snapshot in rows {
            let snapshot = snapshot?;
            health.insert(snapshot.provider_id.clone(), snapshot);
        }
        Ok(health)
    }
}

fn ensure_telemetry_agent_tier_column(conn: &Connection) -> anyhow::Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(telemetry)")?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "agent_tier" {
            return Ok(());
        }
    }
    conn.execute(
        "ALTER TABLE telemetry ADD COLUMN agent_tier TEXT NOT NULL DEFAULT ''",
        [],
    )?;
    Ok(())
}

fn local_period_start_utc(
    now: chrono::DateTime<chrono::FixedOffset>,
    year: i32,
    month: u32,
    day: u32,
) -> String {
    use chrono::TimeZone;

    now.offset()
        .with_ymd_and_hms(year, month, day, 0, 0, 0)
        .single()
        .expect("valid local period boundary")
        .with_timezone(&chrono::Utc)
        .to_rfc3339()
}

fn cost_since(conn: &Connection, start: &str) -> anyhow::Result<f64> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(cost_usd), 0) FROM telemetry WHERE timestamp >= ?1",
        params![start],
        |row| row.get(0),
    )?)
}

fn cost_breakdown(conn: &Connection, column: &str) -> anyhow::Result<Vec<serde_json::Value>> {
    let key = match column {
        "agent_tier" => "agent_tier",
        "routing_strategy" => "routing_strategy",
        _ => anyhow::bail!("unsupported cost breakdown column"),
    };
    let sql = format!(
        "SELECT {key}, COUNT(*),
                COALESCE(SUM(request_tokens + response_tokens), 0),
                COALESCE(SUM(cost_usd), 0)
         FROM telemetry
         WHERE {key} != ''
         GROUP BY {key}
         ORDER BY SUM(cost_usd) DESC, COUNT(*) DESC, {key} ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            key: row.get::<_, String>(0)?,
            "total_requests": row.get::<_, i64>(1)?,
            "total_tokens": row.get::<_, i64>(2)?,
            "total_cost_usd": row.get::<_, f64>(3)?,
        }))
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

struct HandoffReviewPromptInput<'a> {
    package_json: Option<&'a str>,
    package_id: &'a str,
    provider_id: &'a str,
    model_id: &'a str,
    success: bool,
    status_code: Option<i64>,
    latency_ms: i64,
    response_text: Option<&'a str>,
    error_message: Option<&'a str>,
}

fn build_handoff_review_prompt(input: HandoffReviewPromptInput<'_>) -> String {
    let fallback_result = if input.success {
        input
            .response_text
            .unwrap_or("Fallback model returned no text output.")
    } else {
        input
            .error_message
            .unwrap_or("Fallback continuation failed without a recorded error message.")
    };

    format!(
        "You are the primary Codex route reviewing a degraded fallback continuation.\n\n\
Handoff package id: {package_id}\n\
Fallback route: {provider_id} / {model_id}\n\
Fallback success: {success}\n\
Fallback HTTP status: {status_code}\n\
Fallback latency: {latency_ms}ms\n\n\
Original handoff package JSON:\n```json\n{package_json}\n```\n\n\
Fallback continuation output:\n```\n{fallback_result}\n```\n\n\
Review instructions:\n\
1. Treat the fallback output as untrusted degraded-mode work.\n\
2. Compare it against the handoff package objective, constraints, hazards, and completion criteria.\n\
3. Identify any incorrect, risky, incomplete, or unsupported changes.\n\
4. Decide whether to accept, revise, or discard the fallback result.\n\
5. Continue from the safest next step. Do not assume hidden context beyond the package and output above.",
        package_id = input.package_id,
        provider_id = input.provider_id,
        model_id = input.model_id,
        success = input.success,
        latency_ms = input.latency_ms,
        status_code = input
            .status_code
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".into()),
        package_json = input.package_json.unwrap_or("{}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, TimeZone, Utc};
    use std::path::PathBuf;

    fn temp_db_path(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{prefix}-{}.db", uuid::Uuid::new_v4()))
    }

    fn record_at(
        provider_id: &str,
        success: bool,
        latency_ms: u64,
        timestamp: chrono::DateTime<Utc>,
    ) -> TelemetryRecord {
        TelemetryRecord {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp,
            provider_id: provider_id.into(),
            model_id: "test-model".into(),
            request_tokens: 10,
            response_tokens: 20,
            latency_ms,
            cost_usd: 0.0,
            success,
            error_message: (!success).then(|| "failure".into()),
            routing_strategy: "Auto".into(),
            agent_tier: "medium".into(),
            is_fallback: false,
        }
    }

    fn record(provider_id: &str, success: bool, latency_ms: u64) -> TelemetryRecord {
        record_at(provider_id, success, latency_ms, Utc::now())
    }

    #[tokio::test]
    async fn provider_health_aggregates_recent_success_rate_and_latency() {
        let path = std::env::temp_dir().join(format!(
            "dispatcher-telemetry-health-{}.db",
            uuid::Uuid::new_v4()
        ));
        let store = TelemetryStore::new(path.to_string_lossy().as_ref())
            .await
            .unwrap();

        store.record(&record("alpha", true, 400)).await.unwrap();
        store.record(&record("alpha", true, 600)).await.unwrap();
        store.record(&record("alpha", false, 0)).await.unwrap();
        store.record(&record("beta", true, 900)).await.unwrap();

        let health = store.get_provider_health().await.unwrap();
        let alpha = health.get("alpha").unwrap();

        assert_eq!(alpha.sample_count, 3);
        assert!((alpha.success_rate - (2.0 / 3.0)).abs() < 0.000001);
        assert_eq!(alpha.avg_latency_ms, 500);

        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn provider_health_ignores_records_older_than_24_hours() {
        let path = std::env::temp_dir().join(format!(
            "dispatcher-telemetry-window-{}.db",
            uuid::Uuid::new_v4()
        ));
        let store = TelemetryStore::new(path.to_string_lossy().as_ref())
            .await
            .unwrap();

        store
            .record(&record_at(
                "alpha",
                false,
                0,
                Utc::now() - chrono::Duration::hours(25),
            ))
            .await
            .unwrap();
        store.record(&record("alpha", true, 500)).await.unwrap();

        let health = store.get_provider_health().await.unwrap();
        let alpha = health.get("alpha").unwrap();

        assert_eq!(alpha.sample_count, 1);
        assert_eq!(alpha.success_rate, 1.0);

        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn telemetry_stats_include_latest_codex_route_decision() {
        let path = std::env::temp_dir().join(format!(
            "dispatcher-codex-telemetry-{}.db",
            uuid::Uuid::new_v4()
        ));
        let store = TelemetryStore::new(path.to_string_lossy().as_ref())
            .await
            .unwrap();

        store
            .record_codex_route(&CodexTelemetryRecord {
                id: uuid::Uuid::new_v4().to_string(),
                timestamp: Utc::now(),
                requested_model: "auto".into(),
                model_id: "gpt-5.5".into(),
                reasoning_effort: "high".into(),
                speed: "priority".into(),
                agent_tier: "reasoning".into(),
                reason: "Reasoning task -> gpt-5.5 with high reasoning and priority speed".into(),
                success: true,
                status_code: Some(200),
                latency_ms: 321,
                error_message: None,
            })
            .await
            .unwrap();

        let stats = store.get_stats().await.unwrap();
        let latest = &stats["latest_codex_route"];

        assert_eq!(latest["requested_model"], "auto");
        assert_eq!(latest["model"], "gpt-5.5");
        assert_eq!(latest["reasoning_effort"], "high");
        assert_eq!(latest["speed"], "priority");
        assert_eq!(latest["agent_tier"], "reasoning");
        assert_eq!(latest["success"], true);
        assert_eq!(latest["status_code"], 200);
        assert_eq!(latest["latency_ms"], 321);

        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn telemetry_stats_include_latest_handoff_package() {
        let db_path = temp_db_path("dispatcher-handoff-telemetry");
        let store = TelemetryStore::new(db_path.to_string_lossy().as_ref())
            .await
            .unwrap();
        let package = crate::handoff::EmergencyHandoffInput {
            requested_model: "gpt-5.5".into(),
            selected_model: "gpt-5.5".into(),
            reasoning_effort: "xhigh".into(),
            speed: "priority".into(),
            agent_tier: "complex".into(),
            dispatcher_mode: "auto".into(),
            latest_user_request: "Finish the implementation".into(),
            cwd: "/workspace/dispatcher".into(),
            error_message: "Codex upstream returned HTTP 429 Too Many Requests".into(),
            signal: crate::handoff::QuotaSignal {
                is_emergency: true,
                status_code: Some(429),
                retry_after_secs: Some(120),
                normalized_headroom: None,
                source: "http_429".into(),
            },
        }
        .build();

        store.record_handoff_package(&package).await.unwrap();

        let stats = store.get_stats().await.unwrap();
        let handoff = &stats["latest_handoff"];
        assert_eq!(handoff["schema_version"], "dispatcher_handoff.v1");
        assert_eq!(handoff["trigger"], "rate_limit_429");
        assert_eq!(handoff["confidence"], "emergency_reconstruction");
        assert_eq!(handoff["latest_user_request"], "Finish the implementation");

        drop(store);
        std::fs::remove_file(db_path).unwrap();
    }

    #[tokio::test]
    async fn telemetry_stats_include_latest_quota_event() {
        let db_path = temp_db_path("dispatcher-quota-telemetry");
        let store = TelemetryStore::new(db_path.to_string_lossy().as_ref())
            .await
            .unwrap();
        let event = QuotaEventRecord {
            id: "quota_test".into(),
            timestamp: chrono::Utc::now(),
            provider_id: "codex".into(),
            model_id: "gpt-5.5".into(),
            status_code: Some(429),
            retry_after_secs: Some(120),
            normalized_headroom: None,
            source: "http_429".into(),
        };

        store.record_quota_event(&event).await.unwrap();

        let stats = store.get_stats().await.unwrap();
        assert_eq!(stats["latest_quota_event"]["provider_id"], "codex");
        assert_eq!(stats["latest_quota_event"]["status_code"], 429);
        assert_eq!(stats["latest_quota_event"]["retry_after_secs"], 120);

        drop(store);
        std::fs::remove_file(db_path).unwrap();
    }

    #[tokio::test]
    async fn telemetry_stats_include_latest_handoff_continuation() {
        let db_path = temp_db_path("dispatcher-handoff-continuation");
        let store = TelemetryStore::new(db_path.to_string_lossy().as_ref())
            .await
            .unwrap();
        let record = HandoffContinuationRecord {
            id: "handoff_continuation_test".into(),
            timestamp: chrono::Utc::now(),
            package_id: "handoff_123".into(),
            provider_id: "deepseek".into(),
            model_id: "deepseek-reasoner".into(),
            success: true,
            status_code: Some(200),
            latency_ms: 456,
            response_text: Some("Implemented the delegated task.".into()),
            error_message: None,
        };

        store.record_handoff_continuation(&record).await.unwrap();

        let stats = store.get_stats().await.unwrap();
        let continuation = &stats["latest_handoff_continuation"];
        assert_eq!(continuation["package_id"], "handoff_123");
        assert_eq!(continuation["provider_id"], "deepseek");
        assert_eq!(continuation["model_id"], "deepseek-reasoner");
        assert_eq!(continuation["success"], true);
        assert_eq!(continuation["status_code"], 200);
        assert_eq!(continuation["latency_ms"], 456);
        assert_eq!(
            continuation["response_text"],
            "Implemented the delegated task."
        );
        assert!(continuation["review_prompt"]
            .as_str()
            .unwrap()
            .contains("reviewing a degraded fallback continuation"));
        assert!(continuation["review_prompt"]
            .as_str()
            .unwrap()
            .contains("deepseek / deepseek-reasoner"));

        drop(store);
        std::fs::remove_file(db_path).unwrap();
    }

    #[tokio::test]
    async fn telemetry_stats_include_provider_model_cost_breakdown() {
        let path = std::env::temp_dir().join(format!(
            "dispatcher-cost-breakdown-{}.db",
            uuid::Uuid::new_v4()
        ));
        let store = TelemetryStore::new(path.to_string_lossy().as_ref())
            .await
            .unwrap();

        store
            .record(&TelemetryRecord {
                id: uuid::Uuid::new_v4().to_string(),
                timestamp: Utc::now(),
                provider_id: "deepseek".into(),
                model_id: "deepseek-v4-flash".into(),
                request_tokens: 100,
                response_tokens: 50,
                latency_ms: 200,
                cost_usd: 0.001,
                success: true,
                error_message: None,
                routing_strategy: "Auto".into(),
                agent_tier: "simple".into(),
                is_fallback: false,
            })
            .await
            .unwrap();
        store
            .record(&TelemetryRecord {
                id: uuid::Uuid::new_v4().to_string(),
                timestamp: Utc::now(),
                provider_id: "deepseek".into(),
                model_id: "deepseek-v4-pro".into(),
                request_tokens: 200,
                response_tokens: 100,
                latency_ms: 400,
                cost_usd: 0.004,
                success: true,
                error_message: None,
                routing_strategy: "Auto".into(),
                agent_tier: "reasoning".into(),
                is_fallback: false,
            })
            .await
            .unwrap();
        store
            .record(&TelemetryRecord {
                id: uuid::Uuid::new_v4().to_string(),
                timestamp: Utc::now(),
                provider_id: "deepseek".into(),
                model_id: "deepseek-v4-flash".into(),
                request_tokens: 40,
                response_tokens: 10,
                latency_ms: 0,
                cost_usd: 0.0005,
                success: false,
                error_message: Some("failure".into()),
                routing_strategy: "Auto".into(),
                agent_tier: "simple".into(),
                is_fallback: false,
            })
            .await
            .unwrap();

        let stats = store.get_stats().await.unwrap();
        let provider = stats["provider_stats"]
            .as_array()
            .unwrap()
            .iter()
            .find(|stat| stat["provider_id"] == "deepseek")
            .unwrap();
        let models = provider["model_stats"].as_array().unwrap();
        let flash = models
            .iter()
            .find(|stat| stat["model_id"] == "deepseek-v4-flash")
            .unwrap();
        let pro = models
            .iter()
            .find(|stat| stat["model_id"] == "deepseek-v4-pro")
            .unwrap();

        assert_eq!(provider["total_tokens"], 500);
        assert_eq!(provider["total_cost_usd"], 0.0055);
        assert_eq!(flash["total_requests"], 2);
        assert_eq!(flash["success_count"], 1);
        assert_eq!(flash["request_tokens"], 140);
        assert_eq!(flash["response_tokens"], 60);
        assert_eq!(flash["total_cost_usd"], 0.0015);
        assert_eq!(pro["total_requests"], 1);
        assert_eq!(pro["total_cost_usd"], 0.004);

        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn telemetry_store_migrates_legacy_database_with_agent_tier_column() {
        let path = std::env::temp_dir().join(format!(
            "dispatcher-legacy-telemetry-{}.db",
            uuid::Uuid::new_v4()
        ));
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE telemetry (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                request_tokens INTEGER NOT NULL DEFAULT 0,
                response_tokens INTEGER NOT NULL DEFAULT 0,
                latency_ms INTEGER NOT NULL DEFAULT 0,
                cost_usd REAL NOT NULL DEFAULT 0.0,
                success INTEGER NOT NULL DEFAULT 1,
                error_message TEXT,
                routing_strategy TEXT NOT NULL DEFAULT '',
                is_fallback INTEGER NOT NULL DEFAULT 0
            );",
        )
        .unwrap();
        drop(conn);

        let store = TelemetryStore::new(path.to_string_lossy().as_ref())
            .await
            .unwrap();
        store.record(&record("alpha", true, 120)).await.unwrap();

        let db = store.db.lock().await;
        let tier: String = db
            .query_row(
                "SELECT agent_tier FROM telemetry WHERE provider_id = 'alpha'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tier, "medium");
        drop(db);

        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn telemetry_stats_include_period_and_routing_cost_breakdowns() {
        let path = std::env::temp_dir().join(format!(
            "dispatcher-period-costs-{}.db",
            uuid::Uuid::new_v4()
        ));
        let store = TelemetryStore::new(path.to_string_lossy().as_ref())
            .await
            .unwrap();
        let offset = FixedOffset::east_opt(8 * 60 * 60).unwrap();
        let now = offset.with_ymd_and_hms(2026, 6, 11, 12, 0, 0).unwrap();

        for (timestamp, cost, strategy, tier) in [
            (
                offset.with_ymd_and_hms(2026, 6, 11, 8, 0, 0).unwrap(),
                0.01,
                "Auto",
                "simple",
            ),
            (
                offset.with_ymd_and_hms(2026, 6, 3, 8, 0, 0).unwrap(),
                0.02,
                "Save",
                "reasoning",
            ),
            (
                offset.with_ymd_and_hms(2026, 5, 31, 23, 0, 0).unwrap(),
                0.04,
                "Auto",
                "simple",
            ),
        ] {
            store
                .record(&TelemetryRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    timestamp: timestamp.with_timezone(&Utc),
                    provider_id: "alpha".into(),
                    model_id: "alpha-model".into(),
                    request_tokens: 100,
                    response_tokens: 50,
                    latency_ms: 100,
                    cost_usd: cost,
                    success: true,
                    error_message: None,
                    routing_strategy: strategy.into(),
                    agent_tier: tier.into(),
                    is_fallback: false,
                })
                .await
                .unwrap();
        }

        let stats = store.get_stats_at(now).await.unwrap();

        assert_eq!(stats["cost_summary"]["today_usd"], 0.01);
        assert_eq!(stats["cost_summary"]["month_usd"], 0.03);
        assert_eq!(stats["cost_summary"]["total_usd"], 0.07);
        assert_eq!(stats["cost_by_tier"][0]["agent_tier"], "simple");
        assert_eq!(stats["cost_by_tier"][0]["total_requests"], 2);
        assert_eq!(stats["cost_by_tier"][0]["total_cost_usd"], 0.05);
        assert_eq!(stats["cost_by_strategy"][0]["routing_strategy"], "Auto");
        assert_eq!(stats["cost_by_strategy"][0]["total_cost_usd"], 0.05);

        drop(store);
        std::fs::remove_file(path).unwrap();
    }
}
