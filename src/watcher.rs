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
        if line.len() >= 19 && line.is_char_boundary(19) && line.as_bytes()[0].is_ascii_digit() {
            let mut s = String::with_capacity(19);
            // Optimized timestamp conversion: "2026.03.08 01:14:09" -> "2026-03-08T01:14:09"
            for (i, &byte) in line.as_bytes().iter().take(19).enumerate() {
                match i {
                    4 | 7 => s.push('-'),
                    10 => s.push('T'),
                    _ => s.push(byte as char),
                }
            }
            s
        } else {
            String::new()
        }
    }

    fn parse_line(&self, line: &str) -> Option<LogEvent> {
        // 1. World Name
        if let Some(pos) = line.find("] Entering Room: ") {
            return Some(LogEvent::Location {
                world_name: line[pos + 17..].to_string(),
                timestamp: Self::parse_timestamp(line),
            });
        }

        // 2. Instance ID
        if let Some(pos) = line.find("] Joining wrld_") {
            return Some(LogEvent::LocationInstance {
                location: line[pos + 10..].to_string(),
                timestamp: Self::parse_timestamp(line),
            });
        }

        // 3. Player Joined
        if let Some(pos) = line.find("] OnPlayerJoined ") {
            let parts = &line[pos + 17..];
            if let Some(caps) = self.player_re.captures(parts) {
                return Some(LogEvent::PlayerJoined {
                    display_name: caps[1].to_string(),
                    user_id: Some(caps[2].to_string()),
                    timestamp: Self::parse_timestamp(line),
                });
            } else {
                return Some(LogEvent::PlayerJoined {
                    display_name: parts.trim().to_string(),
                    user_id: None,
                    timestamp: Self::parse_timestamp(line),
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
                        timestamp: Self::parse_timestamp(line),
                    });
                } else {
                    return Some(LogEvent::PlayerLeft {
                        display_name: parts.trim().to_string(),
                        user_id: None,
                        timestamp: Self::parse_timestamp(line),
                    });
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

        let mut lines = content.lines().peekable();

        while let Some(line) = lines.next() {
            if lines.peek().is_none() && !has_trailing_newline {
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
    fn find_last_room_offset(&self, path: &Path) -> u64 {
        use std::io::{BufRead, BufReader};

        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(_) => return 0,
        };

        // We will read line by line and track the byte offset of the start of the line.
        // If the file is huge, reading forwards is still very fast in Rust, and easier than backwards seeking for text.
        let mut reader = BufReader::new(file);
        let mut offset: u64 = 0;
        let mut last_found_offset: Option<u64> = None;
        let mut line = String::new();

        while let Ok(bytes_read) = reader.read_line(&mut line) {
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
            self.current_log_file = self.get_latest_log_file();
            if let Some(ref log_file) = self.current_log_file {
                if let Ok(meta) = fs::metadata(log_file) {
                    if was_running {
                        // App started AFTER VRChat: find exact location of last room join
                        info!("{}", t!("vrchat_was_running_catchup"));
                        self.last_read_pos = self.find_last_room_offset(log_file);
                        info!("{}", t!("resuming_log_tracking", self.last_read_pos));
                    } else {
                        // App started BEFORE VRChat: ignore past session, only read new lines
                        info!("{}", t!("scanning_from_eof"));
                        self.last_read_pos = meta.len();
                    }
                }
                self.read_new_lines(&tx);
            }

            // 3. Normal polling loop (simpler and more reliable on Windows than fsnotify)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_timestamp() {
        let line = "2026.03.08 01:14:09 [Behaviour] Entering Room: Test World";
        let ts = LogWatcher::parse_timestamp(line);
        assert_eq!(ts, "2026-03-08T01:14:09");

        let invalid_line = "short line";
        let ts2 = LogWatcher::parse_timestamp(invalid_line);
        assert_eq!(ts2, "");
    }

    #[test]
    fn test_parse_line_world() {
        let watcher = LogWatcher {
            log_dir: PathBuf::new(),
            current_log_file: None,
            last_read_pos: 0,
            incomplete_line: String::new(),
            player_re: Regex::new(r"(.+) \((usr_[a-f0-9-]+)\)").unwrap(),
        };

        let line = "2026.03.08 01:14:09 [Behaviour] Entering Room: Test World";
        let event = watcher.parse_line(line).unwrap();
        if let LogEvent::Location {
            world_name,
            timestamp,
        } = event
        {
            assert_eq!(world_name, "Test World");
            assert_eq!(timestamp, "2026-03-08T01:14:09");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_parse_line_joining() {
        let watcher = LogWatcher {
            log_dir: PathBuf::new(),
            current_log_file: None,
            last_read_pos: 0,
            incomplete_line: String::new(),
            player_re: Regex::new(r"(.+) \((usr_[a-f0-9-]+)\)").unwrap(),
        };

        let line = "2026.03.08 01:14:10 [Behaviour] Joining wrld_12345:67890";
        let event = watcher.parse_line(line).unwrap();
        if let LogEvent::LocationInstance {
            location,
            timestamp,
        } = event
        {
            assert_eq!(location, "wrld_12345:67890");
            assert_eq!(timestamp, "2026-03-08T01:14:10");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_parse_line_player_joined() {
        let watcher = LogWatcher {
            log_dir: PathBuf::new(),
            current_log_file: None,
            last_read_pos: 0,
            incomplete_line: String::new(),
            player_re: Regex::new(r"(.+) \((usr_[a-f0-9-]+)\)").unwrap(),
        };

        let line = "2026.03.08 01:14:11 [Behaviour] OnPlayerJoined Bolt (usr_abcd-1234)";
        let event = watcher.parse_line(line).unwrap();
        if let LogEvent::PlayerJoined {
            display_name,
            user_id,
            timestamp,
        } = event
        {
            assert_eq!(display_name, "Bolt");
            assert_eq!(user_id, Some("usr_abcd-1234".to_string()));
            assert_eq!(timestamp, "2026-03-08T01:14:11");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_parse_line_player_left() {
        let watcher = LogWatcher {
            log_dir: PathBuf::new(),
            current_log_file: None,
            last_read_pos: 0,
            incomplete_line: String::new(),
            player_re: Regex::new(r"(.+) \((usr_[a-f0-9-]+)\)").unwrap(),
        };

        let line = "2026.03.08 01:14:12 [Behaviour] OnPlayerLeft Bolt (usr_abcd-1234)";
        let event = watcher.parse_line(line).unwrap();
        if let LogEvent::PlayerLeft {
            display_name,
            user_id,
            timestamp,
        } = event
        {
            assert_eq!(display_name, "Bolt");
            assert_eq!(user_id, Some("usr_abcd-1234".to_string()));
            assert_eq!(timestamp, "2026-03-08T01:14:12");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_read_new_lines_incomplete() {
        use std::io::Write;

        let test_dir = Path::new("test_logs");
        fs::create_dir_all(test_dir).unwrap();
        let log_file = test_dir.join("output_log_test.txt");
        {
            let mut f = fs::File::create(&log_file).unwrap();
            f.write_all(b"2026.03.08 01:14:09 [Behaviour] Entering Room: World1\n2026.03.08 01:14:10 [Behaviour] Joining ").unwrap();
        }

        let mut watcher = LogWatcher {
            log_dir: test_dir.to_path_buf(),
            current_log_file: Some(log_file.clone()),
            last_read_pos: 0,
            incomplete_line: String::new(),
            player_re: Regex::new(r"(.+) \((usr_[a-f0-9-]+)\)").unwrap(),
        };

        let (tx, mut rx) = mpsc::unbounded_channel();
        watcher.read_new_lines(&tx);

        let event = rx.try_recv().unwrap();
        if let LogEvent::Location { world_name, .. } = event {
            assert_eq!(world_name, "World1");
        } else {
            panic!("Wrong event");
        }
        assert!(rx.try_recv().is_err());
        assert_eq!(
            watcher.incomplete_line,
            "2026.03.08 01:14:10 [Behaviour] Joining "
        );

        // Append the rest
        {
            let mut f = fs::OpenOptions::new().append(true).open(&log_file).unwrap();
            f.write_all(b"wrld_1234\n").unwrap();
        }

        watcher.read_new_lines(&tx);
        let event = rx.try_recv().unwrap();
        if let LogEvent::LocationInstance { location, .. } = event {
            assert_eq!(location, "wrld_1234");
        } else {
            panic!("Wrong event");
        }
        assert!(watcher.incomplete_line.is_empty());

        fs::remove_dir_all(test_dir).unwrap();
    }
}
