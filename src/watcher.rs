use anyhow::Result;
use regex::Regex;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
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
    player_re: Regex,
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
            player_re: Regex::new(r"(.+) \((usr_[a-f0-9-]+)\)").unwrap(),
        }
    }

    async fn get_latest_log_file(&self) -> Option<PathBuf> {
        if !self.log_dir.exists() {
            return None;
        }

        let mut entries = fs::read_dir(&self.log_dir).await.ok()?;
        let mut latest_file = None;
        let mut latest_time = None;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("output_log_") && name_str.ends_with(".txt") {
                if let Ok(meta) = entry.metadata().await {
                    if let Ok(time) = meta.modified() {
                        if latest_time.is_none() || Some(time) > latest_time {
                            latest_time = Some(time);
                            latest_file = Some(entry.path());
                        }
                    }
                }
            }
        }

        latest_file
    }

    fn parse_timestamp(line: &str) -> String {
        // VRChat log timestamps are local time: "2026.03.08 01:14:09"
        // Guard against non-ASCII lines (e.g. Japanese text) that would panic on byte slicing
        if line.len() >= 19 && line.is_char_boundary(19) && line.as_bytes()[0].is_ascii_digit() {
            let date_str = &line[..19];
            date_str.replace('.', "-").replacen(' ', "T", 1)
        } else {
            String::new()
        }
    }

    fn parse_line(&self, line: &str) -> Option<LogEvent> {
        // Optimization: Early return for most common irrelevant lines
        if !line.contains("[Behaviour]") && !line.contains("uSpeak") {
            return None;
        }

        let timestamp = Self::parse_timestamp(line);

        // 1. World Name
        if let Some(pos) = line.find("] Entering Room: ") {
            let world_name = &line[pos + 17..];
            return Some(LogEvent::Location {
                world_name: world_name.to_string(),
                timestamp,
            });
        }

        // 2. Instance ID
        if let Some(pos) = line.find("] Joining wrld_") {
            let location = &line[pos + 10..];
            return Some(LogEvent::LocationInstance {
                location: location.to_string(),
                timestamp,
            });
        }

        // 3. Player Joined
        if let Some(pos) = line.find("] OnPlayerJoined ") {
            let parts = &line[pos + 17..];
            if let Some(caps) = self.player_re.captures(parts) {
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

        // 4. Player Left
        if let Some(pos) = line.find("] OnPlayerLeft ") {
            if !line.contains("OnPlayerLeftRoom") {
                let parts = &line[pos + 15..];
                if let Some(caps) = self.player_re.captures(parts) {
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

    async fn read_new_lines(&mut self, tx: &mpsc::UnboundedSender<LogEvent>) {
        let log_file = match &self.current_log_file {
            Some(f) => f.clone(),
            None => return,
        };

        let metadata = match fs::metadata(&log_file).await {
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

        let mut file = match fs::File::open(&log_file).await {
            Ok(f) => f,
            Err(_) => return,
        };

        if file
            .seek(SeekFrom::Start(self.last_read_pos))
            .await
            .is_err()
        {
            return;
        }

        let bytes_to_read = (file_size - self.last_read_pos) as usize;
        let mut buffer = vec![0u8; bytes_to_read];
        if file.read_exact(&mut buffer).await.is_err() {
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

        let mut lines = content.lines().peekable();

        while let Some(line) = lines.next() {
            if lines.peek().is_none() && !content.ends_with('\n') && !content.ends_with('\r') {
                // Save the incomplete last line for next read
                self.incomplete_line = line.to_string();
                break;
            }

            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if let Some(event) = self.parse_line(trimmed) {
                    let _ = tx.send(event);
                }
            }
        }
    }

    /// Read the log file to find the byte offset just before the last `Entering Room` event.
    async fn find_last_room_offset(&self, path: &Path) -> u64 {
        use tokio::io::{AsyncBufReadExt, BufReader};

        let file = match fs::File::open(path).await {
            Ok(f) => f,
            Err(_) => return 0,
        };

        // We will read line by line and track the byte offset of the start of the line.
        let mut reader = BufReader::new(file);
        let mut offset: u64 = 0;
        let mut last_found_offset: Option<u64> = None;
        let mut line = String::new();

        while let Ok(bytes_read) = reader.read_line(&mut line).await {
            if bytes_read == 0 {
                break; // EOF
            }

            if line.contains("[Behaviour] Entering Room:") {
                last_found_offset = Some(offset);
            }

            offset += bytes_read as u64;
            line.clear();
        }

        last_found_offset.unwrap_or(0)
    }

    /// Start watching logs. Returns a receiver for log events.
    pub async fn start(mut self) -> Result<mpsc::UnboundedReceiver<LogEvent>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let log_dir = self.log_dir.clone();

        tokio::spawn(async move {
            // 1. Check if VRChat is currently running when we start
            let was_running = crate::recorder::find_vrchat_pid().is_some();

            if !was_running {
                info!("{}", t!("pacing_wait_for_vrchat"));
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    if crate::recorder::find_vrchat_pid().is_some() {
                        info!("{}", t!("vrchat_started_tracking"));
                        break;
                    }
                }
            }

            // 2. Initialize log file tracking
            self.current_log_file = self.get_latest_log_file().await;
            if let Some(ref log_file) = self.current_log_file {
                if let Ok(meta) = fs::metadata(log_file).await {
                    if was_running {
                        // App started AFTER VRChat: find exact location of last room join
                        info!("{}", t!("vrchat_was_running_catchup"));
                        self.last_read_pos = self.find_last_room_offset(log_file).await;
                        info!("{}", t!("resuming_log_tracking", self.last_read_pos));
                    } else {
                        // App started BEFORE VRChat: ignore past session, only read new lines
                        info!("{}", t!("scanning_from_eof"));
                        self.last_read_pos = meta.len();
                    }
                }
                self.read_new_lines(&tx).await;
            }

            // 3. Normal polling loop (simpler and more reliable on Windows than fsnotify)
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;

                // Check for new log files
                if let Some(latest) = self.get_latest_log_file().await {
                    if self.current_log_file.as_ref() != Some(&latest) {
                        info!("{}", t!("new_log_file", latest.display()));
                        self.current_log_file = Some(latest);
                        self.last_read_pos = 0;
                    }
                }

                self.read_new_lines(&tx).await;
            }
        });

        info!("{}", t!("watching_directory", log_dir.display()));
        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_line() {
        let watcher = LogWatcher::new();

        // World Name
        let line = "2026.03.08 01:14:09 Log        -  [Behaviour] Entering Room: The Great Pug";
        if let Some(LogEvent::Location { world_name, .. }) = watcher.parse_line(line) {
            assert_eq!(world_name, "The Great Pug");
        } else {
            panic!("Failed to parse world name");
        }

        // Instance ID
        let line = "2026.03.08 01:14:10 Log        -  [Behaviour] Joining wrld_12345678:09876";
        if let Some(LogEvent::LocationInstance { location, .. }) = watcher.parse_line(line) {
            assert_eq!(location, "wrld_12345678:09876");
        } else {
            panic!("Failed to parse instance ID");
        }

        // Player Joined (with ID)
        let line = "2026.03.08 01:14:11 Log        -  [Behaviour] OnPlayerJoined Alice (usr_123)";
        if let Some(LogEvent::PlayerJoined {
            display_name,
            user_id,
            ..
        }) = watcher.parse_line(line)
        {
            assert_eq!(display_name, "Alice");
            assert_eq!(user_id, Some("usr_123".to_string()));
        } else {
            panic!("Failed to parse player joined with ID");
        }

        // Player Joined (without ID)
        let line = "2026.03.08 01:14:11 Log        -  [Behaviour] OnPlayerJoined Alice";
        if let Some(LogEvent::PlayerJoined {
            display_name,
            user_id,
            ..
        }) = watcher.parse_line(line)
        {
            assert_eq!(display_name, "Alice");
            assert_eq!(user_id, None);
        } else {
            panic!("Failed to parse player joined without ID");
        }

        // Player Left
        let line = "2026.03.08 01:14:12 Log        -  [Behaviour] OnPlayerLeft Bob (usr_456)";
        if let Some(LogEvent::PlayerLeft {
            display_name,
            user_id,
            ..
        }) = watcher.parse_line(line)
        {
            assert_eq!(display_name, "Bob");
            assert_eq!(user_id, Some("usr_456".to_string()));
        } else {
            panic!("Failed to parse player left");
        }

        // uSpeak
        let line = "2026.03.08 01:14:13 Log        -  uSpeak Start Microphone";
        assert!(watcher.parse_line(line).is_some());

        // Random line
        let line = "2026.03.08 01:14:14 Log        -  Some other log";
        assert!(watcher.parse_line(line).is_none());
    }
}
