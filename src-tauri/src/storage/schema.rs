//! Schema migrations driven by `PRAGMA user_version` (CLAUDE.md §7).
//!
//! Each element of [`MIGRATIONS`] takes the DB from `user_version = idx`
//! to `user_version = idx + 1`. Migrations run inside a transaction so a
//! partial failure rolls back cleanly; the version bump is part of the
//! same transaction so the runner can be killed and resumed safely.

use rusqlite::{Connection, Transaction};

/// Migration SQL indexed by *source* version. `MIGRATIONS[0]` brings the
/// database from v0 (empty) to v1 (the schema in CLAUDE.md §7).
const MIGRATIONS: &[&str] = &[
    // v0 -> v1: initial schema.
    r#"
    CREATE TABLE scans (
        id              INTEGER PRIMARY KEY AUTOINCREMENT,
        batch_id        TEXT    NOT NULL,
        content         TEXT    NOT NULL,
        kind            TEXT    NOT NULL,
        monitor_index   INTEGER NOT NULL,
        scanned_at      INTEGER NOT NULL,
        opened          INTEGER NOT NULL DEFAULT 0,
        opened_at       INTEGER
    );
    CREATE INDEX idx_scans_scanned_at ON scans(scanned_at DESC);
    CREATE INDEX idx_scans_batch      ON scans(batch_id);
    CREATE INDEX idx_scans_kind       ON scans(kind);
    CREATE INDEX idx_scans_opened     ON scans(opened);
    "#,
    // v1 -> v2: track copied status so the History filter can show
    // Opened / Copied / Untouched as separate states (CLAUDE.md §10).
    r#"
    ALTER TABLE scans ADD COLUMN copied    INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE scans ADD COLUMN copied_at INTEGER;
    CREATE INDEX idx_scans_copied ON scans(copied);
    "#,
];

pub fn run_migrations(conn: &mut Connection) -> rusqlite::Result<()> {
    let current_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    for (idx, sql) in MIGRATIONS.iter().enumerate() {
        let target_version = (idx as i64) + 1;
        if current_version >= target_version {
            continue;
        }
        let tx = conn.transaction()?;
        apply(&tx, sql, target_version)?;
        tx.commit()?;
    }
    Ok(())
}

fn apply(tx: &Transaction<'_>, sql: &str, target_version: i64) -> rusqlite::Result<()> {
    tx.execute_batch(sql)?;
    // `PRAGMA user_version = N` doesn't accept a bind parameter, so we
    // format it ourselves — `target_version` is an internal i64, not user
    // input, so SQL injection isn't a concern here.
    tx.execute_batch(&format!("PRAGMA user_version = {target_version}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_db_reaches_latest_version() {
        let mut conn = Connection::open_in_memory().expect("open");
        run_migrations(&mut conn).expect("migrate");
        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("user_version");
        assert_eq!(v, MIGRATIONS.len() as i64);
    }

    #[test]
    fn migrations_are_idempotent() {
        let mut conn = Connection::open_in_memory().expect("open");
        run_migrations(&mut conn).expect("first migrate");
        run_migrations(&mut conn).expect("second migrate is no-op");
        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .expect("user_version");
        assert_eq!(v, MIGRATIONS.len() as i64);
    }

    #[test]
    fn scans_table_exists_after_migration() {
        let mut conn = Connection::open_in_memory().expect("open");
        run_migrations(&mut conn).expect("migrate");
        let _ = conn
            .prepare(
                "SELECT id, batch_id, content, kind, monitor_index, scanned_at, \
                 opened, opened_at, copied, copied_at FROM scans",
            )
            .expect("scans table queryable");
    }

    #[test]
    fn migration_v2_adds_copied_columns_with_default_zero() {
        let mut conn = Connection::open_in_memory().expect("open");
        run_migrations(&mut conn).expect("migrate");
        conn.execute(
            "INSERT INTO scans (batch_id, content, kind, monitor_index, scanned_at) \
             VALUES ('b', 'c', 'text', 0, 0)",
            [],
        )
        .expect("insert");
        let (copied, copied_at): (i64, Option<i64>) = conn
            .query_row("SELECT copied, copied_at FROM scans LIMIT 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .expect("select copied columns");
        assert_eq!(copied, 0);
        assert_eq!(copied_at, None);
    }
}
