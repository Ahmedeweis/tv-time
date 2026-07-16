// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tauri_plugin_shell::ShellExt;
use tokio::sync::Mutex; // For opening auth URL in browser
                        // Define structs for our DB
#[derive(Debug, Serialize, Deserialize, Clone)]
struct MediaCache {
    media_id: i64,
    title: String,
    poster_path: Option<String>,
    media_type: String,
    release_date: Option<String>,
    synopsis_cached_json_data: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
struct UserWatchlistItem {
    media_id: i64,
    status: String,
    last_watched_date: Option<String>,
    notes: Option<String>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
struct WatchlistDashboard {
    released_watchlist: Vec<(MediaCache, UserWatchlistItem)>,
    upcoming_watchlist: Vec<(MediaCache, UserWatchlistItem)>,
}
struct AppState {
    db: Arc<Mutex<Connection>>,
}
const TMDB_API_KEY: &str = "a67a5dfca36019defcb75694aee8d98b"; // Replace this with your actual TMDB API key
const TMDB_BASE_URL: &str = "https://api.themoviedb.org/3";
// Initialize database tables
fn init_db(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS media_cache (
            media_id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            poster_path TEXT,
            media_type TEXT NOT NULL,
            release_date TEXT,
            synopsis_cached_json_data TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_watchlist (
            media_id INTEGER PRIMARY KEY,
            status TEXT NOT NULL,
            last_watched_date TEXT,
            notes TEXT,
            FOREIGN KEY (media_id) REFERENCES media_cache(media_id)
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tmdb_auth (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            session_id TEXT,
            account_id INTEGER
        )",
        [],
    )?;
    Ok(())
}
// Helper to get app data dir (use OS-standard app data directory)
fn get_app_data_dir() -> std::path::PathBuf {
    // Use OS's standard app data directory (e.g., %APPDATA% on Windows, ~/.config on Linux, ~/Library/Application Support on macOS)
    let data_dir = dirs::data_dir()
        .expect("Failed to get app data directory")
        .join("tv-time-clone");
    std::fs::create_dir_all(&data_dir).expect("Failed to create app data directory");
    data_dir
}
// Tauri command: Search TMDB API
#[tauri::command]
async fn search_tmdb_api(query: String) -> Result<serde_json::Value, String> {
    println!("الـ Rust استقبل كلمة البحث بنجاح: {}", query);
    let client = reqwest::Client::new();
    let url = format!(
        "{}/search/multi?api_key={}&query={}",
        TMDB_BASE_URL, TMDB_API_KEY, query
    );
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(json)
}
// Helper function to fetch media details from TMDB
async fn fetch_media_details_from_tmdb(
    media_id: i64,
    media_type: &str,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let endpoint = match media_type {
        "movie" => "movie",
        "tv" => "tv",
        _ => return Err("Invalid media type".to_string()),
    };
    let url = format!(
        "{}/{}/{}?api_key={}",
        TMDB_BASE_URL, endpoint, media_id, TMDB_API_KEY
    );
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(json)
}
// Tauri command: Get upcoming movies from TMDB
#[tauri::command]
async fn get_upcoming_movies(
    page: u32,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/movie/upcoming?api_key={}&page={}",
        TMDB_BASE_URL, TMDB_API_KEY, page
    );
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    // Cache basic info to DB (don't hold lock across await)
    if let Some(results) = json.get("results").and_then(|r| r.as_array()) {
        let conn = state.db.lock().await;
        for movie in results {
            if let Some(id) = movie.get("id").and_then(|i| i.as_i64()) {
                let title = movie
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                let poster_path = movie
                    .get("poster_path")
                    .and_then(|p| p.as_str())
                    .map(|s| s.to_string());
                let release_date = movie
                    .get("release_date")
                    .and_then(|r| r.as_str())
                    .map(|s| s.to_string());
                let synopsis_json = serde_json::to_string(movie).unwrap();
                // Upsert media_cache
                conn.execute(
                    "INSERT OR REPLACE INTO media_cache (media_id, title, poster_path, media_type, release_date, synopsis_cached_json_data)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![id, title, poster_path, "movie", release_date, synopsis_json],
                ).ok();
            }
        }
    }
    Ok(json)
}
// Tauri command: Add media to watchlist
#[tauri::command]
async fn add_media_to_watchlist(
    media_id: i64,
    media_type: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Check if media is already cached
    let cached = {
        let conn = state.db.lock().await;
        conn.query_row(
            "SELECT 1 FROM media_cache WHERE media_id = ?1",
            params![media_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
    };
    let media_json = if cached.is_none() {
        // Fetch media details from TMDB (without holding DB lock!)
        fetch_media_details_from_tmdb(media_id, &media_type).await?
    } else {
        serde_json::Value::Null // Not needed, already cached
    };
    // Now get lock and update DB
    let mut conn = state.db.lock().await;
    if cached.is_none() {
        // Cache the newly fetched media
        let title = match media_type.as_str() {
            "movie" => media_json.get("title").and_then(|t| t.as_str()),
            "tv" => media_json.get("name").and_then(|t| t.as_str()),
            _ => None,
        }
        .unwrap_or("")
        .to_string();
        let poster_path = media_json
            .get("poster_path")
            .and_then(|p| p.as_str())
            .map(|s| s.to_string());
        let release_date = match media_type.as_str() {
            "movie" => media_json.get("release_date"),
            "tv" => media_json.get("first_air_date"),
            _ => None,
        }
        .and_then(|r| r.as_str())
        .map(|s| s.to_string());
        let synopsis_json = serde_json::to_string(&media_json).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO media_cache (media_id, title, poster_path, media_type, release_date, synopsis_cached_json_data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![media_id, title, poster_path, media_type, release_date, synopsis_json],
        ).map_err(|e| e.to_string())?;
    }
    // Add to watchlist
    conn.execute(
        "INSERT OR REPLACE INTO user_watchlist (media_id, status, last_watched_date, notes)
         VALUES (?1, ?2, ?3, ?4)",
        params![media_id, "to_watch", None::<String>, None::<String>],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
// Tauri command: Mark as watched
#[tauri::command]
async fn mark_as_watched(media_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().await;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE user_watchlist SET status = ?1, last_watched_date = ?2 WHERE media_id = ?3",
        params!["watched", now, media_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
// Tauri command: Get watchlist dashboard
#[tauri::command]
async fn get_watchlist_dashboard(state: State<'_, AppState>) -> Result<WatchlistDashboard, String> {
    let conn = state.db.lock().await;
    let mut stmt = conn
    .prepare(
      "SELECT mc.media_id, mc.title, mc.poster_path, mc.media_type, mc.release_date, mc.synopsis_cached_json_data,
              uw.status, uw.last_watched_date, uw.notes
       FROM media_cache mc
       JOIN user_watchlist uw ON mc.media_id = uw.media_id",
    )
    .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let media = MediaCache {
                media_id: row.get(0)?,
                title: row.get(1)?,
                poster_path: row.get(2)?,
                media_type: row.get(3)?,
                release_date: row.get(4)?,
                synopsis_cached_json_data: row.get(5)?,
            };
            let watchlist = UserWatchlistItem {
                media_id: row.get(0)?,
                status: row.get(6)?,
                last_watched_date: row.get(7)?,
                notes: row.get(8)?,
            };
            Ok((media, watchlist))
        })
        .map_err(|e| e.to_string())?;
    let mut all_items = Vec::new();
    for row in rows {
        all_items.push(row.map_err(|e| e.to_string())?);
    }
    let today = chrono::Utc::now().date_naive();
    let mut released_watchlist = Vec::new();
    let mut upcoming_watchlist = Vec::new();
    for (media, watchlist) in all_items {
        if let Some(rd_str) = &media.release_date {
            if let Ok(rd) = chrono::NaiveDate::parse_from_str(rd_str, "%Y-%m-%d") {
                if rd <= today {
                    released_watchlist.push((media, watchlist));
                } else {
                    upcoming_watchlist.push((media, watchlist));
                }
                continue;
            }
        }
        // If no release date, add to released
        released_watchlist.push((media, watchlist));
    }
    // Sort upcoming by release date ascending
    upcoming_watchlist.sort_by(|a, b| {
        let a_rd = a.0.release_date.as_deref().unwrap_or("");
        let b_rd = b.0.release_date.as_deref().unwrap_or("");
        a_rd.cmp(b_rd)
    });
    Ok(WatchlistDashboard {
        released_watchlist,
        upcoming_watchlist,
    })
}
// --- TMDB Account Commands --- //
#[derive(Debug, Serialize, Deserialize)]
struct TmdbRequestTokenResponse {
    success: bool,
    expires_at: String,
    request_token: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct TmdbSessionResponse {
    success: bool,
    session_id: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct TmdbGravatar {
    hash: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct TmdbAvatar {
    avatar_path: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
struct TmdbAccountAvatar {
    gravatar: TmdbGravatar,
    tmdb: TmdbAvatar,
}
#[derive(Debug, Serialize, Deserialize)]
struct TmdbAccountResponse {
    avatar: TmdbAccountAvatar,
    id: i64,
    iso_639_1: Option<String>,
    iso_3166_1: Option<String>,
    name: Option<String>,
    include_adult: bool,
    username: String,
}
// Get TMDB request token
#[tauri::command]
async fn get_tmdb_request_token() -> Result<String, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/authentication/token/new?api_key={}",
        TMDB_BASE_URL, TMDB_API_KEY
    );
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let json: TmdbRequestTokenResponse = response.json().await.map_err(|e| e.to_string())?;
    if !json.success {
        return Err("Failed to get request token".to_string());
    }
    Ok(json.request_token)
}
// Open TMDB auth URL in browser
#[tauri::command]
async fn open_tmdb_auth_url(app: tauri::AppHandle, request_token: String) -> Result<(), String> {
    let auth_url = format!("https://www.themoviedb.org/authenticate/{}", request_token);
    app.shell()
        .open(&auth_url, None)
        .map_err(|e| e.to_string())?;
    Ok(())
}
// Exchange request token for session ID
#[tauri::command]
async fn create_tmdb_session(
    request_token: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/authentication/session/new?api_key={}",
        TMDB_BASE_URL, TMDB_API_KEY
    );
    let response = client
        .post(&url)
        .json(&serde_json::json!({ "request_token": request_token }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let json: TmdbSessionResponse = response.json().await.map_err(|e| e.to_string())?;
    if !json.success {
        return Err("Failed to create session".to_string());
    }
    let session_id = json.session_id;
    // Get account ID
    let account = get_tmdb_account(&session_id).await?;
    // Save session and account ID to DB
    let conn = state.db.lock().await;
    conn.execute(
        "INSERT OR REPLACE INTO tmdb_auth (id, session_id, account_id) VALUES (1, ?1, ?2)",
        params![session_id, account.id],
    )
    .map_err(|e| e.to_string())?;
    Ok(session_id)
}
// Helper to get TMDB account details
async fn get_tmdb_account(session_id: &str) -> Result<TmdbAccountResponse, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/account?api_key={}&session_id={}",
        TMDB_BASE_URL, TMDB_API_KEY, session_id
    );
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let account: TmdbAccountResponse = response.json().await.map_err(|e| e.to_string())?;
    Ok(account)
}
// Get saved TMDB session (if any)
#[tauri::command]
async fn get_saved_tmdb_session(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let conn = state.db.lock().await;
    let session_id: Option<String> = conn
        .query_row("SELECT session_id FROM tmdb_auth WHERE id = 1", [], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(session_id)
}
// Get TMDB account details (requires active session)
#[tauri::command]
async fn get_tmdb_account_details(
    state: State<'_, AppState>,
) -> Result<TmdbAccountResponse, String> {
    let conn = state.db.lock().await;
    let session_id: Option<String> = conn
        .query_row("SELECT session_id FROM tmdb_auth WHERE id = 1", [], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(session_id) = session_id else {
        return Err("Not logged in to TMDB".to_string());
    };
    get_tmdb_account(&session_id).await
}
// Add to TMDB favorites
#[tauri::command]
async fn add_to_tmdb_favorites(
    media_id: i64,
    media_type: String,
    favorite: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().await;
    let session_id: Option<String> = conn
        .query_row("SELECT session_id FROM tmdb_auth WHERE id = 1", [], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(session_id) = session_id else {
        return Err("Not logged in to TMDB".to_string());
    };
    let account_id: i64 = conn
        .query_row("SELECT account_id FROM tmdb_auth WHERE id = 1", [], |row| {
            row.get(0)
        })
        .map_err(|e| e.to_string())?;
    let client = reqwest::Client::new();
    let url = format!(
        "{}/account/{}/favorite?api_key={}&session_id={}",
        TMDB_BASE_URL, account_id, TMDB_API_KEY, session_id
    );
    let media_type_str = if media_type == "movie" { "movie" } else { "tv" };
    let response = client
        .post(&url)
        .json(&serde_json::json!({
          "media_type": media_type_str,
          "media_id": media_id,
          "favorite": favorite
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err("Failed to update favorites".to_string());
    }
    Ok(())
}
// Add to TMDB watchlist
#[tauri::command]
async fn add_to_tmdb_watchlist(
    media_id: i64,
    media_type: String,
    watchlist: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().await;
    let session_id: Option<String> = conn
        .query_row("SELECT session_id FROM tmdb_auth WHERE id = 1", [], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(session_id) = session_id else {
        return Err("Not logged in to TMDB".to_string());
    };
    let account_id: i64 = conn
        .query_row("SELECT account_id FROM tmdb_auth WHERE id = 1", [], |row| {
            row.get(0)
        })
        .map_err(|e| e.to_string())?;
    let client = reqwest::Client::new();
    let url = format!(
        "{}/account/{}/watchlist?api_key={}&session_id={}",
        TMDB_BASE_URL, account_id, TMDB_API_KEY, session_id
    );
    let media_type_str = if media_type == "movie" { "movie" } else { "tv" };
    let response = client
        .post(&url)
        .json(&serde_json::json!({
          "media_type": media_type_str,
          "media_id": media_id,
          "watchlist": watchlist
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err("Failed to update watchlist".to_string());
    }
    Ok(())
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Create app data dir if it doesn't exist
    let app_data_dir = get_app_data_dir();
    std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");
    let db_path = app_data_dir.join("tv-time-clone.db");
    let conn = Connection::open(db_path).expect("failed to open database");
    init_db(&conn).expect("failed to initialize database");
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init()) // Add shell plugin!
        .manage(AppState {
            db: Arc::new(Mutex::new(conn)),
        })
        .invoke_handler(tauri::generate_handler![
            search_tmdb_api,
            get_upcoming_movies,
            add_media_to_watchlist,
            mark_as_watched,
            get_watchlist_dashboard,
            // New TMDB account commands
            get_tmdb_request_token,
            open_tmdb_auth_url,
            create_tmdb_session,
            get_saved_tmdb_session,
            get_tmdb_account_details,
            add_to_tmdb_favorites,
            add_to_tmdb_watchlist
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
fn main() {
    run();
}
