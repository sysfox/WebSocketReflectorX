use std::cell::RefCell;

use i_slint_backend_winit::WinitWindowAccessor;
use slint::ComponentHandle;
use tracing::{debug, error};

use crate::ui::{MainWindow, SettingsBridge, WsrxTray};

thread_local! {
    static TRAY: RefCell<Option<WsrxTray>> = const { RefCell::new(None) };
}

/// Localised labels for the tray context menu, picked from the active UI
/// language so the tray matches the rest of the interface.
fn localized_labels(language: &str) -> (&'static str, &'static str) {
    match language {
        "zh_CN" => ("显示窗口", "退出"),
        "zh_TW" => ("顯示視窗", "退出"),
        _ => ("Show", "Quit"),
    }
}

/// Restore and focus the main window.
fn show_window(window_weak: &slint::Weak<MainWindow>) {
    if let Some(window) = window_weak.upgrade() {
        window.window().with_winit_window(|winit_window| {
            // Restore the window that was hidden via winit::Window::set_visible(false).
            winit_window.set_visible(true);
            winit_window.set_minimized(false);
            winit_window.focus_window();
        });
    }
}

/// No global setup is required when using Slint's built-in [`SystemTrayIcon`].
/// The tray instance is created on demand in [`ensure_created`].
pub fn setup(_window: &MainWindow) {}

/// Create the tray icon if it does not already exist.  Safe to call
/// repeatedly — subsequent calls are no-ops.
pub fn ensure_created(window: &MainWindow) {
    let already_exists = TRAY.with(|t| t.borrow().is_some());
    if already_exists {
        return;
    }

    let language = window.global::<SettingsBridge>().get_language().to_string();
    let (show_label, quit_label) = localized_labels(&language);
    let window_weak = window.as_weak();

    let tray = match WsrxTray::new() {
        Ok(tray) => tray,
        Err(e) => {
            error!("Failed to create system tray icon: {e}");
            return;
        }
    };

    tray.set_show_label(show_label.into());
    tray.set_quit_label(quit_label.into());

    let weak = window_weak.clone();
    tray.on_show_window(move || show_window(&weak));

    let weak = window_weak;
    tray.on_quit(move || crate::launcher::shutdown(&weak));

    if let Err(e) = tray.show() {
        error!("Failed to show system tray icon: {e}");
        return;
    }

    TRAY.with(|t| {
        *t.borrow_mut() = Some(tray);
    });
    debug!("System tray icon created");
}

/// Hide and drop the tray icon if it exists.  Dropping the [`WsrxTray`]
/// instance removes it from the system tray.
pub fn destroy() {
    TRAY.with(|t| {
        if t.borrow().is_some() {
            debug!("Destroying system tray icon");
        }
        *t.borrow_mut() = None;
    });
}

/// Whether a tray icon is currently active.
pub fn is_created() -> bool {
    TRAY.with(|t| t.borrow().is_some())
}
