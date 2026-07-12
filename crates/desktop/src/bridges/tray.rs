use std::cell::RefCell;

use i_slint_backend_winit::WinitWindowAccessor;
use slint::ComponentHandle;
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};
use tracing::{debug, error};

use crate::ui::{MainWindow, SettingsBridge};

/// Holds the live tray icon together with the menu item ids we need to
/// match incoming [`MenuEvent`]s against.  The icon is not `Send` (it has
/// thread affinity on Windows / macOS), so we keep it in a `thread_local`
/// that is only ever touched from the UI thread.
struct TrayState {
    _icon: TrayIcon,
    show_item_id: tray_icon::menu::MenuId,
    quit_item_id: tray_icon::menu::MenuId,
}

thread_local! {
    static TRAY: RefCell<Option<TrayState>> = const { RefCell::new(None) };
}

/// Decode the bundled application logo into the RGBA buffer expected by
/// [`tray_icon::Icon::from_rgba`].  Returns `None` on failure — the tray
/// will still be created, just without a custom icon.
fn load_icon() -> Option<Icon> {
    let bytes: &[u8] = include_bytes!("../../ui/assets/logo.png");
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            match Icon::from_rgba(rgba.into_raw(), width, height) {
                Ok(icon) => Some(icon),
                Err(e) => {
                    error!("Failed to build tray icon from RGBA data: {e}");
                    None
                }
            }
        }
        Err(e) => {
            error!("Failed to decode embedded logo.png for tray icon: {e}");
            None
        }
    }
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

/// Register the global tray / menu event handlers.  Must be called once
/// during startup, before any tray icon is created.
pub fn setup(window: &MainWindow) {
    let window_weak = window.as_weak();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let id = event.id;
        let window_weak = window_weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            handle_menu_event(&window_weak, &id);
        });
    }));

    let window_weak = window.as_weak();
    TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            let window_weak = window_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                show_window(&window_weak);
            });
        }
    }));
}

fn handle_menu_event(window_weak: &slint::Weak<MainWindow>, id: &tray_icon::menu::MenuId) {
    // 0 = show, 1 = quit
    let action = TRAY.with(|t| {
        t.borrow().as_ref().and_then(|state| {
            if id == &state.show_item_id {
                Some(0u8)
            } else if id == &state.quit_item_id {
                Some(1u8)
            } else {
                None
            }
        })
    });

    match action {
        Some(0) => show_window(window_weak),
        Some(1) => crate::launcher::shutdown(window_weak),
        _ => {}
    }
}

fn show_window(window_weak: &slint::Weak<MainWindow>) {
    if let Some(window) = window_weak.upgrade() {
        let _ = window.show();
        window.window().with_winit_window(|winit_window| {
            winit_window.set_minimized(false);
            let _ = winit_window.focus_window();
        });
    }
}

/// Create the tray icon if it does not already exist.  Safe to call
/// repeatedly — subsequent calls are no-ops.
pub fn ensure_created(window: &MainWindow) {
    let already_exists = TRAY.with(|t| t.borrow().is_some());
    if already_exists {
        return;
    }

    let language = window.global::<SettingsBridge>().get_language().to_string();
    let (show_label, quit_label) = localized_labels(&language);

    let show_item = MenuItem::new(show_label, true, None);
    let quit_item = MenuItem::new(quit_label, true, None);
    let show_item_id = show_item.id().clone();
    let quit_item_id = quit_item.id().clone();

    let menu = Menu::new();
    if let Err(e) = menu.append(&show_item) {
        error!("Failed to append show item to tray menu: {e}");
    }
    if let Err(e) = menu.append(&PredefinedMenuItem::separator()) {
        error!("Failed to append separator to tray menu: {e}");
    }
    if let Err(e) = menu.append(&quit_item) {
        error!("Failed to append quit item to tray menu: {e}");
    }

    let mut builder = TrayIconBuilder::new()
        .with_tooltip("WebSocket Reflector X")
        .with_menu(Box::new(menu))
        // Left-click should bring the window back instead of opening the menu;
        // the menu is still reachable via right-click.
        .with_menu_on_left_click(false);

    if let Some(icon) = load_icon() {
        builder = builder.with_icon(icon);
    }

    match builder.build() {
        Ok(tray) => {
            TRAY.with(|t| {
                *t.borrow_mut() = Some(TrayState {
                    _icon: tray,
                    show_item_id,
                    quit_item_id,
                });
            });
            debug!("System tray icon created");
        }
        Err(e) => {
            error!("Failed to create system tray icon: {e}");
        }
    }
}

/// Remove the tray icon if it exists.  Dropping the [`TrayIcon`] removes it
/// from the system tray.
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
