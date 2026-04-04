use anyhow::Result;
use regex::Regex;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt};
use tokio::sync::mpsc;
use tracing::info;

use crate::t;

#[derive(Debug, Clone, PartialEq)]
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

        let mut entries = tokio::fs::read_dir(&self.log_dir).await.ok()?;
        let mut latest_file: Option<(PathBuf, std::time::SystemTime)> = None;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("output_log_") && name_str.ends_with(".txt") {
                if let Ok(meta) = entry.metadata().await {
                    if let Ok(modified) = meta.modified() {
                        if let Some((_, latest_time)) = &latest_file {
                            if modified > *latest_time {
                                latest_file = Some((entry.path(), modified));
                            }
                        } else {
                            latest_file = Some((entry.path(), modified));
                        }
                    }
                }
            }
        }

        latest_file.map(|(path, _)| path)
    }

    fn parse_timestamp(line: &str) -> String {
        // VRChat log timestamps are local time: "2026.03.08 01:14:09"
        if line.len() >= 19 && line.is_char_boundary(19) && line.as_bytes()[0].is_ascii_digit() {
            let bytes = &line.as_bytes()[..19];
            let mut ts_vec = Vec::with_capacity(19);
            for (i, &b) in bytes.iter().enumerate() {
                if (i == 4 || i == 7) && b == b'.' {
                    ts_vec.push(b'-');
                } else if i == 10 && b == b' ' {
                    ts_vec.push(b'T');
                } else {
                    ts_vec.push(b);
                }
            }
            String::from_utf8(ts_vec).unwrap_or_default()
        } else {
            String::new()
        }
    }

    fn parse_line(&self, line: &str) -> Option<LogEvent> {
        // Early returns for most common non-event lines
        if line.contains("[Behaviour]") {
            let get_timestamp = || Self::parse_timestamp(line);

            // 1. World Name
            if let Some(pos) = line.find(" Entering Room: ") {
                let world_name = &line[pos + 16..];
                return Some(LogEvent::Location {
                    world_name: world_name.to_string(),
                    timestamp: get_timestamp(),
                });
            }

            // 2. Instance ID
            if let Some(pos) = line.find(" Joining wrld_") {
                let location = &line[pos + 9..];
                return Some(LogEvent::LocationInstance {
                    location: location.to_string(),
                    timestamp: get_timestamp(),
                });
            }

            // 3. Player Joined
            if let Some(pos) = line.find(" OnPlayerJoined ") {
                let parts = &line[pos + 16..];
                if let Some(caps) = self.player_re.captures(parts) {
                    return Some(LogEvent::PlayerJoined {
                        display_name: caps[1].to_string(),
                        user_id: Some(caps[2].to_string()),
                        timestamp: get_timestamp(),
                    });
                } else {
                    return Some(LogEvent::PlayerJoined {
                        display_name: parts.trim().to_string(),
                        user_id: None,
                        timestamp: get_timestamp(),
                    });
                }
            }

            // 4. Player Left
            if let Some(pos) = line.find(" OnPlayerLeft ") {
                if !line.contains("OnPlayerLeftRoom") {
                    let parts = &line[pos + 14..];
                    if let Some(caps) = self.player_re.captures(parts) {
                        return Some(LogEvent::PlayerLeft {
                            display_name: caps[1].to_string(),
                            user_id: Some(caps[2].to_string()),
                            timestamp: get_timestamp(),
                        });
                    } else {
                        return Some(LogEvent::PlayerLeft {
                            display_name: parts.trim().to_string(),
                            user_id: None,
                            timestamp: get_timestamp(),
                        });
                    }
                }
            }
        }

        // 5. uSpeak / Voice Ready
        if line.contains("uSpeak") && line.contains("Start Microphone") {
            return Some(LogEvent::VoiceReady {
                timestamp: Self::parse_timestamp(line),
            });
        }

        None
    }

    async fn read_new_lines(&mut self, tx: &mpsc::UnboundedSender<LogEvent>) {
        let log_file = match &self.current_log_file {
            Some(f) => f,
            None => return,
        };

        let metadata = match tokio::fs::metadata(log_file).await {
            Ok(m) => m,
            Err(_) => return,
        };

        let file_size = metadata.len();

        if file_size < self.last_read_pos {
            self.last_read_pos = 0;
        }

        if file_size == self.last_read_pos {
            return;
        }

        let mut file = match tokio::fs::File::open(log_file).await {
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

        let mut buffer = Vec::with_capacity((file_size - self.last_read_pos) as usize);
        if file.read_to_end(&mut buffer).await.is_err() {
            return;
        }

        self.last_read_pos = file_size;

        let content = if self.incomplete_line.is_empty() {
            String::from_utf8_lossy(&buffer)
        } else {
            let mut s = std::mem::take(&mut self.incomplete_line);
            s.push_str(&String::from_utf8_lossy(&buffer));
            std::borrow::Cow::Owned(s)
        };

        let has_trailing_newline = content.ends_with('\n') || content.ends_with('\r');
        let mut lines = content.lines().peekable();

        while let Some(line) = lines.next() {
            if lines.peek().is_none() && !has_trailing_newline {
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

    async fn find_last_room_offset(&self, path: &Path) -> u64 {
        let file = match tokio::fs::File::open(path).await {
            Ok(f) => f,
            Err(_) => return 0,
        };

        let mut reader = tokio::io::BufReader::new(file);
        let mut offset: u64 = 0;
        let mut last_found_offset: Option<u64> = None;
        let mut line = String::new();

        while let Ok(bytes_read) = reader.read_line(&mut line).await {
            if bytes_read == 0 {
                break;
            }

            if line.contains("[Behaviour] Entering Room:") {
                last_found_offset = Some(offset);
            }

            offset += bytes_read as u64;
            line.clear();
        }

        last_found_offset.unwrap_or(0)
    }

    pub async fn start(mut self) -> Result<mpsc::UnboundedReceiver<LogEvent>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let log_dir = self.log_dir.clone();

        tokio::spawn(async move {
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

            self.current_log_file = self.get_latest_log_file().await;
            if let Some(ref log_file) = self.current_log_file {
                if let Ok(meta) = tokio::fs::metadata(log_file).await {
                    if was_running {
                        info!("{}", t!("vrchat_was_running_catchup"));
                        self.last_read_pos = self.find_last_room_offset(log_file).await;
                        info!("{}", t!("resuming_log_tracking", self.last_read_pos));
                    } else {
                        info!("{}", t!("scanning_from_eof"));
                        self.last_read_pos = meta.len();
                    }
                }
                self.read_new_lines(&tx).await;
            }

            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;

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

    #[test]
    fn test_parse_timestamp() {
        let line = "2026.03.08 01:14:09 Log        -  ...";
        assert_eq!(LogWatcher::parse_timestamp(line), "2026-03-08T01:14:09");

        let invalid = "invalid line";
        assert_eq!(LogWatcher::parse_timestamp(invalid), "");
    }

    #[test]
    fn test_parse_line_world_entering() {
        let watcher = LogWatcher::new();
        let line = "2026.03.08 01:14:09 [Behaviour] Entering Room: The Great Pug";
        let event = watcher.parse_line(line).unwrap();
        match event {
            LogEvent::Location {
                world_name,
                timestamp,
            } => {
                assert_eq!(world_name, "The Great Pug");
                assert_eq!(timestamp, "2026-03-08T01:14:09");
            }
            _ => panic!("Expected Location event"),
        }
    }

    #[test]
    fn test_parse_line_player_joined() {
        let watcher = LogWatcher::new();
        let line = "2026.03.08 01:14:09 [Behaviour] OnPlayerJoined Bolt (usr_1234-5678)";
        let event = watcher.parse_line(line).unwrap();
        match event {
            LogEvent::PlayerJoined {
                display_name,
                user_id,
                timestamp,
            } => {
                assert_eq!(display_name, "Bolt");
                assert_eq!(user_id, Some("usr_1234-5678".to_string()));
                assert_eq!(timestamp, "2026-03-08T01:14:09");
            }
            _ => panic!("Expected PlayerJoined event"),
        }
    }

    #[test]
    fn test_parse_line_player_left() {
        let watcher = LogWatcher::new();
        let line = "2026.03.08 01:14:09 [Behaviour] OnPlayerLeft Bolt (usr_1234-5678)";
        let event = watcher.parse_line(line).unwrap();
        match event {
            LogEvent::PlayerLeft {
                display_name,
                user_id,
                timestamp,
            } => {
                assert_eq!(display_name, "Bolt");
                assert_eq!(user_id, Some("usr_1234-5678".to_string()));
                assert_eq!(timestamp, "2026-03-08T01:14:09");
            }
            _ => panic!("Expected PlayerLeft event"),
        }
    }
}
