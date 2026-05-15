//! CRUD over the `scans` table. The DB-row shape lives here so the
//! storage module owns its own type (CLAUDE.md §6); commands.rs imports
//! `ScanRow` to return it over IPC.

use crate::decoder::QrKind;
use crate::storage::Storage;
use rusqlite::{params, params_from_iter, Row, ToSql};
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

/// Filter parameters for [`history_query`]. Matches the §7 frontend type.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFilter {
    pub search: Option<String>,
    pub kind: Option<QrKind>,
    pub opened_only: Option<bool>,
    pub unopened_only: Option<bool>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub limit: i64,
    pub offset: i64,
}

/// Paginated, sorted history query. Newest first (scanned_at DESC, id DESC
/// breaks ties). Filters compose with AND; an absent field is unfiltered.
pub fn history_query(
    storage: &Storage,
    filter: &HistoryFilter,
) -> rusqlite::Result<Vec<ScanRow>> {
    let mut sql = String::from(
        "SELECT id, batch_id, content, kind, monitor_index, scanned_at, opened, opened_at
         FROM scans WHERE 1=1",
    );
    let mut bound: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(s) = filter.search.as_deref().filter(|s| !s.is_empty()) {
        sql.push_str(" AND content LIKE ?");
        bound.push(Box::new(format!("%{s}%")));
    }
    if let Some(k) = filter.kind {
        sql.push_str(" AND kind = ?");
        bound.push(Box::new(qrkind_to_str(k).to_string()));
    }
    if filter.opened_only == Some(true) {
        sql.push_str(" AND opened = 1");
    }
    if filter.unopened_only == Some(true) {
        sql.push_str(" AND opened = 0");
    }
    if let Some(from) = filter.from {
        sql.push_str(" AND scanned_at >= ?");
        bound.push(Box::new(from));
    }
    if let Some(to) = filter.to {
        sql.push_str(" AND scanned_at <= ?");
        bound.push(Box::new(to));
    }
    sql.push_str(" ORDER BY scanned_at DESC, id DESC LIMIT ? OFFSET ?");
    bound.push(Box::new(filter.limit.max(0)));
    bound.push(Box::new(filter.offset.max(0)));

    let conn = storage.lock();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            params_from_iter(bound.iter().map(|b| b.as_ref())),
            ScanRow::from_db_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Mark a row as opened, stamping `opened_at` to `now_ms` (Unix epoch ms).
/// Returns the number of rows updated — `0` if the row doesn't exist.
pub fn mark_opened(
    storage: &Storage,
    id: i64,
    now_ms: i64,
) -> rusqlite::Result<usize> {
    let conn = storage.lock();
    conn.execute(
        "UPDATE scans SET opened = 1, opened_at = ?1 WHERE id = ?2",
        params![now_ms, id],
    )
}

/// Delete one row. Returns the number of rows deleted.
pub fn delete_by_id(storage: &Storage, id: i64) -> rusqlite::Result<usize> {
    let conn = storage.lock();
    conn.execute("DELETE FROM scans WHERE id = ?1", params![id])
}

/// Wipe the entire history. Returns the number of rows deleted.
pub fn delete_all(storage: &Storage) -> rusqlite::Result<usize> {
    let conn = storage.lock();
    conn.execute("DELETE FROM scans", [])
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

    fn empty_filter(limit: i64) -> HistoryFilter {
        HistoryFilter {
            search: None,
            kind: None,
            opened_only: None,
            unopened_only: None,
            from: None,
            to: None,
            limit,
            offset: 0,
        }
    }

    fn seed_history(storage: &Storage) -> Vec<i64> {
        let rows = vec![
            NewScanRow {
                batch_id: "B1",
                content: "https://alpha.test",
                kind: QrKind::Url,
                monitor_index: 0,
                scanned_at: 100,
            },
            NewScanRow {
                batch_id: "B1",
                content: "mailto:bob@beta.test",
                kind: QrKind::Email,
                monitor_index: 0,
                scanned_at: 200,
            },
            NewScanRow {
                batch_id: "B2",
                content: "https://gamma.test",
                kind: QrKind::Url,
                monitor_index: 1,
                scanned_at: 300,
            },
            NewScanRow {
                batch_id: "B3",
                content: "plain delta text",
                kind: QrKind::Text,
                monitor_index: 0,
                scanned_at: 400,
            },
        ];
        insert_batch(storage, &rows).expect("seed")
    }

    #[test]
    fn history_query_returns_newest_first() {
        let storage = fresh();
        seed_history(&storage);
        let rows = history_query(&storage, &empty_filter(10)).expect("query");
        let contents: Vec<&str> = rows.iter().map(|r| r.content.as_str()).collect();
        assert_eq!(
            contents,
            vec![
                "plain delta text",       // scanned_at = 400
                "https://gamma.test",     // 300
                "mailto:bob@beta.test",   // 200
                "https://alpha.test",     // 100
            ],
        );
    }

    #[test]
    fn history_query_search_filters_by_content_like() {
        let storage = fresh();
        seed_history(&storage);
        let mut f = empty_filter(10);
        f.search = Some("gamma".into());
        let rows = history_query(&storage, &f).expect("query");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].content, "https://gamma.test");
    }

    #[test]
    fn history_query_kind_filter_narrows_to_one_kind() {
        let storage = fresh();
        seed_history(&storage);
        let mut f = empty_filter(10);
        f.kind = Some(QrKind::Url);
        let rows = history_query(&storage, &f).expect("query");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.kind == QrKind::Url));
    }

    #[test]
    fn history_query_date_range_filters_inclusive() {
        let storage = fresh();
        seed_history(&storage);
        let mut f = empty_filter(10);
        f.from = Some(200);
        f.to = Some(300);
        let rows = history_query(&storage, &f).expect("query");
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.scanned_at >= 200 && r.scanned_at <= 300));
    }

    #[test]
    fn history_query_paginates_via_limit_and_offset() {
        let storage = fresh();
        seed_history(&storage);
        let mut f = empty_filter(2);
        let page1 = history_query(&storage, &f).expect("page1");
        assert_eq!(page1.len(), 2);
        f.offset = 2;
        let page2 = history_query(&storage, &f).expect("page2");
        assert_eq!(page2.len(), 2);
        // No overlap between pages
        let ids1: Vec<i64> = page1.iter().map(|r| r.id).collect();
        let ids2: Vec<i64> = page2.iter().map(|r| r.id).collect();
        assert!(ids1.iter().all(|id| !ids2.contains(id)));
    }

    #[test]
    fn history_query_opened_only_and_unopened_only_split_correctly() {
        let storage = fresh();
        let ids = seed_history(&storage);
        // Mark the first two as opened
        mark_opened(&storage, ids[0], 500).expect("mark0");
        mark_opened(&storage, ids[1], 600).expect("mark1");

        let mut f = empty_filter(10);
        f.opened_only = Some(true);
        let opened = history_query(&storage, &f).expect("query");
        assert_eq!(opened.len(), 2);
        assert!(opened.iter().all(|r| r.opened));

        let mut f = empty_filter(10);
        f.unopened_only = Some(true);
        let unopened = history_query(&storage, &f).expect("query");
        assert_eq!(unopened.len(), 2);
        assert!(unopened.iter().all(|r| !r.opened));
    }

    #[test]
    fn mark_opened_sets_opened_and_opened_at() {
        let storage = fresh();
        let ids = seed_history(&storage);
        let updated = mark_opened(&storage, ids[0], 1234).expect("mark");
        assert_eq!(updated, 1);
        let row = get_by_id(&storage, ids[0]).expect("get").expect("some");
        assert!(row.opened);
        assert_eq!(row.opened_at, Some(1234));
    }

    #[test]
    fn mark_opened_missing_row_returns_zero() {
        let storage = fresh();
        let updated = mark_opened(&storage, 9999, 1).expect("mark");
        assert_eq!(updated, 0);
    }

    #[test]
    fn delete_by_id_removes_a_single_row() {
        let storage = fresh();
        let ids = seed_history(&storage);
        let removed = delete_by_id(&storage, ids[1]).expect("delete");
        assert_eq!(removed, 1);
        assert!(get_by_id(&storage, ids[1]).expect("get").is_none());
        let remaining =
            history_query(&storage, &empty_filter(10)).expect("query");
        assert_eq!(remaining.len(), 3);
    }

    #[test]
    fn delete_all_empties_the_table() {
        let storage = fresh();
        seed_history(&storage);
        let removed = delete_all(&storage).expect("clear");
        assert_eq!(removed, 4);
        let remaining =
            history_query(&storage, &empty_filter(10)).expect("query");
        assert!(remaining.is_empty());
    }
}
