use crate::api::VRChatAPI;
use crate::t;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration, Local, Utc};
use rusqlite::{params, Connection, OpenFlags};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct VrcxReportManager {
    api: Arc<VRChatAPI>,
}

impl VrcxReportManager {
    pub fn new(api: Arc<VRChatAPI>) -> Self {
        Self { api }
    }

    pub async fn export_user_report(&self, user_id: &str) -> Result<PathBuf> {
        let store = VrcxStore::open_default()?;
        let since = Utc::now() - Duration::days(30);

        let profile = self.fetch_profile(user_id).await;
        let fallback = store.load_user_snapshot(user_id)?;
        let timeline = store.load_timeline(user_id, since)?;

        let display_name = profile
            .as_ref()
            .and_then(|data| data.get("displayName").and_then(Value::as_str))
            .or(fallback.display_name.as_deref())
            .unwrap_or(user_id);
        let safe_display_name = sanitize_name(display_name);

        let output_dir = PathBuf::from(&safe_display_name);
        fs::create_dir_all(&output_dir)?;

        let report = self.render_markdown(
            user_id,
            display_name,
            profile.as_ref(),
            &fallback,
            &timeline,
        );
        let file_name = format!(
            "{}_L1-L{}_PROFILE_L1-L{}_TIMELINE_L{}-L{}_GROUPS_L{}-L{}.md",
            safe_display_name,
            report.total_lines,
            report.profile_end_line,
            report.timeline_start_line,
            report.timeline_end_line,
            report.groups_start_line,
            report.groups_end_line
        );
        let report_path = output_dir.join(file_name);

        fs::write(&report_path, report.content)?;
        Ok(report_path)
    }

    async fn fetch_profile(&self, user_id: &str) -> Option<Value> {
        let user_data = self.api.get_user_info(user_id).await.ok()?;
        let groups_data = self
            .api
            .get_user_groups(user_id)
            .await
            .unwrap_or(Value::Array(Vec::new()));

        let mut full_data = user_data;
        if let Some(obj) = full_data.as_object_mut() {
            obj.insert("groups".to_string(), groups_data);
        }
        Some(full_data)
    }

    fn render_markdown(
        &self,
        user_id: &str,
        display_name: &str,
        profile: Option<&Value>,
        fallback: &UserSnapshot,
        timeline: &[TimelineEvent],
    ) -> RenderedReport {
        let mut md = Vec::new();
        md.push(t!("user_info_title"));
        md.push(String::new());

        md.push(format!("**{}**: {}", t!("display_name"), display_name));
        md.push(String::new());

        let bio = profile
            .and_then(|data| data.get("bio").and_then(Value::as_str))
            .or(fallback.bio.as_deref())
            .unwrap_or("");
        md.push(format!("**{}**: ", t!("bio")));
        md.push(String::new());
        if bio.is_empty() {
            md.push(t!("no_data"));
        } else {
            for line in bio.lines() {
                md.push(format!("{}  ", line));
            }
        }
        md.push(String::new());

        md.push(format!("**{}**: ", t!("bio_links")));
        md.push(String::new());
        if let Some(links) = profile
            .and_then(|data| data.get("bioLinks"))
            .and_then(Value::as_array)
        {
            if links.is_empty() {
                md.push(t!("no_data"));
            } else {
                for item in links {
                    if let Some(link) = item.as_str() {
                        md.push(format!("- {}", link));
                    }
                }
            }
        } else {
            md.push(t!("no_data"));
        }
        md.push(String::new());

        md.push(format!("**{}**: {}", t!("user_id"), user_id));
        md.push(String::new());

        let joined = profile
            .and_then(|data| data.get("date_joined").and_then(Value::as_str))
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| t!("no_data"));
        md.push(format!("**{}**: {}", t!("date_joined"), joined));
        md.push(String::new());

        let avatar_image = profile
            .and_then(|data| data.get("currentAvatarImageUrl").and_then(Value::as_str))
            .or(fallback.current_avatar_image_url.as_deref())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| t!("no_data"));
        md.push(format!(
            "**{}**: {}",
            t!("current_avatar_image_url"),
            avatar_image
        ));
        md.push(String::new());

        md.push(format!("**{}**: ", t!("badges")));
        md.push(String::new());
        if let Some(badges) = profile
            .and_then(|data| data.get("badges"))
            .and_then(Value::as_array)
        {
            if badges.is_empty() {
                md.push(t!("no_data"));
            } else {
                for badge in badges {
                    if let Some(name) = badge.get("badgeName").and_then(Value::as_str) {
                        let desc = badge
                            .get("badgeDescription")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        md.push(format!("- **{}**: {}", name, desc));
                    }
                }
            }
        } else {
            md.push(t!("no_data"));
        }
        md.push(String::new());

        let age_status = profile
            .and_then(|data| data.get("ageVerificationStatus"))
            .map(value_to_string)
            .unwrap_or_else(|| t!("no_data"));
        md.push(format!(
            "**{}**: {}",
            t!("age_verification_status"),
            age_status
        ));
        md.push(String::new());

        let age_verified = profile
            .and_then(|data| data.get("ageVerified"))
            .map(value_to_string)
            .unwrap_or_else(|| t!("no_data"));
        md.push(format!("**{}**: {}", t!("age_verified"), age_verified));
        md.push(String::new());

        let profile_end_line = md.len();

        md.push("---".to_string());
        md.push(format!(
            "> 检索时间: {}",
            Local::now().format("%Y年%-m月%-d日")
        ));
        md.push("---".to_string());
        md.push(String::new());

        let timeline_start_line = md.len() + 1;
        md.push(t!("user_report_timeline_title"));
        md.push(String::new());
        md.push(format!(
            "**{}**: {}",
            t!("user_report_query_range"),
            t!("user_report_last_30_days")
        ));
        md.push(format!(
            "**{}**: {}",
            t!("user_report_generated_at"),
            Local::now().format("%Y/%-m/%-d %H:%M:%S")
        ));
        md.push(String::new());

        if timeline.is_empty() {
            md.push(t!("user_report_no_timeline"));
        } else {
            md.push(format!(
                "| {} | {} | {} |",
                t!("user_report_table_time"),
                t!("user_report_table_type"),
                t!("user_report_table_detail")
            ));
            md.push("| ------------ | ---- | -------- |".to_string());

            for event in timeline {
                md.push(format!(
                    "| {} | {} | {} |",
                    event
                        .occurred_at
                        .with_timezone(&Local)
                        .format("%m/%d %H:%M:%S"),
                    event.kind.label(),
                    escape_table(&event.detail)
                ));
            }
        }

        let timeline_end_line = md.len();

        md.push(String::new());
        md.push("---".to_string());
        md.push(String::new());
        let groups_start_line = md.len() + 1;
        md.push(t!("user_info_groups_title"));
        md.push(String::new());

        if let Some(groups) = profile
            .and_then(|data| data.get("groups"))
            .and_then(Value::as_array)
        {
            if groups.is_empty() {
                md.push(t!("no_data"));
            } else {
                for group in groups {
                    let name = group.get("name").and_then(Value::as_str).unwrap_or("N/A");
                    let group_id = group
                        .get("groupId")
                        .and_then(Value::as_str)
                        .unwrap_or("N/A");
                    let description = group
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or("");

                    md.push(format!("## {}", name));
                    md.push(format!("- **{}**: `{}`", t!("group_id"), group_id));
                    md.push(format!("- **{}**:", t!("description")));
                    if description.is_empty() {
                        md.push(format!("> {}", t!("no_data")));
                    } else {
                        for line in description.lines() {
                            md.push(format!("> {}", line));
                        }
                    }
                    md.push(String::new());
                }
            }
        } else if fallback.group_names.is_empty() {
            md.push(t!("no_data"));
        } else {
            for name in &fallback.group_names {
                md.push(format!("## {}", name));
                md.push(String::new());
            }
        }

        let groups_end_line = md.len();
        let total_lines = md.len();

        RenderedReport {
            content: md.join("\n"),
            total_lines,
            profile_end_line,
            timeline_start_line,
            timeline_end_line,
            groups_start_line,
            groups_end_line,
        }
    }
}

struct RenderedReport {
    content: String,
    total_lines: usize,
    profile_end_line: usize,
    timeline_start_line: usize,
    timeline_end_line: usize,
    groups_start_line: usize,
    groups_end_line: usize,
}

#[derive(Default)]
struct UserSnapshot {
    display_name: Option<String>,
    bio: Option<String>,
    current_avatar_image_url: Option<String>,
    group_names: Vec<String>,
}

struct VrcxStore {
    _snapshot: SnapshotDir,
    conn: Connection,
    account_prefix: String,
}

impl VrcxStore {
    fn open_default() -> Result<Self> {
        let db_path = resolve_vrcx_db_path()?;
        let snapshot = SnapshotDir::from_source(&db_path)?;
        let conn = Connection::open_with_flags(
            &snapshot.db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )?;

        let last_user_id: String = conn.query_row(
            "SELECT value FROM configs WHERE key = 'config:lastuserloggedin'",
            [],
            |row| row.get(0),
        )?;

        Ok(Self {
            _snapshot: snapshot,
            conn,
            account_prefix: normalize_table_prefix(&last_user_id),
        })
    }

    fn load_user_snapshot(&self, user_id: &str) -> Result<UserSnapshot> {
        let mut snapshot = UserSnapshot::default();

        let bio_table = self.table_name("feed_bio");
        if self.table_exists(&bio_table)? {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT display_name, bio FROM [{bio_table}] WHERE user_id = ?1 ORDER BY created_at DESC LIMIT 1"
            ))?;
            if let Ok((display_name, bio)) =
                stmt.query_row(params![user_id], |row| Ok((row.get(0)?, row.get(1)?)))
            {
                snapshot.display_name = display_name;
                snapshot.bio = bio;
            }
        }

        let avatar_table = self.table_name("feed_avatar");
        if self.table_exists(&avatar_table)? {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT display_name, current_avatar_image_url FROM [{avatar_table}] WHERE user_id = ?1 ORDER BY created_at DESC LIMIT 1"
            ))?;
            if let Ok((display_name, image_url)) =
                stmt.query_row(params![user_id], |row| Ok((row.get(0)?, row.get(1)?)))
            {
                snapshot.display_name = snapshot.display_name.or(display_name);
                snapshot.current_avatar_image_url = image_url;
            }
        }

        let gps_table = self.table_name("feed_gps");
        if self.table_exists(&gps_table)? {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT display_name, group_name FROM [{gps_table}] WHERE user_id = ?1 AND group_name <> '' ORDER BY created_at DESC LIMIT 10"
            ))?;
            let rows = stmt.query_map(params![user_id], |row| {
                Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows.flatten() {
                snapshot.display_name = snapshot.display_name.clone().or(row.0);
                if !snapshot.group_names.iter().any(|name| name == &row.1) {
                    snapshot.group_names.push(row.1);
                }
            }
        }

        let online_table = self.table_name("feed_online_offline");
        if self.table_exists(&online_table)? {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT display_name FROM [{online_table}] WHERE user_id = ?1 ORDER BY created_at DESC LIMIT 1"
            ))?;
            if let Some(display_name) = stmt
                .query_row(params![user_id], |row| row.get::<_, Option<String>>(0))
                .ok()
                .flatten()
            {
                snapshot.display_name = snapshot.display_name.or(Some(display_name));
            }
        }

        Ok(snapshot)
    }

    fn load_timeline(&self, user_id: &str, since: DateTime<Utc>) -> Result<Vec<TimelineEvent>> {
        let mut events = Vec::new();
        let since_text = since.to_rfc3339();

        let gps_table = self.table_name("feed_gps");
        if self.table_exists(&gps_table)? {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT created_at, world_name FROM [{gps_table}] WHERE user_id = ?1 AND created_at >= ?2 ORDER BY created_at DESC"
            ))?;
            let rows = stmt.query_map(params![user_id, since_text.as_str()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })?;
            for row in rows.flatten() {
                if let Some(world_name) = row.1 {
                    events.push(TimelineEvent::new(
                        row.0,
                        TimelineKind::Location,
                        world_name,
                    )?);
                }
            }
        }

        let online_table = self.table_name("feed_online_offline");
        if self.table_exists(&online_table)? {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT created_at, type FROM [{online_table}] WHERE user_id = ?1 AND created_at >= ?2 ORDER BY created_at DESC"
            ))?;
            let rows = stmt.query_map(params![user_id, since_text.as_str()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows.flatten() {
                let kind = match row.1.as_str() {
                    "Online" => TimelineKind::Online,
                    "Offline" => TimelineKind::Offline,
                    _ => continue,
                };
                events.push(TimelineEvent::new(row.0, kind, String::new())?);
            }
        }

        let bio_table = self.table_name("feed_bio");
        if self.table_exists(&bio_table)? {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT created_at, bio, previous_bio FROM [{bio_table}] WHERE user_id = ?1 AND created_at >= ?2 ORDER BY created_at DESC"
            ))?;
            let rows = stmt.query_map(params![user_id, since_text.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?;
            for row in rows.flatten() {
                let bio = row.1.unwrap_or_default();
                let previous_bio = row.2.unwrap_or_default();
                if bio != previous_bio {
                    events.push(TimelineEvent::new(
                        row.0,
                        TimelineKind::Bio,
                        format_bio_detail(&bio, &previous_bio),
                    )?);
                }
            }
        }

        let status_table = self.table_name("feed_status");
        if self.table_exists(&status_table)? {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT created_at, status, status_description FROM [{status_table}] WHERE user_id = ?1 AND created_at >= ?2 ORDER BY created_at DESC"
            ))?;
            let rows = stmt.query_map(params![user_id, since_text.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?;
            for row in rows.flatten() {
                let status = row.1.unwrap_or_default();
                let description = row.2.unwrap_or_default();
                events.push(TimelineEvent::new(
                    row.0,
                    TimelineKind::Status,
                    format_status_detail(&status, &description),
                )?);
            }
        }

        let avatar_table = self.table_name("feed_avatar");
        if self.table_exists(&avatar_table)? {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT created_at, avatar_name FROM [{avatar_table}] WHERE user_id = ?1 AND created_at >= ?2 ORDER BY created_at DESC"
            ))?;
            let rows = stmt.query_map(params![user_id, since_text.as_str()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })?;
            for row in rows.flatten() {
                if let Some(avatar_name) = row.1 {
                    events.push(TimelineEvent::new(
                        row.0,
                        TimelineKind::Avatar,
                        avatar_name,
                    )?);
                }
            }
        }

        events.sort_by(|a, b| b.occurred_at.cmp(&a.occurred_at));
        Ok(events)
    }

    fn table_name(&self, suffix: &str) -> String {
        format!("{}_{}", self.account_prefix, suffix)
    }

    fn table_exists(&self, table_name: &str) -> Result<bool> {
        let exists = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name = ?1)",
            params![table_name],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(exists == 1)
    }
}

struct SnapshotDir {
    dir: PathBuf,
    db_path: PathBuf,
}

impl SnapshotDir {
    fn from_source(source_db: &Path) -> Result<Self> {
        let file_name = source_db
            .file_name()
            .ok_or_else(|| anyhow!("Invalid VRCX database path"))?;
        let temp_dir = std::env::temp_dir().join(format!(
            "vrmemoir-vrcx-{}-{}",
            std::process::id(),
            Utc::now().timestamp_millis()
        ));
        fs::create_dir_all(&temp_dir)?;

        let target_db = temp_dir.join(file_name);
        fs::copy(source_db, &target_db).with_context(|| {
            format!("Failed to copy VRCX database from {}", source_db.display())
        })?;

        for suffix in ["-wal", "-shm"] {
            let sidecar_source = PathBuf::from(format!("{}{}", source_db.display(), suffix));
            if sidecar_source.exists() {
                let sidecar_target = PathBuf::from(format!("{}{}", target_db.display(), suffix));
                let _ = fs::copy(sidecar_source, sidecar_target);
            }
        }

        Ok(Self {
            dir: temp_dir,
            db_path: target_db,
        })
    }
}

impl Drop for SnapshotDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[derive(Clone, Copy)]
enum TimelineKind {
    Location,
    Online,
    Offline,
    Bio,
    Status,
    Avatar,
}

impl TimelineKind {
    fn label(self) -> &'static str {
        match self {
            TimelineKind::Location => "位置",
            TimelineKind::Online => "上线",
            TimelineKind::Offline => "下线",
            TimelineKind::Bio => "简介",
            TimelineKind::Status => "状态",
            TimelineKind::Avatar => "模型",
        }
    }
}

struct TimelineEvent {
    occurred_at: DateTime<Utc>,
    kind: TimelineKind,
    detail: String,
}

impl TimelineEvent {
    fn new(created_at: String, kind: TimelineKind, detail: String) -> Result<Self> {
        let occurred_at = DateTime::parse_from_rfc3339(&created_at)
            .with_context(|| format!("Invalid VRCX timestamp: {}", created_at))?
            .with_timezone(&Utc);

        Ok(Self {
            occurred_at,
            kind,
            detail,
        })
    }
}

fn resolve_vrcx_db_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("VRCX_DB_PATH") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    let appdata = std::env::var("APPDATA").context("APPDATA is not set")?;
    let default = PathBuf::from(appdata).join("VRCX").join("VRCX.sqlite3");
    if default.exists() {
        Ok(default)
    } else {
        Err(anyhow!("VRCX.sqlite3 not found at {}", default.display()))
    }
}

fn normalize_table_prefix(user_id: &str) -> String {
    user_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

fn sanitize_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "unknown_user".to_string();
    }

    trimmed
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => t!("no_data"),
        Value::String(s) => s.clone(),
        _ => value.to_string(),
    }
}

fn format_bio_detail(current: &str, previous: &str) -> String {
    let current_summary = summarize_bio(current);
    let previous_summary = summarize_bio(previous);

    match (previous_summary.is_empty(), current_summary.is_empty()) {
        (true, false) => format!("新增: {}", current_summary),
        (false, true) => format!("清空: {}", previous_summary),
        (false, false) => format!("由「{}」改为「{}」", previous_summary, current_summary),
        (true, true) => "简介已变更".to_string(),
    }
}

fn summarize_bio(value: &str) -> String {
    let first_line = value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");

    let mut summary = first_line.replace('|', "/");
    if summary.chars().count() > 32 {
        summary = format!("{}...", summary.chars().take(32).collect::<String>());
    }
    summary
}

fn format_status_detail(status: &str, description: &str) -> String {
    let status_name = match status {
        "active" => "在线",
        "join me" => "跟随我",
        "ask me" => "请先询问",
        "busy" => "忙碌",
        "offline" => "离线",
        other => other,
    };

    if description.trim().is_empty() {
        format!("[{}]", status_name)
    } else {
        format!("[{}] {}", status_name, description.trim())
    }
}

fn escape_table(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        value.replace('|', "\\|")
    }
}
