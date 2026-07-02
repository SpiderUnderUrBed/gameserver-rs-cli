use std::fs;
use std::path::Path;
use crate::databasespec::Database;

const DB_PATH: &str = "db.json";

pub struct DbConn {}

impl DbConn {
    pub async fn first_connection() -> Self {
        Self {}
    }
}

fn resolve_path(override_path: Option<&str>) -> &str {
    override_path.unwrap_or(DB_PATH)
}

pub fn ensure_db(override_path: Option<&str>) -> std::io::Result<()> {
    let path = resolve_path(override_path);
    if !Path::new(path).exists() {
        let db = Database::default();
        save_db(&db, override_path);
    }
    Ok(())
}

pub fn load_db(override_path: Option<&str>) -> Database {
    let path = resolve_path(override_path);
    if !Path::new(path).exists() {
        let db = Database::default();
        save_db(&db, override_path);
        return db;
    }
    let contents = fs::read_to_string(path).expect("Failed to read DB file");
    match serde_json::from_str(&contents) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to parse DB (will be resetting): {}", e);
            let db = Database::default();
            save_db(&db, override_path);
            db
        }
    }
}

pub fn save_db(db: &Database, override_path: Option<&str>) {
    let path = resolve_path(override_path);
    let contents = serde_json::to_string_pretty(db).expect("Failed to serialize");
    fs::write(path, contents).expect("Failed to write DB file");
}