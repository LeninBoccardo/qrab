//! System-tray icon: a Scan/Quit menu plus left-click-to-scan.
//!
//! The tray is the always-present surface — the results window is hidden
//! by default and shows on hotkey, tray click, or the "Scan now" menu
//! entry. Closing the results window via its X button hides it instead
//! of exiting; only the tray "Quit" exits the process.

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};

pub const TRAY_ID: &str = "qrab-tray";

pub fn install<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    let scan = MenuItem::with_id(app, "scan", "Scan now", true, None::<&str>)?;
    let history = MenuItem::with_id(
        app,
        "history",
        "View history",
        true,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(
        app,
        "settings",
        "Settings…",
        true,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&scan, &history, &settings, &separator, &quit],
    )?;

    let icon = app
        .default_window_icon()
        .ok_or_else(|| anyhow::anyhow!("default window icon not configured"))?
        .clone();

    TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("qrab — scan QR codes on screen")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "scan" => crate::hotkey::trigger_scan(app),
            "history" => show_history(app),
            "settings" => show_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // Left-click just surfaces the existing window — no scan.
                // Explicit scans go through the "Scan now" menu entry or
                // the global hotkey.
                crate::windows::show_results_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Bring the results window forward and navigate it to `#history`.
fn show_history<R: Runtime>(app: &AppHandle<R>) {
    navigate(app, "history");
}

/// Bring the results window forward and navigate it to `#settings`.
fn show_settings<R: Runtime>(app: &AppHandle<R>) {
    navigate(app, "settings");
}

fn navigate<R: Runtime>(app: &AppHandle<R>, route: &str) {
    // Defense in depth: route is `format!`-interpolated into a window.eval
    // string. Current callers pass hardcoded literals ("history", "settings"),
    // but a future caller wiring user input here would otherwise be a JS
    // injection bug. Lowercase-ASCII-only catches that loudly in debug.
    debug_assert!(
        !route.is_empty() && route.chars().all(|c| c.is_ascii_lowercase()),
        "navigate() route must be non-empty lowercase ASCII letters; got {route:?}"
    );
    crate::windows::show_results_window(app);
    if let Some(window) =
        app.get_webview_window(crate::windows::RESULTS_WINDOW)
    {
        let js = format!("window.location.hash = '{route}';");
        if let Err(e) = window.eval(&js) {
            log::warn!("failed to navigate to #{route}: {e}");
        }
    }
}
