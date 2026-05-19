// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod player;
mod stream;

use commands::AppState;
use player::Player;
use reqwest::Client;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

fn main() {
    // Required on some Linux/Wayland setups to prevent WebKit DMA-BUF crash
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    tauri::Builder::default()
        .setup(|app| {
            // ── System tray ───────────────────────────────────────────────
            let show_item = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let icon = tauri::image::Image::from_path(
                app.path().resource_dir().unwrap().join("icons/radio-app.png")
            )
            .unwrap_or_else(|_| app.default_window_icon().unwrap().clone());

            TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .tooltip("radioBOX")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => {
                        // stop playback cleanly before exit
                        if let Some(state) = app.try_state::<AppState>() {
                            state.player.lock().unwrap().stop();
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // left-click → show window
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .manage(AppState {
            player: Mutex::new(Player::new()),
            client: Client::new(),
        })
        // ── Hide window on close instead of quitting ──────────────────────
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_top_voted,
            commands::search_stations,
            commands::play,
            commands::stop,
            commands::set_volume,
            commands::get_volume,
            commands::is_playing,
            commands::get_stream_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
