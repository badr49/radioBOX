use eframe::egui;
use egui::{
    Color32, Frame, Layout, Margin, RichText, ScrollArea, Spinner, Stroke, Vec2,
};
use reqwest::Client;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

use crate::config::{RadioStation, StationQuery};
use crate::player::Player;
use crate::stream::{fetch_stations, fetch_top_voted};

// ── Palette ───────────────────────────────────────────────────────────────────
// Matches the website: pure black + green accent (#b8e986)

const BG:         Color32 = Color32::from_rgb(0,   0,   0);    // pure black
const SURFACE:    Color32 = Color32::from_rgb(0,  0,  0);   // sidebar / panel
const SURFACE2:   Color32 = Color32::from_rgb(18,  18,  20);   // cards
const SURFACE3:   Color32 = Color32::from_rgb(28,  28,  32);   // hovered card
const ACCENT:     Color32 = Color32::from_rgb(184, 233, 134);  // #b8e986 green
const ACCENT_DIM: Color32 = Color32::from_rgb(90,  120, 55);   // dimmed green
const BORDER:     Color32 = Color32::from_rgb(40,  40,  46);   // subtle border
const BORDER_ACC: Color32 = Color32::from_rgb(60,  80,  35);   // green-tinted border
const TEXT:       Color32 = Color32::from_rgb(230, 230, 230);  // primary text
const SUBTEXT:    Color32 = Color32::from_rgb(100, 100, 108);  // muted text
const GREEN:      Color32 = Color32::from_rgb(184, 233, 134);  // playing indicator = accent

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
        let mut app = Self {
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
        };
        app.trigger_top_voted();
        app
    }

    fn trigger_top_voted(&mut self) {
        let state  = Arc::clone(&self.search_state);
        let client = self.client.clone();
        *state.lock().unwrap() = SearchState::Loading;
        self.rt.spawn(async move {
            match fetch_top_voted(&client, 50).await {
                Ok(s)  => *state.lock().unwrap() = SearchState::Done(s),
                Err(e) => *state.lock().unwrap() = SearchState::Error(e.to_string()),
            }
        });
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

    style.visuals.window_fill             = BG;
    style.visuals.panel_fill              = BG;
    style.visuals.faint_bg_color          = SURFACE;
    style.visuals.extreme_bg_color        = SURFACE2;
    style.visuals.override_text_color     = Some(TEXT);

    // borders
    style.visuals.widgets.noninteractive.bg_fill        = SURFACE;
    style.visuals.widgets.noninteractive.bg_stroke      = Stroke::new(1.0, BORDER);
    style.visuals.widgets.noninteractive.fg_stroke      = Stroke::new(1.0, SUBTEXT);
    style.visuals.widgets.noninteractive.corner_radius  = 6.0.into();

    style.visuals.widgets.inactive.bg_fill              = SURFACE2;
    style.visuals.widgets.inactive.bg_stroke            = Stroke::new(1.0, BORDER);
    style.visuals.widgets.inactive.fg_stroke            = Stroke::new(1.0, TEXT);
    style.visuals.widgets.inactive.corner_radius        = 6.0.into();

    style.visuals.widgets.hovered.bg_fill               = SURFACE3;
    style.visuals.widgets.hovered.bg_stroke             = Stroke::new(1.0, BORDER_ACC);
    style.visuals.widgets.hovered.fg_stroke             = Stroke::new(1.0, ACCENT);
    style.visuals.widgets.hovered.corner_radius         = 6.0.into();

    style.visuals.widgets.active.bg_fill                = ACCENT_DIM;
    style.visuals.widgets.active.bg_stroke              = Stroke::new(1.0, ACCENT);
    style.visuals.widgets.active.fg_stroke              = Stroke::new(1.0, Color32::BLACK);
    style.visuals.widgets.active.corner_radius          = 6.0.into();

    style.visuals.selection.bg_fill                     = ACCENT_DIM;
    style.visuals.selection.stroke                      = Stroke::new(1.0, ACCENT);

    // text cursor
    style.visuals.text_cursor.stroke.color              = ACCENT;

    // spacing — tighter, more modern
    style.spacing.item_spacing                          = Vec2::new(8.0, 5.0);
    style.spacing.button_padding                        = Vec2::new(14.0, 7.0);
    style.spacing.indent                                = 14.0;
    style.spacing.interact_size                         = Vec2::new(20.0, 20.0);

    style.visuals.window_corner_radius                  = 10.0.into();
    style.visuals.menu_corner_radius                    = 8.0.into();

    ctx.set_style(style);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn text_input(ui: &mut egui::Ui, value: &mut String, hint: &str) {
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(RichText::new(hint).color(SUBTEXT))
            .desired_width(f32::INFINITY)
            .font(egui::TextStyle::Body)
            .text_color(TEXT)
            .frame(true)
            .margin(Vec2::new(10.0, 7.0)),
    );
}

fn filter_row(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
    ui.label(RichText::new(label).size(10.0).color(SUBTEXT));
    text_input(ui, value, hint);
    ui.add_space(5.0);
}

fn section_label(ui: &mut egui::Ui, label: &str) {
    ui.add_space(10.0);
    ui.label(
        RichText::new(label)
            .size(9.5)
            .color(SUBTEXT)
            .extra_letter_spacing(1.2),
    );
    ui.add_space(3.0);
}

// ── Main render ───────────────────────────────────────────────────────────────

impl eframe::App for RadioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let loading = matches!(*self.search_state.lock().unwrap(), SearchState::Loading);
        if loading || self.player.is_playing() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }

        if ctx.input(|i| i.viewport().close_requested()) {
            self.player.stop();
        }

        // ── Left sidebar ───────────────────────────────────────────────────
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(230.0)
            .frame(
                Frame::new()
                    .fill(SURFACE)
                    .inner_margin(Margin::same(14))
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("radioBOX").size(16.0).color(TEXT).strong());
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(6.0, 6.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 3.0, ACCENT);
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        let q = egui::Button::new(RichText::new("\u{E219}").size(12.0).color(SUBTEXT))
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .corner_radius(4.0);
                        if ui.add(q).on_hover_text("Quit").clicked() {
                            self.player.stop();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
                ui.add_space(10.0);

                // ── Genre pills ────────────────────────────────────────────
                section_label(ui, "GENRE");
                egui::ScrollArea::vertical()
                    .id_salt("genre_scroll")
                    .max_height(410.0)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

                            // ── Top Rated pill (special) ───────────────────
                            let top_btn = egui::Button::new(
                                RichText::new("⭐ Top Rated").size(10.0).color(Color32::BLACK).strong(),
                            )
                            .fill(ACCENT)
                            .corner_radius(100.0)
                            .stroke(Stroke::NONE)
                            .min_size(Vec2::new(0.0, 22.0));
                            if ui.add(top_btn).on_hover_text("Top 50 most voted stations").clicked() {
                                self.q_genre = String::new();
                                self.trigger_top_voted();
                            }

                            for &g in GENRES {
                                let active = self.q_genre.eq_ignore_ascii_case(g);
                                let btn = egui::Button::new(
                                    RichText::new(g).size(10.0).color(
                                        if active { Color32::BLACK } else { SUBTEXT },
                                    ),
                                )
                                .fill(if active { ACCENT } else { SURFACE2 })
                                .corner_radius(100.0)
                                .stroke(if active { Stroke::NONE } else { Stroke::new(1.0, BORDER) })
                                .min_size(Vec2::new(0.0, 22.0));
                                if ui.add(btn).clicked() {
                                    self.q_genre = if active { String::new() } else { g.to_string() };
                                }
                            }
                        });
                    });

                // ── Filters ────────────────────────────────────────────────
                section_label(ui, "FILTERS");
                filter_row(ui, "Station name", &mut self.q_name,    "");
                filter_row(ui, "Genre / tag",  &mut self.q_genre,   "");
                filter_row(ui, "Country",      &mut self.q_country, "");
                filter_row(ui, "Codec",        &mut self.q_codec,   "");
                ui.add_space(12.0);

                let searching = matches!(*self.search_state.lock().unwrap(), SearchState::Loading);
                let search_btn = egui::Button::new(
                    RichText::new(if searching { "Searching…" } else { "Search" })
                        .size(13.0).color(Color32::BLACK).strong(),
                )
                .fill(ACCENT)
                .corner_radius(8.0)
                .min_size(Vec2::new(ui.available_width(), 38.0));

                if ui.add_enabled(!searching, search_btn).clicked()
                    || (ctx.input(|i| i.key_pressed(egui::Key::Enter)) && !searching)
                {
                    self.trigger_search();
                }

                if searching {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add(Spinner::new().size(11.0).color(ACCENT));
                        ui.label(RichText::new("fetching…").size(11.0).color(SUBTEXT));
                    });
                }
            });

        // ── Bottom now-playing bar ─────────────────────────────────────────
        if self.player.is_playing() {
            let info = self.player.info.lock().unwrap().clone();

            egui::TopBottomPanel::bottom("now_playing")
                .exact_height(68.0)
                .frame(
                    Frame::new()
                        .fill(SURFACE)
                        .inner_margin(Margin::symmetric(20, 10))
                        .stroke(Stroke::new(1.0, BORDER_ACC)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 4.0, GREEN);
                        ui.add_space(10.0);

                        ui.vertical(|ui| {
                            let title = info.media_title.as_deref()
                                .unwrap_or(self.now_playing.as_deref().unwrap_or("—"));
                            ui.label(RichText::new(title).size(13.0).color(TEXT).strong());
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 12.0;
                                metric(ui, "bitrate", &info.bitrate_kbps());
                                if !info.audio_codec.is_empty() {
                                    metric(ui, "codec", &info.audio_codec.to_uppercase());
                                }
                                if info.sample_rate > 0 {
                                    metric(ui, "sr", &format!("{} Hz", info.sample_rate));
                                }
                                if info.channels > 0 {
                                    let ch = match info.channels { 1 => "Mono".into(), 2 => "Stereo".into(), n => format!("{n}ch") };
                                    metric(ui, "ch", &ch);
                                }
                                if info.cache_duration > 0.0 {
                                    metric(ui, "buf", &format!("{:.1}s", info.cache_duration));
                                }
                                metric(ui, "uptime", &info.uptime());
                            });
                        });

                        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                            let stop = egui::Button::new(
                                RichText::new("⏹  Stop").size(12.0).color(Color32::BLACK).strong(),
                            )
                            .fill(ACCENT)
                            .corner_radius(7.0)
                            .min_size(Vec2::new(72.0, 30.0));
                            if ui.add(stop).clicked() {
                                self.player.stop();
                                self.now_playing = None;
                            }
                            ui.add_space(16.0);
                            let mut vol = self.player.volume as f32;
                            if ui.add_sized(Vec2::new(90.0, 18.0), egui::Slider::new(&mut vol, 0.0..=100.0).show_value(false)).changed() {
                                self.player.set_volume(vol as u32);
                            }
                            ui.label(RichText::new(format!("🔊 {}%", self.player.volume)).size(11.0).color(SUBTEXT));
                        });
                    });
                });
        }

        // ── Main results panel ─────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG).inner_margin(Margin::same(16)))
            .show(ctx, |ui| {
                let state = self.search_state.lock().unwrap();
                match &*state {
                    SearchState::Idle => {
                        ui.centered_and_justified(|ui| {
                            ui.vertical(|ui| {
                                ui.add_space(60.0);
                                ui.label(RichText::new("📻").size(44.0).color(BORDER));
                                ui.add_space(14.0);
                                ui.label(RichText::new("Search for a station to get started").size(14.0).color(SUBTEXT));
                                ui.add_space(6.0);
                                ui.label(RichText::new("Pick a genre or use the filters on the left").size(11.5).color(Color32::from_gray(50)));
                            });
                        });
                    }
                    SearchState::Loading => {
                        ui.centered_and_justified(|ui| {
                            ui.vertical(|ui| {
                                ui.add(Spinner::new().size(28.0).color(ACCENT));
                                ui.add_space(14.0);
                                ui.label(RichText::new("Searching stations…").size(13.0).color(SUBTEXT));
                            });
                        });
                    }
                    SearchState::Error(e) => {
                        ui.centered_and_justified(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new("⚠").size(40.0).color(Color32::from_rgb(200, 70, 70)));
                                ui.add_space(12.0);
                                ui.label(RichText::new(format!("Error: {e}")).size(13.0).color(Color32::from_rgb(200, 70, 70)));
                            });
                        });
                    }
                    SearchState::Done(stations) => {
                        if stations.is_empty() {
                            ui.centered_and_justified(|ui| {
                                ui.vertical(|ui| {
                                    ui.add_space(60.0);
                                    ui.label(RichText::new("🔍").size(40.0).color(BORDER));
                                    ui.add_space(14.0);
                                    ui.label(RichText::new("No stations found").size(14.0).color(SUBTEXT));
                                });
                            });
                        } else {
                            ui.label(RichText::new(format!("{} stations found", stations.len())).size(11.0).color(SUBTEXT));
                            ui.add_space(10.0);

                            let mut play_action: Option<(String, String)> = None;

                            ScrollArea::vertical().show(ui, |ui| {
                                for station in stations {
                                    let is_playing = self.now_playing.as_deref() == Some(station.name.as_str());

                                    let row_frame = Frame::new()
                                        .fill(if is_playing { SURFACE2 } else { SURFACE })
                                        .inner_margin(Margin::symmetric(14, 10))
                                        .corner_radius(10.0)
                                        .stroke(if is_playing { Stroke::new(1.0, ACCENT) } else { Stroke::new(1.0, BORDER) });

                                    row_frame.show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    RichText::new(&station.name).size(13.5)
                                                        .color(if is_playing { ACCENT } else { TEXT }).strong(),
                                                );
                                                ui.add_space(3.0);
                                                let mut chips: Vec<String> = Vec::new();
                                                if let Some(c) = &station.country { if !c.is_empty() { chips.push(c.clone()); } }
                                                if let Some(g) = &station.genre   { if !g.is_empty() { chips.push(truncate(g, 28).to_string()); } }
                                                if let Some(br) = station.bitrate  { if br > 0 { chips.push(format!("{br} kbps")); } }
                                                if let Some(co) = &station.codec   { if !co.is_empty() { chips.push(co.clone()); } }
                                                if !chips.is_empty() {
                                                    ui.label(RichText::new(chips.join("  ·  ")).size(11.0).color(SUBTEXT));
                                                }
                                            });
                                            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                                if is_playing {
                                                    ui.label(RichText::new("▶  Playing").size(11.0).color(ACCENT).strong());
                                                } else {
                                                    let btn = egui::Button::new(RichText::new("▶").size(13.0).color(Color32::BLACK))
                                                        .fill(ACCENT).corner_radius(7.0).min_size(Vec2::new(34.0, 30.0));
                                                    if ui.add(btn).clicked() {
                                                        play_action = Some((station.name.clone(), station.url.clone()));
                                                    }
                                                }
                                            });
                                        });
                                    });
                                    ui.add_space(5.0);
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
