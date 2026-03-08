use anyhow::Result;
use chrono::Local;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{info, warn};

use crate::t;

use crate::recorder::{AudioRecorder, MicConfig};

/// Type of player event for the timeline
#[derive(Debug, Clone)]
pub enum PlayerEventType {
    Joined,
    Left,
}

/// A single player event with timestamp
#[derive(Debug, Clone)]
pub struct PlayerEvent {
    pub time: chrono::DateTime<Local>,
    pub event_type: PlayerEventType,
    pub display_name: String,
    pub user_id: Option<String>,
}

/// A recording session for a single room visit
pub struct RecordingSession {
    pub world_name: String,
    pub instance_id: String,
    pub start_time: chrono::DateTime<Local>,
    pub output_dir: PathBuf,
    timeline_path: PathBuf,
    audio_filename: Option<String>,
    recorder: Option<AudioRecorder>,
    events: Vec<PlayerEvent>,
    pub pid: Option<u32>,
}

impl RecordingSession {
    /// Start a new recording session.
    /// Creates the output directory, writes the initial timeline header, and starts audio recording.
    pub fn start(
        base_dir: &Path,
        world_name: &str,
        instance_id: &str,
        mic_config: MicConfig,
        pid: u32,
    ) -> Result<Self> {
        let now = Local::now();

        // Build directory: base_dir/recordings/YYYY-MM/MMdd_HH-mm WorldName
        let safe_name = sanitize_filename(world_name);
        let month_dir = now.format("%Y-%m").to_string();
        let folder_name = format!("{} {}", now.format("%m%d_%H-%M"), safe_name,);

        let output_dir = base_dir
            .join("recordings")
            .join(&month_dir)
            .join(&folder_name);

        std::fs::create_dir_all(&output_dir)?;
        info!("{}", t!("recording_output_dir", output_dir.display()));

        let timeline_path = output_dir.join("timeline.md");

        // Write initial header to timeline.md immediately
        {
            let mut f = std::fs::File::create(&timeline_path)?;
            writeln!(f, "\n{}\n", t!("timeline_title", world_name))?;
            writeln!(f, "- **{}**: {}", t!("world_name_label"), world_name)?;
            writeln!(f, "- **{}**: {}", t!("instance_id_label"), instance_id)?;
            writeln!(
                f,
                "- **{}**: {}",
                t!("recording_start_label"),
                now.format("%Y-%m-%d %H:%M:%S")
            )?;
            writeln!(f, "- **{}**", t!("recording_status_active"))?;
            writeln!(f, "\n---\n")?;
            writeln!(f, "{}\n", t!("player_timeline_title"))?;
            writeln!(
                f,
                "| {} | {} | {} | {} | {} |",
                t!("table_header_time"),
                t!("table_header_offset"),
                t!("table_header_event"),
                t!("table_header_player"),
                t!("table_header_uid")
            )?;
            writeln!(f, "| :--- | :--- | :--- | :--- | :--- |")?;
        }
        info!("{}", t!("timeline_header_written", timeline_path.display()));

        // Try to start audio recording
        let audio_path = output_dir.join("audio.ogg");
        let recorder = match AudioRecorder::start(pid, audio_path, mic_config) {
            Ok(rec) => {
                info!("{}", t!("audio_recording_started", pid));
                Some(rec)
            }
            Err(e) => {
                warn!("{}", t!("audio_recording_start_failed", e));
                None
            }
        };

        Ok(Self {
            world_name: world_name.to_string(),
            instance_id: instance_id.to_string(),
            start_time: now,
            output_dir,
            timeline_path,
            audio_filename: None,
            recorder,
            events: Vec::new(),
            pid: Some(pid),
        })
    }

    /// Add a player join/leave event to the timeline.
    /// Writes the event to timeline.md immediately (real-time).
    pub fn add_player_event(
        &mut self,
        event_type: PlayerEventType,
        display_name: &str,
        user_id: Option<&str>,
    ) {
        let event = PlayerEvent {
            time: Local::now(),
            event_type,
            display_name: display_name.to_string(),
            user_id: user_id.map(|s| s.to_string()),
        };

        // Append this event row to timeline.md immediately
        if let Err(e) = self.append_event_to_file(&event) {
            warn!("{}", t!("timeline_write_failed", e));
        }

        self.events.push(event);
    }

    /// Check if the recorded process is still running.
    pub fn is_alive(&self) -> bool {
        if let Some(pid) = self.pid {
            crate::recorder::is_process_running(pid)
        } else {
            false
        }
    }

    /// Append a single event row to the timeline.md file
    fn append_event_to_file(&self, event: &PlayerEvent) -> Result<()> {
        let mut f = OpenOptions::new().append(true).open(&self.timeline_path)?;

        let time_str = event.time.format("%H:%M:%S").to_string();
        let offset = (event.time - self.start_time)
            .to_std()
            .unwrap_or(Duration::from_secs(0));
        let offset_str = format!("{:02}:{:02}", offset.as_secs() / 60, offset.as_secs() % 60);

        let event_icon = match event.event_type {
            PlayerEventType::Joined => t!("event_joined"),
            PlayerEventType::Left => t!("event_left"),
        };

        let uid = event
            .user_id
            .as_deref()
            .map(|id| format!("`{}`", id))
            .unwrap_or_else(|| "-".to_string());

        writeln!(
            f,
            "| {} | {} | {} | {} | {} |",
            time_str, offset_str, event_icon, event.display_name, uid
        )?;

        Ok(())
    }

    /// Finish the session: stop recording and rewrite timeline.md with final info
    pub fn finish(mut self) -> Result<PathBuf> {
        let end_time = Local::now();

        // Stop audio recording
        let has_audio = if let Some(recorder) = self.recorder.take() {
            match recorder.stop() {
                Ok(_dur) => true,
                Err(e) => {
                    warn!("{}", t!("stop_recorder_error", e));
                    false
                }
            }
        } else {
            false
        };

        // Calculate duration
        let total_duration = (end_time - self.start_time)
            .to_std()
            .unwrap_or(Duration::from_secs(0));

        // Rename folder to include duration
        let duration_str = format_duration(&total_duration);
        let current_name = self
            .output_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let new_name = format!("{} {}", current_name, duration_str);
        let new_dir = self.output_dir.with_file_name(&new_name);

        if std::fs::rename(&self.output_dir, &new_dir).is_ok() {
            self.output_dir = new_dir;
            self.timeline_path = self.output_dir.join("timeline.md");
        } else {
            warn!("{}", t!("rename_dir_failed"));
        }

        // Rename audio.ogg to detailed filename
        if has_audio {
            let safe_name = sanitize_filename(&self.world_name);
            let audio_new_name = format!(
                "{}_{}_{}_{}.ogg",
                self.start_time.format("%m%d"),
                self.start_time.format("%H-%M"),
                safe_name,
                duration_str,
            );
            let old_audio = self.output_dir.join("audio.ogg");
            let new_audio = self.output_dir.join(&audio_new_name);
            if std::fs::rename(&old_audio, &new_audio).is_ok() {
                self.audio_filename = Some(audio_new_name);
            } else {
                self.audio_filename = Some("audio.ogg".to_string());
            }
        }

        // Rewrite timeline.md with complete information (header + end time + all events)
        let content = self.generate_timeline_md(&end_time, &total_duration, has_audio);
        std::fs::write(&self.timeline_path, &content)?;

        info!(
            "{}",
            t!(
                "session_saved",
                self.output_dir.display(),
                total_duration.as_secs_f64(),
                self.events.len()
            )
        );

        Ok(self.output_dir.clone())
    }

    /// Generate the final timeline markdown content
    fn generate_timeline_md(
        &self,
        end_time: &chrono::DateTime<Local>,
        duration: &Duration,
        has_audio: bool,
    ) -> String {
        let mins = duration.as_secs() / 60;
        let secs = duration.as_secs() % 60;

        let mut md = String::new();

        // Header
        md.push_str(&format!("{}\n\n", t!("timeline_title", self.world_name)));
        md.push_str(&format!(
            "- **{}**: {}\n",
            t!("world_name_label"),
            self.world_name
        ));
        md.push_str(&format!(
            "- **{}**: {}\n",
            t!("instance_id_label"),
            self.instance_id
        ));
        md.push_str(&format!(
            "- **{}**: {}\n",
            t!("recording_start_label"),
            self.start_time.format("%Y-%m-%d %H:%M:%S")
        ));
        md.push_str(&format!(
            "- **{}**: {}\n",
            t!("recording_end_label"),
            end_time.format("%Y-%m-%d %H:%M:%S")
        ));
        md.push_str(&format!(
            "- **{}**: {} {} {} {}\n",
            t!("recording_duration_label"),
            mins,
            t!("mins_label"),
            secs,
            t!("secs_label")
        ));

        if has_audio {
            let fname = self.audio_filename.as_deref().unwrap_or("audio.ogg");
            md.push_str(&format!("- **{}**: {}\n", t!("audio_file_label"), fname));
        } else {
            md.push_str(&format!(
                "- **{}**: {}\n",
                t!("audio_file_label"),
                t!("no_audio_recorded")
            ));
        }

        md.push_str("\n---\n\n");

        // Player timeline
        md.push_str(&format!("{}\n\n", t!("player_timeline_title")));

        if self.events.is_empty() {
            md.push_str(&format!("{}\n", t!("no_player_events")));
        } else {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                t!("table_header_time"),
                t!("table_header_offset"),
                t!("table_header_event"),
                t!("table_header_player"),
                t!("table_header_uid")
            ));
            md.push_str("| ------ | ------ | ------ | --------- | -------- |\n");

            for event in &self.events {
                let time_str = event.time.format("%H:%M:%S").to_string();

                let offset = (event.time - self.start_time)
                    .to_std()
                    .unwrap_or(Duration::from_secs(0));
                let offset_str =
                    format!("{:02}:{:02}", offset.as_secs() / 60, offset.as_secs() % 60);

                let event_icon = match event.event_type {
                    PlayerEventType::Joined => t!("event_joined"),
                    PlayerEventType::Left => t!("event_left"),
                };

                let uid = event
                    .user_id
                    .as_deref()
                    .map(|id| format!("`{}`", id))
                    .unwrap_or_else(|| "-".to_string());

                md.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    time_str, offset_str, event_icon, event.display_name, uid
                ));
            }
        }

        md
    }
}

/// Format duration as human-readable string for folder name
fn format_duration(d: &Duration) -> String {
    let total_secs = d.as_secs();
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hours > 0 {
        t!("duration_format_h_m", hours, mins)
    } else if mins > 0 {
        t!("duration_format_m", mins)
    } else {
        t!("duration_format_s", secs)
    }
}

/// Replace characters that are invalid in Windows file names
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if r#"\/:*?"<>|"#.contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}
