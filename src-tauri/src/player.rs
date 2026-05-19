use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use serde::Serialize;

// ── Live stream metrics ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize)]
pub struct StreamInfo {
    pub media_title:    Option<String>,
    pub audio_bitrate:  f64,
    pub audio_codec:    String,
    pub sample_rate:    u32,
    pub channels:       u32,
    pub cache_duration: f64,
    pub uptime_secs:    u64,
}



pub type SharedInfo = Arc<Mutex<StreamInfo>>;

// ── Player ────────────────────────────────────────────────────────────────────

pub struct Player {
    process:      Option<Child>,
    socket_path:  PathBuf,
    ipc_write:    Arc<Mutex<Option<UnixStream>>>,
    pub info:     SharedInfo,
    pub volume:   u32,
    connected_at: Option<Instant>,
}

impl Player {
    pub fn new() -> Self {
        Self {
            process:      None,
            socket_path:  std::env::temp_dir()
                .join(format!("radiobox-mpv-{}", std::process::id())),
            ipc_write:    Arc::new(Mutex::new(None)),
            info:         Arc::new(Mutex::new(StreamInfo::default())),
            volume:       60,
            connected_at: None,
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
                self.process      = Some(c);
                self.connected_at = Some(Instant::now());
                *self.info.lock().unwrap() = StreamInfo::default();

                let socket_path = self.socket_path.clone();
                let info        = Arc::clone(&self.info);
                let ipc_write   = Arc::clone(&self.ipc_write);
                let connected_at = self.connected_at.unwrap();
                thread::spawn(move || poll_loop(socket_path, info, ipc_write, connected_at));
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
        self.connected_at = None;
    }

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

// ── IPC poll loop ─────────────────────────────────────────────────────────────

fn poll_loop(
    socket_path:  PathBuf,
    info:         SharedInfo,
    ipc_write:    Arc<Mutex<Option<UnixStream>>>,
    connected_at: Instant,
) {
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
            None    => return,
        }
    };

    stream.set_read_timeout(Some(Duration::from_millis(50))).ok();
    let mut write_stream = stream.try_clone().expect("clone ipc stream");

    *ipc_write.lock().unwrap() = write_stream.try_clone().ok();

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
        tick += 1;
        if tick % 10 == 0 {
            let _ = write_stream.write_all(
                b"{\"command\":[\"get_property\",\"audio-bitrate\"],\"request_id\":200}\n",
            );
            let _ = write_stream.write_all(
                b"{\"command\":[\"get_property\",\"demuxer-cache-duration\"],\"request_id\":201}\n",
            );
        }

        // update uptime every tick
        info.lock().unwrap().uptime_secs = connected_at.elapsed().as_secs();

        match line {
            Ok(text) => parse_line(&text, &info),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
            Err(_) => break,
        }
    }

    info.lock().unwrap().audio_bitrate = 0.0;
}

fn parse_line(text: &str, info: &SharedInfo) {
    let mut g = info.lock().unwrap();

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
    let end  = rest.find('"')?;
    Some(rest[..end].to_string())
}
