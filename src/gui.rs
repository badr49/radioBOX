use eframe::egui;
use egui::{
    Color32, Frame, Layout, Margin, RichText, ScrollArea, Separator, Spinner, Stroke, Vec2,
};
use reqwest::Client;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

use crate::config::{RadioStation, StationQuery};
use crate::player::Player;
use crate::stream::fetch_stations;

// ── Palette ───────────────────────────────────────────────────────────────────

const BG:        Color32 = Color32::from_rgb(15,  15,  18);
const SURFACE:   Color32 = Color32::from_rgb(22,  22,  28);
const SURFACE2:  Color32 = Color32::from_rgb(30,  30,  38);
const ACCENT:    Color32 = Color32::from_rgb(220, 80,  80);
const ACCENT_DIM:Color32 = Color32::from_rgb(120, 35,  35);
const TEXT:      Color32 = Color32::from_rgb(220, 220, 225);
const SUBTEXT:   Color32 = Color32::from_rgb(120, 120, 130);
const GREEN:     Color32 = Color32::from_rgb(80,  200, 120);

// ── Genre presets ─────────────────────────────────────────────────────────────

const GENRES: &[&str] = &[
    "Lo-fi", "Jazz", "Rock", "Classical", "Chill",
    "Blues", "Electronic", "Ambient", "Pop", "Metal",
    "Hip Hop", "R&B", "Soul", "Funk", "Reggae",
    "Country", "Folk", "Punk", "Indie", "Latin",
    "House", "Techno", "Drum and Bass", "Trance", "Dubstep",
    "Synthwave", "Retrowave", "Vaporwave",
    "Soundtrack", "World", "Disco", "Ska",
    "News", "Talk", "Chat", "Sports",
];

// ── Search state ──────────────────────────────────────────────────────────────

enum SearchState {
    Idle,
    Loading,
    Done(Vec<RadioStation>),
    Error(String),
}

impl Default for SearchState {
    fn default() -> Self { SearchState::Idle }
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct RadioApp {
    // search
    q_name:       String,
    q_genre:      String,
    q_country:    String,
    q_codec:      String,
    q_bitrate:    String,
    search_state: Arc<Mutex<SearchState>>,
    // playback
    player:       Player,
    now_playing:  Option<String>, // station name
    rt:           Runtime,
    client:       Client,
}

impl RadioApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        Self {
            q_name:       String::new(),
            q_genre:      String::new(),
            q_country:    String::new(),
            q_codec:      String::new(),
            q_bitrate:    String::new(),
            search_state: Arc::new(Mutex::new(SearchState::Idle)),
            player:       Player::new(),
            now_playing:  None,
            rt:           Runtime::new().expect("tokio runtime"),
            client:       Client::new(),
        }
    }

    fn trigger_search(&mut self) {
        let query = StationQuery {
            name:        non_empty(&self.q_name),
            genre:       non_empty(&self.q_genre),
            country:     non_empty(&self.q_country),
            codec:       non_empty(&self.q_codec),
            min_bitrate: self.q_bitrate.trim().parse().ok(),
        };
        let state  = Arc::clone(&self.search_state);
        let client = self.client.clone();
        *state.lock().unwrap() = SearchState::Loading;
        self.rt.spawn(async move {
            match fetch_stations(&client, query).await {
                Ok(s)  => *state.lock().unwrap() = SearchState::Done(s),
                Err(e) => *state.lock().unwrap() = SearchState::Error(e.to_string()),
            }
        });
    }
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t.to_string()) }
}

// ── Theme ─────────────────────────────────────────────────────────────────────

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.window_fill        = BG;
    style.visuals.panel_fill         = BG;
    style.visuals.faint_bg_color     = SURFACE;
    style.visuals.extreme_bg_color   = SURFACE2;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.widgets.noninteractive.bg_fill   = SURFACE;
    style.visuals.widgets.noninteractive.fg_stroke  = Stroke::new(1.0, SUBTEXT);
    style.visuals.widgets.inactive.bg_fill          = SURFACE2;
    style.visuals.widgets.inactive.fg_stroke        = Stroke::new(1.0, TEXT);
    style.visuals.widgets.hovered.bg_fill           = Color32::from_rgb(40, 40, 52);
    style.visuals.widgets.hovered.fg_stroke         = Stroke::new(1.0, TEXT);
    style.visuals.widgets.active.bg_fill            = ACCENT_DIM;
    style.visuals.widgets.active.fg_stroke          = Stroke::new(1.0, TEXT);
    style.visuals.selection.bg_fill                 = ACCENT_DIM;
    style.visuals.selection.stroke                  = Stroke::new(1.0, ACCENT);
    style.spacing.item_spacing                      = Vec2::new(6.0, 4.0);
    style.spacing.button_padding                    = Vec2::new(8.0, 4.0);
    ctx.set_style(style);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn text_input(ui: &mut egui::Ui, value: &mut String, hint: &str) {
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(f32::INFINITY)
            .font(egui::TextStyle::Body)
            .text_color(TEXT)
            .frame(true),
    );
}

fn section_label(ui: &mut egui::Ui, label: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(label).small().color(SUBTEXT));
    ui.add_space(2.0);
}

// ── Main render ───────────────────────────────────────────────────────────────

impl eframe::App for RadioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // repaint while loading or playing (for uptime / metrics ticker)
        let loading = matches!(*self.search_state.lock().unwrap(), SearchState::Loading);
        if loading || self.player.is_playing() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }

        // quit on close
        if ctx.input(|i| i.viewport().close_requested()) {
            self.player.stop();
        }

        // ── Left sidebar ───────────────────────────────────────────────────
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(220.0)
            .frame(Frame::new().fill(SURFACE).inner_margin(Margin::same(12)))
            .show(ctx, |ui| {
                // title + quit
                ui.horizontal(|ui| {
                    ui.label(RichText::new("📻 radioBOX").size(16.0).color(TEXT).strong());
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        let q = egui::Button::new(RichText::new("✕").size(12.0).color(SUBTEXT))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE);
                        if ui.add(q).on_hover_text("Quit").clicked() {
                            self.player.stop();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });

                ui.add(Separator::default().spacing(10.0));

                // ── Genre pills ────────────────────────────────────────────
                section_label(ui, "GENRE");
                egui::ScrollArea::vertical()
                    .id_salt("genre_scroll")
                    .max_height(200.0)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(3.0, 3.0);
                            for &g in GENRES {
                                let active = self.q_genre.eq_ignore_ascii_case(g);
                                let btn = egui::Button::new(
                                    RichText::new(g).size(11.0).color(if active { Color32::WHITE } else { SUBTEXT })
                                )
                                .fill(if active { ACCENT } else { SURFACE2 })
                                .corner_radius(10.0)
                                .stroke(Stroke::NONE);
                                if ui.add(btn).clicked() {
                                    self.q_genre = if active { String::new() } else { g.to_string() };
                                }
                            }
                        });
                    });

                ui.add(Separator::default().spacing(10.0));

                // ── Filters ────────────────────────────────────────────────
                section_label(ui, "FILTERS");

                ui.label(RichText::new("Station name").size(11.0).color(SUBTEXT));
                text_input(ui, &mut self.q_name, "e.g. BBC Radio 1");
                ui.add_space(4.0);

                ui.label(RichText::new("Genre / tag").size(11.0).color(SUBTEXT));
                text_input(ui, &mut self.q_genre, "or type here");
                ui.add_space(4.0);

                ui.label(RichText::new("Country code").size(11.0).color(SUBTEXT));
                text_input(ui, &mut self.q_country, "DE  US  GB …");
                ui.add_space(4.0);

                ui.label(RichText::new("Codec").size(11.0).color(SUBTEXT));
                text_input(ui, &mut self.q_codec, "FLAC  AAC+  MP3 …");
                ui.add_space(4.0);

                ui.label(RichText::new("Min bitrate (kbps)").size(11.0).color(SUBTEXT));
                text_input(ui, &mut self.q_bitrate, "128");
                ui.add_space(8.0);

                let searching = matches!(*self.search_state.lock().unwrap(), SearchState::Loading);
                let search_btn = egui::Button::new(
                    RichText::new(if searching { "Searching…" } else { "Search" })
                        .color(Color32::WHITE)
                )
                .fill(ACCENT)
                .corner_radius(6.0)
                .min_size(Vec2::new(ui.available_width(), 30.0));

                if ui.add_enabled(!searching, search_btn).clicked()
                    || (ctx.input(|i| i.key_pressed(egui::Key::Enter)) && !searching)
                {
                    self.trigger_search();
                }

                if searching {
                    ui.horizontal(|ui| {
                        ui.add(Spinner::new().size(12.0).color(ACCENT));
                        ui.label(RichText::new("fetching…").small().color(SUBTEXT));
                    });
                }
            });

        // ── Bottom now-playing bar ─────────────────────────────────────────
        if self.player.is_playing() {
            let info = self.player.info.lock().unwrap().clone();

            egui::TopBottomPanel::bottom("now_playing")
                .exact_height(64.0)
                .frame(Frame::new().fill(SURFACE2).inner_margin(Margin::symmetric(16, 10)))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        // playing indicator dot
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 4.0, GREEN);
                        ui.add_space(6.0);

                        ui.vertical(|ui| {
                            // station name + media title
                            let title = info.media_title.as_deref()
                                .unwrap_or(self.now_playing.as_deref().unwrap_or("—"));
                            ui.label(RichText::new(title).size(13.0).color(TEXT).strong());

                            // metrics row
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 12.0;
                                metric(ui, "bitrate", &info.bitrate_kbps());
                                if !info.audio_codec.is_empty() {
                                    metric(ui, "codec", &info.audio_codec.to_uppercase());
                                }
                                if info.sample_rate > 0 {
                                    metric(ui, "sample rate", &format!("{} Hz", info.sample_rate));
                                }
                                if info.channels > 0 {
                                    let ch = match info.channels {
                                        1 => "Mono".into(),
                                        2 => "Stereo".into(),
                                        n => format!("{n}ch"),
                                    };
                                    metric(ui, "channels", &ch);
                                }
                                if info.cache_duration > 0.0 {
                                    metric(ui, "buffer", &format!("{:.1}s", info.cache_duration));
                                }
                                metric(ui, "uptime", &info.uptime());
                            });
                        });

                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            let stop = egui::Button::new(
                                RichText::new("⏹  Stop").color(Color32::WHITE)
                            )
                            .fill(ACCENT_DIM)
                            .corner_radius(6.0);
                            if ui.add(stop).clicked() {
                                self.player.stop();
                                self.now_playing = None;
                            }

                            ui.add_space(8.0);

                            // volume slider
                            let mut vol = self.player.volume as f32;
                            let slider = egui::Slider::new(&mut vol, 0.0..=100.0)
                                .show_value(false);
                            if ui.add_sized(Vec2::new(80.0, 16.0), slider).changed() {
                                self.player.set_volume(vol as u32);
                            }
                            ui.label(
                                RichText::new(format!("🔊 {}%", self.player.volume))
                                    .size(11.0)
                                    .color(SUBTEXT),
                            );
                        });
                    });
                });
        }

        // ── Main results panel ─────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG).inner_margin(Margin::same(12)))
            .show(ctx, |ui| {
                let state = self.search_state.lock().unwrap();
                match &*state {
                    SearchState::Idle => {
                        ui.centered_and_justified(|ui| {
                            ui.label(
                                RichText::new("Search for a station to get started.")
                                    .color(SUBTEXT)
                                    .size(14.0),
                            );
                        });
                    }
                    SearchState::Loading => {
                        ui.centered_and_justified(|ui| {
                            ui.add(Spinner::new().size(24.0).color(ACCENT));
                        });
                    }
                    SearchState::Error(e) => {
                        ui.centered_and_justified(|ui| {
                            ui.label(RichText::new(format!("Error: {e}")).color(Color32::RED));
                        });
                    }
                    SearchState::Done(stations) => {
                        if stations.is_empty() {
                            ui.centered_and_justified(|ui| {
                                ui.label(RichText::new("No stations found.").color(SUBTEXT));
                            });
                        } else {
                            ui.label(
                                RichText::new(format!("{} stations", stations.len()))
                                    .small()
                                    .color(SUBTEXT),
                            );
                            ui.add_space(6.0);

                            let mut play_action: Option<(String, String)> = None;

                            ScrollArea::vertical().show(ui, |ui| {
                                for station in stations {
                                    let is_playing = self.now_playing.as_deref()
                                        == Some(station.name.as_str());

                                    let row_fill = if is_playing { SURFACE2 } else { SURFACE };
                                    let row_frame = Frame::new()
                                        .fill(row_fill)
                                        .inner_margin(Margin::symmetric(10, 8))
                                        .corner_radius(6.0)
                                        .stroke(if is_playing {
                                            Stroke::new(1.0, ACCENT)
                                        } else {
                                            Stroke::NONE
                                        });

                                    row_frame.show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            // left: name + meta
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    RichText::new(&station.name)
                                                        .size(13.0)
                                                        .color(TEXT)
                                                        .strong(),
                                                );
                                                let mut chips: Vec<String> = Vec::new();
                                                if let Some(c) = &station.country { if !c.is_empty() { chips.push(format!("🌍 {c}")); } }
                                                if let Some(g) = &station.genre   { if !g.is_empty() { chips.push(format!("🎵 {}", truncate(g, 30))); } }
                                                if let Some(br) = station.bitrate  { if br > 0 { chips.push(format!("{br} kbps")); } }
                                                if let Some(co) = &station.codec   { if !co.is_empty() { chips.push(co.clone()); } }
                                                if !chips.is_empty() {
                                                    ui.label(
                                                        RichText::new(chips.join("  ·  "))
                                                            .size(11.0)
                                                            .color(SUBTEXT),
                                                    );
                                                }
                                            });

                                            // right: play button
                                            ui.with_layout(
                                                Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if is_playing {
                                                        ui.label(
                                                            RichText::new("▶ playing")
                                                                .size(11.0)
                                                                .color(GREEN),
                                                        );
                                                    } else {
                                                        let btn = egui::Button::new(
                                                            RichText::new("▶").color(Color32::WHITE),
                                                        )
                                                        .fill(ACCENT_DIM)
                                                        .corner_radius(4.0)
                                                        .min_size(Vec2::new(28.0, 24.0));
                                                        if ui.add(btn).clicked() {
                                                            play_action = Some((
                                                                station.name.clone(),
                                                                station.url.clone(),
                                                            ));
                                                        }
                                                    }
                                                },
                                            );
                                        });
                                    });

                                    ui.add_space(4.0);
                                }
                            });

                            drop(state);

                            if let Some((name, url)) = play_action {
                                self.player.play(&url);
                                self.now_playing = Some(name);
                            }

                            return;
                        }
                    }
                }
            });
    }
}

// ── Small helpers ─────────────────────────────────────────────────────────────

fn metric(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.label(RichText::new(label).size(10.0).color(SUBTEXT));
        ui.label(RichText::new(value).size(10.0).color(TEXT));
    });
}

fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
