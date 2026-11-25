mod ai;
mod commands;
mod rules;
mod scanner;
mod snapshots;

use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_positioner::{Position, WindowExt};

use crate::ai::AppLanguage;
use crate::scanner::{read_disk_health, DiskHealth};

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "macleaner-tray";
const QUIT_MENU_ID: &str = "quit";
const FOCUS_HIDE_DEBOUNCE: Duration = Duration::from_millis(200);
const DISK_POLL_INTERVAL: Duration = Duration::from_secs(300);
const LOW_DISK_TRIGGER_PERCENT: u64 = 15;
const LOW_DISK_RESET_PERCENT: u64 = 18;

#[derive(Default)]
struct PopoverState {
    last_hidden: Mutex<Option<Instant>>,
}

#[must_use]
fn is_below_free_percent(health: &DiskHealth, threshold_percent: u64) -> bool {
    health.total_bytes > 0
        && u128::from(health.available_bytes).saturating_mul(100)
            < u128::from(health.total_bytes).saturating_mul(u128::from(threshold_percent))
}

#[must_use]
fn format_compact_bytes(bytes: u64) -> String {
    const TIB: f64 = 1024.0 * 1024.0 * 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    #[expect(clippy::cast_precision_loss, reason = "display formatting only")]
    let b = bytes as f64;
    if b >= TIB {
        format!("{:.1} TB", b / TIB)
    } else {
        format!("{:.0} GB", (b / GIB).max(0.0))
    }
}

#[must_use]
fn format_tray_title(health: &DiskHealth) -> String {
    if health.total_bytes == 0 {
        return String::new();
    }
    if is_below_free_percent(health, LOW_DISK_TRIGGER_PERCENT) {
        let free_pct = 100_u8.saturating_sub(health.used_percent);
        return format!("⚠ {free_pct}%");
    }
    format_compact_bytes(health.available_bytes)
}

#[must_use]
fn format_tray_tooltip(health: &DiskHealth, language: AppLanguage) -> String {
    if health.total_bytes == 0 {
        return "Macleaner".to_string();
    }
    let free_str = format_compact_bytes(health.available_bytes);
    let total_str = format_compact_bytes(health.total_bytes);
    match language {
        AppLanguage::PtBr => format!(
            "Macleaner — {free_str} livres de {total_str} ({}% em uso)",
            health.used_percent
        ),
        AppLanguage::En => format!(
            "Macleaner — {free_str} free of {total_str} ({}% used)",
            health.used_percent
        ),
        AppLanguage::Es => format!(
            "Macleaner — {free_str} libres de {total_str} ({}% en uso)",
            health.used_percent
        ),
    }
}

fn format_low_disk_notification(health: &DiskHealth, language: AppLanguage) -> (String, String) {
    let free_str = format_compact_bytes(health.available_bytes);
    let free_pct = 100_u8.saturating_sub(health.used_percent);
    match language {
        AppLanguage::PtBr => (
            "Espaço em disco baixo".to_string(),
            format!(
                "{} tem apenas {free_str} ({free_pct}%) livres. Abra o Macleaner para liberar espaço.",
                health.volume_name
            ),
        ),
        AppLanguage::En => (
            "Low disk space".to_string(),
            format!(
                "{} has only {free_str} ({free_pct}%) free. Open Macleaner to reclaim space.",
                health.volume_name
            ),
        ),
        AppLanguage::Es => (
            "Poco espacio en disco".to_string(),
            format!(
                "{} tiene solo {free_str} ({free_pct}%) libres. Abre Macleaner para liberar espacio.",
                health.volume_name
            ),
        ),
    }
}

/// Refreshes the menu bar tray title/tooltip and sends a low-disk notification when below 15%.
pub(crate) fn sync_tray_disk_status(app: &tauri::AppHandle) {
    let health = read_disk_health();
    let Some(state) = app.try_state::<commands::AppState>() else {
        return;
    };
    let (language, alert_enabled) = {
        let cfg = state.ai_config.lock().expect("ai_config lock poisoned");
        (cfg.language, cfg.low_disk_alert_enabled)
    };

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let title = format_tray_title(&health);
        let tooltip = format_tray_tooltip(&health, language);
        let _ = tray.set_title(Some(title));
        let _ = tray.set_tooltip(Some(tooltip));
    }

    if !alert_enabled || health.total_bytes == 0 {
        state.low_disk_notified.store(false, Ordering::Release);
        return;
    }

    if is_below_free_percent(&health, LOW_DISK_TRIGGER_PERCENT) {
        if state
            .low_disk_notified
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let (title, body) = format_low_disk_notification(&health, language);
            let _ = app.notification().builder().title(title).body(body).show();
        }
    } else if !is_below_free_percent(&health, LOW_DISK_RESET_PERCENT) {
        state.low_disk_notified.store(false, Ordering::Release);
    }
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
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_notification::init())
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
            sync_tray_disk_status(app.handle());

            let monitor_handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(DISK_POLL_INTERVAL);
                sync_tray_disk_status(&monitor_handle);
            });

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
            commands::reveal_in_finder,
            commands::list_apfs_snapshots,
            commands::delete_apfs_snapshots,
            commands::scan_system,
            commands::trash_items,
            commands::check_ai_status,
            commands::list_ollama_models,
            commands::get_ai_config,
            commands::set_ai_config,
            commands::add_ignored_path,
            commands::remove_ignored_path,
            commands::get_autostart_enabled,
            commands::set_autostart_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_formatting_and_low_disk_threshold_behave_accurately() {
        let healthy = DiskHealth {
            volume_name: "Macintosh HD".to_string(),
            total_bytes: 500 * 1024 * 1024 * 1024,
            available_bytes: 120 * 1024 * 1024 * 1024,
            used_bytes: 380 * 1024 * 1024 * 1024,
            used_percent: 76,
        };
        assert!(!is_below_free_percent(&healthy, LOW_DISK_TRIGGER_PERCENT));
        assert_eq!(format_tray_title(&healthy), "120 GB");
        assert!(format_tray_tooltip(&healthy, AppLanguage::En).contains("120 GB free"));

        let low = DiskHealth {
            volume_name: "Macintosh HD".to_string(),
            total_bytes: 500 * 1024 * 1024 * 1024,
            available_bytes: 50 * 1024 * 1024 * 1024,
            used_bytes: 450 * 1024 * 1024 * 1024,
            used_percent: 90,
        };
        assert!(is_below_free_percent(&low, LOW_DISK_TRIGGER_PERCENT));
        assert_eq!(format_tray_title(&low), "⚠ 10%");
    }
}
