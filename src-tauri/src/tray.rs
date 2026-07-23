use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WindowEvent,
};

const MAIN_WINDOW: &str = "main";
const TRAY_ID: &str = "mcp-switch-tray";

/// Build the tray icon + context menu and wire its handlers. Called once
/// from `lib::run` after the main window has been created.
///
/// Tray-menu semantics deliberately mirror the reference Python GUI's
/// `tray.py`: left-click toggles the window, right-click opens the menu,
/// the X button on the window hides to tray instead of quitting, and the
/// only way to actually exit is "Quit" in the menu.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Open MCP Switch", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Hide to tray", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit MCP Switch", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &sep, &hide, &sep, &quit])?;

    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().expect("app icon missing").clone())
        .tooltip("MCP Switch")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "hide" => {
                if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
                    let _ = w.hide();
                }
            }
            "quit" => quit_app(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
            {
                if button == MouseButton::Left && button_state == MouseButtonState::Up {
                    toggle_main(tray.app_handle());
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Intercept the window's close button so it hides to tray instead of
/// exiting. The user's only path out is the tray menu's "Quit" item.
pub fn hook_close_to_tray(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    let w = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = w.hide();
        }
    });
}


/// Quit MCP Switch: first stop every spawned MCP server child (so we
/// don't leak background processes across restarts), then exit the Tauri
/// app.
fn quit_app(app: &AppHandle) {
    if let Some(state) = app.try_state::<crate::runner::RunnerState>() {
        let stopped = state.stop_all();
        if !stopped.is_empty() {
            eprintln!(
                "Quit: stopped {} runner-managed MCP server children",
                stopped.iter().filter(|(_, _, k)| *k).count()
            );
        }
    }
    app.exit(0);
}fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn toggle_main(app: &AppHandle) {
    let Some(w) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    let visible = w.is_visible().unwrap_or(false);
    let minimized = w.is_minimized().unwrap_or(false);
    if visible && !minimized {
        let _ = w.hide();
    } else {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
