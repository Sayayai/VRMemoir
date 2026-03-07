use anyhow::Result;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

#[derive(Debug, Serialize, Clone)]
#[allow(dead_code)]
pub struct User {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(rename = "trustLevel")]
    pub trust_level: Option<String>,
    #[serde(rename = "lastBio")]
    pub last_bio: Option<String>,
    pub pronouns: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ActivePlayer {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(rename = "trustLevel")]
    pub trust_level: Option<String>,
    #[serde(rename = "lastBio")]
    pub last_bio: Option<String>,
    pub pronouns: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    #[serde(rename = "worldName")]
    pub world_name: Option<String>,
    #[serde(rename = "joinedAt")]
    pub joined_at: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct UserWithPersonality {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(rename = "trustLevel")]
    pub trust_level: Option<String>,
    #[serde(rename = "lastBio")]
    pub last_bio: Option<String>,
    pub pronouns: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<String>,
    #[serde(rename = "mbtiType")]
    pub mbti_type: Option<String>,
    pub ni: Option<i32>,
    pub te: Option<i32>,
    pub fi: Option<i32>,
    pub se: Option<i32>,
    pub ne: Option<i32>,
    pub ti: Option<i32>,
    pub fe: Option<i32>,
    pub si: Option<i32>,
}

#[derive(Debug, serde::Deserialize)]
pub struct PersonalityData {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "mbtiType")]
    pub mbti_type: Option<String>,
    pub ni: Option<i32>,
    pub te: Option<i32>,
    pub fi: Option<i32>,
    pub se: Option<i32>,
    pub ne: Option<i32>,
    pub ti: Option<i32>,
    pub fe: Option<i32>,
    pub si: Option<i32>,
}

impl Database {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS users (
                userId TEXT PRIMARY KEY,
                displayName TEXT,
                trustLevel TEXT,
                lastBio TEXT,
                pronouns TEXT,
                updatedAt DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS user_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                userId TEXT,
                type TEXT,
                displayName TEXT,
                previousDisplayName TEXT,
                trustLevel TEXT,
                previousTrustLevel TEXT,
                FOREIGN KEY(userId) REFERENCES users(userId)
            );

            CREATE TABLE IF NOT EXISTS bio_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                userId TEXT,
                displayName TEXT,
                bio TEXT,
                previousBio TEXT,
                FOREIGN KEY(userId) REFERENCES users(userId)
            );

            CREATE TABLE IF NOT EXISTS visits (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                userId TEXT,
                worldName TEXT,
                instanceId TEXT,
                joinedAt DATETIME,
                leftAt DATETIME,
                FOREIGN KEY(userId) REFERENCES users(userId)
            );

            CREATE TABLE IF NOT EXISTS cookies (
                key TEXT PRIMARY KEY,
                value TEXT
            );

            CREATE TABLE IF NOT EXISTS personalities (
                userId TEXT PRIMARY KEY,
                mbtiType TEXT,
                ni INTEGER DEFAULT 0,
                te INTEGER DEFAULT 0,
                fi INTEGER DEFAULT 0,
                se INTEGER DEFAULT 0,
                ne INTEGER DEFAULT 0,
                ti INTEGER DEFAULT 0,
                fe INTEGER DEFAULT 0,
                si INTEGER DEFAULT 0,
                updatedAt DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(userId) REFERENCES users(userId)
            );

            UPDATE visits SET leftAt = datetime('now') WHERE leftAt IS NULL;
            ",
        )?;

        // Migration: add pronouns column if missing
        let _ = conn.execute("ALTER TABLE users ADD COLUMN pronouns TEXT", []);

        Ok(())
    }

    pub fn register_user(
        &self,
        user_id: &str,
        display_name: &str,
        bio: Option<&str>,
        pronouns: Option<&str>,
        trust_level: Option<&str>,
    ) {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "INSERT INTO users (userId, displayName, lastBio, pronouns, trustLevel, updatedAt)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(userId) DO UPDATE SET
                displayName = excluded.displayName,
                lastBio = COALESCE(excluded.lastBio, users.lastBio),
                pronouns = COALESCE(excluded.pronouns, users.pronouns),
                trustLevel = COALESCE(excluded.trustLevel, users.trustLevel),
                updatedAt = excluded.updatedAt",
            params![user_id, display_name, bio, pronouns, trust_level, now],
        );
    }

    pub fn start_visit(&self, user_id: &str, world_name: &str, instance_id: &str, joined_at: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "UPDATE visits SET leftAt = ?1 WHERE userId = ?2 AND leftAt IS NULL",
            params![joined_at, user_id],
        );
        let _ = conn.execute(
            "INSERT INTO visits (userId, worldName, instanceId, joinedAt) VALUES (?1, ?2, ?3, ?4)",
            params![user_id, world_name, instance_id, joined_at],
        );
    }

    pub fn end_visit(&self, user_id: &str, left_at: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "UPDATE visits SET leftAt = ?1 WHERE userId = ?2 AND leftAt IS NULL",
            params![left_at, user_id],
        );
    }

    pub fn get_active_players(&self) -> Vec<ActivePlayer> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT u.userId, u.displayName, u.trustLevel, u.lastBio, u.pronouns, u.updatedAt,
                        v.worldName, v.joinedAt
                 FROM users u
                 JOIN visits v ON u.userId = v.userId
                 WHERE v.leftAt IS NULL",
            )
            .unwrap();

        stmt.query_map([], |row| {
            Ok(ActivePlayer {
                user_id: row.get(0)?,
                display_name: row.get(1)?,
                trust_level: row.get(2)?,
                last_bio: row.get(3)?,
                pronouns: row.get(4)?,
                updated_at: row.get(5)?,
                world_name: row.get(6)?,
                joined_at: row.get(7)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn get_all_users_with_personality(&self) -> Vec<UserWithPersonality> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT u.userId, u.displayName, u.trustLevel, u.lastBio, u.pronouns, u.updatedAt,
                        p.mbtiType, p.ni, p.te, p.fi, p.se, p.ne, p.ti, p.fe, p.si
                 FROM users u
                 LEFT JOIN personalities p ON u.userId = p.userId",
            )
            .unwrap();

        stmt.query_map([], |row| {
            Ok(UserWithPersonality {
                user_id: row.get(0)?,
                display_name: row.get(1)?,
                trust_level: row.get(2)?,
                last_bio: row.get(3)?,
                pronouns: row.get(4)?,
                updated_at: row.get(5)?,
                mbti_type: row.get(6)?,
                ni: row.get(7)?,
                te: row.get(8)?,
                fi: row.get(9)?,
                se: row.get(10)?,
                ne: row.get(11)?,
                ti: row.get(12)?,
                fe: row.get(13)?,
                si: row.get(14)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn update_personality(&self, data: &PersonalityData) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT INTO personalities (userId, mbtiType, ni, te, fi, se, ne, ti, fe, si, updatedAt)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))
             ON CONFLICT(userId) DO UPDATE SET
                mbtiType = excluded.mbtiType,
                ni = excluded.ni,
                te = excluded.te,
                fi = excluded.fi,
                se = excluded.se,
                ne = excluded.ne,
                ti = excluded.ti,
                fe = excluded.fe,
                si = excluded.si,
                updatedAt = excluded.updatedAt",
            params![
                data.user_id,
                data.mbti_type,
                data.ni.unwrap_or(0),
                data.te.unwrap_or(0),
                data.fi.unwrap_or(0),
                data.se.unwrap_or(0),
                data.ne.unwrap_or(0),
                data.ti.unwrap_or(0),
                data.fe.unwrap_or(0),
                data.si.unwrap_or(0),
            ],
        );
    }

    pub fn save_cookie(&self, key: &str, value: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO cookies (key, value) VALUES (?1, ?2)",
            params![key, value],
        );
    }

    pub fn get_cookie(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT value FROM cookies WHERE key = ?1", params![key], |row| {
            row.get(0)
        })
        .ok()
    }

    pub fn clear_cookies(&self) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute("DELETE FROM cookies", []);
    }
}
