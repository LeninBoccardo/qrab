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
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu =
        Menu::with_items(app, &[&scan, &history, &separator, &quit])?;

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
                crate::hotkey::trigger_scan(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Bring the results window forward and navigate it to `#history`.
fn show_history<R: Runtime>(app: &AppHandle<R>) {
    crate::windows::show_results_window(app);
    if let Some(window) =
        app.get_webview_window(crate::windows::RESULTS_WINDOW)
    {
        // Use a JS literal — no interpolation of user-controlled data, so
        // there's no injection surface. The hashchange event our
        // App.tsx router listens for fires from this.
        if let Err(e) = window.eval("window.location.hash = 'history';") {
            eprintln!("[qrab] failed to navigate to #history: {e}");
        }
    }
}
