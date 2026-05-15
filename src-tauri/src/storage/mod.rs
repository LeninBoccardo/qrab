//! SQLite-backed history store.
//!
//! One connection per process, guarded by a `Mutex` (CLAUDE.md §9 — app
//! volume doesn't justify a pool). [`Storage::open`] runs pending
//! migrations on every startup; [`Storage::in_memory`] is the test
//! constructor — :memory: SQLite is the real implementation in tests,
//! so we don't need a `HistoryRepository` trait abstraction (§4).

use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

pub mod queries;
pub mod schema;

#[derive(Clone)]
pub struct Storage {
    inner: Arc<Mutex<Connection>>,
}

impl Storage {
    /// Open or create the database at `path` and run migrations. The
    /// parent directory must already exist.
    pub fn open<P: AsRef<Path>>(path: P) -> rusqlite::Result<Self> {
        let mut conn = Connection::open(path)?;
        // WAL gives better concurrent reads alongside our occasional
        // writes; CLAUDE.md §7 calls this out as a hard requirement.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // Wait briefly on writer contention rather than failing fast.
        conn.busy_timeout(std::time::Duration::from_secs(2))?;
        schema::run_migrations(&mut conn)?;
        Ok(Self { inner: Arc::new(Mutex::new(conn)) })
    }

    /// In-memory database — used by tests. Gated on `cfg(test)` so it
    /// doesn't sit in release builds as dead code.
    #[cfg(test)]
    pub(crate) fn in_memory() -> rusqlite::Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        schema::run_migrations(&mut conn)?;
        Ok(Self { inner: Arc::new(Mutex::new(conn)) })
    }

    /// Lock the underlying connection. Recovers from poisoning since the
    /// data inside SQLite isn't corrupted by a panic on the Rust side.
    /// Submodule-private — callers outside `storage::` should use the
    /// query helpers in `storage::queries`.
    pub(super) fn lock(&self) -> MutexGuard<'_, Connection> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }
}
