//! SQLite persistence for jobs (queue + history), settings and presets.
//!
//! Jobs are stored as a JSON blob (`data`) plus a few indexed columns used for ordering and
//! history queries, so adding fields to `Job` never needs a schema migration.
//! Schema changes are applied by `migrate()` based on `PRAGMA user_version`.

use crate::{
    engine::job::{Job, JobStatus},
    presets::Preset,
    settings::Settings,
};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};

const SCHEMA_VERSION: i32 = 1;

pub struct Db {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HistoryQuery {
    pub search: String,
    /// "all" or a finished status: "completed" | "failed" | "canceled".
    pub status: String,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub items: Vec<Job>,
    pub total: u64,
}

type DbResult<T> = Result<T, String>;

fn err(e: rusqlite::Error) -> String {
    format!("Database error: {e}")
}

fn status_str(s: JobStatus) -> String {
    serde_json::to_value(s)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

impl Db {
    pub fn open(path: &Path) -> DbResult<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
        }
        let conn = Connection::open(path).map_err(err)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> DbResult<Self> {
        Self::init(Connection::open_in_memory().map_err(err)?)
    }

    fn init(conn: Connection) -> DbResult<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(err)?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ---- Settings -------------------------------------------------------------------

    pub fn load_settings(&self) -> DbResult<Option<Settings>> {
        let raw: Option<String> = self
            .conn()
            .query_row(
                "SELECT value FROM settings WHERE key = 'settings'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        raw.map(|s| {
            serde_json::from_str(&s).map_err(|e| format!("Stored settings are corrupt: {e}"))
        })
        .transpose()
    }

    pub fn save_settings(&self, settings: &Settings) -> DbResult<()> {
        let json = serde_json::to_string(settings).map_err(|e| e.to_string())?;
        self.conn()
            .execute(
                "INSERT INTO settings (key, value) VALUES ('settings', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![json],
            )
            .map_err(err)?;
        Ok(())
    }

    // ---- Presets (user presets only; built-ins live in code) -------------------------

    pub fn load_presets(&self) -> DbResult<Vec<Preset>> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT data FROM presets ORDER BY position")
            .map_err(err)?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(err)?;
        let mut out = Vec::new();
        for row in rows {
            match serde_json::from_str::<Preset>(&row.map_err(err)?) {
                Ok(p) => out.push(p),
                Err(e) => log::warn!("Skipping unreadable preset: {e}"),
            }
        }
        Ok(out)
    }

    /// Replace all user presets.
    pub fn save_presets(&self, presets: &[Preset]) -> DbResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        tx.execute("DELETE FROM presets", []).map_err(err)?;
        for (i, p) in presets.iter().enumerate() {
            let json = serde_json::to_string(p).map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT INTO presets (id, position, data) VALUES (?1, ?2, ?3)",
                params![p.id, i as i64, json],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)
    }

    // ---- Jobs -----------------------------------------------------------------------

    /// Jobs still in the queue (not archived to history), in queue order.
    pub fn load_queue(&self) -> DbResult<Vec<Job>> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT data FROM jobs WHERE archived = 0 ORDER BY position")
            .map_err(err)?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(err)?;
        let mut out = Vec::new();
        for row in rows {
            match serde_json::from_str::<Job>(&row.map_err(err)?) {
                Ok(j) => out.push(j),
                Err(e) => log::warn!("Skipping unreadable job row: {e}"),
            }
        }
        Ok(out)
    }

    /// Insert or update jobs with their queue positions, in one transaction.
    pub fn upsert_jobs(&self, jobs: &[(Job, usize)]) -> DbResult<()> {
        if jobs.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO jobs (id, position, status, archived, url, title, created_at, finished_at, data)
                     VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(id) DO UPDATE SET
                        position = excluded.position, status = excluded.status, url = excluded.url,
                        title = excluded.title, finished_at = excluded.finished_at, data = excluded.data",
                )
                .map_err(err)?;
            for (job, pos) in jobs {
                let json = serde_json::to_string(job).map_err(|e| e.to_string())?;
                stmt.execute(params![
                    job.id,
                    *pos as i64,
                    status_str(job.status),
                    job.url,
                    job.title,
                    job.created_at as i64,
                    job.finished_at.map(|v| v as i64),
                    json
                ])
                .map_err(err)?;
            }
        }
        tx.commit().map_err(err)
    }

    pub fn set_positions(&self, ids: &[String]) -> DbResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        for (i, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE jobs SET position = ?1 WHERE id = ?2",
                params![i as i64, id],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)
    }

    /// Move jobs out of the queue into history.
    pub fn archive_jobs(&self, ids: &[String]) -> DbResult<()> {
        self.exec_for_ids("UPDATE jobs SET archived = 1 WHERE id IN", ids)
    }

    pub fn delete_jobs(&self, ids: &[String]) -> DbResult<()> {
        self.exec_for_ids("DELETE FROM jobs WHERE id IN", ids)
    }

    fn exec_for_ids(&self, sql_prefix: &str, ids: &[String]) -> DbResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let placeholders = vec!["?"; ids.len()].join(",");
        self.conn()
            .execute(
                &format!("{sql_prefix} ({placeholders})"),
                params_from_iter(ids.iter()),
            )
            .map_err(err)?;
        Ok(())
    }

    /// Finished jobs (queue and archived), newest first.
    pub fn history(&self, q: &HistoryQuery) -> DbResult<HistoryPage> {
        let mut where_sql = String::from("status IN ('completed','failed','canceled')");
        let mut args: Vec<String> = Vec::new();
        if matches!(q.status.as_str(), "completed" | "failed" | "canceled") {
            where_sql.push_str(" AND status = ?");
            args.push(q.status.clone());
        }
        let search = q.search.trim();
        if !search.is_empty() {
            where_sql.push_str(" AND (title LIKE ? ESCAPE '\\' OR url LIKE ? ESCAPE '\\')");
            let like = format!(
                "%{}%",
                search
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            );
            args.push(like.clone());
            args.push(like);
        }
        let conn = self.conn();
        let total: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM jobs WHERE {where_sql}"),
                params_from_iter(args.iter()),
                |r| r.get(0),
            )
            .map_err(err)?;

        let limit = if q.limit == 0 { 50 } else { q.limit.min(500) };
        let sql = format!(
            "SELECT data FROM jobs WHERE {where_sql}
             ORDER BY COALESCE(finished_at, created_at) DESC LIMIT {limit} OFFSET {}",
            q.offset
        );
        let mut stmt = conn.prepare(&sql).map_err(err)?;
        let rows = stmt
            .query_map(params_from_iter(args.iter()), |r| r.get::<_, String>(0))
            .map_err(err)?;
        let mut items = Vec::new();
        for row in rows {
            if let Ok(j) = serde_json::from_str::<Job>(&row.map_err(err)?) {
                items.push(j);
            }
        }
        Ok(HistoryPage {
            items,
            total: total.max(0) as u64,
        })
    }

    pub fn job(&self, id: &str) -> DbResult<Option<Job>> {
        let raw: Option<String> = self
            .conn()
            .query_row("SELECT data FROM jobs WHERE id = ?1", params![id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(err)?;
        Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
    }

    /// Ids of all finished jobs that are archived (not in the queue).
    pub fn archived_finished_ids(&self) -> DbResult<Vec<String>> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT id FROM jobs WHERE archived = 1 AND status IN ('completed','failed','canceled')")
            .map_err(err)?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        Ok(ids)
    }
}

fn migrate(conn: &Connection) -> DbResult<()> {
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(err)?;
    if version > SCHEMA_VERSION {
        return Err(format!(
            "The database was created by a newer TubeDeck (schema {version}); please update TubeDeck."
        ));
    }
    if version < 1 {
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE presets (id TEXT PRIMARY KEY, position INTEGER NOT NULL, data TEXT NOT NULL);
             CREATE TABLE jobs (
                id TEXT PRIMARY KEY,
                position INTEGER NOT NULL,
                status TEXT NOT NULL,
                archived INTEGER NOT NULL DEFAULT 0,
                url TEXT NOT NULL,
                title TEXT,
                created_at INTEGER NOT NULL,
                finished_at INTEGER,
                data TEXT NOT NULL
             );
             CREATE INDEX jobs_queue ON jobs (archived, position);
             CREATE INDEX jobs_history ON jobs (status, finished_at);
             PRAGMA user_version = 1;
             COMMIT;",
        )
        .map_err(err)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::DownloadOptions;

    fn job(id: &str, status: JobStatus, title: &str, finished_at: Option<u64>) -> Job {
        let mut j = Job::new(
            id.into(),
            format!("https://example.com/{id}"),
            DownloadOptions::default(),
            None,
        );
        j.status = status;
        j.title = Some(title.into());
        j.finished_at = finished_at;
        j
    }

    #[test]
    fn settings_round_trip() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.load_settings().unwrap().is_none());
        let s = Settings {
            max_concurrent: 7,
            ..Default::default()
        };
        db.save_settings(&s).unwrap();
        db.save_settings(&s).unwrap(); // upsert
        assert_eq!(db.load_settings().unwrap().unwrap().max_concurrent, 7);
    }

    #[test]
    fn presets_replace_all_in_order() {
        let db = Db::open_in_memory().unwrap();
        let p = |id: &str| Preset {
            id: id.into(),
            name: id.into(),
            description: String::new(),
            builtin: false,
            options: DownloadOptions::default(),
        };
        db.save_presets(&[p("b"), p("a")]).unwrap();
        db.save_presets(&[p("c"), p("b")]).unwrap();
        let ids: Vec<_> = db
            .load_presets()
            .unwrap()
            .into_iter()
            .map(|p| p.id)
            .collect();
        assert_eq!(ids, ["c", "b"]);
    }

    #[test]
    fn queue_order_upsert_and_archive() {
        let db = Db::open_in_memory().unwrap();
        let a = job("a", JobStatus::Queued, "A", None);
        let b = job("b", JobStatus::Paused, "B", None);
        let c = job("c", JobStatus::Completed, "C", Some(10));
        db.upsert_jobs(&[(a.clone(), 0), (b.clone(), 1), (c.clone(), 2)])
            .unwrap();
        db.set_positions(&["c".into(), "b".into(), "a".into()])
            .unwrap();
        let ids: Vec<_> = db.load_queue().unwrap().into_iter().map(|j| j.id).collect();
        assert_eq!(ids, ["c", "b", "a"]);

        // Updating keeps the row and stores the new data.
        let mut a2 = a.clone();
        a2.status = JobStatus::Downloading;
        db.upsert_jobs(&[(a2, 2)]).unwrap();
        assert_eq!(db.job("a").unwrap().unwrap().status, JobStatus::Downloading);

        db.archive_jobs(&["c".into()]).unwrap();
        let ids: Vec<_> = db.load_queue().unwrap().into_iter().map(|j| j.id).collect();
        assert_eq!(ids, ["b", "a"]);
        assert_eq!(db.archived_finished_ids().unwrap(), ["c"]);

        db.delete_jobs(&["a".into(), "b".into()]).unwrap();
        assert!(db.load_queue().unwrap().is_empty());
        assert!(db.job("c").unwrap().is_some());
    }

    #[test]
    fn history_filters_search_and_pages() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_jobs(&[
            (job("1", JobStatus::Completed, "Cats 100%", Some(100)), 0),
            (job("2", JobStatus::Failed, "Dogs", Some(300)), 1),
            (job("3", JobStatus::Completed, "More cats", Some(200)), 2),
            (job("4", JobStatus::Queued, "Cats queued", None), 3),
        ])
        .unwrap();
        db.archive_jobs(&["1".into()]).unwrap();

        let all = db.history(&HistoryQuery::default()).unwrap();
        assert_eq!(all.total, 3);
        assert_eq!(
            all.items.iter().map(|j| j.id.as_str()).collect::<Vec<_>>(),
            ["2", "3", "1"]
        );

        let cats = db
            .history(&HistoryQuery {
                search: "cats".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(cats.total, 2);

        // LIKE wildcards in the search are literal.
        let pct = db
            .history(&HistoryQuery {
                search: "100%".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(pct.total, 1);

        let failed = db
            .history(&HistoryQuery {
                status: "failed".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(failed.items[0].id, "2");

        let page = db
            .history(&HistoryQuery {
                limit: 1,
                offset: 1,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.items[0].id, "3");
    }

    #[test]
    fn reopening_a_file_keeps_data_and_schema() {
        let path = std::env::temp_dir().join(format!("tubedeck-db-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let db = Db::open(&path).unwrap();
            db.upsert_jobs(&[(job("x", JobStatus::Paused, "X", None), 0)])
                .unwrap();
        }
        let db = Db::open(&path).unwrap();
        assert_eq!(db.load_queue().unwrap()[0].id, "x");
        drop(db);
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
        }
    }
}
