use tauri::State;
use std::sync::Mutex;
use reqwest::Client;

use crate::config::{RadioStation, StationQuery};
use crate::player::{Player, StreamInfo};
use crate::stream::{fetch_top_voted, fetch_stations};

pub struct AppState {
    pub player: Mutex<Player>,
    pub client: Client,
}

// ── Station discovery ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_top_voted(
    limit: usize,
    state: State<'_, AppState>,
) -> Result<Vec<RadioStation>, String> {
    fetch_top_voted(&state.client, limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_stations(
    query: StationQuery,
    state: State<'_, AppState>,
) -> Result<Vec<RadioStation>, String> {
    fetch_stations(&state.client, query)
        .await
        .map_err(|e| e.to_string())
}

// ── Playback ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn play(url: String, state: State<'_, AppState>) {
    state.player.lock().unwrap().play(&url);
}

#[tauri::command]
pub fn stop(state: State<'_, AppState>) {
    state.player.lock().unwrap().stop();
}

#[tauri::command]
pub fn set_volume(vol: u32, state: State<'_, AppState>) {
    state.player.lock().unwrap().set_volume(vol);
}

#[tauri::command]
pub fn get_volume(state: State<'_, AppState>) -> u32 {
    state.player.lock().unwrap().volume
}

// ── Status ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn is_playing(state: State<'_, AppState>) -> bool {
    state.player.lock().unwrap().is_playing()
}

#[tauri::command]
pub fn get_stream_info(state: State<'_, AppState>) -> StreamInfo {
    state.player.lock().unwrap().info.lock().unwrap().clone()
}
