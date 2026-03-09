use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, warn};

use crate::db::Database;
use crate::recorder::{find_vrchat_pid, MicConfig};
use crate::session::{PlayerEventType, RecordingSession};
use crate::t;
use crate::watcher::LogEvent;

// ---------------------------------------------------------------------------
// 状态定义
// ---------------------------------------------------------------------------

/// 应用程序的有限状态
enum AppState {
    /// 空闲 — 未进入任何世界
    Idle,

    /// 已进入世界（缓存了世界名称），但尚未收到 `Joining wrld_` 开始录制
    InWorld { world_name: String },

    /// 录制中 — 有活跃的 RecordingSession
    Recording {
        world_name: String,
        instance_id: String,
        session: Box<RecordingSession>,
        session_start_time: String,
        retry_cooldown: HashMap<String, Instant>,
        processed: std::collections::HashSet<String>,
        last_bio_check: Instant,
    },
}

// ---------------------------------------------------------------------------
// 有限状态机
// ---------------------------------------------------------------------------

/// 集中管理所有运行时状态与状态转换逻辑的有限状态机。
pub struct AppFsm {
    state: AppState,
    db: Arc<Database>,
    mic_config: Arc<MicConfig>,
    base_dir: PathBuf,
}

impl AppFsm {
    /// 创建一个新的 FSM，初始状态为 `Idle`。
    pub fn new(db: Arc<Database>, mic_config: Arc<MicConfig>, base_dir: PathBuf) -> Self {
        Self {
            state: AppState::Idle,
            db,
            mic_config,
            base_dir,
        }
    }

    // -----------------------------------------------------------------------
    // 公开方法
    // -----------------------------------------------------------------------

    /// 处理一个日志事件，驱动状态转换。
    pub fn handle_event(&mut self, event: LogEvent) {
        match event {
            LogEvent::Location {
                world_name,
                timestamp: _,
            } => self.on_location(world_name),

            LogEvent::LocationInstance {
                location,
                timestamp: _,
            } => self.on_location_instance(location),

            LogEvent::VoiceReady { timestamp: _ } => {
                info!("{}", t!("audio_ready"));
            }

            LogEvent::PlayerJoined {
                display_name,
                user_id,
                timestamp,
            } => self.on_player_joined(display_name, user_id, timestamp),

            LogEvent::PlayerLeft {
                display_name,
                user_id,
                timestamp,
            } => self.on_player_left(display_name, user_id, timestamp),
        }
    }

    /// 检测 VRChat 进程是否已退出。若已退出则自动结束录音并回到 Idle。
    pub fn check_process_alive(&mut self) {
        let should_finish =
            matches!(&self.state, AppState::Recording { session, .. } if !session.is_alive());

        if should_finish {
            info!("{}", t!("vrchat_exited"));
            self.finish_current_session(&t!("vrchat_exit"));
        }
    }

    /// 如果当前处于 InWorld 但没有开始录制（可能是启动时错过了事件或 PID 没找到），尝试补课开始。
    pub fn try_auto_start_recording(&mut self) {
        if let AppState::InWorld { world_name: _ } = &self.state {
            if let Some(pid) = find_vrchat_pid() {
                info!("{}", t!("auto_start_detected_pid", pid));

                // 模拟一个 LocationInstance 事件来驱动转换，Instance ID 暂时未知则设为 Unknown
                self.on_location_instance(format!("wrld_unknown:{}", t!("unknown_instance")));
                // 修正：如果还是 InWorld (比如 on_location_instance 失败了)，不重复处理
            }
        }
    }

    /// Retrieve the next user ID that needs a BIO fetch, if cooldown has passed.
    pub fn get_next_bio_candidate(&mut self) -> Option<(String, String, PathBuf)> {
        if let AppState::Recording {
            session_start_time,
            retry_cooldown,
            processed,
            last_bio_check,
            session,
            ..
        } = &mut self.state
        {
            let now = Instant::now();
            if now.duration_since(*last_bio_check) < Duration::from_secs(6) {
                return None; // Still cooling down the main loop
            }

            *last_bio_check = now;

            // 1. Get all active players who joined AFTER the session started
            let active = self.db.get_active_players_without_bio(session_start_time);

            // 2. Filter out those already processed or in retry cooldown (failed in the last 60 seconds)
            let candidate = active.into_iter().find(|(uid, _)| {
                if processed.contains(uid) {
                    return false;
                }
                if let Some(last_fail) = retry_cooldown.get(uid) {
                    now.duration_since(*last_fail) > Duration::from_secs(60)
                } else {
                    true
                }
            });

            if let Some((uid, name)) = candidate {
                return Some((uid, name, session.output_dir.clone()));
            }
        }
        None
    }

    /// Update the tracking state based on the result of a fetch.
    pub fn mark_bio_result(&mut self, uid: &str, success: bool) {
        if let AppState::Recording {
            retry_cooldown,
            processed,
            ..
        } = &mut self.state
        {
            if success {
                retry_cooldown.remove(uid);
                processed.insert(uid.to_string());
            } else {
                retry_cooldown.insert(uid.to_string(), Instant::now());
            }
        }
    }

    /// 程序退出时调用，安全结束任何进行中的录音。
    pub fn shutdown(&mut self) {
        if matches!(&self.state, AppState::Recording { .. }) {
            info!("{}", t!("shutdown_saving_recording"));
            self.finish_current_session(&t!("program_exit"));
        }
    }

    // -----------------------------------------------------------------------
    // 事件处理（内部）
    // -----------------------------------------------------------------------

    /// `Entering Room:` — 更新世界名称缓存
    fn on_location(&mut self, world_name: String) {
        info!("{}", t!("world", world_name));

        match &mut self.state {
            AppState::Idle => {
                self.state = AppState::InWorld { world_name };
            }
            AppState::InWorld { world_name: wn } => {
                *wn = world_name;
            }
            AppState::Recording { world_name: wn, .. } => {
                *wn = world_name;
            }
        }
    }

    /// `Joining wrld_` — 切换房间 / 开始录制
    fn on_location_instance(&mut self, location: String) {
        // 1. 获取当前缓存的世界名称（需在结束旧会话前获取，以防状态被重置为 Idle）
        let world_name = match &self.state {
            AppState::InWorld { world_name } => world_name.clone(),
            AppState::Idle => t!("unknown_world"),
            AppState::Recording { world_name, .. } => world_name.clone(),
        };

        // 2. 如果当前正在录音，先结束旧的
        if matches!(&self.state, AppState::Recording { .. }) {
            self.finish_current_session(&t!("switching_room"));
        }

        // Parse instance ID from location string
        let instance_id = location.split(':').nth(1).unwrap_or("").to_string();
        let instance_id_str = if instance_id.is_empty() {
            t!("unknown_instance")
        } else {
            instance_id.clone()
        };

        info!("{}", t!("instance", location));

        // 3. 尝试开始新录音
        if let Some(pid) = find_vrchat_pid() {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let world_name_str = world_name.clone();

            let mc = (*self.mic_config).clone();
            match RecordingSession::start(
                &self.base_dir,
                &world_name_str,
                &instance_id_str,
                mc,
                pid,
            ) {
                Ok(session) => {
                    info!("{}", t!("entering_recording_state"));

                    self.state = AppState::Recording {
                        world_name: world_name_str.clone(),
                        instance_id: instance_id_str,
                        session: Box::new(session),
                        session_start_time: timestamp,
                        retry_cooldown: HashMap::new(),
                        processed: std::collections::HashSet::new(),
                        last_bio_check: Instant::now(),
                    };
                    info!("{}", t!("starting_bio_pacing_track", world_name_str));
                }
                Err(e) => {
                    warn!("{}", t!("mic_start_failed", e));
                    self.state = AppState::InWorld { world_name };
                }
            }
        } else {
            warn!("{}", t!("no_vrchat_process"));
            self.state = AppState::InWorld { world_name };
        }
    }

    /// 玩家加入
    fn on_player_joined(
        &mut self,
        display_name: String,
        user_id: Option<String>,
        timestamp: String,
    ) {
        let uid_display = user_id
            .as_deref()
            .map(|id| format!(" ({})", id))
            .unwrap_or_default();
        info!("{}", t!("player_joined", display_name, uid_display));

        // 强行纠正状态：如果在 Idle 收到了玩家加入，说明漏掉了 Entering Room
        if matches!(&self.state, AppState::Idle) {
            info!("{}", t!("catchup_player_joined"));
            self.state = AppState::InWorld {
                world_name: t!("unknown_world"),
            };
        }

        // 录制时间线
        if let AppState::Recording { session, .. } = &mut self.state {
            session.add_player_event(PlayerEventType::Joined, &display_name, user_id.as_deref());
        }

        // 数据库记录
        if let Some(ref uid) = user_id {
            self.db.register_user(uid, &display_name, None, None, None);
            let (wn, inst) = self.current_world_instance();
            self.db.start_visit(uid, &wn, &inst, &timestamp);

            // Note: Auto-BIO is now handled by a polling loop in the Recording state
        }
    }

    fn on_player_left(&mut self, display_name: String, user_id: Option<String>, timestamp: String) {
        info!("{}", t!("player_left", display_name));

        // 强行纠正状态：如果在 Idle 收到了玩家离开，说明漏掉了 Entering Room
        if matches!(&self.state, AppState::Idle) {
            info!("{}", t!("catchup_player_left"));
            self.state = AppState::InWorld {
                world_name: t!("unknown_world"),
            };
        }

        // 录制时间线
        if let AppState::Recording { session, .. } = &mut self.state {
            session.add_player_event(PlayerEventType::Left, &display_name, user_id.as_deref());
        }

        // 数据库记录
        if let Some(ref uid) = user_id {
            self.db.end_visit(uid, &timestamp);
        }
    }

    // -----------------------------------------------------------------------
    // 辅助方法
    // -----------------------------------------------------------------------

    /// 结束当前录音会话并将状态回退到 Idle。
    fn finish_current_session(&mut self, reason: &str) {
        let old_state = std::mem::replace(&mut self.state, AppState::Idle);

        if let AppState::Recording { session, .. } = old_state {
            match session.finish() {
                Ok(path) => info!("{}", t!("recording_finished", reason, path.display())),
                Err(e) => warn!("{}", t!("recording_save_failed", e)),
            }
        }
    }

    /// 获取当前的世界名称和实例 ID（用于数据库记录）。
    fn current_world_instance(&self) -> (String, String) {
        match &self.state {
            AppState::Idle => (t!("unknown_world"), t!("unknown_instance")),
            AppState::InWorld { world_name } => (world_name.clone(), t!("unknown_instance")),
            AppState::Recording {
                world_name,
                instance_id,
                ..
            } => (world_name.clone(), instance_id.clone()),
        }
    }
}
