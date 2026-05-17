use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// ── Live stream metrics ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct StreamInfo {
    pub media_title:    Option<String>,
    pub audio_bitrate:  f64,    // bits/sec from mpv
    pub audio_codec:    String,
    pub sample_rate:    u32,
    pub channels:       u32,
    pub cache_duration: f64,    // seconds buffered
    pub connected_at:   Option<Instant>,
}

impl StreamInfo {
    pub fn uptime(&self) -> String {
        match self.connected_at {
            Some(t) => {
                let s = t.elapsed().as_secs();
                if s >= 3600 { format!("{}h {}m", s / 3600, (s % 3600) / 60) }
                else if s >= 60 { format!("{}m {}s", s / 60, s % 60) }
                else { format!("{}s", s) }
            }
            None => "—".into(),
        }
    }

    pub fn bitrate_kbps(&self) -> String {
        if self.audio_bitrate > 0.0 {
            format!("{:.0} kbps", self.audio_bitrate / 1000.0)
        } else {
            "—".into()
        }
    }
}

// ── Shared state pushed from the poll thread ──────────────────────────────────

pub type SharedInfo = Arc<Mutex<StreamInfo>>;

// ── Player ────────────────────────────────────────────────────────────────────

pub struct Player {
    process:     Option<Child>,
    socket_path: PathBuf,
    ipc_write:   Arc<Mutex<Option<UnixStream>>>, // write end shared with poll thread
    pub info:    SharedInfo,
    pub volume:  u32,   // 0–100
}

impl Player {
    pub fn new() -> Self {
        Self {
            process:     None,
            socket_path: std::env::temp_dir()
                .join(format!("radiobox-mpv-{}", std::process::id())),
            ipc_write:  Arc::new(Mutex::new(None)),
            info:   Arc::new(Mutex::new(StreamInfo::default())),
            volume: 60,
        }
    }

    pub fn is_playing(&self) -> bool {
        self.process.is_some()
    }

    pub fn play(&mut self, url: &str) {
        self.stop();

        let socket = self.socket_path.to_string_lossy().to_string();

        let child = Command::new("mpv")
            .args([
                "--no-video",
                "--really-quiet",
                &format!("--volume={}", self.volume),
                &format!("--input-ipc-server={socket}"),
                url,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match child {
            Ok(c) => {
                self.process = Some(c);
                *self.info.lock().unwrap() = StreamInfo {
                    connected_at: Some(Instant::now()),
                    ..Default::default()
                };
                // give mpv a moment to create the socket, then start polling
                let socket_path = self.socket_path.clone();
                let info        = Arc::clone(&self.info);
                let ipc_write   = Arc::clone(&self.ipc_write);
                thread::spawn(move || poll_loop(socket_path, info, ipc_write));
            }
            Err(e) => eprintln!("mpv launch failed: {e}"),
        }
    }

    pub fn stop(&mut self) {
        *self.ipc_write.lock().unwrap() = None;
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_file(&self.socket_path);
        *self.info.lock().unwrap() = StreamInfo::default();
    }

    /// Send a volume change to the running mpv instance via IPC.
    pub fn set_volume(&mut self, vol: u32) {
        self.volume = vol;
        let cmd = format!(
            "{{\"command\":[\"set_property\",\"volume\",{}]}}\n", vol
        );
        if let Some(ref mut stream) = *self.ipc_write.lock().unwrap() {
            let _ = stream.write_all(cmd.as_bytes());
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) { self.stop(); }
}

// ── IPC poll loop (runs on its own thread) ────────────────────────────────────

fn poll_loop(socket_path: PathBuf, info: SharedInfo, ipc_write: Arc<Mutex<Option<UnixStream>>>) {
    // wait up to 2 s for mpv to create the socket
    let stream = {
        let mut s = None;
        for _ in 0..20 {
            if let Ok(conn) = UnixStream::connect(&socket_path) {
                s = Some(conn);
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        match s {
            Some(s) => s,
            None => return,
        }
    };

    stream.set_read_timeout(Some(Duration::from_millis(50))).ok();
    let mut write_stream = stream.try_clone().expect("clone ipc stream");

    // store write end so set_volume can reach it
    *ipc_write.lock().unwrap() = write_stream.try_clone().ok();

    // observe properties — mpv will push events whenever they change
    let observe = [
        r#"{"command":["observe_property",1,"media-title"]}"#,
        r#"{"command":["observe_property",2,"audio-codec-name"]}"#,
        r#"{"command":["observe_property",3,"audio-params/samplerate"]}"#,
        r#"{"command":["observe_property",4,"audio-params/channel-count"]}"#,
    ];
    for cmd in &observe {
        let _ = write_stream.write_all(format!("{cmd}\n").as_bytes());
    }

    let reader = BufReader::new(stream);
    let mut tick: u64 = 0;

    for line in reader.lines() {
        // poll-request properties that aren't pushed automatically
        tick += 1;
        if tick % 10 == 0 {
            let _ = write_stream.write_all(
                b"{\"command\":[\"get_property\",\"audio-bitrate\"],\"request_id\":200}\n",
            );
            let _ = write_stream.write_all(
                b"{\"command\":[\"get_property\",\"demuxer-cache-duration\"],\"request_id\":201}\n",
            );
        }

        match line {
            Ok(text) => parse_line(&text, &info),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(_) => break, // socket closed → mpv stopped
        }
    }

    // mpv exited — clear info but preserve connected_at so uptime stays visible briefly
    info.lock().unwrap().audio_bitrate = 0.0;
}

fn parse_line(text: &str, info: &SharedInfo) {
    let mut g = info.lock().unwrap();

    // observed property events
    if text.contains("\"media-title\"") {
        if let Some(v) = extract_str(text) {
            g.media_title = if v.is_empty() { None } else { Some(v) };
        }
    }
    if text.contains("\"audio-codec-name\"") {
        if let Some(v) = extract_str(text) { g.audio_codec = v; }
    }
    if text.contains("\"audio-params/samplerate\"") {
        if let Some(v) = extract_num(text) { g.sample_rate = v as u32; }
    }
    if text.contains("\"audio-params/channel-count\"") {
        if let Some(v) = extract_num(text) { g.channels = v as u32; }
    }

    // polled property responses
    if text.contains("\"request_id\":200") || text.contains("\"request_id\": 200") {
        if let Some(v) = extract_num(text) { g.audio_bitrate = v; }
    }
    if text.contains("\"request_id\":201") || text.contains("\"request_id\": 201") {
        if let Some(v) = extract_num(text) { g.cache_duration = v; }
    }
}

fn extract_num(json: &str) -> Option<f64> {
    let idx = json.find("\"data\":")?;
    let after = json[idx + 7..].trim_start();
    let s: String = after.chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    s.parse().ok()
}

fn extract_str(json: &str) -> Option<String> {
    let idx = json.find("\"data\":")?;
    let after = json[idx + 7..].trim_start();
    if !after.starts_with('"') { return None; }
    let rest = &after[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
