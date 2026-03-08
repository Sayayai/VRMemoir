use crate::api::VRChatAPI;
use crate::db::Database;
use crate::t;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub struct BioManager {
    api: Arc<VRChatAPI>,
    db: Arc<Database>,
    last_global_fetch: Mutex<Option<Instant>>,
    user_requests: Mutex<HashMap<String, Instant>>,
}

impl BioManager {
    pub fn new(api: Arc<VRChatAPI>, db: Arc<Database>) -> Self {
        Self {
            api,
            db,
            last_global_fetch: Mutex::new(None),
            user_requests: Mutex::new(HashMap::new()),
        }
    }

    /// Process a user's BIO: Check DB -> Rate Limit -> Fetch -> Save DB -> Save MD -> Optional Symlink
    pub async fn process_user(
        &self,
        user_id: &str,
        force_refresh: bool,
        session_dir: Option<&Path>,
        skip_rate_limit: bool,
    ) -> Result<serde_json::Value> {
        // Find if local file already exists for this user ID
        // The display name might have changed so we rely on fetching basic info or just check if it was fetched previously
        // Actually, without fetching, we don't know the display name to check the local file accurately.
        // Let's use the Database's `displayName` for this user to check local file existence.

        // 1. Check local file existence first (unless forced)
        if !force_refresh {
            if let Some(display_name) = self.db.get_display_name(user_id) {
                if let Some(existing_file) = self.find_existing_bio_file(&display_name) {
                    tracing::info!(
                        "BIO already exists locally for {} ({}), skipping fetch.",
                        display_name,
                        user_id
                    );
                    // Also create symlink if needed using the existing file
                    if let Some(target_dir) = session_dir {
                        let _ = self.create_symlink(&existing_file, target_dir, &display_name);
                    }
                    return Ok(
                        serde_json::json!({ "id": user_id, "cached_file": existing_file.display().to_string(), "cached": true }),
                    );
                }
            }
        }

        // 2. Rate Limiting
        if !skip_rate_limit {
            self.check_rate_limit(user_id).await?;
        }

        // 3. Fetch from VRChat API
        let user_data = self.api.get_user_info(user_id).await?;
        let groups_data = self
            .api
            .get_user_groups(user_id)
            .await
            .unwrap_or(serde_json::Value::Array(vec![]));

        let mut full_data = user_data.clone();
        if let Some(obj) = full_data.as_object_mut() {
            obj.insert("groups".to_string(), groups_data);
        }

        // 4. Save to Database
        let display_name = full_data["displayName"].as_str().unwrap_or("Unknown");
        let bio = full_data["bio"].as_str().unwrap_or("");

        // 4. Update Database (Always update if we fetched it)
        self.db
            .register_user(user_id, display_name, Some(bio), None, None);
        self.db.update_bio_history(user_id, display_name, bio);

        // 5. Generate Markdown in ./bio/
        let md_path = self.save_markdown(&full_data)?;

        // 6. Optional Symlink to session directory
        if let Some(target_dir) = session_dir {
            self.create_symlink(&md_path, target_dir, display_name)?;
        }

        Ok(full_data)
    }

    async fn check_rate_limit(&self, user_id: &str) -> Result<()> {
        let now = Instant::now();
        let minute = Duration::from_secs(60);
        let six_seconds = Duration::from_secs(6);

        // Global limit: Strict 6-second gap
        {
            let mut last_fetch = self.last_global_fetch.lock().await;
            if let Some(last_time) = *last_fetch {
                if now.duration_since(last_time) < six_seconds {
                    return Err(anyhow!(
                        "Rate limit reached: Minimum 6s between any BIO fetches"
                    ));
                }
            }
            *last_fetch = Some(now);
        }

        // Per user limit: 1 per minute
        {
            let mut users = self.user_requests.lock().await;
            if let Some(last_time) = users.get(user_id) {
                if now.duration_since(*last_time) < minute {
                    return Err(anyhow!("Rate limit reached for user {} (1/min)", user_id));
                }
            }
            users.insert(user_id.to_string(), now);
        }

        Ok(())
    }

    fn save_markdown(&self, user_data: &serde_json::Value) -> Result<PathBuf> {
        let bio_dir = Path::new("bio");
        if !bio_dir.exists() {
            std::fs::create_dir_all(bio_dir)?;
        }

        let display_name_val = user_data
            .get("displayName")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown_user");
        let display_name = display_name_val.trim();
        let re = regex::Regex::new(r#"[\\/:*?"<>|]"#)?;
        let display_name_safe = re.replace_all(display_name, "_").to_string();

        let mut md_content = Vec::new();
        // L1 is the title
        md_content.push(t!("user_info_title"));
        md_content.push("".to_string());

        let allowed_keys = vec![
            "id",
            "displayName",
            "date_joined",
            "currentAvatarImageUrl",
            "bioLinks",
            "bio",
            "badges",
            "ageVerificationStatus",
            "ageVerified",
        ];
        let priority_keys = vec!["displayName", "bio", "bioLinks"];

        self.generate_md_body(user_data, &allowed_keys, &priority_keys, &mut md_content);
        let bio_end_line = md_content.len();

        // Separate section with timestamp
        md_content.push("---".to_string());
        let current_time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        md_content.push(format!("> Retrieval Time: {}", current_time)); // Placing timestamp here
        md_content.push("---".to_string());
        md_content.push("".to_string());

        let groups_start_line = md_content.len() + 1;
        md_content.push(t!("user_info_groups_title"));
        md_content.push("".to_string());

        if let Some(groups) = user_data.get("groups").and_then(|g| g.as_array()) {
            for group in groups {
                let name = group.get("name").and_then(|v| v.as_str()).unwrap_or("N/A");
                let group_id = group
                    .get("groupId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("N/A");
                let description = group
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let formatted_desc = description.replace('\n', "\n> ");

                md_content.push(format!("## {}", name));
                md_content.push(format!("- **{}**: `{}`", t!("group_id"), group_id));
                md_content.push(format!("- **{}**:", t!("description")));
                md_content.push(format!("> {}", formatted_desc));
                md_content.push("".to_string());
                md_content.push("---".to_string());
                md_content.push("".to_string());
            }
        }
        let groups_end_line = md_content.len();

        // New filename pattern: DisplayName_L1-LX_BIO_LY-LZ_GROUPS.md
        let file_name = format!(
            "{}_L1-L{}_BIO_L{}-L{}_GROUPS.md",
            display_name_safe, bio_end_line, groups_start_line, groups_end_line
        );

        let file_path = bio_dir.join(&file_name);
        let new_content = md_content.join("\n");

        // Idempotent write: Avoid Error 32 if file is open and content hasn't changed
        if file_path.exists() {
            if let Ok(existing) = std::fs::read_to_string(&file_path) {
                if existing == new_content {
                    return Ok(file_path);
                }
            }
        }

        std::fs::write(&file_path, new_content)?;

        Ok(file_path)
    }

    fn generate_md_body(
        &self,
        user_data: &serde_json::Value,
        allowed_keys: &[&str],
        priority_keys: &[&str],
        md: &mut Vec<String>,
    ) {
        if let Some(obj) = user_data.as_object() {
            for key in priority_keys {
                if let Some(val) = obj.get(*key) {
                    if allowed_keys.contains(key) {
                        self.format_field(key, val, md);
                    }
                }
            }
            for key in allowed_keys {
                if !priority_keys.contains(key) && obj.contains_key(*key) {
                    if let Some(val) = obj.get(*key) {
                        self.format_field(key, val, md);
                    }
                }
            }
        }
    }

    fn format_field(&self, key: &str, value: &serde_json::Value, md: &mut Vec<String>) {
        let translated_key = match key {
            "ageVerificationStatus" => t!("age_verification_status"),
            "ageVerified" => t!("age_verified"),
            "badges" => t!("badges"),
            "bio" => t!("bio"),
            "bioLinks" => t!("bio_links"),
            "currentAvatarImageUrl" => t!("current_avatar_image_url"),
            "date_joined" => t!("date_joined"),
            "displayName" => t!("display_name"),
            "id" => t!("user_id"),
            _ => key.to_string(),
        };

        let mut first_line = format!("**{}**: ", translated_key);

        match value {
            serde_json::Value::Array(arr) => {
                if arr.is_empty() {
                    first_line.push_str(&t!("no_data"));
                    md.push(first_line);
                    md.push("".to_string());
                } else {
                    md.push(first_line);
                    md.push("".to_string());
                    for item in arr {
                        if let serde_json::Value::Object(obj) = item {
                            if let Some(badge_name) = obj.get("badgeName").and_then(|v| v.as_str())
                            {
                                let badge_desc = obj
                                    .get("badgeDescription")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                md.push(format!("- **{}**: {}", badge_name, badge_desc));
                            } else {
                                md.push(format!(
                                    "- {}",
                                    serde_json::to_string(item).unwrap_or_default()
                                ));
                            }
                        } else if let Some(s) = item.as_str() {
                            md.push(format!("- {}", s));
                        } else {
                            md.push(format!("- {}", item));
                        }
                    }
                    md.push("".to_string());
                }
            }
            serde_json::Value::Object(obj) => {
                md.push(first_line);
                md.push("".to_string());
                md.push("```json".to_string());
                if let Ok(json_str) = serde_json::to_string_pretty(obj) {
                    for line in json_str.split('\n') {
                        md.push(line.to_string());
                    }
                }
                md.push("```".to_string());
                md.push("".to_string());
            }
            serde_json::Value::String(s) => {
                if key == "bio" {
                    md.push(first_line);
                    md.push("".to_string());
                    for line in s.split('\n') {
                        md.push(format!("{}  ", line));
                    }
                    md.push("".to_string());
                } else {
                    let replaced = s.replace('\n', "<br>");
                    first_line.push_str(&replaced);
                    md.push(first_line);
                    md.push("".to_string());
                }
            }
            _ => {
                let s = value.to_string();
                if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                    first_line.push_str(&s[1..s.len() - 1]);
                } else {
                    first_line.push_str(&s);
                }
                md.push(first_line);
                md.push("".to_string());
            }
        }
    }

    fn create_symlink(
        &self,
        target_path: &Path,
        session_dir: &Path,
        _display_name: &str,
    ) -> Result<()> {
        let abs_target = std::fs::canonicalize(target_path)?;

        // Create the bio/ subdirectory in the session folder
        let session_bio_dir = session_dir.join("bio");
        if !session_bio_dir.exists() {
            std::fs::create_dir_all(&session_bio_dir)?;
        }

        // Use the exact filename from the target path for the symlink
        let link_name = target_path
            .file_name()
            .ok_or_else(|| anyhow!("Invalid target path for symlink"))?;
        let link_path = session_bio_dir.join(link_name);

        // Idempotency: skip if already correct
        if link_path.exists() {
            if let (Ok(src_meta), Ok(dst_meta)) = (
                std::fs::metadata(target_path),
                std::fs::metadata(&link_path),
            ) {
                if src_meta.len() == dst_meta.len() {
                    return Ok(());
                }
            }
        }

        #[cfg(windows)]
        {
            if link_path.exists() {
                let _ = std::fs::remove_file(&link_path);
            }
            if let Err(_) = std::os::windows::fs::symlink_file(&abs_target, &link_path) {
                if let Err(_) = std::fs::hard_link(&abs_target, &link_path) {
                    std::fs::copy(&abs_target, &link_path)?;
                }
            }
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&abs_target, &link_path)?;
        }

        Ok(())
    }

    /// Search the `bio/` directory for any file that matches the user's display name.
    fn find_existing_bio_file(&self, display_name: &str) -> Option<PathBuf> {
        let bio_dir = Path::new("bio");
        if !bio_dir.exists() {
            return None;
        }

        let re = regex::Regex::new(r#"[\\/:*?"<>|]"#).ok()?;
        let display_name_safe = re.replace_all(display_name.trim(), "_").to_string();
        let search_pattern = format!("{}_L1-", display_name_safe);

        if let Ok(entries) = std::fs::read_dir(bio_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains(&search_pattern) && name.ends_with(".md") {
                    return Some(entry.path());
                }
            }
        }
        None
    }
}
