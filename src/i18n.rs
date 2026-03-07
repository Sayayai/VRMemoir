use once_cell::sync::Lazy;
use std::collections::HashMap;
use sys_locale::get_locale;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    En,
    Zh,
    Ja,
    Ko,
}

impl Language {
    pub fn from_locale(locale: &str) -> Self {
        let lang_code = locale.split('-').next().unwrap_or("en").to_lowercase();
        match lang_code.as_str() {
            "zh" => Language::Zh,
            "ja" => Language::Ja,
            "ko" => Language::Ko,
            _ => Language::En,
        }
    }
}

pub static CURRENT_LANG: Lazy<Language> = Lazy::new(|| {
    let locale = get_locale().unwrap_or_else(|| "en-US".to_string());
    Language::from_locale(&locale)
});

type TranslationMap = HashMap<&'static str, &'static str>;

pub static TRANSLATIONS: Lazy<HashMap<Language, TranslationMap>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // English
    let mut en = HashMap::new();
    en.insert("db_initialized", "Database initialized: {}");
    en.insert("authenticating", "Authenticating...");
     en.insert("auth_success", "Authentication success");
    en.insert("tfa_required", "[2FA] Two-factor authentication required (Method: {})");
    en.insert("tfa_prompt", "[2FA] Please enter the verification code (enter empty to skip): ");
    en.insert("tfa_skipped", "Verification code not entered, skipping 2FA.");
    en.insert("tfa_verifying", "[2FA] Verifying (type: {})...");
    en.insert("tfa_success", "2FA success! Authenticated.");
    en.insert("tfa_failed", "[2FA] Verification failed: {}");
    en.insert("tfa_retry", "[2FA] Please try again.");
    en.insert("auth_failed", "Authentication failed: {}");
    en.insert("mic_device_env", "Microphone device (Environment): \"{}\"");
    en.insert("mic_device_reg", "Microphone device (VRChat Registry)");
    en.insert("mic_device_default", "Using system default microphone device");
    en.insert("mic_recording_status", "Mic recording: {}");
    en.insert("mic_device_env", "Mic device (env var): \"{}\"");
    en.insert("mic_device_reg", "Mic device (VRChat registry)");
    en.insert("mic_device_default", "Using system default mic device");
    en.insert("enabled", "Enabled");
    en.insert("disabled", "Disabled");
    en.insert("api_server_started", "--- API server started: http://127.0.0.1:3001 ---");
    en.insert("server_error", "Server error: {}");
    en.insert("watcher_started", "VRChat monitor started, press Ctrl+C to stop");
    en.insert("shutdown_saving_recording", "Saving recording before exit...");
    en.insert("exited", "Exited");
    en.insert("audio_ready", "Audio ready: VRChat voice channel connected.");
    en.insert("vrchat_exited", "VRChat process exited, finishing recording...");
    en.insert("world", "World: {}");
    en.insert("instance", "Instance: {}");
    en.insert("unknown_world", "Unknown World");
    en.insert("unknown_instance", "Unknown Instance");
    en.insert("mic_start", "🎤 Recording started (Joining, PID {}): {} ({})");
    en.insert("mic_start_failed", "Failed to start recording: {}");
    en.insert("mic_capture_success", "Microphone capture initialized successfully.");
    en.insert("mic_capture_failed", "Microphone capture setup failed: {}. Recording VRChat audio only.");
    en.insert("mic_recording_disabled", "Microphone recording disabled.");
    en.insert("with_mic", " (with mic)");
    en.insert("vrchat_exited_stop", "VRChat process ended, stopping recording");
    en.insert("recording_stopped", "Recording stopped. Duration: {:.1}s");
    en.insert("unknown_device", "Unknown");
    en.insert("using_mic_device", "Using microphone device: \"{}\"");
    en.insert("found_mic_device", "Found microphone device: \"{}\"");
    en.insert("no_vrchat_process", "VRChat process not found, instance recording signal skipped.");
    en.insert("recording_output_dir", "Recording output directory: {}");
    en.insert("timeline_title", "# 🎤 Recording Record - {}");
    en.insert("world_name_label", "Room Name");
    en.insert("instance_id_label", "Instance ID");
    en.insert("recording_start_label", "Recording Start");
    en.insert("recording_status_active", "Recording Status: 🔴 Recording...");
    en.insert("player_timeline_title", "## 👥 Player Timeline");
    en.insert("table_header_time", "Time");
    en.insert("table_header_offset", "Offset");
    en.insert("table_header_event", "Event");
    en.insert("table_header_player", "Player Name");
    en.insert("table_header_uid", "User ID");
    en.insert("timeline_header_written", "Timeline header written: {}");
    en.insert("audio_recording_started", "PID {} audio recording started");
    en.insert("audio_recording_start_failed", "Audio recording start failed: {}. Continuing timeline only (no audio).");
    en.insert("timeline_write_failed", "Failed to write timeline event: {}");
    en.insert("event_joined", "➡️ Joined");
    en.insert("event_left", "⬅️ Left");
    en.insert("stop_recorder_error", "Error stopping recorder: {}");
    en.insert("rename_dir_failed", "Failed to rename recording directory (including duration)");
    en.insert("session_saved", "Session saved: {} ({:.0}s, {} events)");
    en.insert("recording_end_label", "Recording End");
    en.insert("recording_duration_label", "Recording Duration");
    en.insert("mins_label", "min");
    en.insert("secs_label", "sec");
    en.insert("audio_file_label", "Audio File");
    en.insert("no_audio_recorded", "❌ Failed to record");
    en.insert("no_player_events", "*(No player join/leave events in this session)*");
    en.insert("duration_format_h_m", "{}h{}m");
    en.insert("duration_format_m", "{}m");
    en.insert("duration_format_s", "{}s");
    en.insert("player_joined", "Joined: {}{}");
    en.insert("player_left", "Left: {}");
    en.insert("recording_finished", "Recording finished ({}): {}");
    en.insert("recording_save_failed", "Failed to save recording: {}");
    en.insert("use_proxy", "Using proxy: {}");
    en.insert("direct_connection", "No proxy configured, connecting directly.");
    en.insert("cookie_loaded", "Loaded cookies from database.");
    en.insert("auto_login_rate_limited", "Auto-login rate limit reached (max {} per hour). Skipped.");
    en.insert("status_401", "Endpoint {} returned 401. Attempting auto-login...");
    en.insert("auto_login_success", "Auto-login success, retrying...");
    en.insert("auth_step_1", "[Auth] Step 1: Checking API availability (GET /config)...");
    en.insert("api_available", "[Auth] API available.");
    en.insert("config_check_failed", "[Auth] API config check failed: {}");
    en.insert("auth_step_2", "[Auth] Step 2: Checking existing session...");
    en.insert("current_user", "[Auth] Existing session valid. Current user: {}");
    en.insert("session_require_2fa", "[Auth] Session requires 2FA.");
    en.insert("no_session", "[Auth] No valid session. Attempting login...");
    en.insert("auth_step_3", "[Auth] Step 3: Logging in with credentials...");
    en.insert("login_success", "[Auth] Login success. Current user: {}");
    en.insert("login_require_2fa", "[Auth] Login requires 2FA.");
    en.insert("login_failed", "[Auth] Login failed: {}");
    en.insert("no_credentials", "[Auth] No credentials configured. Please login via API.");
    en.insert("logout_success", "Logged out and cleared cookies.");
    en.insert("new_log_file", "New log file detected: {}");
    en.insert("watching_directory", "Log watcher started, monitoring directory: {}");
    m.insert(Language::En, en);

    // Chinese
    let mut zh = HashMap::new();
    zh.insert("db_initialized", "数据库已初始化: {}");
    zh.insert("authenticating", "正在认证...");
    zh.insert("auth_success", "认证成功");
    zh.insert("tfa_required", "[2FA] 需要二步验证 (方式: {})");
    zh.insert("tfa_prompt", "[2FA] 请输入验证码 (输入空行跳过): ");
    zh.insert("tfa_skipped", "未输入验证码，跳过二步验证。");
    zh.insert("tfa_verifying", "[2FA] 验证中 (type: {})...");
    zh.insert("tfa_success", "二步验证成功！已认证。");
    zh.insert("tfa_failed", "[2FA] 验证失败: {}");
    zh.insert("tfa_retry", "[2FA] 请重试。");
    zh.insert("auth_failed", "认证失败: {}");
    zh.insert("mic_device_env", "麦克风设备 (环境变量): \"{}\"");
    zh.insert("mic_device_reg", "麦克风设备 (VRChat 注册表)");
    zh.insert("mic_device_default", "使用系统默认麦克风设备");
    zh.insert("mic_recording_status", "麦克风录制: {}");
    zh.insert("mic_device_env", "麦克风设备 (环境变量): \"{}\"");
    zh.insert("mic_device_reg", "麦克风设备 (VRChat 注册表)");
    zh.insert("mic_device_default", "使用系统默认麦克风设备");
    zh.insert("enabled", "已开启");
    zh.insert("disabled", "已关闭");
    zh.insert("api_server_started", "--- API 服务已启动: http://127.0.0.1:3001 ---");
    zh.insert("server_error", "服务器错误: {}");
    zh.insert("watcher_started", "VRChat 监控已启动，按 Ctrl+C 停止");
    zh.insert("shutdown_saving_recording", "退出前保存录音...");
    zh.insert("exited", "已退出");
    zh.insert("audio_ready", "音频就绪: VRChat 语音频道已连上。");
    zh.insert("vrchat_exited", "检测到 VRChat 进程已退出，自动结束当前录音...");
    zh.insert("world", "世界: {}");
    zh.insert("instance", "实例: {}");
    zh.insert("unknown_world", "未知世界");
    zh.insert("unknown_instance", "未知实例");
    zh.insert("mic_start", "🎤 开始录音 (Joining, PID {}): {} ({})");
    zh.insert("mic_start_failed", "录音启动失败: {}");
    zh.insert("mic_capture_success", "麦克风捕获初始化成功。");
    zh.insert("mic_capture_failed", "麦克风捕获设置失败: {}。仅录制 VRChat 音频。");
    zh.insert("mic_recording_disabled", "麦克风录制已禁用。");
    zh.insert("with_mic", " (含麦克风)");
    zh.insert("vrchat_exited_stop", "VRChat 进程已结束，停止录音");
    zh.insert("recording_stopped", "录音已停止。时长: {:.1}s");
    zh.insert("unknown_device", "未知");
    zh.insert("using_mic_device", "使用麦克风设备: \"{}\"");
    zh.insert("found_mic_device", "找到麦克风设备: \"{}\"");
    zh.insert("no_vrchat_process", "未找到 VRChat 进程，本次实例录音未启动信号。");
    zh.insert("recording_output_dir", "录音输出目录: {}");
    zh.insert("timeline_title", "# 🎤 录音记录 - {}");
    zh.insert("world_name_label", "房间名称");
    zh.insert("instance_id_label", "实例 ID");
    zh.insert("recording_start_label", "录制开始");
    zh.insert("recording_status_active", "录制状态: 🔴 录制中...");
    zh.insert("player_timeline_title", "## 👥 人员时间线");
    zh.insert("table_header_time", "时间");
    zh.insert("table_header_offset", "偏移");
    zh.insert("table_header_event", "事件");
    zh.insert("table_header_player", "玩家名称");
    zh.insert("table_header_uid", "User ID");
    zh.insert("timeline_header_written", "时间轴页眉已写入: {}");
    zh.insert("audio_recording_started", "PID {} 音频录制已启动");
    zh.insert("audio_recording_start_failed", "音频录制启动失败: {}。将继续录制时间轴（无音频）。");
    zh.insert("timeline_write_failed", "写入时间轴事件失败: {}");
    zh.insert("event_joined", "➡️ 加入");
    zh.insert("event_left", "⬅️ 离开");
    zh.insert("stop_recorder_error", "停止录制器时出错: {}");
    zh.insert("rename_dir_failed", "重命名录音目录（包含时长）失败");
    zh.insert("session_saved", "会话已保存: {} ({:.0}s, {} events)");
    zh.insert("recording_end_label", "录制结束");
    zh.insert("recording_duration_label", "录制时长");
    zh.insert("mins_label", "分");
    zh.insert("secs_label", "秒");
    zh.insert("audio_file_label", "音频文件");
    zh.insert("no_audio_recorded", "❌ 未能录制");
    zh.insert("no_player_events", "*（本次会话无玩家加入/离开事件）*");
    zh.insert("duration_format_h_m", "{}h{}分");
    zh.insert("duration_format_m", "{}分");
    zh.insert("duration_format_s", "{}秒");
    zh.insert("player_joined", "加入: {}{}");
    zh.insert("player_left", "离开: {}");
    zh.insert("recording_finished", "录音已结束 ({}): {}");
    zh.insert("recording_save_failed", "录音保存失败: {}");
    zh.insert("use_proxy", "使用代理: {}");
    zh.insert("direct_connection", "未配置代理，正在直接连接。");
    zh.insert("cookie_loaded", "已从数据库加载 Cookie。");
    zh.insert("auto_login_rate_limited", "自动登录已达到频率限制（一小时内最多 {} 次）。已跳过。");
    zh.insert("status_401", "接口 {} 返回 401。正在尝试自动登录...");
    zh.insert("auto_login_success", "自动登录成功，正在重试...");
    zh.insert("auth_step_1", "[认证] 步骤 1: 检查 API 可用性 (GET /config)...");
    zh.insert("api_available", "[认证] API 可用。");
    zh.insert("config_check_failed", "[认证] API 配置检查失败: {}");
    zh.insert("auth_step_2", "[认证] 步骤 2: 检查现有会话...");
    zh.insert("current_user", "[认证] 现有会话有效。当前用户: {}");
    zh.insert("session_require_2fa", "[认证] 会话需要二步验证。");
    zh.insert("no_session", "[认证] 无有效会话。正在尝试登录...");
    zh.insert("auth_step_3", "[认证] 步骤 3: 使用账号密码登录...");
    zh.insert("login_success", "[认证] 登录成功。当前用户: {}");
    zh.insert("login_require_2fa", "[认证] 登录需要二步验证。");
    zh.insert("login_failed", "[认证] 登录失败: {}");
    zh.insert("no_credentials", "[认证] 未配置账号密码。请通过 API 登录。");
    zh.insert("logout_success", "已登出并清除 Cookie。");
    zh.insert("new_log_file", "检测到新日志文件: {}");
    zh.insert("watching_directory", "日志监控已启动，正在监控目录: {}");
    m.insert(Language::Zh, zh);

    // Japanese
    let mut ja = HashMap::new();
    ja.insert("db_initialized", "データベースを初期化しました: {}");
    ja.insert("authenticating", "認証中...");
    ja.insert("auth_success", "認証成功");
    ja.insert("tfa_required", "[2FA] 二要素認証が必要です (方式: {})");
    ja.insert("tfa_prompt", "[2FA] 認証コードを入力してください（スキップする場合は空のまま入力）: ");
    ja.insert("tfa_skipped", "認証コードが入力されなかったため、2FAをスキップしました。");
    ja.insert("tfa_verifying", "[2FA] 検証中 (タイプ: {})...");
    ja.insert("tfa_success", "2FA認証に成功しました！");
    ja.insert("tfa_failed", "[2FA] 検証失敗: {}");
    ja.insert("tfa_retry", "[2FA] もう一度お試しください。");
    ja.insert("auth_failed", "認証失敗: {}");
    ja.insert("mic_device_env", "マイクデバイス (環境変数): \"{}\"");
    ja.insert("mic_device_reg", "マイクデバイス (VRChatレジストリ)");
    ja.insert("mic_device_default", "システム規定のマイクデバイスを使用します");
    ja.insert("mic_recording_status", "マイク録音: {}");
    ja.insert("mic_device_env", "マイクデバイス (環境変数): \"{}\"");
    ja.insert("mic_device_reg", "マイクデバイス (VRChatレジストリ)");
    ja.insert("mic_device_default", "システムデフォルトのマイクデバイスを使用");
    ja.insert("enabled", "有効");
    ja.insert("disabled", "無効");
    ja.insert("api_server_started", "--- APIサーバーが起動しました: http://127.0.0.1:3001 ---");
    ja.insert("server_error", "サーバーエラー: {}");
    ja.insert("watcher_started", "VRChat監視が開始されました。Ctrl+Cで停止します");
    ja.insert("shutdown_saving_recording", "終了前に録音を保存しています...");
    ja.insert("exited", "終了しました");
    ja.insert("audio_ready", "オーディオ準備完了: VRChatボイスチャンネルに接続されました。");
    ja.insert("vrchat_exited", "VRChatプロセスが終了しました。録音を終了しています...");
    ja.insert("world", "ワールド: {}");
    ja.insert("instance", "インスタンス: {}");
    ja.insert("unknown_world", "未知のワールド");
    ja.insert("unknown_instance", "未知のインスタンス");
    ja.insert("mic_start", "🎤 録音開始 (Join, PID {}): {} ({})");
    ja.insert("mic_start_failed", "録音の開始に失敗しました: {}");
    ja.insert("mic_capture_success", "マイクキャプチャが正常に初期化されました。");
    ja.insert("mic_capture_failed", "マイクキャプチャの設定に失敗しました: {}。VRChatの音声のみを録音します。");
    ja.insert("mic_recording_disabled", "マイク録音が無効になっています。");
    ja.insert("with_mic", " (マイク込み)");
    ja.insert("vrchat_exited_stop", "VRChatプロセスが終了しました。録音を停止します");
    ja.insert("recording_stopped", "録音停止。長さ: {:.1}s");
    ja.insert("unknown_device", "不明");
    ja.insert("using_mic_device", "マイクデバイスを使用中: \"{}\"");
    ja.insert("found_mic_device", "マイクデバイスが見つかりました: \"{}\"");
    ja.insert("no_vrchat_process", "VRChatプロセスが見つからないため、インスタンス録音信号をスキップしました。");
    ja.insert("recording_output_dir", "録音出力ディレクトリ: {}");
    ja.insert("timeline_title", "# 🎤 録音記録 - {}");
    ja.insert("world_name_label", "ワールド名");
    ja.insert("instance_id_label", "インスタンス ID");
    ja.insert("recording_start_label", "録音開始");
    ja.insert("recording_status_active", "録音ステータス: 🔴 録音中...");
    ja.insert("player_timeline_title", "## 👥 プレイヤータイムライン");
    ja.insert("table_header_time", "時間");
    ja.insert("table_header_offset", "オフセット");
    ja.insert("table_header_event", "イベント");
    ja.insert("table_header_player", "プレイヤー名");
    ja.insert("table_header_uid", "ユーザー ID");
    ja.insert("timeline_header_written", "タイムラインヘッダーが書き込まれました: {}");
    ja.insert("audio_recording_started", "PID {} オーディオ録音が開始されました");
    ja.insert("audio_recording_start_failed", "オーディオ録音の開始に失敗しました: {}。タイムラインのみを続行します（オーディオなし）。");
    ja.insert("timeline_write_failed", "タイムラインイベントの書き込みに失敗しました: {}");
    ja.insert("event_joined", "➡️ 入室");
    ja.insert("event_left", "⬅️ 退室");
    ja.insert("stop_recorder_error", "レコーダー停止中にエラーが発生しました: {}");
    ja.insert("rename_dir_failed", "録音ディレクトリ（期間を含む）のリネームに失敗しました");
    ja.insert("session_saved", "セッションが保存されました: {} ({:.0}s, {} イベント)");
    ja.insert("recording_end_label", "録音終了");
    ja.insert("recording_duration_label", "録音時間");
    ja.insert("mins_label", "分");
    ja.insert("secs_label", "秒");
    ja.insert("audio_file_label", "オーディオファイル");
    ja.insert("no_audio_recorded", "❌ 録音に失敗しました");
    ja.insert("no_player_events", "*（このセッションではプレイヤーの入退室イベントはありませんでした）*");
    ja.insert("duration_format_h_m", "{}時間{}分");
    ja.insert("duration_format_m", "{}分");
    ja.insert("duration_format_s", "{}秒");
    ja.insert("player_joined", "入室: {}{}");
    ja.insert("player_left", "退室: {}");
    ja.insert("recording_finished", "録音終了 ({}): {}");
    ja.insert("recording_save_failed", "録音の保存に失敗しました: {}");
    ja.insert("use_proxy", "プロキシを使用: {}");
    ja.insert("direct_connection", "プロキシが設定されていないため、直接接続します。");
    ja.insert("cookie_loaded", "データベースからCookieを読み込みました。");
    ja.insert("auto_login_rate_limited", "自動ログインの制限に達しました（1時間に最大 {} 回）。スキップしました。");
    ja.insert("status_401", "エンドポイント {} が 401 を返しました。自動ログインを試行します...");
    ja.insert("auto_login_success", "自動ログイン成功、再試行中...");
    ja.insert("auth_step_1", "[認証] ステップ 1: API の可用性を確認中 (GET /config)...");
    ja.insert("api_available", "[認証] API は利用可能です。");
    ja.insert("config_check_failed", "[認証] API 設定の確認に失敗しました: {}");
    ja.insert("auth_step_2", "[認証] ステップ 2: 既存のセッションを確認中...");
    ja.insert("current_user", "[認証] 既存のセッションは有効です。現在のユーザー: {}");
    ja.insert("session_require_2fa", "[認証] セッションに二要素認証が必要です。");
    ja.insert("no_session", "[認証] 有効なセッションがありません。ログインを試行します...");
    ja.insert("auth_step_3", "[認証] ステップ 3: 認証情報でログイン中...");
    ja.insert("login_success", "[認証] ログイン成功。現在のユーザー: {}");
    ja.insert("login_require_2fa", "[認証] ログインに二要素認証が必要です。");
    ja.insert("login_failed", "[認証] ログイン失敗: {}");
    ja.insert("no_credentials", "[認証] 認証情報が設定されていません。API経由でログインしてください。");
    ja.insert("logout_success", "ログアウトし、Cookieをクリアしました。");
    ja.insert("new_log_file", "新しいログファイルを検出しました: {}");
    ja.insert("watching_directory", "ログ監視を開始しました。監視ディレクトリ: {}");
    m.insert(Language::Ja, ja);

    // Korean
    let mut ko = HashMap::new();
    ko.insert("db_initialized", "데이터베이스 초기화됨: {}");
    ko.insert("authenticating", "인증 중...");
    ko.insert("auth_success", "인증 성공");
    ko.insert("tfa_required", "[2FA] 2단계 인증이 필요합니다 (방식: {})");
    ko.insert("tfa_prompt", "[2FA] 인증 코드를 입력하세요 (건너뛰려면 빈칸 입력): ");
    ko.insert("tfa_skipped", "인증 코드가 입력되지 않았습니다. 2단계 인증을 건너뜁니다.");
    ko.insert("tfa_verifying", "[2FA] 확인 중 (유형: {})...");
    ko.insert("tfa_success", "2단계 인증 성공! 인증되었습니다.");
    ko.insert("tfa_failed", "[2FA] 확인 실패: {}");
    ko.insert("tfa_retry", "[2FA] 다시 시도해 주세요.");
    ko.insert("auth_failed", "인증 실패: {}");
    ko.insert("mic_device_env", "마이크 장치 (환경 변수): \"{}\"");
    ko.insert("mic_device_reg", "마이크 장치 (VRChat 레지스트리)");
    ko.insert("mic_device_default", "시스템 기본 마이크 장치를 사용합니다");
    ko.insert("mic_recording_status", "마이크 녹음: {}");
    ko.insert("enabled", "활성화됨");
    ko.insert("disabled", "비활성화됨");
    ko.insert("api_server_started", "--- API 서버 시작됨: http://127.0.0.1:3001 ---");
    ko.insert("server_error", "서버 오류: {}");
    ko.insert("watcher_started", "VRChat 모니터링이 시작되었습니다. Ctrl+C를 눌러 중지하세요");
    ko.insert("shutdown_saving_recording", "종료 전 녹음 저장 중...");
    ko.insert("exited", "종료됨");
    ko.insert("audio_ready", "오디오 준비됨: VRChat 음성 채널에 연결되었습니다.");
    ko.insert("vrchat_exited", "VRChat 프로세스가 종료되었습니다. 녹음을 종료합니다...");
    ko.insert("world", "월드: {}");
    ko.insert("instance", "인스턴스: {}");
    ko.insert("unknown_world", "알 수 없는 월드");
    ko.insert("unknown_instance", "알 수 없는 인스턴스");
    ko.insert("mic_start", "🎤 녹음 시작 (Join, PID {}): {} ({})");
    ko.insert("mic_start_failed", "녹음 시작 실패: {}");
    ko.insert("mic_capture_success", "마이크 캡처가 성공적으로 초기화되었습니다.");
    ko.insert("mic_capture_failed", "마이크 캡처 설정 실패: {}。VRChat 오디오만 녹음합니다.");
    ko.insert("mic_recording_disabled", "마이크 녹음이 비활성화되었습니다.");
    ko.insert("with_mic", " (마이크 포함)");
    ko.insert("vrchat_exited_stop", "VRChat 프로세스가 종료되어 녹음을 중지합니다");
    ko.insert("recording_stopped", "녹음 중지됨. 길이: {:.1}s");
    ko.insert("unknown_device", "알 수 없음");
    ko.insert("using_mic_device", "마이크 장치 사용 중: \"{}\"");
    ko.insert("found_mic_device", "마이크 장치를 찾았습니다: \"{}\"");
    ko.insert("no_vrchat_process", "VRChat 프로세스를 찾을 수 없습니다. 인스턴스 녹음 신호를 건너뜁니다.");
    ko.insert("recording_output_dir", "녹음 출력 디렉터리: {}");
    ko.insert("timeline_title", "# 🎤 녹음 기록 - {}");
    ko.insert("world_name_label", "월드 이름");
    ko.insert("instance_id_label", "인스턴스 ID");
    ko.insert("recording_start_label", "녹음 시작");
    ko.insert("recording_status_active", "녹음 상태: 🔴 녹음 중...");
    ko.insert("player_timeline_title", "## 👥 플레이어 타임라인");
    ko.insert("table_header_time", "시간");
    ko.insert("table_header_offset", "오프셋");
    ko.insert("table_header_event", "이벤트");
    ko.insert("table_header_player", "플레이어 이름");
    ko.insert("table_header_uid", "사용자 ID");
    ko.insert("timeline_header_written", "타임라인 헤더가 기록되었습니다: {}");
    ko.insert("audio_recording_started", "PID {} 오디오 녹음이 시작되었습니다");
    ko.insert("audio_recording_start_failed", "오디오 녹음 시작 실패: {}。타임라인만 계속합니다(오디오 없음).");
    ko.insert("timeline_write_failed", "타임라인 이벤트 기록 실패: {}");
    ko.insert("event_joined", "➡️ 입장");
    ko.insert("event_left", "⬅️ 퇴장");
    ko.insert("stop_recorder_error", "레코더 중지 중 오류 발생: {}");
    ko.insert("rename_dir_failed", "녹음 디렉터리(기간 포함) 이름 변경 실패");
    ko.insert("session_saved", "세션 저장됨: {} ({:.0}s, {} 이벤트)");
    ko.insert("recording_end_label", "녹음 종료");
    ko.insert("recording_duration_label", "녹음 시간");
    ko.insert("mins_label", "분");
    ko.insert("secs_label", "초");
    ko.insert("audio_file_label", "오디오 파일");
    ko.insert("no_audio_recorded", "❌ 녹음 실패");
    ko.insert("no_player_events", "*(이 세션에는 플레이어 입장/퇴장 이벤트가 없습니다)*");
    ko.insert("duration_format_h_m", "{}시간{}분");
    ko.insert("duration_format_m", "{}분");
    ko.insert("duration_format_s", "{}초");
    ko.insert("player_joined", "입장: {}{}");
    ko.insert("player_left", "퇴장: {}");
    ko.insert("recording_finished", "녹음 종료 ({}): {}");
    ko.insert("recording_save_failed", "녹음 저장 실패: {}");
    ko.insert("use_proxy", "프록시 사용: {}");
    ko.insert("direct_connection", "프록시가 설정되지 않았습니다. 직접 연결합니다.");
    ko.insert("cookie_loaded", "데이터베이스에서 쿠키를 로드했습니다.");
    ko.insert("auto_login_rate_limited", "자동 로그인 빈도 제한에 도달했습니다(1시간 내 최대 {}회). 건너뜁니다.");
    ko.insert("status_401", "엔드포인트 {}에서 401을 반환했습니다. 자동 로그인을 시도합니다...");
    ko.insert("auto_login_success", "자동 로그인 성공, 다시 시도 중...");
    ko.insert("auth_step_1", "[인증] 1단계: API 가용성 확인 중 (GET /config)...");
    ko.insert("api_available", "[인증] API 사용 가능.");
    ko.insert("config_check_failed", "[인증] API 설정 확인 실패: {}");
    ko.insert("auth_step_2", "[인증] 2단계: 기존 세션 확인 중...");
    ko.insert("current_user", "[인증] 기존 세션이 유효합니다. 현재 사용자: {}");
    ko.insert("session_require_2fa", "[인증] 세션에 2단계 인증이 필요합니다.");
    ko.insert("no_session", "[인증] 유효한 세션이 없습니다. 로그인을 시도합니다...");
    ko.insert("auth_step_3", "[인증] 3단계: 계정 정보로 로그인 중...");
    ko.insert("login_success", "[인증] 로그인 성공. 현재 사용자: {}");
    ko.insert("login_require_2fa", "[인증] 로그인에 2단계 인증이 필요합니다.");
    ko.insert("login_failed", "[인증] 로그인 실패: {}");
    ko.insert("no_credentials", "[인증] 계정 정보가 설정되지 않았습니다. API를 통해 로그인해 주세요.");
    ko.insert("logout_success", "로그아웃하고 쿠키를 삭제했습니다.");
    ko.insert("new_log_file", "새 로그 파일 감지됨: {}");
    ko.insert("watching_directory", "로그 감시가 시작되었습니다. 모니터링 디렉터리: {}");
    m.insert(Language::Ko, ko);

    m
});

pub fn get_translation(key: &str) -> String {
    let lang = *CURRENT_LANG;
    if let Some(map) = TRANSLATIONS.get(&lang) {
        if let Some(t) = map.get(key) {
            return t.to_string();
        }
    }
    // Fallback to English
    if let Some(map) = TRANSLATIONS.get(&Language::En) {
        if let Some(t) = map.get(key) {
            return t.to_string();
        }
    }
    key.to_string()
}

/// Runtime format: replace `{}`, `{:.Nf}`, `{:?}` etc. placeholders sequentially with args.
pub fn format_translation(template: &str, args: &[String]) -> String {
    let mut result = String::with_capacity(template.len() + args.iter().map(|a| a.len()).sum::<usize>());
    let mut arg_idx = 0;
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            // Scan ahead for closing '}'
            let mut placeholder = String::new();
            let mut found_close = false;
            for inner in chars.by_ref() {
                if inner == '}' {
                    found_close = true;
                    break;
                }
                placeholder.push(inner);
            }
            if found_close && arg_idx < args.len() {
                result.push_str(&args[arg_idx]);
                arg_idx += 1;
            } else if found_close {
                // No more args, output placeholder as-is
                result.push('{');
                result.push_str(&placeholder);
                result.push('}');
            } else {
                result.push('{');
                result.push_str(&placeholder);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::get_translation($key)
    };
    ($key:expr, $($arg:expr),+ $(,)?) => {
        {
            let template = $crate::i18n::get_translation($key);
            let args: Vec<String> = vec![$(format!("{}", $arg)),+];
            $crate::i18n::format_translation(&template, &args)
        }
    };
}
