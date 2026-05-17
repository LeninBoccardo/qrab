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
    pub copied: bool,
    /// Unix epoch milliseconds, or `None` if never copied.
    pub copied_at: Option<i64>,
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
            copied: row.get::<_, i64>("copied")? != 0,
            copied_at: row.get("copied_at")?,
        })
    }
}

/// Status filter for [`HistoryFilter`]. `Opened` and `Copied` overlap —
/// a row that was both opened and copied counts for either filter.
/// `Untouched` selects rows with neither flag set.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StatusFilter {
    All,
    Opened,
    Copied,
    Untouched,
}

/// Sort direction for [`history_query`]. Defaults to `Desc` (newest first).
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortDir {
    #[default]
    Desc,
    Asc,
}

impl SortDir {
    fn as_sql(self) -> &'static str {
        match self {
            SortDir::Desc => "DESC",
            SortDir::Asc => "ASC",
        }
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
        "SELECT id, batch_id, content, kind, monitor_index, scanned_at, \
                opened, opened_at, copied, copied_at
         FROM scans WHERE id = ?1",
    )?;
    match stmt.query_row(params![id], ScanRow::from_db_row) {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Fetch multiple rows by primary key in one statement. The returned `Vec`
/// is ordered to match `ids` (with any missing IDs simply omitted). Empty
/// input is a no-op that hits no DB.
pub fn get_by_ids(
    storage: &Storage,
    ids: &[i64],
) -> rusqlite::Result<Vec<ScanRow>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!(
        "SELECT id, batch_id, content, kind, monitor_index, scanned_at, \
                opened, opened_at, copied, copied_at
         FROM scans WHERE id IN ({placeholders})"
    );
    let conn = storage.lock();
    let mut stmt = conn.prepare(&sql)?;
    let fetched = stmt
        .query_map(
            params_from_iter(ids.iter()),
            ScanRow::from_db_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    // Preserve caller-provided order so JSON output and per-row mark
    // operations remain deterministic.
    let mut by_id: std::collections::HashMap<i64, ScanRow> =
        fetched.into_iter().map(|r| (r.id, r)).collect();
    Ok(ids.iter().filter_map(|id| by_id.remove(id)).collect())
}

/// Filter parameters for [`history_query`]. Matches the §7 frontend type.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFilter {
    pub search: Option<String>,
    pub kind: Option<QrKind>,
    /// `None` is equivalent to [`StatusFilter::All`].
    pub status: Option<StatusFilter>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    /// `None` is equivalent to [`SortDir::Desc`] (newest first).
    pub sort_dir: Option<SortDir>,
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
        "SELECT id, batch_id, content, kind, monitor_index, scanned_at, \
                opened, opened_at, copied, copied_at
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
    match filter.status {
        Some(StatusFilter::Opened) => sql.push_str(" AND opened = 1"),
        Some(StatusFilter::Copied) => sql.push_str(" AND copied = 1"),
        Some(StatusFilter::Untouched) => {
            sql.push_str(" AND opened = 0 AND copied = 0")
        }
        Some(StatusFilter::All) | None => {}
    }
    if let Some(from) = filter.from {
        sql.push_str(" AND scanned_at >= ?");
        bound.push(Box::new(from));
    }
    if let Some(to) = filter.to {
        sql.push_str(" AND scanned_at <= ?");
        bound.push(Box::new(to));
    }
    // SortDir variants resolve to literal SQL keywords, so format!-ing
    // them in is safe (no user-controlled SQL).
    let dir = filter.sort_dir.unwrap_or_default().as_sql();
    sql.push_str(&format!(
        " ORDER BY scanned_at {dir}, id {dir} LIMIT ? OFFSET ?"
    ));
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

/// Mark a row as copied, stamping `copied_at` to `now_ms` (Unix epoch ms).
/// Returns the number of rows updated — `0` if the row doesn't exist.
pub fn mark_copied(
    storage: &Storage,
    id: i64,
    now_ms: i64,
) -> rusqlite::Result<usize> {
    let conn = storage.lock();
    conn.execute(
        "UPDATE scans SET copied = 1, copied_at = ?1 WHERE id = ?2",
        params![now_ms, id],
    )
}

/// Bulk-mark rows as opened with a shared `opened_at`. Single UPDATE so
/// the mutex is held once. Empty input is a no-op.
pub fn mark_opened_many(
    storage: &Storage,
    ids: &[i64],
    now_ms: i64,
) -> rusqlite::Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!(
        "UPDATE scans SET opened = 1, opened_at = ? WHERE id IN ({placeholders})"
    );
    let conn = storage.lock();
    let mut stmt = conn.prepare(&sql)?;
    let mut binds: Vec<Box<dyn ToSql>> = Vec::with_capacity(ids.len() + 1);
    binds.push(Box::new(now_ms));
    for id in ids {
        binds.push(Box::new(*id));
    }
    stmt.execute(params_from_iter(binds.iter().map(|b| b.as_ref())))
}

/// Bulk-mark rows as copied with a shared `copied_at`. Single UPDATE so
/// the mutex is held once. Empty input is a no-op.
pub fn mark_copied_many(
    storage: &Storage,
    ids: &[i64],
    now_ms: i64,
) -> rusqlite::Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!(
        "UPDATE scans SET copied = 1, copied_at = ? WHERE id IN ({placeholders})"
    );
    let conn = storage.lock();
    let mut stmt = conn.prepare(&sql)?;
    let mut binds: Vec<Box<dyn ToSql>> = Vec::with_capacity(ids.len() + 1);
    binds.push(Box::new(now_ms));
    for id in ids {
        binds.push(Box::new(*id));
    }
    stmt.execute(params_from_iter(binds.iter().map(|b| b.as_ref())))
}

/// Delete one row. Returns the number of rows deleted.
pub fn delete_by_id(storage: &Storage, id: i64) -> rusqlite::Result<usize> {
    let conn = storage.lock();
    conn.execute("DELETE FROM scans WHERE id = ?1", params![id])
}

/// Delete multiple rows in one statement. Empty input is a no-op.
pub fn delete_by_ids(
    storage: &Storage,
    ids: &[i64],
) -> rusqlite::Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!("DELETE FROM scans WHERE id IN ({placeholders})");
    let conn = storage.lock();
    let mut stmt = conn.prepare(&sql)?;
    stmt.execute(params_from_iter(ids.iter()))
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
        let ids = insert_batch(
            &storage,
            &[NewScanRow {
                batch_id: "B-1",
                content: "https://example.com",
                kind: QrKind::Url,
                monitor_index: 0,
                scanned_at: 1_234_567_890_000,
            }],
        )
        .expect("insert");
        let id = ids[0];

        let row = get_by_id(&storage, id).expect("get").expect("some");
        assert_eq!(row.id, id);
        assert_eq!(row.batch_id, "B-1");
        assert_eq!(row.content, "https://example.com");
        assert_eq!(row.kind, QrKind::Url);
        assert_eq!(row.monitor_index, 0);
        assert_eq!(row.scanned_at, 1_234_567_890_000);
        assert!(!row.opened);
        assert_eq!(row.opened_at, None);
        assert!(!row.copied);
        assert_eq!(row.copied_at, None);
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
            let ids = insert_batch(
                &storage,
                &[NewScanRow {
                    batch_id: "B-kind",
                    content: "x",
                    kind: *k,
                    monitor_index: 0,
                    scanned_at: i as i64,
                }],
            )
            .expect("insert");
            let row = get_by_id(&storage, ids[0]).expect("get").expect("some");
            assert_eq!(row.kind, *k, "kind round-trip failed for {:?}", k);
        }
    }

    fn empty_filter(limit: i64) -> HistoryFilter {
        HistoryFilter {
            search: None,
            kind: None,
            status: None,
            from: None,
            to: None,
            sort_dir: None,
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
    fn history_query_ascending_sort_reverses_order() {
        let storage = fresh();
        seed_history(&storage);
        let mut f = empty_filter(10);
        f.sort_dir = Some(SortDir::Asc);
        let rows = history_query(&storage, &f).expect("query");
        let scans: Vec<i64> = rows.iter().map(|r| r.scanned_at).collect();
        assert_eq!(scans, vec![100, 200, 300, 400], "ascending should be oldest first");
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
    fn history_query_status_filter_splits_opened_copied_untouched() {
        let storage = fresh();
        let ids = seed_history(&storage);
        // ids[0]: opened only, ids[1]: copied only, ids[2]: both,
        // ids[3]: untouched
        mark_opened(&storage, ids[0], 500).expect("mark0 opened");
        mark_copied(&storage, ids[1], 510).expect("mark1 copied");
        mark_opened(&storage, ids[2], 520).expect("mark2 opened");
        mark_copied(&storage, ids[2], 530).expect("mark2 copied");

        let mut f = empty_filter(10);
        f.status = Some(StatusFilter::Opened);
        let opened = history_query(&storage, &f).expect("query opened");
        assert_eq!(opened.len(), 2);
        assert!(opened.iter().all(|r| r.opened));

        let mut f = empty_filter(10);
        f.status = Some(StatusFilter::Copied);
        let copied = history_query(&storage, &f).expect("query copied");
        assert_eq!(copied.len(), 2);
        assert!(copied.iter().all(|r| r.copied));

        let mut f = empty_filter(10);
        f.status = Some(StatusFilter::Untouched);
        let untouched = history_query(&storage, &f).expect("query untouched");
        assert_eq!(untouched.len(), 1);
        assert!(untouched.iter().all(|r| !r.opened && !r.copied));

        let mut f = empty_filter(10);
        f.status = Some(StatusFilter::All);
        let all = history_query(&storage, &f).expect("query all");
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn mark_copied_sets_copied_and_copied_at() {
        let storage = fresh();
        let ids = seed_history(&storage);
        let updated = mark_copied(&storage, ids[0], 4321).expect("mark");
        assert_eq!(updated, 1);
        let row = get_by_id(&storage, ids[0]).expect("get").expect("some");
        assert!(row.copied);
        assert_eq!(row.copied_at, Some(4321));
        // Opened state untouched.
        assert!(!row.opened);
    }

    #[test]
    fn mark_copied_missing_row_returns_zero() {
        let storage = fresh();
        let updated = mark_copied(&storage, 9999, 1).expect("mark");
        assert_eq!(updated, 0);
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
    fn get_by_ids_returns_rows_in_caller_order() {
        let storage = fresh();
        let ids = seed_history(&storage);
        // Request in a deliberately scrambled order; ensure the result
        // preserves that ordering rather than DB-natural order.
        let requested = vec![ids[2], ids[0], ids[3], ids[1]];
        let rows = get_by_ids(&storage, &requested).expect("get_by_ids");
        let returned: Vec<i64> = rows.iter().map(|r| r.id).collect();
        assert_eq!(returned, requested);
    }

    #[test]
    fn get_by_ids_skips_missing_ids() {
        let storage = fresh();
        let ids = seed_history(&storage);
        let rows =
            get_by_ids(&storage, &[ids[0], 9999, ids[1]]).expect("get_by_ids");
        let returned: Vec<i64> = rows.iter().map(|r| r.id).collect();
        assert_eq!(returned, vec![ids[0], ids[1]]);
    }

    #[test]
    fn get_by_ids_empty_input_is_no_op() {
        let storage = fresh();
        seed_history(&storage);
        let rows = get_by_ids(&storage, &[]).expect("get_by_ids");
        assert!(rows.is_empty());
    }

    #[test]
    fn mark_opened_many_stamps_every_id_and_skips_others() {
        let storage = fresh();
        let ids = seed_history(&storage);
        let updated = mark_opened_many(&storage, &[ids[0], ids[2]], 7777)
            .expect("mark_opened_many");
        assert_eq!(updated, 2);
        for (i, id) in ids.iter().enumerate() {
            let row = get_by_id(&storage, *id).expect("get").expect("some");
            if i == 0 || i == 2 {
                assert!(row.opened, "id {} should be opened", id);
                assert_eq!(row.opened_at, Some(7777));
            } else {
                assert!(!row.opened, "id {} should still be unopened", id);
                assert_eq!(row.opened_at, None);
            }
        }
    }

    #[test]
    fn mark_copied_many_stamps_every_id_and_skips_others() {
        let storage = fresh();
        let ids = seed_history(&storage);
        let updated = mark_copied_many(&storage, &[ids[1], ids[3]], 9999)
            .expect("mark_copied_many");
        assert_eq!(updated, 2);
        for (i, id) in ids.iter().enumerate() {
            let row = get_by_id(&storage, *id).expect("get").expect("some");
            if i == 1 || i == 3 {
                assert!(row.copied);
                assert_eq!(row.copied_at, Some(9999));
            } else {
                assert!(!row.copied);
                assert_eq!(row.copied_at, None);
            }
        }
    }

    #[test]
    fn mark_opened_many_empty_input_is_no_op() {
        let storage = fresh();
        seed_history(&storage);
        let updated = mark_opened_many(&storage, &[], 1).expect("mark");
        assert_eq!(updated, 0);
    }

    #[test]
    fn delete_by_ids_removes_listed_rows_only() {
        let storage = fresh();
        let ids = seed_history(&storage);
        let removed =
            delete_by_ids(&storage, &[ids[0], ids[2]]).expect("delete_by_ids");
        assert_eq!(removed, 2);
        let remaining =
            history_query(&storage, &empty_filter(10)).expect("query");
        let remaining_ids: Vec<i64> = remaining.iter().map(|r| r.id).collect();
        assert_eq!(remaining_ids.len(), 2);
        assert!(remaining_ids.contains(&ids[1]));
        assert!(remaining_ids.contains(&ids[3]));
    }

    #[test]
    fn delete_by_ids_empty_input_is_no_op() {
        let storage = fresh();
        seed_history(&storage);
        let removed = delete_by_ids(&storage, &[]).expect("delete_by_ids");
        assert_eq!(removed, 0);
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
