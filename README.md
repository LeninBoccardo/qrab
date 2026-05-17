<p align="left">
  <img src="docs/branding/extracted/primary-logo.png" alt="qrab" height="80" />
</p>

# qrab

A lightweight desktop utility that captures QR codes visible anywhere on your
screen and decodes them to text or URLs — solving the problem of needing to
point a phone camera at a QR code that's already on your PC.

**Core promise:** press a hotkey → see the decoded content within a second → copy or open it.

> Status: feature-complete, pre-1.0. Awaiting icon polish and a `v1.0` tag.
> Built and tested on Windows 11. macOS and Linux are supported through the
> Tauri abstractions but have not been runtime-verified in this branch —
> contributions confirming they work are welcome.

---

## Features

- **One hotkey, zero friction.** `Ctrl+Shift+Q` on Windows / Linux, `Cmd+Shift+M`
  on macOS by default; rebindable from the Settings page.
- **Multi-monitor capture.** Every connected display is scanned in one pass.
- **Multiple QR codes per scan.** All visible codes are decoded and shown as
  selectable cards.
- **Region select.** When the fullscreen pass finds nothing — or you want to
  refine — drag a rectangle over a frozen copy of the screen and re-decode
  the crop.
- **Persistent history.** Every scan is recorded in a local SQLite database
  with timestamps, content kind (URL, text, Wi-Fi, vCard, email, phone), and
  open / copied status. Filter by content, kind, status, and date range.
  Sort by scan date.
- **Safe bulk-open.** Opening more than three URLs triggers a confirmation
  modal listing every URL so you can spot phishing domains before opening.
- **Copy as JSON.** Bulk-select rows in history and copy them to the
  clipboard as JSON for use in other tools.
- **System-aware settings.** Theme (system / light / dark), launch on login,
  auto-copy single results, close-after-copy / open. The launch-on-login
  state reconciles with the OS on every startup so external changes are
  respected.
- **Per-launch log file.** Every run writes to `logs/qrab-<timestamp>.log`
  in the project directory, capturing setup, scan flow, errors, and drift
  events. Untracked by git.

## Install

No pre-built binaries yet. Build from source — see below.

## Usage

1. Launch qrab. A tray icon appears; the window opens automatically in dev,
   stays hidden in release (left-click the tray icon to show it).
2. Press the global hotkey (`Ctrl+Shift+Q` / `Cmd+Shift+M`) or "Scan now"
   from the tray menu.
3. Results appear in a card list. Click Copy or Open (URLs only) or press
   Enter on the focused card. ↑ / ↓ navigates.
4. Found nothing? You'll be dropped into the region selector with a frozen
   screenshot — drag a rectangle around the QR and click Decode.
5. The tray menu also surfaces View history, Settings…, and Config….

## Build from source

Requirements:

- **Rust** stable (edition 2021+)
- **Node.js** 20+ and **npm**
- **Tauri 2** prerequisites for your platform —
  see <https://tauri.app/start/prerequisites/>

```bash
git clone https://github.com/lenin/qrab.git
cd qrab
npm install
npm run tauri dev       # development with hot reload
npm run tauri build     # production build (in src-tauri/target/release)
```

## Scripts

| Script | Purpose |
|---|---|
| `npm run tauri dev` | Run the app with Vite HMR + Tauri rebuild on change |
| `npm run tauri build` | Production build (binary + bundle) |
| `npm run lint` | TypeScript type-check (`tsc --noEmit`) |
| `npm run fmt` | Prettier on `src/` |
| `npm run fmt:rust` | `cargo fmt` |
| `npm run clippy` | `cargo clippy -- -D warnings` |
| `npm test` | Run Vitest once (frontend unit tests) |
| `npm run test:watch` | Vitest in watch mode |
| `npm run test:rust` | `cargo test` |

## Architecture

qrab is a single Tauri webview with hash-based routing. Five routes share
one window:

- `#results` — Raycast-style scan results card list
- `#region` — rubber-band region selector over the held screenshot
- `#history` — table of past scans with filters, multi-select, bulk actions
- `#settings` — user preferences (hotkey, theme, behavior toggles)
- `#config` — app metadata + OS-level config (autostart)

Backend in Rust, frontend in SolidJS + TypeScript + Tailwind v4. SQLite via
`rusqlite` (bundled) with WAL mode and schema migrations via
`PRAGMA user_version`.

The full design — module-level SRP, trait seams, schema, IPC surface, UI/UX
requirements, design-decision log — lives in [CLAUDE.md](./CLAUDE.md).

## Tech stack

| Layer | Choice |
|---|---|
| Shell | Tauri 2 |
| Backend | Rust |
| Frontend framework | SolidJS |
| Frontend language | TypeScript (strict) |
| Bundler | Vite |
| Styling | Tailwind CSS v4 |
| Headless UI | Kobalte (`@kobalte/core`) |
| Icons | lucide-solid |
| Database | SQLite via `rusqlite` (bundled) |
| Capture | `xcap` |
| QR decode | `rqrr` |
| Test (Rust) | built-in `cargo test` |
| Test (TS) | Vitest + jsdom |

## What qrab is not

By design, qrab handles "readable" QR codes — links, text, Wi-Fi, vCards,
email, phone numbers. Codes that require a phone (mobile-only auth flows,
app-bound credentials) are out of scope. There is no cloud sync, no
telemetry, and no network access outside opening URLs you explicitly ask
to open. Database lives entirely on your machine.

## License

MIT — see [LICENSE](./LICENSE).
