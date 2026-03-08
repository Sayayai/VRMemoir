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
        
        // Optional: Get previous values to populate history if desired
        // For now, focus on updating the main record
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

    pub fn get_user_bio(&self, user_id: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT lastBio FROM users WHERE userId = ?1",
            params![user_id],
            |row| row.get(0),
        )
        .ok()
        .flatten()
    }

    pub fn update_bio_history(&self, user_id: &str, display_name: &str, bio: &str) {
        let conn = self.conn.lock().unwrap();
        let previous_bio: Option<String> = conn
            .query_row(
                "SELECT lastBio FROM users WHERE userId = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .ok()
            .flatten();

        if previous_bio.as_deref() != Some(bio) {
            let _ = conn.execute(
                "INSERT INTO bio_history (userId, displayName, bio, previousBio)
                 VALUES (?1, ?2, ?3, ?4)",
                params![user_id, display_name, bio, previous_bio],
            );
        }
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

    pub fn get_active_players_without_bio(&self, since_timestamp: &str) -> Vec<(String, String)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT v.userId, u.displayName 
                 FROM visits v
                 JOIN users u ON v.userId = u.userId
                 WHERE v.leftAt IS NULL 
                 AND v.joinedAt >= ?1
                 AND (u.lastBio IS NULL OR u.lastBio = '')
                 AND (u.updatedAt IS NULL OR u.updatedAt < datetime('now', '-2 hours'))",
            )
            .unwrap();
        let rows = stmt
            .query_map(params![since_timestamp], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap();
        rows.filter_map(|r| r.ok()).collect()
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

    pub fn get_all_users(&self) -> Vec<User> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT userId, displayName, lastBio, pronouns, trustLevel, updatedAt FROM users")
            .unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok(User {
                    user_id: row.get(0)?,
                    display_name: row.get(1)?,
                    last_bio: row.get(2)?,
                    pronouns: row.get(3)?,
                    trust_level: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .unwrap();
        rows.filter_map(|r| r.ok()).collect()
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
