use anyhow::Result;
use regex::Regex;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tracing::info;

use crate::t;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum LogEvent {
    Location {
        world_name: String,
        timestamp: String,
    },
    LocationInstance {
        location: String,
        timestamp: String,
    },
    PlayerJoined {
        display_name: String,
        user_id: Option<String>,
        timestamp: String,
    },
    PlayerLeft {
        display_name: String,
        user_id: Option<String>,
        timestamp: String,
    },
    VoiceReady {
        timestamp: String,
    },
}

pub struct LogWatcher {
    log_dir: PathBuf,
    current_log_file: Option<PathBuf>,
    last_read_pos: u64,
    incomplete_line: String,
    player_joined_re: Regex,
    player_left_re: Regex,
}

impl LogWatcher {
    pub fn new() -> Self {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        let log_dir = Path::new(&appdata)
            .parent()
            .unwrap_or(Path::new(&appdata))
            .join("LocalLow")
            .join("VRChat")
            .join("VRChat");

        Self {
            log_dir,
            current_log_file: None,
            last_read_pos: 0,
            incomplete_line: String::new(),
            player_joined_re: Regex::new(r"(.+) \((usr_[a-f0-9-]+)\)").unwrap(),
            player_left_re: Regex::new(r"(.+) \((usr_[a-f0-9-]+)\)").unwrap(),
        }
    }

    fn get_latest_log_file(&self) -> Option<PathBuf> {
        if !self.log_dir.exists() {
            return None;
        }

        let mut files: Vec<_> = fs::read_dir(&self.log_dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("output_log_") && name.ends_with(".txt")
            })
            .collect();

        files.sort_by(|a, b| {
            let ma = a.metadata().and_then(|m| m.modified()).ok();
            let mb = b.metadata().and_then(|m| m.modified()).ok();
            mb.cmp(&ma)
        });

        files.first().map(|e| e.path())
    }

    fn parse_timestamp(line: &str) -> String {
        // VRChat log timestamps are local time: "2026.03.08 01:14:09"
        // Guard against non-ASCII lines (e.g. Japanese text) that would panic on byte slicing
        if line.len() >= 19
            && line.is_char_boundary(19)
            && line.as_bytes()[0].is_ascii_digit()
        {
            let date_str = &line[..19];
            date_str.replace('.', "-").replacen(' ', "T", 1)
        } else {
            String::new()
        }
    }

    fn parse_line(&self, line: &str) -> Option<LogEvent> {
        let timestamp = Self::parse_timestamp(line);

        // 1. World Name
        if line.contains("[Behaviour] Entering Room: ") {
            if let Some(world_name) = line.split("] Entering Room: ").nth(1) {
                return Some(LogEvent::Location {
                    world_name: world_name.to_string(),
                    timestamp,
                });
            }
        }

        // 2. Instance ID
        if line.contains("[Behaviour] Joining wrld_") {
            if let Some(location) = line.split("] Joining ").nth(1) {
                return Some(LogEvent::LocationInstance {
                    location: location.to_string(),
                    timestamp,
                });
            }
        }

        // 3. Player Joined
        if line.contains("[Behaviour] OnPlayerJoined") {
            if let Some(parts) = line.split("] OnPlayerJoined ").nth(1) {
                if let Some(caps) = self.player_joined_re.captures(parts) {
                    return Some(LogEvent::PlayerJoined {
                        display_name: caps[1].to_string(),
                        user_id: Some(caps[2].to_string()),
                        timestamp,
                    });
                } else {
                    return Some(LogEvent::PlayerJoined {
                        display_name: parts.trim().to_string(),
                        user_id: None,
                        timestamp,
                    });
                }
            }
        }

        // 4. Player Left
        if line.contains("[Behaviour] OnPlayerLeft") && !line.contains("OnPlayerLeftRoom") {
            if let Some(parts) = line.split("] OnPlayerLeft ").nth(1) {
                if let Some(caps) = self.player_left_re.captures(parts) {
                    return Some(LogEvent::PlayerLeft {
                        display_name: caps[1].to_string(),
                        user_id: Some(caps[2].to_string()),
                        timestamp,
                    });
                } else {
                    return Some(LogEvent::PlayerLeft {
                        display_name: parts.trim().to_string(),
                        user_id: None,
                        timestamp,
                    });
                }
            }
        }

        // 5. uSpeak / Voice Ready
        if line.contains("uSpeak") && line.contains("Start Microphone") {
            return Some(LogEvent::VoiceReady { timestamp });
        }

        None
    }

    fn read_new_lines(&mut self, tx: &mpsc::UnboundedSender<LogEvent>) {
        let log_file = match &self.current_log_file {
            Some(f) => f.clone(),
            None => return,
        };

        let metadata = match fs::metadata(&log_file) {
            Ok(m) => m,
            Err(_) => return,
        };

        let file_size = metadata.len();

        // File was truncated or rotated
        if file_size < self.last_read_pos {
            self.last_read_pos = 0;
        }

        if file_size == self.last_read_pos {
            return;
        }

        let mut file = match fs::File::open(&log_file) {
            Ok(f) => f,
            Err(_) => return,
        };

        if file.seek(SeekFrom::Start(self.last_read_pos)).is_err() {
            return;
        }

        let bytes_to_read = (file_size - self.last_read_pos) as usize;
        let mut buffer = vec![0u8; bytes_to_read];
        if file.read_exact(&mut buffer).is_err() {
            return;
        }

        self.last_read_pos = file_size;

        // Prepend any incomplete line from the previous read
        let content = if self.incomplete_line.is_empty() {
            String::from_utf8_lossy(&buffer).into_owned()
        } else {
            let mut s = std::mem::take(&mut self.incomplete_line);
            s.push_str(&String::from_utf8_lossy(&buffer));
            s
        };

        // If the content doesn't end with a newline, the last line is incomplete
        let has_trailing_newline = content.ends_with('\n') || content.ends_with('\r');

        let mut lines: Vec<&str> = content.lines().collect();

        if !has_trailing_newline && !lines.is_empty() {
            // Save the incomplete last line for next read
            self.incomplete_line = lines.pop().unwrap().to_string();
        }

        for line in lines {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if let Some(event) = self.parse_line(trimmed) {
                    let _ = tx.send(event);
                }
            }
        }
    }

    /// Start watching logs. Returns a receiver for log events.
    pub async fn start(mut self) -> Result<mpsc::UnboundedReceiver<LogEvent>> {
        let (tx, rx) = mpsc::unbounded_channel();

        // Find and read the latest log on startup (last 5KB)
        self.current_log_file = self.get_latest_log_file();
        if let Some(ref log_file) = self.current_log_file {
            if let Ok(meta) = fs::metadata(log_file) {
                self.last_read_pos = meta.len().saturating_sub(50_000);
            }
            self.read_new_lines(&tx);
        }

        let log_dir = self.log_dir.clone();

        // Polling-based watcher (simpler and more reliable on Windows than fsnotify for log tailing)
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;

                // Check for new log files
                if let Some(latest) = self.get_latest_log_file() {
                    if self.current_log_file.as_ref() != Some(&latest) {
                        info!("{}", t!("new_log_file", latest.display()));
                        self.current_log_file = Some(latest);
                        self.last_read_pos = 0;
                    }
                }

                self.read_new_lines(&tx);
            }
        });

        info!("{}", t!("watching_directory", log_dir.display()));
        Ok(rx)
    }
}
