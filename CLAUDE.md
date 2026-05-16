# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

It is the canonical context for any Claude session working on this project. Read it fully before writing code or making architectural decisions. If something here conflicts with what the user asks, ask before overriding.

---

## 1. What we're building

**qrab** is a lightweight desktop utility that captures QR codes visible anywhere on the user's screen and decodes them to text/URLs — solving the problem of needing to point a phone camera at a QR code that's already on the PC. The app handles "readable" codes (links, plain text, Wi-Fi, vCards). Codes that require a phone (mobile-only auth flows, app-bound credentials) are out of scope by design.

Core promise: **press a hotkey → see the decoded content within a second → copy or open it**. No friction, no setup per scan.

This project is also a **portfolio piece**. Code quality, commit hygiene, README, and architecture diagrams matter — but not at the cost of over-engineering. Clean and small beats clever and abstract.

---

## 1.5. Current state (as of 2026-05-15)

**v1.0 feature-complete, robustness pass applied.** All five phases from §14 shipped, post-ship audit (perf / bugs / dead-code / security) addressed, plus a second robustness wave (single-instance, hotkey-status surfaced, macOS permission helper, window-state persistence, scan-flow logging, frontend Vitest setup). Awaiting v1.0 tag pending the in-flight app icon work.

What's shipped end-to-end:
- Capture (`xcap`) + decode (`rqrr`) with the `Capturer` / `Decoder` trait seams from §4.
- All IPC commands in §8 plus: `copy_row`, `copy_rows_as_json`, `get_app_info`, `get_hotkey_status`, `open_screen_recording_prefs`, `consume_pending_scan`, `get_screenshot_monitors`, `get_screenshot_monitor_png`, `get_default_settings`. `mark_opened` was deleted as dead code (open_url marks internally).
- SQLite WAL + schema v2 with `copied`/`copied_at` (v1 docs in §7) and full filter/sort plumbing (`status` enum, `sort_dir` enum, from/to date range).
- Hash routes: `#results`, `#region`, `#history`, `#settings`, `#config`. Each non-Results route has a back button.
- Tray menu: Scan now, View history, Settings…, Config…, Quit. Left-click surfaces the window without scanning.
- Tauri plugins: opener, clipboard-manager, global-shortcut, log, store, autostart, single-instance, window-state.
- Per-launch file logging (`<repo>/logs/qrab-<timestamp>.log`) covering setup, scan/copy/open flow, error paths, and autostart drift detection.
- Window: proportional sizing on first launch, then `tauri-plugin-window-state` restores user's size/position.
- Autostart with OS-truth reconcile (external disabling wins).
- 48 Rust unit + 4 Rust integration + 17 TS unit tests, all green.

Tested on: Windows 11 (development host). macOS and Linux paths exist via Tauri abstractions but are not runtime-verified in this branch — see README.

Deferred to v1.1 (Phase 6):
- App icon polish (in progress).
- CI matrix (GitHub Actions).
- Code signing (Windows EV, macOS notarization).
- README screenshots once icon set lands.
- `v1.0` git tag.

---

## 2. Hard constraints

These are not negotiable without checking with the user first:

- **Lightweight.** Target: binary under 18 MB (SQLite adds ~1–2 MB to the earlier 15 MB target), idle RAM under 80 MB, cold start under 1 second.
- **Modern UI.** No native widgets that look like Windows XP. Clean typography, sensible spacing, subtle motion, dark-mode aware.
- **Cross-platform desktop.** Windows, macOS, and Linux (both X11 and Wayland) must all work. No platform left behind. If a feature can't work on one platform, it degrades gracefully there, never breaks the build.
- **No bundled Chromium, no bundled Python, no JVM.** Tauri's webview, Rust binary, that's it.
- **User safety.** QR contents are untrusted input. The app never auto-opens URLs without explicit user action. Bulk-open requires confirmation above the threshold defined in §10.

---

## 3. Stack (decided — do not swap without asking)

| Layer | Choice | Why |
|---|---|---|
| Shell | **Tauri 2.x** | Native webview, tiny binary, multi-platform, mature plugin system |
| Backend | **Rust** (stable, edition 2021+) | Required by Tauri, fast, good ecosystem for our needs |
| Frontend framework | **SolidJS** | Fastest reactive framework, minimal runtime, fits the lightweight goal |
| Frontend language | **TypeScript** (strict mode) | Type safety across the IPC boundary |
| Bundler | **Vite** | Tauri default, fast HMR |
| Styling | **Tailwind CSS v4** | Utility-first, no runtime cost, easy theming |
| Component primitives | **Kobalte** (`@kobalte/core` + `@kobalte/tailwindcss`) | Headless + WAI-ARIA accessible; Solid-native; only used where behavior is non-trivial |
| Icons | **lucide-solid** | Clean, consistent, tree-shakeable |
| Database | **SQLite via rusqlite (bundled)** | Sort/filter/search on history requires real queries; rusqlite + bundled SQLite avoids system deps |

### Key Rust crates

- `xcap` — cross-platform screen capture
- `rqrr` — pure-Rust QR decoder, finds multiple codes per image
- `image` — image manipulation (xcap returns its own types; convert to `image::RgbaImage`)
- `rusqlite` with features `["bundled"]` — embedded SQLite, no system dependency
- `serde` + `serde_json` — IPC serialization
- `chrono` — timestamps
- `ulid` — sortable IDs for scan batches
- `tauri-plugin-global-shortcut` — hotkey registration
- `tauri-plugin-clipboard-manager` — copy to clipboard
- `tauri-plugin-opener` — open URLs in default browser
- `tauri-plugin-store` — settings persistence
- `tauri-plugin-autostart` — optional: launch on login
- `anyhow` — error handling in non-command code
- `thiserror` — typed errors at module boundaries

If `rqrr` proves limiting on real-world codes, evaluate `bardecoder` or `zxing-cpp` via FFI before swapping.

---

## 4. Architecture principles

The project follows **SOLID at module level** with one specific abstraction point. Do not introduce additional abstractions speculatively.

**Required abstractions:**
- `trait Capturer` — abstracts screen capture so the decoder pipeline is testable without a real display. Production impl wraps `xcap`. Test impl returns a fixed image.
- `trait Decoder` — abstracts QR decoding so capture logic is testable independently. Production impl wraps `rqrr`.

**Storage does not need a trait.** Tests use SQLite with `:memory:` databases — the real implementation is the test implementation. Adding a `HistoryRepository` trait with two impls would be ceremony with no benefit.

**Rules:**
- SRP per module. If a module does two unrelated things, split it.
- Errors are typed at module boundaries (`thiserror`), converted to `String` only at the `#[tauri::command]` boundary.
- No `unwrap()` / `expect()` in production paths.
- No abstraction whose only justification is "we might need it later."
- No Java/C#-style class hierarchies translated to Rust. Use Rust idioms (traits + generics, sum types, ownership) instead of GoF patterns where they fit naturally.

---

## 5. End-to-end flow

```
┌──────────────────────────────────────────────────────────────┐
│  Tray icon (always running, no main window visible)          │
│         │                                                    │
│         └──► Global hotkey: Ctrl+Shift+Q (Cmd on macOS)      │
│                  │                                           │
│                  ▼                                           │
│         ┌────────────────────────┐                           │
│         │  1. Capture all        │   (Rust, spawn_blocking)  │
│         │     monitors (xcap)    │                           │
│         └────────────────────────┘                           │
│                  │                                           │
│                  ▼                                           │
│         ┌────────────────────────┐                           │
│         │  2. Decode every       │                           │
│         │     screenshot (rqrr)  │                           │
│         └────────────────────────┘                           │
│                  │                                           │
│          ┌───────┴───────┐                                   │
│          │               │                                   │
│      results > 0      results = 0                            │
│          │               │                                   │
│          ▼               ▼                                   │
│   ┌──────────┐    ┌──────────────────┐                       │
│   │ Show     │    │ Show region      │                       │
│   │ results  │    │ selector over    │                       │
│   │ window   │    │ captured screen- │                       │
│   │          │    │ shot. User drags │                       │
│   │ + persist│    │ rectangle. Crop, │                       │
│   │   to DB  │    │ re-decode crop.  │                       │
│   └──────────┘    └──────────────────┘                       │
│                          │                                   │
│                          ▼                                   │
│                    (loops back to results or empty state)    │
└──────────────────────────────────────────────────────────────┘
```

### Region selection — design rationale

The chosen approach is **always-fullscreen-capture, then in-app crop on the captured image**. We do *not* use a transparent overlay window for live drag-selection over the desktop.

Why this design:
- One capture path across all platforms (no per-OS overlay window code).
- The screen state at capture time is frozen — works for short-lived QRs (auth popups, ephemeral codes).
- Region selector is pure web UI (an `<img>` plus a rubber-band overlay in Solid) — no platform code.
- User can re-select the region without re-capturing.

The full screenshot is held in memory only between capture and dismissal of the results/region-select window. Freed immediately after.

**Window strategy:**
1. App starts hidden; tray icon only.
2. Hotkey → run `scan_screen` → auto-decode entire screenshot.
3. If results > 0: show results window with them. User can optionally click "Select region" to refine if the wrong QR was picked or there's an unscanned area.
4. If results = 0: show region selector directly with the captured screenshot. User drags a rectangle; cropped region is re-decoded.
5. Results window auto-hides on Esc, blur, or after the user copies/opens (configurable in settings).
6. Settings and history windows are separate Tauri windows opened from the tray menu.
7. No telemetry. No network calls except when the user explicitly opens a URL.

---

## 6. Project structure

```
qrab/
├── CLAUDE.md                       ← this file
├── README.md                       ← portfolio-quality, with screenshots
├── package.json
├── package-lock.json               ← npm
├── vite.config.ts
├── tsconfig.json
├── tailwind.config.ts
├── index.html
├── src/                            ← Solid frontend
│   ├── index.tsx                   ← entry; routes to window kind via URL hash
│   ├── windows/
│   │   ├── ResultsWindow.tsx
│   │   ├── RegionSelectWindow.tsx
│   │   ├── HistoryWindow.tsx
│   │   └── SettingsWindow.tsx
│   ├── components/
│   │   ├── ui/                     ← Kobalte-based primitives, styled with Tailwind
│   │   │   ├── Button.tsx          ← hand-rolled (no Kobalte needed)
│   │   │   ├── Card.tsx            ← hand-rolled
│   │   │   ├── Dialog.tsx          ← wraps @kobalte/core/dialog
│   │   │   ├── DropdownMenu.tsx    ← wraps @kobalte/core/dropdown-menu
│   │   │   ├── Select.tsx          ← wraps @kobalte/core/select
│   │   │   ├── TextField.tsx       ← wraps @kobalte/core/text-field
│   │   │   ├── Checkbox.tsx        ← wraps @kobalte/core/checkbox
│   │   │   ├── Switch.tsx          ← wraps @kobalte/core/switch
│   │   │   └── Toast.tsx           ← wraps @kobalte/core/toast
│   │   ├── ResultCard.tsx
│   │   ├── EmptyState.tsx
│   │   ├── HotkeyInput.tsx
│   │   ├── RegionSelector.tsx      ← canvas-based rubber-band selection
│   │   ├── HistoryTable.tsx
│   │   ├── HistoryFilters.tsx
│   │   └── ConfirmOpenAll.tsx
│   ├── lib/
│   │   ├── ipc.ts                  ← typed wrappers over invoke()
│   │   ├── types.ts                ← shared types mirroring Rust structs
│   │   └── classify.ts             ← frontend URL/kind helpers for display
│   └── styles.css                  ← Tailwind directives
└── src-tauri/                      ← Rust backend
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── build.rs
    ├── capabilities/
    │   └── default.json            ← permissions
    ├── icons/
    └── src/
        ├── main.rs                 ← entrypoint
        ├── lib.rs                  ← run() fn, plugin & state registration
        ├── capture/
        │   ├── mod.rs              ← Capturer trait
        │   └── xcap_impl.rs        ← production impl
        ├── decoder/
        │   ├── mod.rs              ← Decoder trait, kind classification
        │   └── rqrr_impl.rs        ← production impl
        ├── storage/
        │   ├── mod.rs              ← public API, opens & holds Connection
        │   ├── schema.rs           ← migrations
        │   └── queries.rs          ← CRUD operations
        ├── commands.rs             ← #[tauri::command] functions
        ├── tray.rs                 ← tray menu + click handlers
        ├── hotkey.rs               ← global shortcut setup
        ├── windows.rs              ← Tauri window show/hide helpers
        ├── settings.rs             ← persisted user settings
        └── error.rs                ← AppError + From impls
```

---

## 7. Data model

### SQLite schema (v1)

```sql
CREATE TABLE scans (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    batch_id        TEXT    NOT NULL,             -- ULID, groups entries from one scan
    content         TEXT    NOT NULL,
    kind            TEXT    NOT NULL,             -- 'url' | 'text' | 'wifi' | 'vcard' | 'email' | 'phone' | 'other'
    monitor_index   INTEGER NOT NULL,
    scanned_at      INTEGER NOT NULL,             -- Unix epoch milliseconds
    opened          INTEGER NOT NULL DEFAULT 0,   -- 0 / 1
    opened_at       INTEGER                       -- Unix epoch ms, NULL if never opened
);

CREATE INDEX idx_scans_scanned_at ON scans(scanned_at DESC);
CREATE INDEX idx_scans_batch      ON scans(batch_id);
CREATE INDEX idx_scans_kind       ON scans(kind);
CREATE INDEX idx_scans_opened     ON scans(opened);
```

**Notes:**
- `id INTEGER PRIMARY KEY` solves the same-timestamp collision concern: every row gets a unique, monotonically increasing ID regardless of how close timestamps are.
- `batch_id` (ULID) groups multiple QRs found in the same hotkey press. Sortable, no extra index needed beyond the explicit one.
- `opened` and `opened_at` track user interaction without separate event tables — we don't need a full audit log, just "did they open it."
- Timestamps as `INTEGER` (Unix ms) for compact storage and trivial range queries. Format in the frontend.

### Search

Start with `WHERE content LIKE '%query%'` — fast enough for tens of thousands of rows with the indexes above. If full-text relevance ranking ever becomes a real need, add an FTS5 virtual table mirroring `content` and switch the search query. Don't add FTS5 preemptively.

### Migrations

Use SQLite's `PRAGMA user_version` for schema versioning. On app start, read `user_version`, compare to compiled-in `LATEST_VERSION`, run each pending migration in a transaction, set `user_version` to new value. Migrations live in `storage/schema.rs` as an array of SQL statements indexed by target version.

### Database location

`tauri::Manager::path().app_data_dir()? / "qrab.db"`. Enable WAL mode on first open (`PRAGMA journal_mode=WAL`) for better concurrent reads. The DB is held as `Arc<Mutex<Connection>>` in Tauri state — single connection is fine for this app's volume.

### Rust ↔ TS types

```rust
// src-tauri/src/storage/queries.rs
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRow {
    pub id: i64,
    pub batch_id: String,
    pub content: String,
    pub kind: QrKind,
    pub monitor_index: i64,
    pub scanned_at: i64,        // ms since epoch
    pub opened: bool,
    pub opened_at: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QrKind { Url, Text, Wifi, Vcard, Email, Phone, Other }
```

```ts
// src/lib/types.ts
export type QrKind = 'url' | 'text' | 'wifi' | 'vcard' | 'email' | 'phone' | 'other';

export interface ScanRow {
  id: number;
  batchId: string;
  content: string;
  kind: QrKind;
  monitorIndex: number;
  scannedAt: number;     // ms
  opened: boolean;
  openedAt: number | null;
}

export interface HistoryFilter {
  search?: string;
  kind?: QrKind;
  openedOnly?: boolean;
  unopenedOnly?: boolean;
  from?: number;         // ms
  to?: number;           // ms
  limit: number;
  offset: number;
}
```

Classify `kind` in Rust from the decoded string prefix (`http`/`https`, `WIFI:`, `BEGIN:VCARD`, `mailto:`, `tel:`).

---

## 8. Tauri commands (the IPC surface)

Keep this list small and stable. Add new commands only when a new feature genuinely needs one.

| Command | Args | Returns | Notes |
|---|---|---|---|
| `scan_screen` | — | `ScanResult` | Captures all monitors, decodes. Persists results to DB. Returns the new rows and a handle to the held screenshot for optional region select. |
| `scan_region` | `screenshot_id`, `bounds` | `ScanResult` | Decodes a cropped region of the held screenshot. Persists if found. |
| `mark_opened` | `id: i64` | `()` | Updates `opened=1, opened_at=now`. |
| `open_url` | `id: i64` | `()` | Opens the URL via `tauri-plugin-opener` and marks opened. |
| `open_urls_bulk` | `ids: Vec<i64>`, `confirmed: bool` | `BulkOpenResult` | Opens many URLs in parallel. Server refuses if `ids.len() > 3 && !confirmed`. |
| `history_query` | `HistoryFilter` | `Vec<ScanRow>` | Paginated, sorted. |
| `history_delete` | `id: i64` | `()` | |
| `history_clear` | — | `()` | Wipes table; UI-confirmed only. |
| `copy_to_clipboard` | `text: String` | `()` | |
| `get_settings` / `set_settings` | — / `Settings` | `Settings` / `()` | Via `tauri-plugin-store`. |

All commands return `Result<T, String>`. Underlying errors stay typed (`AppError`) until the boundary.

---

## 9. Implementation notes

### Capture

```rust
// src-tauri/src/capture/mod.rs
pub trait Capturer: Send + Sync {
    fn capture_all(&self) -> Result<Vec<MonitorImage>, CaptureError>;
}

pub struct MonitorImage {
    pub index: usize,
    pub image: image::RgbaImage,
}
```

Production impl in `xcap_impl.rs` wraps `xcap::Monitor::all()` and `capture_image()`. Run capture inside `tokio::task::spawn_blocking` from the command — xcap is sync and may take tens of ms.

**Gotchas:**
- Wayland: first call triggers xdg-desktop-portal permission prompt. Document in README.
- macOS: Screen Recording permission must be granted in System Settings. Surface a clear error if capture fails with permission-denied.
- HiDPI: images come back at physical resolution, not logical. rqrr handles this fine.

### Decode

```rust
// src-tauri/src/decoder/mod.rs
pub trait Decoder: Send + Sync {
    fn decode(&self, img: &image::RgbaImage) -> Vec<String>;
}
```

Production impl uses `rqrr::PreparedImage::prepare(...).detect_grids()`. Run inside `spawn_blocking` — rqrr is CPU-bound.

**Dedup:** within a single scan, identical content from multiple monitors counts as one entry but record which monitor (pick the first one found). Across scans (different `batch_id`), always insert new rows — each scan is a distinct event.

**Classification:** after decoding, run a small `classify_kind(&str) -> QrKind` function on each result. Pure function, easy to unit-test.

### Region crop

The full screenshot from the most recent scan is kept in a `Mutex<Option<HeldScreenshot>>` in Tauri state, keyed by a `screenshot_id`. `scan_region` takes that `screenshot_id` and `bounds` `{x, y, w, h, monitorIndex}`, crops with `image::imageops::crop_imm`, and runs the decoder on the crop.

The screenshot is cleared when:
- A new scan replaces it.
- Both results window and region-select window are closed.
- App-defined TTL (e.g. 60 seconds) expires, to avoid holding tens of MB indefinitely.

The Solid `RegionSelector` component uses an HTML canvas overlaid on an `<img>` with the screenshot as `src`. Pass the screenshot via `convertFileSrc` over a temp file rather than a base64 data URL — keeps frontend memory low.

### Open all (safety)

```rust
#[tauri::command]
pub async fn open_urls_bulk(
    app: AppHandle,
    ids: Vec<i64>,
    confirmed: bool,
) -> Result<BulkOpenResult, String> {
    if ids.len() > 3 && !confirmed {
        return Err("Confirmation required for opening more than 3 URLs".into());
    }
    // ... open in parallel, mark each opened
}
```

**Frontend rule:** if `urlIds.length > 3`, render `<ConfirmOpenAll>` with the full list of URLs visible (not just count). User reads them, clicks "Open all N" or "Cancel." Only after confirmation, call the IPC with `confirmed: true`.

**Filtering:** "Open all" operates only on `kind === 'url'`. Other kinds in the selection are silently skipped (with a small "+N non-URL items not opened" footnote).

**Implementation:** open URLs in parallel via `futures::future::join_all` of opener plugin calls. Update each row's `opened` status after success.

### Settings

Stored via `tauri-plugin-store` (JSON, app data dir). Fields:
- `hotkey`: string (e.g. `"Ctrl+Shift+Q"`, `"Cmd+Shift+Q"`)
- `autostart`: bool
- `autoCopyOnSingleResult`: bool
- `theme`: `"system" | "light" | "dark"`
- `closeAfterCopy`: bool
- `closeAfterOpen`: bool

Hotkey changes re-register the global shortcut live.

---

## 10. UI/UX requirements

### Results window
Feels like Raycast/Spotlight, not a dialog box.
- One result: large card with content, kind icon, "Copy" and (if URL) "Open" buttons. Enter triggers the primary action.
- Multiple results: vertically stacked cards, keyboard-navigable with arrow keys, Enter on focused card. Footer shows "Open all" and "Copy all" buttons.
- Zero results: drops into region selector directly (the captured screenshot is shown with a rubber-band selector).
- Subtle fade+scale animation on appear (~150ms). Nothing flashy.
- Window: `decorations: false`, `transparent: true`, `alwaysOnTop: false`, `skipTaskbar: true`, sized proportionally to the primary monitor on launch (~60% capped at 1200×800, floored at 520×420), resizable. The window stacks normally so the user can park it behind other apps; the tray icon brings it back forward.

### Region selector
- Shows the screenshot at fit-to-window scale.
- Rubber-band drag to define a rectangle. Dimmed overlay outside the rectangle.
- "Decode" button confirms. Esc cancels.
- Re-drag freely until decoded.
- If decode of region returns no results, show inline "Nothing found in that region — try again" without leaving the selector.

### History window
- Table with columns: timestamp (relative + absolute on hover), content (truncated with full on hover), kind (icon + label), opened (checkmark or dash), actions (open / copy / delete).
- Top bar: search input, kind filter dropdown, "Opened" filter (all / opened / unopened), date range picker.
- Pagination or virtualized list — don't load 10K rows into the DOM.
- Multi-select with checkboxes → bulk actions: open selected URLs (subject to the 3-threshold), copy as JSON, delete.
- "Clear history" button at the bottom, requires confirmation modal.

### Open-all confirmation modal
- Shown whenever `urlIds.length > 3`.
- Lists every URL (scrollable if long). User can spot phishing domains before opening.
- Primary button: "Open all N". Secondary: "Cancel". No third option.

### General
- Respect `prefers-color-scheme`. Dark mode default.
- All interactive elements have visible focus states (keyboard nav must work).
- No emoji-as-icons. Use lucide-solid.
- Loading state (skeleton or subtle spinner) if any async op exceeds 200ms.

---

## 11. Frontend conventions (Solid)

- Functional components only.
- State: `createSignal` for local, `createStore` for structured state, `createResource` for async fetches from Tauri.
- Wrap every `invoke` in `src/lib/ipc.ts` so calls are typed and centralized.
  ```ts
  import { invoke } from '@tauri-apps/api/core';
  import type { ScanRow, HistoryFilter } from './types';
  export const scanScreen = () => invoke<ScanResult>('scan_screen');
  export const historyQuery = (f: HistoryFilter) => invoke<ScanRow[]>('history_query', { filter: f });
  ```
- Tailwind-first. No CSS-in-JS. `clsx` for conditional classes.
- Components under ~150 lines. Split when they grow.
- No global state library — Solid's primitives are enough. If something truly is global (e.g. settings), put it in a single `createStore` exported from `src/lib/state.ts`.

### Kobalte rules

These are not suggestions — follow them so the codebase stays consistent.

- **Wrap once, use everywhere.** Every Kobalte primitive used in the app lives in exactly one wrapper file under `src/components/ui/` (e.g. `ui/Dialog.tsx`). App components import from `ui/`, never directly from `@kobalte/core`. This makes a design-token change a one-file edit.
- **Compound component pattern is preserved through the wrappers.** Re-export the parts (`Dialog.Root`, `Dialog.Trigger`, `Dialog.Content`, etc.) — don't collapse them into a single `<Dialog title="..." />` API. The composition is the value.
- **Import as namespaces** in the wrapper file:
  ```tsx
  import * as KDialog from "@kobalte/core/dialog";
  ```
- **Install `@kobalte/tailwindcss` and use `ui-*` modifiers** for state-driven styling (`ui-expanded:`, `ui-highlighted:`, `ui-disabled:`, etc.). Verify v4 plugin compatibility on first install; if there's a lag, fall back to native v4 attribute selectors (`data-[expanded]:...`).
- **Uncontrolled by default.** Don't pass `open` / `onOpenChange` unless external state genuinely needs to drive open/close (e.g. opening the confirmation modal from a non-trigger source like a keyboard shortcut). Premature control is the most common Kobalte misuse.
- **Use `TextField` for every form input.** It wires up label/description/error message a11y automatically. Don't roll your own `<label>` + `<input>` + `<span>` for errors.
- **Portal-rendered components (Dialog, DropdownMenu, Popover, Toast) escape the parent DOM tree.** Plan z-index globally — define a small z-tier scheme in `styles.css` and stick to it. CSS variables that need to apply to portal content must be set on `:root`, not on a wrapper ancestor.
- **Don't reach into refs or use `as` to bypass parts of a Kobalte component.** If you find yourself doing that, you've missed a sub-component in the anatomy. Read the docs before going off-road.
- **Components NOT to use from Kobalte in this app:** Accordion, HoverCard, NavigationMenu, Pagination (roll your own — it's 30 lines).
- **Hand-roll, don't Kobalte:** Button, Card, simple containers. Kobalte is for behavior + a11y; plain visual elements don't need it.
- **No Kobalte Table component exists.** History table is hand-rolled HTML. If sorting/selection/virtualization grow complex, evaluate `@tanstack/solid-table` — but start simple.

---

## 12. Development commands

```bash
# install deps
npm install

# dev mode (hot reload) — runs Vite + Tauri together
npm run tauri dev

# production build
npm run tauri build

# format/lint
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
npx prettier --write src/
npx tsc --noEmit

# tests (Rust)
cargo test --manifest-path src-tauri/Cargo.toml

# run a single Rust test (substring match on test name)
cargo test --manifest-path src-tauri/Cargo.toml -- <test_name_substring>
```

Add these to `package.json` scripts as they get used regularly. Current scripts are minimal: `start`, `dev`, `build`, `serve`, `tauri` — the format/lint/test commands above are not yet wired up.

---

## 13. Platform gotchas — read before debugging

**Linux/Wayland**
- xdg-desktop-portal must be installed; user's compositor must implement the ScreenCast portal (GNOME, KDE, sway with `xdg-desktop-portal-wlr` all do).
- First scan prompts for permission. Document this in the README.
- Global shortcuts on Wayland are restricted — fall back to tray click if registration fails. Show a non-blocking warning in the tray menu.

**macOS**
- Screen Recording permission required (System Settings → Privacy & Security → Screen Recording). Must re-launch app after granting.
- Hotkey defaults: detect platform; present `Cmd+Shift+Q` on macOS, `Ctrl+Shift+Q` elsewhere.
- For distribution: sign + notarize. Not needed for dev.

**Windows**
- No permission system. Capture just works.
- SmartScreen warns on unsigned builds. For personal use, ignore. For distribution, sign with an EV cert.

**All platforms**
- SQLite WAL mode creates `.db-wal` and `.db-shm` sidecar files. Document this so backups include them.

---

## 14. Build phases — work in this order

Don't skip ahead. Each phase ends with a functional, commitable, demoable state.

**Phase 1: MVP capture-decode-display**
1. ✅ Scaffold project (`npm create tauri-app` → Solid + TypeScript) — done. Add Tailwind v4 + dark mode wiring (Tailwind installed, not yet configured).
2. Define `Capturer` and `Decoder` traits with production impls.
3. Implement `scan_screen` command end-to-end: capture all monitors → decode → return results (in-memory only, no DB yet).
4. Build the results window (single + multi result display, copy, open).
5. Wire global hotkey.
6. Tray icon with "Scan now" and "Quit."

At this point you have a working tool. Ship it to yourself.

**Phase 2: Region selection**
7. Hold the captured screenshot in Tauri state with TTL.
8. Build the `RegionSelector` component (canvas rubber-band over `<img>`).
9. Implement `scan_region` command (crop + decode).
10. Auto-show region selector when fullscreen scan returns zero results.
11. Add "Select region" button to the results window for refinement.

**Phase 3: Persistence + history**
12. Set up SQLite (rusqlite, bundled, WAL mode) and the schema migration system.
13. Persist scan results in `scan_screen` and `scan_region`.
14. Implement `history_query`, `mark_opened`, `history_delete`, `history_clear`.
15. Build the history window: table, filters, search, pagination.
16. Add "View history" to the tray menu.

**Phase 4: Bulk open + safety**
17. Implement `open_urls_bulk` with server-side guard (refuse > 3 without `confirmed: true`).
18. Build `<ConfirmOpenAll>` modal (lists all URLs when count > 3).
19. Wire "Open all" into results and history windows.

**Phase 5: Settings**
20. Settings window: hotkey customization (live re-register), autostart, theme, behavior toggles.
21. Persist via `tauri-plugin-store`.

**Phase 6: Distribution & portfolio polish**
22. App icons for all platforms.
23. README with screenshots, architecture diagram, install instructions.
24. CI builds for Win/macOS/Linux (GitHub Actions matrix).
25. Code signing where applicable.
26. Tag v1.0.

---

## 15. Code style

**Rust**
- `cargo fmt`, no exceptions.
- `cargo clippy -- -D warnings` must pass.
- Public functions get doc comments. Private only when non-obvious.
- `anyhow::Result` in internal code, typed errors (`thiserror`) at module boundaries, `Result<T, String>` at command boundaries.
- No `unwrap()` / `expect()` in production paths.
- Prefer generics over `Box<dyn Trait>` unless heterogeneity is required.

**TypeScript**
- `strict: true`. No `any` — use `unknown` and narrow.
- Named exports preferred over default.
- Async/await over `.then()`.

**Tests**
- Rust unit tests next to the code in `#[cfg(test)] mod tests`.
- Capture/decode logic tested with fixture images (commit a small `tests/fixtures/` directory of PNGs with known QR contents).
- Storage tested against `:memory:` SQLite.

**Commits**
- Conventional commits: `feat:`, `fix:`, `refactor:`, `docs:`, `chore:`, `test:`.
- Each commit builds and passes `cargo clippy` + `tsc --noEmit`.
- Squash merge feature branches into main with descriptive commit messages — this is portfolio-visible.

---

## 16. Out of scope (don't build unless asked)

- OCR or non-QR barcode formats.
- Cloud sync of history.
- Browser extension.
- Mobile companion app.
- Plugin system / scripting.
- Authentication, accounts, payments.
- Full-text search relevance ranking (revisit only if `LIKE` becomes too slow).
- Multi-user / sharing features.

---

## 17. References

- Tauri 2 docs: https://tauri.app/start/
- Tauri 2 plugins: https://tauri.app/plugin/
- SolidJS docs: https://docs.solidjs.com/
- xcap: https://github.com/nashaofu/xcap
- rqrr: https://docs.rs/rqrr/
- rusqlite: https://docs.rs/rusqlite/
- SQLite WAL mode: https://www.sqlite.org/wal.html
- ULID spec: https://github.com/ulid/spec
- Tailwind v4: https://tailwindcss.com/docs
- Kobalte: https://kobalte.dev/
- Kobalte Tailwind plugin: https://www.npmjs.com/package/@kobalte/tailwindcss

---

## 18. Design decisions log

These decisions were made deliberately during planning. Don't reverse them without surfacing the trade-off explicitly to the user.

| Decision | Alternative considered | Why we chose this |
|---|---|---|
| Tauri 2 + Solid | Electron, Flutter, Avalonia | Smallest binary + RAM, modern UI, true cross-platform |
| Rust backend, no Node sidecar | Node child process for some work | Single binary, no runtime dep, lightweight |
| Fullscreen capture then in-app crop | Live transparent overlay region selector | Simpler cross-platform, frozen screen state, no compositor quirks |
| SQLite via rusqlite (bundled) | JSONL append-only file | User explicitly wants sort/filter/search — real UX value justifies the +1–2 MB binary cost |
| INTEGER PK + ULID batch_id | ULID PK only | INTEGER PK is idiomatic in SQLite, ULID still groups batches |
| `LIKE '%query%'` for search initially | FTS5 from day one | Adequate at expected scale; FTS5 is a clean upgrade path |
| One `Capturer`/`Decoder` trait pair | More abstractions (repository, etc.) | These two need mocking for tests; nothing else does |
| Open-all confirmation threshold = 3 | No confirmation / always confirm | 3 balances convenience and safety against tab-bomb / phishing risk |
| Single SQLite connection behind `Mutex` | r2d2 pool | App-level volume is too low to need a pool |
| Kobalte (headless) + hand-rolled Tailwind | Flowbite, SUID, Hope UI, build everything | Flowbite fights Solid's reactivity (vanilla DOM); styled libs hurt portfolio narrative; Kobalte gives a11y for free, Tailwind gives visual ownership |
| Kobalte primitives wrapped in `components/ui/` | Import `@kobalte/core` directly in app components | One-file restyling, consistent design tokens, shadcn-pattern proven at scale |

---

## 19. Working with the user

The user prefers concise, direct communication. They've made deliberate, informed choices on stack and architecture — surface trade-offs rather than second-guessing. When uncertain about a design decision not covered here, ask one focused question rather than guessing. This is a portfolio project, so code quality and commit hygiene matter as much as the running app.