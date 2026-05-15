//! CRUD over the `scans` table. The DB-row shape lives here so the
//! storage module owns its own type (CLAUDE.md §6); commands.rs imports
//! `ScanRow` to return it over IPC.

use crate::decoder::QrKind;
use crate::storage::Storage;
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRow {
    pub id: i64,
    pub batch_id: String,
    pub content: String,
    pub kind: QrKind,
    pub monitor_index: i64,
    /// Unix epoch milliseconds.
    pub scanned_at: i64,
    pub opened: bool,
    /// Unix epoch milliseconds, or `None` if never opened.
    pub opened_at: Option<i64>,
}

impl ScanRow {
    fn from_db_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let kind_str: String = row.get("kind")?;
        let kind = str_to_qrkind(&kind_str).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("unknown QrKind '{kind_str}'").into(),
            )
        })?;
        Ok(ScanRow {
            id: row.get("id")?,
            batch_id: row.get("batch_id")?,
            content: row.get("content")?,
            kind,
            monitor_index: row.get("monitor_index")?,
            scanned_at: row.get("scanned_at")?,
            opened: row.get::<_, i64>("opened")? != 0,
            opened_at: row.get("opened_at")?,
        })
    }
}

/// Fields needed to insert a new row — `id`, `opened`, and `opened_at`
/// are filled in by the DB (PK / defaults).
pub struct NewScanRow<'a> {
    pub batch_id: &'a str,
    pub content: &'a str,
    pub kind: QrKind,
    pub monitor_index: i64,
    pub scanned_at: i64,
}

pub fn insert_scan(
    storage: &Storage,
    row: &NewScanRow<'_>,
) -> rusqlite::Result<i64> {
    let conn = storage.lock();
    conn.execute(
        "INSERT INTO scans (batch_id, content, kind, monitor_index, scanned_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            row.batch_id,
            row.content,
            qrkind_to_str(row.kind),
            row.monitor_index,
            row.scanned_at,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Insert multiple rows in a single transaction. Returns the row IDs in
/// the same order as the input slice.
pub fn insert_batch(
    storage: &Storage,
    rows: &[NewScanRow<'_>],
) -> rusqlite::Result<Vec<i64>> {
    let mut conn = storage.lock();
    let tx = conn.transaction()?;
    let mut ids = Vec::with_capacity(rows.len());
    {
        let mut stmt = tx.prepare(
            "INSERT INTO scans (batch_id, content, kind, monitor_index, scanned_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for row in rows {
            stmt.execute(params![
                row.batch_id,
                row.content,
                qrkind_to_str(row.kind),
                row.monitor_index,
                row.scanned_at,
            ])?;
            ids.push(tx.last_insert_rowid());
        }
    }
    tx.commit()?;
    Ok(ids)
}

/// Fetch a single row by primary key, returning `None` if absent.
pub fn get_by_id(storage: &Storage, id: i64) -> rusqlite::Result<Option<ScanRow>> {
    let conn = storage.lock();
    let mut stmt = conn.prepare(
        "SELECT id, batch_id, content, kind, monitor_index, scanned_at, opened, opened_at
         FROM scans WHERE id = ?1",
    )?;
    match stmt.query_row(params![id], ScanRow::from_db_row) {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

fn qrkind_to_str(kind: QrKind) -> &'static str {
    match kind {
        QrKind::Url => "url",
        QrKind::Text => "text",
        QrKind::Wifi => "wifi",
        QrKind::Vcard => "vcard",
        QrKind::Email => "email",
        QrKind::Phone => "phone",
        QrKind::Other => "other",
    }
}

fn str_to_qrkind(s: &str) -> Option<QrKind> {
    Some(match s {
        "url" => QrKind::Url,
        "text" => QrKind::Text,
        "wifi" => QrKind::Wifi,
        "vcard" => QrKind::Vcard,
        "email" => QrKind::Email,
        "phone" => QrKind::Phone,
        "other" => QrKind::Other,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Storage {
        Storage::in_memory().expect("open :memory:")
    }

    #[test]
    fn insert_then_get_by_id_roundtrip() {
        let storage = fresh();
        let id = insert_scan(
            &storage,
            &NewScanRow {
                batch_id: "B-1",
                content: "https://example.com",
                kind: QrKind::Url,
                monitor_index: 0,
                scanned_at: 1_234_567_890_000,
            },
        )
        .expect("insert");

        let row = get_by_id(&storage, id).expect("get").expect("some");
        assert_eq!(row.id, id);
        assert_eq!(row.batch_id, "B-1");
        assert_eq!(row.content, "https://example.com");
        assert_eq!(row.kind, QrKind::Url);
        assert_eq!(row.monitor_index, 0);
        assert_eq!(row.scanned_at, 1_234_567_890_000);
        assert!(!row.opened);
        assert_eq!(row.opened_at, None);
    }

    #[test]
    fn get_by_id_returns_none_for_missing() {
        let storage = fresh();
        let row = get_by_id(&storage, 9999).expect("get");
        assert!(row.is_none());
    }

    #[test]
    fn insert_batch_writes_all_rows_in_order() {
        let storage = fresh();
        let rows = vec![
            NewScanRow {
                batch_id: "B",
                content: "a",
                kind: QrKind::Text,
                monitor_index: 0,
                scanned_at: 1,
            },
            NewScanRow {
                batch_id: "B",
                content: "b",
                kind: QrKind::Url,
                monitor_index: 1,
                scanned_at: 2,
            },
            NewScanRow {
                batch_id: "B",
                content: "c",
                kind: QrKind::Email,
                monitor_index: 0,
                scanned_at: 3,
            },
        ];
        let ids = insert_batch(&storage, &rows).expect("batch insert");
        assert_eq!(ids.len(), 3);
        // IDs should be sequential since AUTOINCREMENT on a fresh table
        assert!(ids[1] == ids[0] + 1 && ids[2] == ids[1] + 1);

        let middle = get_by_id(&storage, ids[1]).expect("get").expect("some");
        assert_eq!(middle.content, "b");
        assert_eq!(middle.kind, QrKind::Url);
        assert_eq!(middle.monitor_index, 1);
    }

    #[test]
    fn all_qrkind_variants_round_trip_through_storage() {
        let storage = fresh();
        let kinds = [
            QrKind::Url,
            QrKind::Text,
            QrKind::Wifi,
            QrKind::Vcard,
            QrKind::Email,
            QrKind::Phone,
            QrKind::Other,
        ];
        for (i, k) in kinds.iter().enumerate() {
            let id = insert_scan(
                &storage,
                &NewScanRow {
                    batch_id: "B-kind",
                    content: "x",
                    kind: *k,
                    monitor_index: 0,
                    scanned_at: i as i64,
                },
            )
            .expect("insert");
            let row = get_by_id(&storage, id).expect("get").expect("some");
            assert_eq!(row.kind, *k, "kind round-trip failed for {:?}", k);
        }
    }
}
