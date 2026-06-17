//! Statistics recorder for tokenless.
//!
//! Provides SQLite-based storage for compression and rewriting metrics.

use std::{fmt, path::Path, str::FromStr, sync::Mutex};

use chrono::DateTime;
use rusqlite::Connection;
use secrecy::{ExposeSecret, SecretBox};

use crate::record::{OperationType, StatsRecord};

/// Result type for stats operations.
pub type StatsResult<T> = Result<T, StatsError>;

/// Error types for stats operations.
#[derive(Debug, thiserror::Error)]
pub enum StatsError {
    /// A database operation failed.
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    /// An I/O operation failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Full column list for SELECT queries on the `stats` table.
///
/// Shared by [`all_records`], [`records_filtered`], [`records_since`],
/// [`records_since_filtered`].
const ALL_COLS: &str = "id, timestamp, operation, agent_id, source_pid, session_id, \
                        tool_use_id, project, namespace, experimental_mode,
         before_chars, before_tokens, after_chars, after_tokens,
         before_text, after_text, before_output, after_output";

/// Maximum number of bytes retained for non-sensitive stats text.
const MAX_REDACTED_TEXT_BYTES: usize = 16 * 1024;

/// Secret-wrapped stats text retained only after redaction.
pub type RedactedText = SecretBox<str>;

/// Result of redacting a stats text payload.
#[derive(Clone)]
pub struct RedactionOutcome {
    text: Option<RedactedText>,
    modified: bool,
    blocked: bool,
}

impl fmt::Debug for RedactionOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedactionOutcome")
            .field("text", &self.text.as_ref().map(|_| "[redacted]"))
            .field("modified", &self.modified)
            .field("blocked", &self.blocked)
            .finish()
    }
}

impl RedactionOutcome {
    #[must_use]
    fn allow(text: String, modified: bool) -> Self {
        Self {
            text: Some(SecretBox::from(text.into_boxed_str())),
            modified,
            blocked: false,
        }
    }

    #[must_use]
    fn block() -> Self {
        Self {
            text: None,
            modified: false,
            blocked: true,
        }
    }

    /// Return the redacted text as a plain string for database insertion.
    #[must_use]
    pub fn into_plain_text(self) -> Option<String> {
        self.text.map(|text| text.expose_secret().to_owned())
    }

    /// Return whether the text was modified during redaction.
    #[must_use]
    pub fn was_modified(&self) -> bool {
        self.modified
    }

    /// Return whether the text was blocked entirely.
    #[must_use]
    pub fn was_blocked(&self) -> bool {
        self.blocked
    }
}

/// Statistics recorder that stores metrics in a SQLite database.
///
/// Manual `Debug` is provided because `rusqlite::Connection` does not implement it.
pub struct StatsRecorder {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for StatsRecorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StatsRecorder")
            .field("conn", &"Mutex<Connection>")
            .finish()
    }
}

impl StatsRecorder {
    /// Lock the mutex-protected connection, recovering from poison if needed.
    fn lock_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| {
            self.conn.clear_poison();
            e.into_inner()
        })
    }

    /// Create a new recorder with the database at `db_path`.
    ///
    /// Creates the database file and tables if they do not exist.
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the database cannot be opened or
    /// the schema cannot be created.
    pub fn new<P: AsRef<Path>>(db_path: P) -> StatsResult<Self> {
        let conn = Connection::open(db_path)?;

        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA busy_timeout=5000;
            PRAGMA synchronous=NORMAL;
        ",
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS stats (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                operation TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                source_pid INTEGER,
                session_id TEXT,
                tool_use_id TEXT,
                project TEXT,
                namespace TEXT,
                experimental_mode INTEGER NOT NULL DEFAULT 1,
                before_chars INTEGER NOT NULL,
                before_tokens INTEGER NOT NULL,
                after_chars INTEGER NOT NULL,
                after_tokens INTEGER NOT NULL,
                before_text TEXT,
                after_text TEXT,
                before_output TEXT,
                after_output TEXT
            )",
            [],
        )?;

        let indexes = [
            "CREATE INDEX IF NOT EXISTS idx_timestamp ON stats(timestamp)",
            "CREATE INDEX IF NOT EXISTS idx_operation ON stats(operation)",
            "CREATE INDEX IF NOT EXISTS idx_agent_id ON stats(agent_id)",
            "CREATE INDEX IF NOT EXISTS idx_session_id ON stats(session_id)",
            "CREATE INDEX IF NOT EXISTS idx_project ON stats(project)",
        ];
        for idx in &indexes {
            conn.execute(idx, [])?;
        }

        // Schema migration: add columns introduced in v0.3.0 if missing
        for col in &["before_output", "after_output"] {
            let sql = format!("ALTER TABLE stats ADD COLUMN {col} TEXT");
            if let Err(e) = conn.execute(&sql, []) {
                if !e.to_string().contains("duplicate column name") {
                    return Err(StatsError::Database(e));
                }
            }
        }

        // Schema migration: add project/namespace columns (v0.4.0+)
        for col in &["project", "namespace"] {
            let sql = format!("ALTER TABLE stats ADD COLUMN {col} TEXT");
            if let Err(e) = conn.execute(&sql, []) {
                if !e.to_string().contains("duplicate column name") {
                    return Err(StatsError::Database(e));
                }
            }
        }

        // Schema migration: add experimental_mode column (v0.4.0+)
        if let Err(e) = conn.execute(
            "ALTER TABLE stats ADD COLUMN experimental_mode INTEGER NOT NULL DEFAULT 1",
            [],
        ) {
            if !e.to_string().contains("duplicate column name") {
                return Err(StatsError::Database(e));
            }
        }

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Record a statistics entry.
    ///
    /// Returns the new row ID on success.
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the insert fails.
    #[allow(clippy::too_many_lines)]
    pub fn record(&self, stats_record: &StatsRecord) -> StatsResult<i64> {
        let conn = self.lock_conn();

        let before_text =
            sanitize_stats_text_option(stats_record.before_text.as_deref(), "before_text");
        let after_text =
            sanitize_stats_text_option(stats_record.after_text.as_deref(), "after_text");
        let before_output =
            sanitize_stats_text_option(stats_record.before_output.as_deref(), "before_output");
        let after_output =
            sanitize_stats_text_option(stats_record.after_output.as_deref(), "after_output");

        conn.execute(
            "INSERT INTO stats (
                timestamp, operation, agent_id, source_pid, session_id, tool_use_id,
                project, namespace, experimental_mode,
                before_chars, before_tokens, after_chars, after_tokens,
                before_text, after_text,
                before_output, after_output
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            rusqlite::params![
                stats_record.timestamp.to_rfc3339(),
                stats_record.operation.as_str(),
                stats_record.agent_id,
                stats_record.source_pid,
                stats_record.session_id,
                stats_record.tool_use_id,
                stats_record.project,
                stats_record.namespace,
                i64::from(stats_record.experimental_mode),
                stats_record.before_chars,
                stats_record.before_tokens,
                stats_record.after_chars,
                stats_record.after_tokens,
                before_text.into_plain_text(),
                after_text.into_plain_text(),
                before_output.into_plain_text(),
                after_output.into_plain_text(),
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Query all records, newest first, with an optional limit.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the query fails.
    pub fn all_records(&self, limit: Option<usize>) -> StatsResult<Vec<StatsRecord>> {
        let conn = self.lock_conn();

        let records = match limit {
            Some(n) => {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {ALL_COLS} FROM stats ORDER BY timestamp DESC LIMIT ?1"
                ))?;
                let rows = stmt.query_map([n as i64], Self::row_to_record)?;
                rows.filter_map(|r| r.ok()).collect()
            }
            None => {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {ALL_COLS} FROM stats ORDER BY timestamp DESC"
                ))?;
                let rows = stmt.query_map([], Self::row_to_record)?;
                rows.filter_map(|r| r.ok()).collect()
            }
        };

        Ok(records)
    }

    /// Get a single record by database ID.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the query fails.
    pub fn record_by_id(&self, id: i64) -> StatsResult<Option<StatsRecord>> {
        let conn = self.lock_conn();

        let mut stmt = conn.prepare(
            "SELECT id, timestamp, operation, agent_id, source_pid, session_id, tool_use_id,
                    project, namespace, experimental_mode,
                    before_chars, before_tokens, after_chars, after_tokens,
                    before_text, after_text, before_output, after_output
             FROM stats WHERE id = ?1",
        )?;

        let mut rows = stmt.query_map([id], Self::row_to_record)?;
        Ok(rows.next().and_then(|r| r.ok()))
    }

    /// Get the total record count.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the query fails.
    pub fn count(&self) -> StatsResult<usize> {
        let conn = self.lock_conn();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM stats", [], |row| row.get(0))?;
        Ok(usize::try_from(count).unwrap_or(0))
    }

    /// Clear all records and reset the auto-increment counter.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the delete fails.
    pub fn clear(&self) -> StatsResult<()> {
        let conn = self.lock_conn();

        conn.execute_batch("DELETE FROM stats; DELETE FROM sqlite_sequence WHERE name='stats';")?;
        Ok(())
    }

    /// Query records with optional filters.
    ///
    /// Supports filtering by exact `agent_id`, `project`, `namespace`, text
    /// search across `agent_id` and `operation` columns, and an optional `limit`.
    /// Returns records ordered by timestamp descending.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the query fails.
    pub fn records_filtered(
        &self,
        agent_id: Option<&str>,
        search: Option<&str>,
        project: Option<&str>,
        namespace: Option<&str>,
        limit: Option<usize>,
    ) -> StatsResult<Vec<StatsRecord>> {
        let conn = self.lock_conn();

        let mut sql = format!("SELECT {ALL_COLS} FROM stats WHERE 1=1");
        let mut params: Vec<rusqlite::types::Value> = Vec::new();

        if let Some(aid) = agent_id {
            sql.push_str(" AND agent_id = ?");
            params.push(rusqlite::types::Value::Text(aid.to_string()));
        }

        if let Some(pat) = search {
            sql.push_str(" AND (agent_id LIKE ? OR operation LIKE ?)");
            let like = rusqlite::types::Value::Text(format!("%{pat}%"));
            params.push(like.clone());
            params.push(like);
        }

        if let Some(proj) = project {
            sql.push_str(" AND project = ?");
            params.push(rusqlite::types::Value::Text(proj.to_string()));
        }

        if let Some(ns) = namespace {
            sql.push_str(" AND namespace = ?");
            params.push(rusqlite::types::Value::Text(ns.to_string()));
        }

        sql.push_str(" ORDER BY timestamp DESC");

        if let Some(n) = limit {
            sql.push_str(" LIMIT ?");
            params.push(rusqlite::types::Value::Integer(n as i64));
        }

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
            .iter()
            .map(|v| v as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt.query_map(param_refs.as_slice(), Self::row_to_record)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get the list of all distinct agent IDs, sorted alphabetically.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the query fails.
    pub fn all_agents(&self) -> StatsResult<Vec<String>> {
        let conn = self.lock_conn();

        let mut stmt = conn.prepare("SELECT DISTINCT agent_id FROM stats ORDER BY agent_id")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let agents: Vec<String> = rows.filter_map(|r| r.ok()).collect();
        Ok(agents)
    }

    /// Get the list of all distinct project names, sorted alphabetically.
    /// Entries with `NULL` project are excluded.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the query fails.
    pub fn all_projects(&self) -> StatsResult<Vec<String>> {
        let conn = self.lock_conn();

        let mut stmt = conn.prepare(
            "SELECT DISTINCT project FROM stats WHERE project IS NOT NULL ORDER BY project",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let projects: Vec<String> = rows.filter_map(|r| r.ok()).collect();
        Ok(projects)
    }

    /// Query records within a time range, newest first.
    ///
    /// `since` and `until` should be RFC 3339 strings (e.g., from
    /// `DateTime::to_rfc3339()`). Records with `timestamp >= since` and
    /// `timestamp <= until` are included.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the query fails.
    pub fn records_since(
        &self,
        since: Option<&str>,
        until: Option<&str>,
    ) -> StatsResult<Vec<StatsRecord>> {
        let conn = self.lock_conn();

        let mut sql = format!("SELECT {ALL_COLS} FROM stats WHERE 1=1");
        let mut params: Vec<rusqlite::types::Value> = Vec::new();

        if let Some(s) = since {
            sql.push_str(" AND timestamp >= ?");
            params.push(rusqlite::types::Value::Text(s.to_string()));
        }
        if let Some(u) = until {
            sql.push_str(" AND timestamp <= ?");
            params.push(rusqlite::types::Value::Text(u.to_string()));
        }

        sql.push_str(" ORDER BY timestamp DESC");

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
            .iter()
            .map(|v| v as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt.query_map(param_refs.as_slice(), Self::row_to_record)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get aggregated summary statistics for a specific agent.
    ///
    /// Returns a single [`AgentSummaryRow`] with summed counts. If the agent
    /// has no records, all totals are zero.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the query fails.
    pub fn agent_summary(&self, agent_id: &str) -> StatsResult<AgentSummaryRow> {
        let conn = self.lock_conn();

        let row = conn.query_row(
            "SELECT ?1, COUNT(*), COALESCE(SUM(before_chars), 0), COALESCE(SUM(after_chars), 0), \
             COALESCE(SUM(before_tokens), 0), COALESCE(SUM(after_tokens), 0) FROM stats WHERE \
             agent_id = ?1",
            [agent_id],
            |row| {
                Ok(AgentSummaryRow {
                    agent_id: row.get(0)?,
                    record_count: usize::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                    total_before_chars: usize::try_from(row.get::<_, i64>(2)?).unwrap_or(0),
                    total_after_chars: usize::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
                    total_before_tokens: usize::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
                    total_after_tokens: usize::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
                })
            },
        )?;

        Ok(row)
    }

    /// Query all records with optional project filter and limit.
    ///
    /// Convenience wrapper around [`Self::records_filtered`] that only exposes
    /// `project` and `limit` filters.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the query fails.
    pub fn all_records_filtered(
        &self,
        project: Option<&str>,
        limit: Option<usize>,
    ) -> StatsResult<Vec<StatsRecord>> {
        self.records_filtered(None, None, project, None, limit)
    }

    /// Query records within a time range and optional project filter, newest
    /// first.
    ///
    /// `since` and `until` should be RFC 3339 strings. Records with
    /// `timestamp >= since` and `timestamp <= until` are included.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the query fails.
    pub fn records_since_filtered(
        &self,
        since: Option<&str>,
        until: Option<&str>,
        project: Option<&str>,
    ) -> StatsResult<Vec<StatsRecord>> {
        let conn = self.lock_conn();

        let mut sql = format!("SELECT {ALL_COLS} FROM stats WHERE 1=1");
        let mut params: Vec<rusqlite::types::Value> = Vec::new();

        if let Some(s) = since {
            sql.push_str(" AND timestamp >= ?");
            params.push(rusqlite::types::Value::Text(s.to_string()));
        }
        if let Some(u) = until {
            sql.push_str(" AND timestamp <= ?");
            params.push(rusqlite::types::Value::Text(u.to_string()));
        }
        if let Some(p) = project {
            sql.push_str(" AND project = ?");
            params.push(rusqlite::types::Value::Text(p.to_string()));
        }

        sql.push_str(" ORDER BY timestamp DESC");

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
            .iter()
            .map(|v| v as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt.query_map(param_refs.as_slice(), Self::row_to_record)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get aggregated summary statistics for a specific project.
    ///
    /// Returns a single [`ProjectSummaryRow`] with summed counts. If the
    /// project has no records, all totals are zero (with the requested
    /// project name filled in).
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the query fails.
    pub fn project_summary(&self, project: &str) -> StatsResult<ProjectSummaryRow> {
        let conn = self.lock_conn();

        let row = conn.query_row(
            "SELECT ?1, COUNT(*), COALESCE(SUM(before_chars), 0), COALESCE(SUM(after_chars), 0), \
             COALESCE(SUM(before_tokens), 0), COALESCE(SUM(after_tokens), 0) FROM stats WHERE \
             project = ?1",
            [project],
            Self::row_to_project_summary,
        )?;

        Ok(row)
    }

    /// Get aggregated summary statistics for all projects (excluding NULL
    /// project records).
    ///
    /// Results are ordered alphabetically by project name.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the query fails.
    pub fn projects_summary(&self) -> StatsResult<Vec<ProjectSummaryRow>> {
        let conn = self.lock_conn();

        let mut stmt = conn.prepare(
            "SELECT project, COUNT(*), COALESCE(SUM(before_chars), 0), \
             COALESCE(SUM(after_chars), 0), COALESCE(SUM(before_tokens), 0), \
             COALESCE(SUM(after_tokens), 0) FROM stats WHERE project IS NOT NULL \
             GROUP BY project ORDER BY project",
        )?;

        let rows = stmt.query_map([], Self::row_to_project_summary)?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get daily trend data points for a specific project or all projects.
    ///
    /// Groups records by `date(timestamp)` and aggregates
    /// `chars_saved = SUM(before_chars - after_chars)` and
    /// `tokens_saved = SUM(before_tokens - after_tokens)`.
    ///
    /// When `project` is `None`, includes all records (including NULL-
    /// project records). Optional `since` and `until` parameters filter by
    /// timestamp range.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the query fails.
    pub fn project_daily_trends(
        &self,
        project: Option<&str>,
        since: Option<&str>,
        until: Option<&str>,
    ) -> StatsResult<Vec<ProjectDaily>> {
        let conn = self.lock_conn();

        let mut sql = String::from(
            "SELECT date(timestamp), \
             COALESCE(SUM(before_chars - after_chars), 0), \
             COALESCE(SUM(before_tokens - after_tokens), 0), \
             COUNT(*) FROM stats WHERE 1=1",
        );
        let mut params: Vec<rusqlite::types::Value> = Vec::new();

        if let Some(p) = project {
            sql.push_str(" AND project = ?");
            params.push(rusqlite::types::Value::Text(p.to_string()));
        }
        if let Some(s) = since {
            sql.push_str(" AND timestamp >= ?");
            params.push(rusqlite::types::Value::Text(s.to_string()));
        }
        if let Some(u) = until {
            sql.push_str(" AND timestamp <= ?");
            params.push(rusqlite::types::Value::Text(u.to_string()));
        }

        sql.push_str(" GROUP BY date(timestamp) ORDER BY date(timestamp)");

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
            .iter()
            .map(|v| v as &dyn rusqlite::types::ToSql)
            .collect();
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(ProjectDaily {
                date: row.get(0)?,
                chars_saved: u64::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                tokens_saved: u64::try_from(row.get::<_, i64>(2)?).unwrap_or(0),
                record_count: usize::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
            })
        })?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Delete a single record by database ID.
    ///
    /// Returns `true` if a matching record was found and deleted.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the delete fails.
    pub fn delete_by_id(&self, id: i64) -> StatsResult<bool> {
        let conn = self.lock_conn();
        let affected = conn.execute("DELETE FROM stats WHERE id = ?1", [id])?;
        Ok(affected > 0)
    }

    /// Delete all records for a given agent ID.
    ///
    /// Returns the number of deleted records.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the delete fails.
    pub fn delete_by_agent(&self, agent_id: &str) -> StatsResult<usize> {
        let conn = self.lock_conn();
        let affected = conn.execute("DELETE FROM stats WHERE agent_id = ?1", [agent_id])?;
        Ok(affected)
    }

    /// Delete all records with timestamps before the given date.
    ///
    /// `date` should be an ISO 8601 date string (e.g. `"2026-05-01"`).
    /// Returns the number of deleted records.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the delete fails.
    pub fn delete_before(&self, date: &str) -> StatsResult<usize> {
        let conn = self.lock_conn();
        let affected = conn.execute("DELETE FROM stats WHERE timestamp < ?1", [date])?;
        Ok(affected)
    }

    /// Run SQLite `VACUUM` to reclaim disk space after large deletes.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if the vacuum fails.
    pub fn vacuum(&self) -> StatsResult<()> {
        let conn = self.lock_conn();
        conn.execute_batch("VACUUM")?;
        Ok(())
    }

    /// Export all records to a JSON file.
    ///
    /// Returns the number of exported records.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Io`] if the file cannot be written,
    /// or [`StatsError::Database`] if the query fails.
    pub fn export_json(&self, path: &Path) -> StatsResult<usize> {
        let records = self.all_records(None)?;
        let json = serde_json::to_string_pretty(&records).map_err(std::io::Error::other)?;
        std::fs::write(path, json)?;
        Ok(records.len())
    }

    /// Get the size of the database file in bytes.
    ///
    /// Returns `None` if the file size cannot be determined (e.g., in-memory
    /// databases).
    #[must_use]
    pub fn db_size_bytes(&self) -> Option<u64> {
        let conn = self.lock_conn();
        conn.query_row("PRAGMA page_count", [], |row| row.get::<_, i64>(0))
            .ok()
            .and_then(|page_count| {
                conn.query_row("PRAGMA page_size", [], |row| row.get::<_, i64>(0))
                    .ok()
                    .map(|page_size| u64::try_from(page_count * page_size).unwrap_or(0))
            })
    }

    /// Get database overview information: record count, file size, date range.
    ///
    /// # Errors
    ///
    /// Returns [`StatsError::Database`] if queries fail.
    pub fn db_info(&self) -> StatsResult<DbInfo> {
        let conn = self.lock_conn();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM stats", [], |row| row.get(0))?;
        let first_ts: Option<String> = conn
            .query_row(
                "SELECT timestamp FROM stats ORDER BY timestamp ASC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();
        let last_ts: Option<String> = conn
            .query_row(
                "SELECT timestamp FROM stats ORDER BY timestamp DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();
        // Query page info on the already-locked connection
        let size_bytes = conn
            .query_row("PRAGMA page_count", [], |row| row.get::<_, i64>(0))
            .ok()
            .and_then(|page_count| {
                conn.query_row("PRAGMA page_size", [], |row| row.get::<_, i64>(0))
                    .ok()
                    .map(|page_size| u64::try_from(page_count * page_size).unwrap_or(0))
            });

        Ok(DbInfo {
            record_count: usize::try_from(count).unwrap_or(0),
            size_bytes,
            first_record: first_ts,
            last_record: last_ts,
        })
    }

    fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<StatsRecord> {
        Ok(StatsRecord {
            id: row.get(0)?,
            timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                .map(|dt| dt.with_timezone(&chrono::Local))
                .unwrap_or_else(|_| chrono::Local::now()),
            operation: OperationType::from_str(&row.get::<_, String>(2)?)
                .unwrap_or(OperationType::CompressSchema),
            agent_id: row.get(3)?,
            source_pid: row.get(4)?,
            session_id: row.get(5)?,
            tool_use_id: row.get(6)?,
            project: row.get(7)?,
            namespace: row.get(8)?,
            experimental_mode: row.get::<_, i64>(9)? != 0,
            before_chars: row.get(10)?,
            before_tokens: row.get(11)?,
            after_chars: row.get(12)?,
            after_tokens: row.get(13)?,
            before_text: row.get(14)?,
            after_text: row.get(15)?,
            before_output: row.get(16)?,
            after_output: row.get(17)?,
        })
    }

    /// Map a database row to a [`ProjectSummaryRow`].
    ///
    /// Columns are assumed to be in order: project, COUNT(*),
    /// COALESCE(SUM(before_chars), 0), COALESCE(SUM(after_chars), 0),
    /// COALESCE(SUM(before_tokens), 0), COALESCE(SUM(after_tokens), 0).
    fn row_to_project_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectSummaryRow> {
        Ok(ProjectSummaryRow {
            project: row.get(0)?,
            record_count: usize::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
            total_before_chars: usize::try_from(row.get::<_, i64>(2)?).unwrap_or(0),
            total_after_chars: usize::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
            total_before_tokens: usize::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
            total_after_tokens: usize::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
        })
    }
}

fn sanitize_stats_text_option(text: Option<&str>, field_name: &str) -> RedactionOutcome {
    text.map_or_else(
        || RedactionOutcome::allow(String::new(), false),
        |value| {
            let outcome = sanitize_stats_text(value);
            match (outcome.was_blocked(), outcome.was_modified()) {
                (true, _) => {
                    tracing::warn!(
                        field = field_name,
                        "Sensitive content detected; skipping stats text recording"
                    );
                }
                (false, true) => {
                    tracing::warn!(
                        field = field_name,
                        "Sensitive content detected; storing redacted stats text"
                    );
                }
                (false, false) => {}
            }
            outcome
        },
    )
}

/// Sanitize stats text with unified redaction rules.
///
/// Long secret-like values are replaced with `[REDACTED]`. If the overall
/// payload remains too sensitive after targeted redaction (for example because
/// it contains authorization material or too many secret markers), recording is
/// blocked entirely.
#[must_use]
pub fn sanitize_stats_text(text: &str) -> RedactionOutcome {
    if text.is_empty() {
        return RedactionOutcome::allow(String::new(), false);
    }

    let lower = text.to_lowercase();
    if lower.contains("authorization:") || lower.contains("proxy-authorization:") {
        return RedactionOutcome::block();
    }

    let mut redacted = text.to_string();
    let mut modified = false;

    for marker in ["Bearer ", "bearer "] {
        while let Some(pos) = redacted.find(marker) {
            let start = pos + marker.len();
            let token_len = redacted[start..]
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != '"' && *c != '\'' && *c != ',')
                .map(char::len_utf8)
                .sum::<usize>();
            if token_len <= 10 {
                break;
            }
            redacted.replace_range(start..start + token_len, "[REDACTED]");
            modified = true;
        }
    }

    for pat in ["api_key", "apikey", "token", "secret", "password"] {
        let mut search_start = 0;
        loop {
            let current_lower = redacted[search_start..].to_lowercase();
            let Some(rel_pos) = current_lower.find(pat) else {
                break;
            };
            let pos = search_start + rel_pos;
            let after_key = &redacted[pos + pat.len()..];
            let trimmed = after_key.trim_start();
            let skipped = after_key.len().saturating_sub(trimmed.len());
            let Some(separator) = trimmed.chars().next() else {
                break;
            };
            if separator != '=' && separator != ':' {
                search_start = pos + pat.len();
                continue;
            }
            let value_start = pos + pat.len() + skipped + separator.len_utf8();
            let value = redacted[value_start..].trim_start();
            let leading_ws = redacted[value_start..].len().saturating_sub(value.len());
            let value_start = value_start + leading_ws;
            let value_len = value
                .chars()
                .take_while(|c| {
                    !c.is_whitespace() && *c != ',' && *c != ';' && *c != '"' && *c != '\''
                })
                .map(char::len_utf8)
                .sum::<usize>();
            if value_len > 10 {
                redacted.replace_range(value_start..value_start + value_len, "[REDACTED]");
                modified = true;
                search_start = value_start + "[REDACTED]".len();
            } else {
                search_start = value_start + value_len;
            }
        }
    }

    let trimmed = truncate_at_char_boundary(&redacted, MAX_REDACTED_TEXT_BYTES);
    if trimmed.len() != redacted.len() {
        modified = true;
        redacted = trimmed;
    }

    let redacted_markers = redacted.matches("[REDACTED]").count();
    if redacted_markers >= 3 {
        return RedactionOutcome::block();
    }

    RedactionOutcome::allow(redacted, modified)
}

fn truncate_at_char_boundary(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }

    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    format!("{}… [truncated]", &text[..end])
}

/// Summary statistics aggregated from multiple records.
#[derive(Debug, Clone, Default)]
pub struct StatsSummary {
    /// Total number of records.
    pub total_records: usize,
    /// Total characters before compression.
    pub total_before_chars: usize,
    /// Total characters after compression.
    pub total_after_chars: usize,
    /// Total tokens before compression.
    pub total_before_tokens: usize,
    /// Total tokens after compression.
    pub total_after_tokens: usize,
}

impl StatsSummary {
    /// Characters saved across all records.
    #[must_use]
    pub fn chars_saved(&self) -> usize {
        self.total_before_chars
            .saturating_sub(self.total_after_chars)
    }

    /// Tokens saved across all records.
    #[must_use]
    pub fn tokens_saved(&self) -> usize {
        self.total_before_tokens
            .saturating_sub(self.total_after_tokens)
    }

    /// Percentage of characters saved.
    #[must_use]
    pub fn chars_percent(&self) -> f64 {
        if self.total_before_chars > 0 {
            (self.chars_saved() as f64 / self.total_before_chars as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Percentage of tokens saved.
    #[must_use]
    pub fn tokens_percent(&self) -> f64 {
        if self.total_before_tokens > 0 {
            (self.tokens_saved() as f64 / self.total_before_tokens as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Build a summary from a slice of records.
    #[must_use]
    pub fn from_records(records: &[StatsRecord]) -> Self {
        let mut summary = Self {
            total_records: records.len(),
            ..Self::default()
        };

        for record in records {
            summary.total_before_chars += record.before_chars;
            summary.total_after_chars += record.after_chars;
            summary.total_before_tokens += record.before_tokens;
            summary.total_after_tokens += record.after_tokens;
        }

        summary
    }
}

/// Aggregated statistics for a single agent.
#[derive(Debug, Clone, Default)]
pub struct AgentSummaryRow {
    /// Agent identifier.
    pub agent_id: String,
    /// Number of records for this agent.
    pub record_count: usize,
    /// Total characters before compression.
    pub total_before_chars: usize,
    /// Total characters after compression.
    pub total_after_chars: usize,
    /// Total tokens before compression.
    pub total_before_tokens: usize,
    /// Total tokens after compression.
    pub total_after_tokens: usize,
}

/// Aggregated statistics for a single project.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectSummaryRow {
    /// Project name.
    pub project: String,
    /// Number of records for this project.
    pub record_count: usize,
    /// Total characters before compression.
    pub total_before_chars: usize,
    /// Total characters after compression.
    pub total_after_chars: usize,
    /// Total tokens before compression.
    pub total_before_tokens: usize,
    /// Total tokens after compression.
    pub total_after_tokens: usize,
}

/// Daily trend data point for a project or all projects.
#[derive(Debug, Clone, Default)]
pub struct ProjectDaily {
    /// Date string (ISO 8601, e.g., "2025-06-01").
    pub date: String,
    /// Characters saved on this date.
    pub chars_saved: u64,
    /// Tokens saved on this date.
    pub tokens_saved: u64,
    /// Number of records on this date.
    pub record_count: usize,
}

/// Overview information about the stats database.
#[derive(Debug, Clone, Default)]
pub struct DbInfo {
    /// Total number of records in the database.
    pub record_count: usize,
    /// Database file size in bytes (`None` for in-memory databases).
    pub size_bytes: Option<u64>,
    /// Timestamp of the earliest record (ISO 8601).
    pub first_record: Option<String>,
    /// Timestamp of the latest record (ISO 8601).
    pub last_record: Option<String>,
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    reason = "Test code uses unwrap/expect/panic idiomatically for assertion on failure; \
             disallowed_methods (e.g. std::fs) are acceptable in test cleanup"
)]
mod tests {
    use super::*;

    fn make_test_recorder() -> StatsRecorder {
        StatsRecorder::new(":memory:").expect("failed to create in-memory database")
    }

    #[test]
    fn test_record_and_retrieve() {
        let recorder = make_test_recorder();
        let record = StatsRecord::new(
            OperationType::CompressSchema,
            "test-agent".to_string(),
            100,
            25,
            50,
            12,
        );
        let id = recorder.record(&record).unwrap();
        assert!(id > 0, "record id should be positive");
    }

    #[test]
    fn test_count() {
        let recorder = make_test_recorder();
        assert_eq!(recorder.count().unwrap(), 0);

        let record = StatsRecord::new(
            OperationType::CompressSchema,
            "test-agent".to_string(),
            100,
            25,
            50,
            12,
        );
        recorder.record(&record).unwrap();
        assert_eq!(recorder.count().unwrap(), 1);
    }

    #[test]
    fn test_all_records_empty() {
        let recorder = make_test_recorder();
        let records = recorder.all_records(None).unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn test_all_records_with_limit() {
        let recorder = make_test_recorder();
        let record = StatsRecord::new(
            OperationType::CompressSchema,
            "test".to_string(),
            100,
            25,
            50,
            12,
        );
        recorder.record(&record).unwrap();
        recorder.record(&record).unwrap();
        let records = recorder.all_records(Some(1)).unwrap();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_clear() {
        let recorder = make_test_recorder();
        let record = StatsRecord::new(
            OperationType::CompressSchema,
            "test".to_string(),
            100,
            25,
            50,
            12,
        );
        recorder.record(&record).unwrap();
        recorder.clear().unwrap();
        assert_eq!(recorder.count().unwrap(), 0);
    }

    #[test]
    fn test_record_by_id_not_found() {
        let recorder = make_test_recorder();
        assert!(recorder.record_by_id(999).unwrap().is_none());
    }

    #[test]
    fn test_delete_by_id() {
        let recorder = make_test_recorder();
        let record = StatsRecord::new(
            OperationType::CompressSchema,
            "test".to_string(),
            100,
            25,
            50,
            12,
        );
        let id = recorder.record(&record).unwrap();
        assert!(recorder.delete_by_id(id).unwrap());
        assert!(!recorder.delete_by_id(id).unwrap());
        assert!(recorder.record_by_id(id).unwrap().is_none());
    }

    #[test]
    fn test_delete_by_agent() {
        let recorder = make_test_recorder();
        let record = StatsRecord::new(
            OperationType::CompressSchema,
            "agent-a".to_string(),
            100,
            25,
            50,
            12,
        );
        recorder.record(&record).unwrap();
        recorder.record(&record).unwrap();
        let r2 = StatsRecord::new(
            OperationType::CompressResponse,
            "agent-b".to_string(),
            200,
            50,
            100,
            25,
        );
        recorder.record(&r2).unwrap();
        assert_eq!(recorder.delete_by_agent("agent-a").unwrap(), 2);
        assert_eq!(recorder.count().unwrap(), 1);
    }

    #[test]
    fn test_delete_before() {
        let recorder = make_test_recorder();
        let mut record = StatsRecord::new(
            OperationType::CompressSchema,
            "test".to_string(),
            100,
            25,
            50,
            12,
        );
        record.timestamp = chrono::Local::now() - chrono::Duration::days(30);
        recorder.record(&record).unwrap();
        let mut recent = StatsRecord::new(
            OperationType::CompressSchema,
            "test".to_string(),
            100,
            25,
            50,
            12,
        );
        recent.timestamp = chrono::Local::now();
        recorder.record(&recent).unwrap();
        // Delete records before 7 days ago
        let cutoff = (chrono::Local::now() - chrono::Duration::days(7)).to_rfc3339();
        let deleted = recorder.delete_before(&cutoff).unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(recorder.count().unwrap(), 1);
    }

    #[test]
    fn test_vacuum() {
        let recorder = make_test_recorder();
        // VACUUM on empty/in-memory DB should not panic
        recorder.vacuum().unwrap();
    }

    #[test]
    fn test_db_info() {
        let recorder = make_test_recorder();
        let info = recorder.db_info().unwrap();
        assert_eq!(info.record_count, 0);
        assert!(info.first_record.is_none());
        assert!(info.last_record.is_none());

        let record = StatsRecord::new(
            OperationType::CompressSchema,
            "test".to_string(),
            100,
            25,
            50,
            12,
        );
        recorder.record(&record).unwrap();
        let info = recorder.db_info().unwrap();
        assert_eq!(info.record_count, 1);
        assert!(info.first_record.is_some());
        assert!(info.last_record.is_some());
    }

    #[test]
    fn test_export_json() {
        let recorder = make_test_recorder();
        let record = StatsRecord::new(
            OperationType::CompressSchema,
            "test".to_string(),
            100,
            25,
            50,
            12,
        );
        recorder.record(&record).unwrap();
        // Use a temp file
        let dir = std::env::temp_dir();
        let path = dir.join("tokenless_test_export.json");
        let count = recorder.export_json(&path).unwrap();
        assert_eq!(count, 1);
        // Clean up
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_stats_summary_from_empty() {
        let summary = StatsSummary::from_records(&[]);
        assert_eq!(summary.total_records, 0);
        assert_eq!(summary.chars_saved(), 0);
    }

    #[test]
    fn test_stats_summary_calculation() {
        let records = vec![
            StatsRecord::new(
                OperationType::CompressSchema,
                "agent".to_string(),
                100,
                25,
                60,
                15,
            ),
            StatsRecord::new(
                OperationType::CompressResponse,
                "agent".to_string(),
                200,
                50,
                100,
                25,
            ),
        ];
        let summary = StatsSummary::from_records(&records);
        assert_eq!(summary.total_records, 2);
        assert_eq!(summary.chars_saved(), 140);
        assert_eq!(summary.tokens_saved(), 35);
    }

    #[test]
    fn test_sanitize_detects_api_key() {
        let result = sanitize_stats_text("Bearer sk-1234567890abcdef1234567890abcdef");
        assert!(result.was_modified());
        assert_eq!(
            result.into_plain_text().as_deref(),
            Some("Bearer [REDACTED]")
        );
    }

    #[test]
    fn test_sanitize_allows_safe_text() {
        let result =
            sanitize_stats_text("compress_schema completed successfully for schema with 3 fields");
        assert!(!result.was_blocked());
        assert_eq!(
            result.into_plain_text().as_deref(),
            Some("compress_schema completed successfully for schema with 3 fields")
        );
    }

    #[test]
    fn test_sanitize_blocks_authorization_header() {
        let result = sanitize_stats_text("Header: Authorization: Bearer xyz123");
        assert!(result.was_blocked());
        assert!(result.into_plain_text().is_none());
    }

    #[test]
    fn test_sanitize_detects_api_key_pattern() {
        let result = sanitize_stats_text("api_key=sk-abc123def456ghi789jkl"); // gitleaks:allow
        assert!(result.was_modified());
        assert_eq!(
            result.into_plain_text().as_deref(),
            Some("api_key=[REDACTED]") // gitleaks:allow
        );
    }

    #[test]
    fn test_sanitize_allows_short_token_value() {
        let result = sanitize_stats_text("token=abc");
        assert!(!result.was_blocked());
        assert_eq!(result.into_plain_text().as_deref(), Some("token=abc"));
    }

    #[test]
    fn test_sanitize_truncates_large_safe_text() {
        let text = "a".repeat(MAX_REDACTED_TEXT_BYTES + 32);
        let result = sanitize_stats_text(&text);
        assert!(result.was_modified());
        let plain = result
            .into_plain_text()
            .expect("text should remain storable");
        assert!(plain.ends_with("… [truncated]"));
    }

    #[test]
    fn test_sanitize_blocks_too_many_redactions() {
        let result =
            sanitize_stats_text("token=abcdefghijk api_key=mnopqrstuvwxyz secret=12345678910");
        assert!(result.was_blocked());
        assert!(result.into_plain_text().is_none());
    }
}
