# qrab smoke-test checklist

A manual smoke test to validate qrab on a target OS. CLAUDE.md §1.5
currently lists Windows 11 as the only runtime-verified platform; this
checklist is the gate for promoting macOS, Linux/X11, and Linux/Wayland
to "verified" status in the README and §1.5.

Run the full checklist on each target. Mark a row pass / fail / N/A in
the run log at the bottom; file a `fix(...)` PR for each failure before
the run is considered complete.

---

## Setup

Before the run, verify you have:

- Rust stable (latest), Node 20+, npm.
- Tauri 2 OS prerequisites for the platform —
  see <https://tauri.app/start/prerequisites/>.
- A QR code generator handy (any web tool that renders a QR for a URL).
- A second QR code visible at the same time for multi-result tests.

Build once via `npm run tauri build` and launch the release binary
directly. Debug builds change capture/decode timings by 10–15× and are
not representative.

---

## 1. Install & launch

| # | Step | Expected |
|---|---|---|
| 1.1 | First launch from a fresh `app_data_dir` | Tray icon appears within ~1 s |
| 1.2 | Window first-opens | Sized proportionally to primary monitor (~60% capped 1200×800, floored 520×420), centered |
| 1.3 | Quit + relaunch | Window restores its last size / position via `tauri-plugin-window-state` |
| 1.4 | Per-launch log file written | `<repo>/logs/qrab-<timestamp>.log` exists and contains the startup banner |
| 1.5 | macOS only — first scan attempt | System prompt asks for Screen Recording permission; after granting, restart and capture works |
| 1.6 | Linux/Wayland only — first scan attempt | xdg-desktop-portal "ScreenCast" prompt appears; after allowing, capture works |
| 1.7 | Linux/X11 only — first scan attempt | Capture works without portal prompt |

## 2. Core scan flow

| # | Step | Expected |
|---|---|---|
| 2.1 | Press global hotkey (`Ctrl+Shift+Q` / `Cmd+Shift+M`) over a single QR | Results window appears within ~1 s, one card shown |
| 2.2 | Display 2–3 QRs and press hotkey | All codes appear as cards, deduped if identical |
| 2.3 | Hotkey over a screen with no QR | Region selector appears with a frozen screenshot of the captured display |
| 2.4 | On a single result, press Enter | Primary action runs (Open for URL, Copy for text) |
| 2.5 | On multi-result list, ↑/↓ arrow keys | Focus moves between cards; Enter triggers primary action on focused card |
| 2.6 | Click "Open" on a URL card | URL opens in the default browser; row marked Opened in History |
| 2.7 | Click "Copy" on a row | Clipboard contains the row's content; row marked Copied in History |
| 2.8 | Press Esc on results window | Window hides; tray icon remains |
| 2.9 | Left-click tray icon | Window resurfaces without triggering a new scan |
| 2.10 | Tray menu → "Scan now" | Same effect as the global hotkey |

## 3. Region select

| # | Step | Expected |
|---|---|---|
| 3.1 | From results, click "Select region" | Region selector appears with the held screenshot |
| 3.2 | Drag a rectangle around a QR | Dimmed overlay outside the rect; "Decode" button enables |
| 3.3 | Click Decode | Cropped region is decoded; results window updates |
| 3.4 | Drag a rectangle that contains no QR | "Nothing found in that region — try again" appears; selector stays open |
| 3.5 | Press Esc inside region select | Returns to results |
| 3.6 | Multi-monitor only — switch monitor button | The displayed screenshot changes to the chosen monitor without stale flashes |

## 4. History

| # | Step | Expected |
|---|---|---|
| 4.1 | Tray menu → "View history" or `#history` route | History table loads with past scans (newest first) |
| 4.2 | Click the "Scanned" column header | Sort flips ASC / DESC |
| 4.3 | Type in the search box | Rows filter to substring matches |
| 4.4 | Pick a kind in the dropdown | Rows filter to that kind only |
| 4.5 | Pick a status (Opened / Copied / Untouched / All) | Rows filter correctly |
| 4.6 | Set a date range | Rows filter inclusively |
| 4.7 | Select 2+ rows, click "Delete" | Rows removed; selection cleared; table refreshes |
| 4.8 | Select 2+ rows, click "Copy as JSON" | Clipboard contains valid JSON array of those rows; all become Copied |
| 4.9 | Select 4+ URL rows, click "Open URLs" | ConfirmOpenAll modal lists every URL; Cancel works; Confirm opens all |
| 4.10 | Click "Clear all" | Confirmation modal; Confirm wipes the table |
| 4.11 | Scroll past 50 rows | "Load more" button appears; clicking it loads the next page |

## 5. Settings

| # | Step | Expected |
|---|---|---|
| 5.1 | Tray menu → "Settings…" | Settings window appears |
| 5.2 | Change hotkey via the HotkeyInput | Old chord no longer fires; new chord triggers scan |
| 5.3 | Enter an invalid chord | Save is rejected with a clear error |
| 5.4 | Settings UI when the OS rejects the chord | Amber warning ("hotkey not registered") shown |
| 5.5 | Toggle theme: system → light → dark | UI re-themes immediately |
| 5.6 | Toggle "auto-copy single result" + scan a single QR | Clipboard contains the content without a Copy click |
| 5.7 | Toggle "close after copy" + click Copy | Window hides automatically |
| 5.8 | Toggle "close after open" + click Open | Window hides automatically |
| 5.9 | Click "Reset to defaults" | All fields revert to Rust-side defaults (platform-aware hotkey) |
| 5.10 | Quit + relaunch | Every setting persists |

## 6. Config

| # | Step | Expected |
|---|---|---|
| 6.1 | Tray menu → "Config…" | Config window appears with logo, version, author, description |
| 6.2 | Toggle "Start app at initialization" on | OS autostart entry created (Windows: registry HKCU; macOS: LaunchAgent; Linux: `~/.config/autostart`) |
| 6.3 | Reboot / re-login | qrab launches automatically |
| 6.4 | Disable the autostart entry externally (Task Manager / launchctl) and relaunch qrab | The Config toggle reflects the OS truth (off) |
| 6.5 | Toggle off | Autostart entry removed |

## 7. System integration

| # | Step | Expected |
|---|---|---|
| 7.1 | Launch qrab twice | Second process exits; first process surfaces and focuses its window |
| 7.2 | Move window to a new monitor / resize | Quit + relaunch → restored on the same monitor at the same size |
| 7.3 | Inspect the log file after a scan | Lines for `scan_screen: start`, `decode: monitor=… took=Xms`, `scan_screen: decoded N code(s)` |

## 8. Platform-specific spot checks

### macOS

- `Cmd+Shift+M` (not `Cmd+Shift+Q`, which is the system Log Out chord) is the default hotkey.
- After revoking Screen Recording permission and relaunching, the next scan surfaces an in-window banner with an "Open Settings" button that opens the Privacy & Security pane.
- `set_activation_policy(Accessory)` keeps the app out of the Dock — only the tray icon is visible.

### Linux / Wayland

- Global shortcuts on Wayland are restricted. If registration fails, the Settings UI shows the registration warning, and tray-click + "Scan now" remain fully functional fallbacks.
- xdg-desktop-portal must be installed; the user's compositor must implement the ScreenCast portal.
- Tray-icon support depends on the DE — GNOME needs a status-area extension installed.

### Linux / X11

- Capture works without portal interaction.
- Global shortcuts register normally.

### Windows

- No permission system around capture or global shortcuts.
- Unsigned builds trigger SmartScreen ("Windows protected your PC") on first launch — expected until code signing lands in v1.4+.

---

## Run log

Copy this block to a fresh PR description when you run the checklist:

```
qrab smoke test — <date>

Platform: <Windows 11 / macOS XX.Y / Ubuntu 24.04 Wayland / Ubuntu 24.04 X11>
qrab version: <vX.Y.Z>

Sections:
  1. Install & launch:       __ / __ passed
  2. Core scan flow:         __ / __ passed
  3. Region select:          __ / __ passed
  4. History:                __ / __ passed
  5. Settings:               __ / __ passed
  6. Config:                 __ / __ passed
  7. System integration:     __ / __ passed
  8. Platform-specific:      pass / fail per bullet

Failures:
- <row>: <description> → opened fix(<scope>): <title> #<PR>
```
