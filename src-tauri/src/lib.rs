mod ai;
mod commands;
mod rules;
mod scanner;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "macleaner-tray";
const QUIT_MENU_ID: &str = "quit";
const FOCUS_HIDE_DEBOUNCE: Duration = Duration::from_millis(200);

#[derive(Default)]
struct PopoverState {
    last_hidden: Mutex<Option<Instant>>,
}

fn toggle_popover(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    if let Some(state) = app.try_state::<PopoverState>() {
        if let Ok(guard) = state.last_hidden.lock() {
            if guard.is_some_and(|hidden_at| hidden_at.elapsed() < FOCUS_HIDE_DEBOUNCE) {
                return;
            }
        }
    }

    let _ = window.move_window(Position::TrayCenter);
    let _ = window.show();
    let _ = window.set_focus();
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let icon = app
        .default_window_icon()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".to_string()))?
        .clone();

    let quit_item = MenuItemBuilder::with_id(QUIT_MENU_ID, "Quit Macleaner").build(app)?;
    let menu = MenuBuilder::new(app).items(&[&quit_item]).build()?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if event.id().as_ref() == QUIT_MENU_ID {
                app.exit(0);
            }
        })
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            tauri_plugin_positioner::on_tray_event(app, &event);

            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_popover(app);
            }
        });

    #[cfg(target_os = "macos")]
    {
        builder = builder.icon_as_template(true);
    }

    builder.build(app)?;
    Ok(())
}

/// Starts the Tauri application configured as a macOS menu bar popover.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(PopoverState::default())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_http::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                    let _ = window_vibrancy::apply_vibrancy(
                        &window,
                        window_vibrancy::NSVisualEffectMaterial::Popover,
                        Some(window_vibrancy::NSVisualEffectState::Active),
                        Some(16.0),
                    );
                }
            }

            let config_path = app
                .path()
                .app_config_dir()
                .ok()
                .map(|dir| dir.join("settings.json"));
            let app_state = commands::AppState::new(config_path)
                .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)?;
            app.manage(app_state);

            setup_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Focused(false) = event {
                if window.label() == MAIN_WINDOW_LABEL {
                    if let Some(state) = window.try_state::<PopoverState>() {
                        if let Ok(mut guard) = state.last_hidden.lock() {
                            *guard = Some(Instant::now());
                        }
                    }
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_version,
            commands::check_for_updates,
            commands::open_release_url,
            commands::get_disk_health,
            commands::scan_system,
            commands::trash_items,
            commands::check_ai_status,
            commands::list_ollama_models,
            commands::get_ai_config,
            commands::set_ai_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
