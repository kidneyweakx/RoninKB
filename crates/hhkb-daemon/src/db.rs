//! SQLite schema init and profile CRUD.
//!
//! Profiles are stored as the full VIA JSON blob in a `profiles` row. The
//! `active_profile` table uses a singleton pattern (single row keyed on
//! `singleton = 1`) to remember which profile is currently active.

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use hhkb_core::ViaProfile;

/// Schema DDL — applied on startup, safe to re-run.
pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS profiles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    via_json TEXT NOT NULL,
    tags TEXT,
    created_at INTEGER,
    updated_at INTEGER
);

CREATE TABLE IF NOT EXISTS active_profile (
    singleton INTEGER PRIMARY KEY DEFAULT 1,
    profile_id TEXT REFERENCES profiles(id)
);
"#;

/// Row representation returned by the API (VIA JSON is a parsed `ViaProfile`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileRecord {
    pub id: String,
    pub name: String,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub via: ViaProfile,
}

/// Initialize the schema on a fresh or existing connection.
pub fn init_schema(conn: &Connection) -> ApiResult<()> {
    conn.execute_batch(SCHEMA_SQL)?;
    Ok(())
}

/// Open a fresh in-memory connection (for tests).
pub fn open_in_memory() -> ApiResult<Connection> {
    let conn = Connection::open_in_memory()?;
    init_schema(&conn)?;
    Ok(conn)
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

/// Insert a new profile. If the `ViaProfile`'s RoninKB extension carries an
/// id, that id is used; otherwise a fresh UUIDv4 is generated.
pub fn create_profile(conn: &Connection, mut via: ViaProfile) -> ApiResult<ProfileRecord> {
    let id = via
        .ronin
        .as_ref()
        .map(|r| r.profile.id.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Keep the in-JSON id in sync with the row id so round-tripping through
    // the API doesn't leave two sources of truth.
    if let Some(ronin) = via.ronin.as_mut() {
        ronin.profile.id = Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4());
    }

    let name = via
        .ronin
        .as_ref()
        .map(|r| r.profile.name.clone())
        .unwrap_or_else(|| via.name.clone());
    let tags = via
        .ronin
        .as_ref()
        .map(|r| r.profile.tags.join(","))
        .unwrap_or_default();
    let now = Utc::now().timestamp();
    let via_json = serde_json::to_string(&via)?;

    conn.execute(
        "INSERT INTO profiles (id, name, via_json, tags, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, name, via_json, tags, now, now],
    )?;

    Ok(ProfileRecord {
        id,
        name,
        tags: via
            .ronin
            .as_ref()
            .map(|r| r.profile.tags.clone())
            .unwrap_or_default(),
        created_at: now,
        updated_at: now,
        via,
    })
}

/// Fetch a profile by id.
pub fn get_profile(conn: &Connection, id: &str) -> ApiResult<ProfileRecord> {
    conn.query_row(
        "SELECT id, name, via_json, tags, created_at, updated_at
         FROM profiles WHERE id = ?1",
        params![id],
        row_to_record,
    )
    .optional()?
    .ok_or(ApiError::NotFound)
}

/// List all profiles in creation order (newest last).
pub fn list_profiles(conn: &Connection) -> ApiResult<Vec<ProfileRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, via_json, tags, created_at, updated_at
         FROM profiles ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map([], row_to_record)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Update a profile by id. The VIA JSON is replaced wholesale.
pub fn update_profile(
    conn: &Connection,
    id: &str,
    via: ViaProfile,
) -> ApiResult<ProfileRecord> {
    // Ensure the row exists first so we return 404 (not a silent no-op).
    let _existing = get_profile(conn, id)?;

    let name = via
        .ronin
        .as_ref()
        .map(|r| r.profile.name.clone())
        .unwrap_or_else(|| via.name.clone());
    let tags = via
        .ronin
        .as_ref()
        .map(|r| r.profile.tags.join(","))
        .unwrap_or_default();
    let now = Utc::now().timestamp();
    let via_json = serde_json::to_string(&via)?;

    conn.execute(
        "UPDATE profiles
         SET name = ?2, via_json = ?3, tags = ?4, updated_at = ?5
         WHERE id = ?1",
        params![id, name, via_json, tags, now],
    )?;

    get_profile(conn, id)
}

/// Delete a profile. Returns `NotFound` if no row matched.
pub fn delete_profile(conn: &Connection, id: &str) -> ApiResult<()> {
    // Best-effort: if this profile was the active one, clear the pointer.
    conn.execute(
        "UPDATE active_profile SET profile_id = NULL WHERE profile_id = ?1",
        params![id],
    )?;

    let n = conn.execute("DELETE FROM profiles WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

/// Return the id of the active profile, if any.
pub fn get_active(conn: &Connection) -> ApiResult<Option<String>> {
    let id: Option<String> = conn
        .query_row(
            "SELECT profile_id FROM active_profile WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    Ok(id)
}

/// Set (or upsert) the active profile id. Verifies the id exists first.
pub fn set_active(conn: &Connection, id: &str) -> ApiResult<()> {
    // Verify the profile exists so callers get a proper 404.
    let _ = get_profile(conn, id)?;

    conn.execute(
        "INSERT INTO active_profile (singleton, profile_id) VALUES (1, ?1)
         ON CONFLICT(singleton) DO UPDATE SET profile_id = excluded.profile_id",
        params![id],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProfileRecord> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let via_json: String = row.get(2)?;
    let tags_str: Option<String> = row.get(3)?;
    let created_at: i64 = row.get(4)?;
    let updated_at: i64 = row.get(5)?;

    let via: ViaProfile = serde_json::from_str(&via_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            Box::new(e),
        )
    })?;

    let tags = tags_str
        .map(|s| {
            s.split(',')
                .filter(|t| !t.is_empty())
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(ProfileRecord {
        id,
        name,
        tags,
        created_at,
        updated_at,
        via,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hhkb_core::via::{ProfileMeta, RoninExtension, ViaProfile};

    fn sample_profile(name: &str) -> ViaProfile {
        ViaProfile {
            name: name.to_string(),
            vendor_id: "0x04FE".to_string(),
            product_id: "0x0021".to_string(),
            matrix: None,
            layouts: None,
            layers: vec![vec!["KC_ESC".to_string()]],
            lighting: None,
            keycodes: vec![],
            ronin: Some(RoninExtension {
                version: "1.0".to_string(),
                profile: ProfileMeta {
                    id: Uuid::new_v4(),
                    name: name.to_string(),
                    icon: None,
                    tags: vec!["work".to_string()],
                },
                hardware: None,
                software: None,
            }),
        }
    }

    #[test]
    fn schema_initializes() {
        let conn = open_in_memory().unwrap();
        // Re-running must be idempotent.
        init_schema(&conn).unwrap();

        // profiles and active_profile tables both exist.
        let profiles_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM profiles", [], |r| r.get(0))
            .unwrap();
        assert_eq!(profiles_count, 0);

        let active_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM active_profile", [], |r| r.get(0))
            .unwrap();
        assert_eq!(active_count, 0);
    }

    #[test]
    fn crud_cycle() {
        let conn = open_in_memory().unwrap();

        // Create
        let rec = create_profile(&conn, sample_profile("Daily")).unwrap();
        assert_eq!(rec.name, "Daily");
        assert_eq!(rec.tags, vec!["work".to_string()]);
        let id = rec.id.clone();

        // List
        let list = list_profiles(&conn).unwrap();
        assert_eq!(list.len(), 1);

        // Get
        let got = get_profile(&conn, &id).unwrap();
        assert_eq!(got.id, id);
        assert_eq!(got.via.name, "Daily");

        // Update
        let mut updated = sample_profile("Updated");
        if let Some(r) = updated.ronin.as_mut() {
            r.profile.id = Uuid::parse_str(&id).unwrap();
        }
        let after = update_profile(&conn, &id, updated).unwrap();
        assert_eq!(after.name, "Updated");

        // Active profile tracking
        set_active(&conn, &id).unwrap();
        assert_eq!(get_active(&conn).unwrap(), Some(id.clone()));

        // Delete clears active pointer.
        delete_profile(&conn, &id).unwrap();
        assert!(list_profiles(&conn).unwrap().is_empty());
        assert_eq!(get_active(&conn).unwrap(), None);
    }

    #[test]
    fn get_missing_returns_not_found() {
        let conn = open_in_memory().unwrap();
        let err = get_profile(&conn, "nonexistent").unwrap_err();
        assert!(matches!(err, ApiError::NotFound));
    }

    #[test]
    fn set_active_missing_returns_not_found() {
        let conn = open_in_memory().unwrap();
        let err = set_active(&conn, "nope").unwrap_err();
        assert!(matches!(err, ApiError::NotFound));
    }

    #[test]
    fn delete_missing_returns_not_found() {
        let conn = open_in_memory().unwrap();
        let err = delete_profile(&conn, "nope").unwrap_err();
        assert!(matches!(err, ApiError::NotFound));
    }
}
