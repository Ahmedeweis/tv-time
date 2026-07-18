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
    watched_episode_count: Option<i64>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
struct WatchedEpisode {
    media_id: i64,
    season_number: i64,
    episode_number: i64,
    watched_at: String,
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
            synopsis_cached_json_data TEXT NOT NULL,
            last_updated TEXT,
            number_of_seasons INTEGER
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
    conn.execute(
        "CREATE TABLE IF NOT EXISTS watched_episodes (
            media_id INTEGER NOT NULL,
            season_number INTEGER NOT NULL,
            episode_number INTEGER NOT NULL,
            watched_at TEXT NOT NULL,
            PRIMARY KEY (media_id, season_number, episode_number),
            FOREIGN KEY (media_id) REFERENCES media_cache(media_id)
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS seasons (
            media_id INTEGER NOT NULL,
            season_number INTEGER NOT NULL,
            season_name TEXT,
            episode_count INTEGER,
            air_date TEXT,
            overview TEXT,
            poster_path TEXT,
            PRIMARY KEY (media_id, season_number),
            FOREIGN KEY (media_id) REFERENCES media_cache(media_id)
        )",
        [],
    )?;
    // Add last_updated and number_of_seasons columns if they don't exist (for existing databases)
    conn.execute(
        "ALTER TABLE media_cache ADD COLUMN last_updated TEXT",
        [],
    ).ok();
    conn.execute(
        "ALTER TABLE media_cache ADD COLUMN number_of_seasons INTEGER",
        [],
    ).ok();
    // Create favorites table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_favorites (
            media_id INTEGER PRIMARY KEY,
            added_at TEXT NOT NULL,
            FOREIGN KEY (media_id) REFERENCES media_cache(media_id)
        )",
        [],
    )?;
    // Create user_lists table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_lists (
            list_id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    )?;
    // Create list_items table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS list_items (
            list_id INTEGER NOT NULL,
            media_id INTEGER NOT NULL,
            added_at TEXT NOT NULL,
            PRIMARY KEY (list_id, media_id),
            FOREIGN KEY (list_id) REFERENCES user_lists(list_id) ON DELETE CASCADE,
            FOREIGN KEY (media_id) REFERENCES media_cache(media_id)
        )",
        [],
    )?;
    Ok(())
}
// Helper to get app data dir (use OS-standard app data directory)
fn get_app_data_dir() -> std::path::PathBuf {
    // Get the current executable path to make the app portable
    let mut exe_path = std::env::current_exe().expect("Failed to get executable path");
    exe_path.pop(); // Remove the executable name to get its directory
    // In development mode (npm run tauri dev), the exe is in src-tauri/target/debug.
    // If we detect we are in 'debug', we will place the database in the main project root.
    if exe_path.ends_with("debug") {
        // Pop target/debug
        exe_path.pop();
        exe_path.pop();
        // Pop src-tauri
        if exe_path.ends_with("src-tauri") {
            exe_path.pop();
        }
    }
    let data_dir = exe_path.join("database");
    std::fs::create_dir_all(&data_dir).expect("Failed to create database directory");
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
// Tauri command: Get TMDB videos (trailers)
#[tauri::command]
async fn get_tmdb_videos(media_id: i64, media_type: String) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let endpoint = match media_type.as_str() {
        "movie" => "movie",
        "tv" => "tv",
        _ => return Err("Invalid media type".to_string()),
    };
    let url = format!(
        "{}/{}/{}/videos?api_key={}",
        TMDB_BASE_URL, endpoint, media_id, TMDB_API_KEY
    );
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(json)
}
// Tauri command: Get similar movies/shows
#[tauri::command]
async fn get_tmdb_similar(media_id: i64, media_type: String) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let endpoint = match media_type.as_str() {
        "movie" => "movie",
        "tv" => "tv",
        _ => return Err("Invalid media type".to_string()),
    };
    let url = format!(
        "{}/{}/{}/similar?api_key={}",
        TMDB_BASE_URL, endpoint, media_id, TMDB_API_KEY
    );
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(json)
}
// Tauri command: Get recommended movies/shows
#[tauri::command]
async fn get_tmdb_recommendations(
    media_id: i64,
    media_type: String,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let endpoint = match media_type.as_str() {
        "movie" => "movie",
        "tv" => "tv",
        _ => return Err("Invalid media type".to_string()),
    };
    let url = format!(
        "{}/{}/{}/recommendations?api_key={}",
        TMDB_BASE_URL, endpoint, media_id, TMDB_API_KEY
    );
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(json)
}
// Tauri command: Get TV seasons and episodes from TMDB
#[tauri::command]
async fn get_tv_seasons(media_id: i64) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/tv/{}?api_key={}",
        TMDB_BASE_URL, media_id, TMDB_API_KEY
    );
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(json)
}
// Tauri command: Get episodes for a specific TV season
#[tauri::command]
async fn get_tv_season_episodes(media_id: i64, season_number: i64) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/tv/{}/season/{}?api_key={}",
        TMDB_BASE_URL, media_id, season_number, TMDB_API_KEY
    );
    let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(json)
}
// Tauri command: Get upcoming episodes for all TV shows in watchlist
#[tauri::command]
async fn get_upcoming_episodes(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let shows = {
        let conn = state.db.lock().await;
        // Get all TV shows from watchlist
        let mut stmt = conn
            .prepare("SELECT media_id, title, poster_path FROM media_cache WHERE media_type = 'tv'")
            .map_err(|e| e.to_string())?;
        let shows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut result = Vec::new();
        for show_result in shows {
            if let Ok(show) = show_result {
                result.push(show);
            }
        }
        result
    };
    let mut all_upcoming = Vec::new();
    let today = chrono::Utc::now().date_naive();
    for (media_id, title, poster_path) in shows {
        // Fetch fresh data from TMDB
        let client = reqwest::Client::new();
        let url = format!("{}/tv/{}?api_key={}", TMDB_BASE_URL, media_id, TMDB_API_KEY);
        match client.get(&url).send().await {
            Ok(response) => {
                match response.json::<serde_json::Value>().await {
                    Ok(show_data) => {
                        if let Some(seasons) = show_data.get("seasons").and_then(|s| s.as_array()) {
                            for season in seasons {
                                if let Some(season_number) = season.get("season_number").and_then(|s| s.as_i64()) {
                                    if season_number > 0 { // Skip special seasons
                                        // Fetch episodes for this season
                                        let season_url = format!("{}/tv/{}/season/{}?api_key={}", TMDB_BASE_URL, media_id, season_number, TMDB_API_KEY);
                                        match client.get(&season_url).send().await {
                                            Ok(season_response) => {
                                                match season_response.json::<serde_json::Value>().await {
                                                    Ok(season_data) => {
                                                        if let Some(episodes) = season_data.get("episodes").and_then(|e| e.as_array()) {
                                                            for episode in episodes {
                                                                if let Some(air_date_str) = episode.get("air_date").and_then(|d| d.as_str()) {
                                                                    if let Ok(air_date) = chrono::NaiveDate::parse_from_str(air_date_str, "%Y-%m-%d") {
                                                                        if air_date >= today {
                                                                            let episode_number = episode.get("episode_number").and_then(|e| e.as_i64()).unwrap_or(0);
                                                                            let episode_name = episode.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                                                            let still_path = episode.get("still_path").and_then(|p| p.as_str()).map(|s| s.to_string());
                                                                            all_upcoming.push(serde_json::json!({
                                                                                "media_id": media_id,
                                                                                "title": title,
                                                                                "poster_path": poster_path,
                                                                                "season_number": season_number,
                                                                                "episode_number": episode_number,
                                                                                "episode_name": episode_name,
                                                                                "air_date": air_date_str,
                                                                                "still_path": still_path,
                                                                                "days_until": (air_date - today).num_days()
                                                                            }));
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    Err(_) => {} // Skip if season data fails
                                                }
                                            }
                                            Err(_) => {} // Skip if season request fails
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {} // Skip if show data fails
                }
            }
            Err(_) => {} // Skip if show request fails
        }
        // Rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    }
    // Sort by air date ascending
    all_upcoming.sort_by(|a, b| {
        let a_date = a.get("air_date").and_then(|d| d.as_str()).unwrap_or("");
        let b_date = b.get("air_date").and_then(|d| d.as_str()).unwrap_or("");
        a_date.cmp(b_date)
    });
    Ok(serde_json::json!({ "upcoming_episodes": all_upcoming }))
}
// Tauri command: Get next episode to watch for each TV show
#[tauri::command]
async fn get_next_episodes_to_watch(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let (shows_with_last_watched, shows_havent_started) = {
        let conn = state.db.lock().await;
        // Get all TV shows from watchlist
        let mut stmt = conn
            .prepare("
                SELECT mc.media_id, mc.title, mc.poster_path
                FROM media_cache mc
                INNER JOIN user_watchlist uw ON mc.media_id = uw.media_id
                WHERE mc.media_type = 'tv'
            ")
            .map_err(|e| e.to_string())?;
        let shows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut result_watched = Vec::new();
        let mut result_havent_started = Vec::new();
        for show_result in shows {
            if let Ok((media_id, title, poster_path)) = show_result {
                // Get the last watched episode for this show
                let mut last_stmt = conn
                    .prepare("
                        SELECT season_number, episode_number
                        FROM watched_episodes
                        WHERE media_id = ?1
                        ORDER BY season_number DESC, episode_number DESC
                        LIMIT 1
                    ")
                    .map_err(|e| e.to_string())?;
                let last_watched = last_stmt
                    .query_row(params![media_id], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                    })
                    .optional()
                    .map_err(|e| e.to_string())?;
                if let Some(last_watched) = last_watched {
                    result_watched.push((media_id, title, poster_path, Some(last_watched)));
                } else {
                    result_havent_started.push((media_id, title, poster_path));
                }
            }
        }
        (result_watched, result_havent_started)
    };
    let mut next_episodes = Vec::new();
    for (media_id, title, poster_path, last_watched) in shows_with_last_watched {
        // Fetch fresh data from TMDB
        let client = reqwest::Client::new();
        let url = format!("{}/tv/{}?api_key={}", TMDB_BASE_URL, media_id, TMDB_API_KEY);
        match client.get(&url).send().await {
            Ok(response) => {
                match response.json::<serde_json::Value>().await {
                    Ok(show_data) => {
                        if let Some(seasons) = show_data.get("seasons").and_then(|s| s.as_array()) {
                            let mut found_next = false;
                            for season in seasons {
                                if found_next { break; }
                                if let Some(season_number) = season.get("season_number").and_then(|s| s.as_i64()) {
                                    if season_number > 0 {
                                        // Fetch episodes for this season
                                        let season_url = format!("{}/tv/{}/season/{}?api_key={}", TMDB_BASE_URL, media_id, season_number, TMDB_API_KEY);
                                        match client.get(&season_url).send().await {
                                            Ok(season_response) => {
                                                match season_response.json::<serde_json::Value>().await {
                                                    Ok(season_data) => {
                                                        if let Some(episodes) = season_data.get("episodes").and_then(|e| e.as_array()) {
                                                            for episode in episodes {
                                                                if let Some(ep_number) = episode.get("episode_number").and_then(|e| e.as_i64()) {
                                                                    match last_watched {
                                                                        Some((last_season, last_episode)) => {
                                                                            // Find the next episode after the last watched one
                                                                            if season_number == last_season && ep_number > last_episode {
                                                                                let episode_name = episode.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                                                                let still_path = episode.get("still_path").and_then(|p| p.as_str()).map(|s| s.to_string());
                                                                                let air_date = episode.get("air_date").and_then(|d| d.as_str()).map(|s| s.to_string());
                                                                                next_episodes.push(serde_json::json!({
                                                                                    "media_id": media_id,
                                                                                    "title": title,
                                                                                    "poster_path": poster_path,
                                                                                    "season_number": season_number,
                                                                                    "episode_number": ep_number,
                                                                                    "episode_name": episode_name,
                                                                                    "still_path": still_path,
                                                                                    "air_date": air_date
                                                                                }));
                                                                                found_next = true;
                                                                                break;
                                                                            } else if season_number > last_season {
                                                                                // First episode of next season
                                                                                let episode_name = episode.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                                                                let still_path = episode.get("still_path").and_then(|p| p.as_str()).map(|s| s.to_string());
                                                                                let air_date = episode.get("air_date").and_then(|d| d.as_str()).map(|s| s.to_string());
                                                                                next_episodes.push(serde_json::json!({
                                                                                    "media_id": media_id,
                                                                                    "title": title,
                                                                                    "poster_path": poster_path,
                                                                                    "season_number": season_number,
                                                                                    "episode_number": ep_number,
                                                                                    "episode_name": episode_name,
                                                                                    "still_path": still_path,
                                                                                    "air_date": air_date
                                                                                }));
                                                                                found_next = true;
                                                                                break;
                                                                            }
                                                                        }
                                                                        None => {
                                                                            // No episodes watched yet, show first episode of first season
                                                                            if season_number == 1 && ep_number == 1 {
                                                                                let episode_name = episode.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                                                                                let still_path = episode.get("still_path").and_then(|p| p.as_str()).map(|s| s.to_string());
                                                                                let air_date = episode.get("air_date").and_then(|d| d.as_str()).map(|s| s.to_string());
                                                                                next_episodes.push(serde_json::json!({
                                                                                    "media_id": media_id,
                                                                                    "title": title,
                                                                                    "poster_path": poster_path,
                                                                                    "season_number": season_number,
                                                                                    "episode_number": ep_number,
                                                                                    "episode_name": episode_name,
                                                                                    "still_path": still_path,
                                                                                    "air_date": air_date
                                                                                }));
                                                                                found_next = true;
                                                                                break;
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    Err(_) => {} // Skip if season data fails
                                                }
                                            }
                                            Err(_) => {} // Skip if season request fails
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {} // Skip if show data fails
                }
            }
            Err(_) => {} // Skip if show request fails
        }
        // Rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    }
    // Add haven't started shows to the result
    for (media_id, title, poster_path) in shows_havent_started {
        next_episodes.push(serde_json::json!({
            "media_id": media_id,
            "title": title,
            "poster_path": poster_path,
            "season_number": 0,
            "episode_number": 0,
            "episode_name": "",
            "still_path": serde_json::Value::Null,
            "air_date": serde_json::Value::Null,
            "is_havent_started": true
        }));
    }
    Ok(serde_json::json!({ "next_episodes": next_episodes }))
}
// Tauri command: Get watched episodes for a TV show
#[tauri::command]
async fn get_watched_episodes(
    media_id: i64,
    state: State<'_, AppState>,
) -> Result<Vec<WatchedEpisode>, String> {
    let conn = state.db.lock().await;
    let mut stmt = conn.prepare("SELECT media_id, season_number, episode_number, watched_at FROM watched_episodes WHERE media_id = ?1")
        .map_err(|e| e.to_string())?;
    let episodes = stmt
        .query_map(params![media_id], |row| {
            Ok(WatchedEpisode {
                media_id: row.get(0)?,
                season_number: row.get(1)?,
                episode_number: row.get(2)?,
                watched_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    for episode in episodes {
        result.push(episode.map_err(|e| e.to_string())?);
    }
    Ok(result)
}
// Tauri command: Toggle episode watched status
#[tauri::command]
async fn toggle_episode_watched(
    media_id: i64,
    season_number: i64,
    episode_number: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().await;
    // Check if episode is already watched
    let exists = conn.query_row(
        "SELECT 1 FROM watched_episodes WHERE media_id = ?1 AND season_number = ?2 AND episode_number = ?3",
        params![media_id, season_number, episode_number],
        |_| Ok(()),
    ).optional().map_err(|e| e.to_string())?;
    if exists.is_some() {
        // Delete it
        conn.execute(
            "DELETE FROM watched_episodes WHERE media_id = ?1 AND season_number = ?2 AND episode_number = ?3",
            params![media_id, season_number, episode_number],
        ).map_err(|e| e.to_string())?;
    } else {
        // Add it
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO watched_episodes (media_id, season_number, episode_number, watched_at) VALUES (?1, ?2, ?3, ?4)",
            params![media_id, season_number, episode_number, now],
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
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
    let conn = state.db.lock().await;
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
// Tauri command: Mark as to watch
#[tauri::command]
async fn mark_as_to_watch(media_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().await;
    conn.execute(
        "UPDATE user_watchlist SET status = ?1, last_watched_date = ?2 WHERE media_id = ?3",
        params!["to_watch", None::<String>, media_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
// Tauri command: Remove from watchlist
#[tauri::command]
async fn remove_from_watchlist(media_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().await;
    conn.execute(
        "DELETE FROM user_watchlist WHERE media_id = ?1",
        params![media_id],
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
              uw.status, uw.last_watched_date, uw.notes,
              (SELECT COUNT(*) FROM watched_episodes we WHERE we.media_id = mc.media_id) as watched_episode_count
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
                watched_episode_count: row.get(9).unwrap_or(Some(0)),
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
// Background task to check for new seasons
async fn check_for_new_seasons(db: Arc<Mutex<Connection>>) {
    // Check if we've done a check in the last 24 hours
    let should_check = {
        let conn = db.lock().await;
        let last_check: Option<String> = conn
            .query_row("SELECT last_updated FROM media_cache WHERE media_type = 'tv' LIMIT 1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| {
                eprintln!("Error checking last update time: {}", e);
                e
            })
            .unwrap();
        if let Some(last_check_str) = last_check {
            if let Ok(last_check) = chrono::DateTime::parse_from_rfc3339(&last_check_str) {
                let now = chrono::Utc::now();
                let duration = now.signed_duration_since(last_check.with_timezone(&chrono::Utc));
                duration.num_hours() >= 24
            } else {
                true // Invalid date format, check anyway
            }
        } else {
            true // No previous check, check now
        }
    };
    if !should_check {
        println!("Skipping season check - last check was less than 24 hours ago.");
        return;
    }
    println!("Checking for new seasons...");
    let tv_shows = {
        let conn = db.lock().await;
        let mut stmt = conn
            .prepare("SELECT media_id, title, number_of_seasons FROM media_cache WHERE media_type = 'tv'")
            .map_err(|e| {
                eprintln!("Error preparing statement: {}", e);
                e
            })
            .unwrap();
        let shows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })
            .map_err(|e| {
                eprintln!("Error querying shows: {}", e);
                e
            })
            .unwrap();
        let mut result = Vec::new();
        for show in shows {
            if let Ok(show) = show {
                result.push(show);
            }
        }
        result
    };
    for (media_id, title, current_seasons) in tv_shows {
        println!("Checking show: {} (ID: {})", title, media_id);
        // Fetch latest data from TMDB
        let client = reqwest::Client::new();
        let url = format!("{}/tv/{}?api_key={}", TMDB_BASE_URL, media_id, TMDB_API_KEY);
        match client.get(&url).send().await {
            Ok(response) => {
                match response.json::<serde_json::Value>().await {
                    Ok(json) => {
                        if let Some(new_seasons) = json.get("number_of_seasons").and_then(|s| s.as_i64()) {
                            let has_new_season = match current_seasons {
                                Some(current) => new_seasons > current,
                                None => true,
                            };
                            if has_new_season {
                                println!("NEW SEASON DETECTED for {}: {} seasons (was {:?})", title, new_seasons, current_seasons);
                                // Update media_cache
                                let now = chrono::Utc::now().to_rfc3339();
                                {
                                    let conn = db.lock().await;
                                    conn.execute(
                                        "UPDATE media_cache SET number_of_seasons = ?1, last_updated = ?2 WHERE media_id = ?3",
                                        params![new_seasons, now, media_id],
                                    ).ok();
                                }
                                // Fetch and cache seasons data
                                if let Some(seasons_array) = json.get("seasons").and_then(|s| s.as_array()) {
                                    let conn = db.lock().await;
                                    for season in seasons_array {
                                        if let Some(season_number) = season.get("season_number").and_then(|s| s.as_i64()) {
                                            if season_number > 0 { // Skip special seasons (season 0)
                                                let season_name = season.get("name").and_then(|s| s.as_str()).unwrap_or("").to_string();
                                                let episode_count = season.get("episode_count").and_then(|e| e.as_i64()).unwrap_or(0);
                                                let air_date = season.get("air_date").and_then(|d| d.as_str()).map(|s| s.to_string());
                                                let overview = season.get("overview").and_then(|o| o.as_str()).map(|s| s.to_string());
                                                let poster_path = season.get("poster_path").and_then(|p| p.as_str()).map(|s| s.to_string());
                                                conn.execute(
                                                    "INSERT OR REPLACE INTO seasons (media_id, season_number, season_name, episode_count, air_date, overview, poster_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                                                    params![media_id, season_number, season_name, episode_count, air_date, overview, poster_path],
                                                ).ok();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error parsing JSON for show {}: {}", title, e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error fetching data for show {}: {}", title, e);
            }
        }
        // Rate limiting: sleep for 250ms between requests to avoid TMDB rate limits
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    }
    println!("Season check completed.");
}
// Start background task for season updates
async fn start_season_update_task(db: Arc<Mutex<Connection>>) {
    // Run immediately on startup
    check_for_new_seasons(db.clone()).await;
    // Then run every 24 hours
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(24 * 60 * 60));
    loop {
        interval.tick().await;
        check_for_new_seasons(db.clone()).await;
    }
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
// --- Local user profile (name + avatar + cover) --- //
#[derive(Debug, Serialize, Deserialize, Clone)]
struct UserProfile {
    name: String,
    avatar_data_url: Option<String>,
    cover_data_url: Option<String>,
}
fn user_profile_path() -> std::path::PathBuf {
    get_app_data_dir().join("user-profile.json")
}
fn default_user_profile() -> UserProfile {
    UserProfile {
        name: String::new(),
        avatar_data_url: None,
        cover_data_url: None,
    }
}
#[tauri::command]
fn get_user_profile() -> Result<UserProfile, String> {
    let path = user_profile_path();
    if !path.exists() {
        return Ok(default_user_profile());
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}
#[tauri::command]
fn save_user_profile(profile: UserProfile) -> Result<UserProfile, String> {
    let path = user_profile_path();
    let json = serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(profile)
}
// --- Favorites Commands --- //
#[derive(Debug, Serialize, Deserialize, Clone)]
struct FavoriteItem {
    media_id: i64,
    title: String,
    poster_path: Option<String>,
    media_type: String,
    release_date: Option<String>,
    added_at: String,
}
#[tauri::command]
async fn toggle_favorite(
    media_id: i64,
    media_type: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let conn = state.db.lock().await;
    // Check if already favorited
    let exists = conn.query_row(
        "SELECT 1 FROM user_favorites WHERE media_id = ?1",
        params![media_id],
        |_| Ok(()),
    ).optional().map_err(|e| e.to_string())?;
    if exists.is_some() {
        // Remove from favorites
        conn.execute(
            "DELETE FROM user_favorites WHERE media_id = ?1",
            params![media_id],
        ).map_err(|e| e.to_string())?;
        Ok(false) // Not favorited anymore
    } else {
        // Add to favorites
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO user_favorites (media_id, added_at) VALUES (?1, ?2)",
            params![media_id, now],
        ).map_err(|e| e.to_string())?;
        Ok(true) // Now favorited
    }
}
#[tauri::command]
async fn is_favorite(media_id: i64, state: State<'_, AppState>) -> Result<bool, String> {
    let conn = state.db.lock().await;
    let exists = conn.query_row(
        "SELECT 1 FROM user_favorites WHERE media_id = ?1",
        params![media_id],
        |_| Ok(()),
    ).optional().map_err(|e| e.to_string())?;
    Ok(exists.is_some())
}
#[tauri::command]
async fn get_favorites(state: State<'_, AppState>) -> Result<Vec<FavoriteItem>, String> {
    let conn = state.db.lock().await;
    let mut stmt = conn
        .prepare(
            "SELECT mc.media_id, mc.title, mc.poster_path, mc.media_type, mc.release_date, uf.added_at
             FROM media_cache mc
             INNER JOIN user_favorites uf ON mc.media_id = uf.media_id
             ORDER BY uf.added_at DESC"
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(FavoriteItem {
                media_id: row.get(0)?,
                title: row.get(1)?,
                poster_path: row.get(2)?,
                media_type: row.get(3)?,
                release_date: row.get(4)?,
                added_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}
// --- Lists Commands --- //
#[derive(Debug, Serialize, Deserialize, Clone)]
struct UserList {
    list_id: i64,
    name: String,
    created_at: String,
    updated_at: String,
    item_count: i64,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
struct ListItem {
    media_id: i64,
    title: String,
    poster_path: Option<String>,
    media_type: String,
    release_date: Option<String>,
    added_at: String,
}
#[tauri::command]
async fn create_list(name: String, state: State<'_, AppState>) -> Result<UserList, String> {
    let conn = state.db.lock().await;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO user_lists (name, created_at, updated_at) VALUES (?1, ?2, ?3)",
        params![name, now, now],
    ).map_err(|e| e.to_string())?;
    let list_id = conn.last_insert_rowid();
    Ok(UserList {
        list_id,
        name,
        created_at: now.clone(),
        updated_at: now,
        item_count: 0,
    })
}
#[tauri::command]
async fn rename_list(list_id: i64, name: String, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().await;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE user_lists SET name = ?1, updated_at = ?2 WHERE list_id = ?3",
        params![name, now, list_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
async fn delete_list(list_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().await;
    conn.execute("DELETE FROM list_items WHERE list_id = ?1", params![list_id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM user_lists WHERE list_id = ?1", params![list_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
async fn get_lists(state: State<'_, AppState>) -> Result<Vec<UserList>, String> {
    let conn = state.db.lock().await;
    let mut stmt = conn
        .prepare(
            "SELECT ul.list_id, ul.name, ul.created_at, ul.updated_at,
                    (SELECT COUNT(*) FROM list_items li WHERE li.list_id = ul.list_id) as item_count
             FROM user_lists ul
             ORDER BY ul.updated_at DESC"
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(UserList {
                list_id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                item_count: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}
#[tauri::command]
async fn add_to_list(list_id: i64, media_id: i64, media_type: String, state: State<'_, AppState>) -> Result<(), String> {
    // Check if media is cached (with lock held briefly)
    let is_cached = {
        let conn = state.db.lock().await;
        conn.query_row(
            "SELECT 1 FROM media_cache WHERE media_id = ?1",
            params![media_id],
            |_| Ok(()),
        ).optional().map_err(|e| e.to_string())?.is_some()
    };
    if !is_cached {
        // Fetch and cache media (without lock)
        let media_json = fetch_media_details_from_tmdb(media_id, &media_type).await?;
        let conn = state.db.lock().await;
        let title = match media_type.as_str() {
            "movie" => media_json.get("title").and_then(|t| t.as_str()),
            "tv" => media_json.get("name").and_then(|t| t.as_str()),
            _ => None,
        }.unwrap_or("").to_string();
        let poster_path = media_json.get("poster_path").and_then(|p| p.as_str()).map(|s| s.to_string());
        let release_date = match media_type.as_str() {
            "movie" => media_json.get("release_date"),
            "tv" => media_json.get("first_air_date"),
            _ => None,
        }.and_then(|r| r.as_str()).map(|s| s.to_string());
        let synopsis_json = serde_json::to_string(&media_json).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO media_cache (media_id, title, poster_path, media_type, release_date, synopsis_cached_json_data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![media_id, title, poster_path, media_type, release_date, synopsis_json],
        ).map_err(|e| e.to_string())?;
    }
    let now = chrono::Utc::now().to_rfc3339();
    let conn = state.db.lock().await;
    conn.execute(
        "INSERT OR IGNORE INTO list_items (list_id, media_id, added_at) VALUES (?1, ?2, ?3)",
        params![list_id, media_id, now],
    ).map_err(|e| e.to_string())?;
    // Update list updated_at
    conn.execute(
        "UPDATE user_lists SET updated_at = ?1 WHERE list_id = ?2",
        params![now, list_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
async fn remove_from_list(list_id: i64, media_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().await;
    conn.execute(
        "DELETE FROM list_items WHERE list_id = ?1 AND media_id = ?2",
        params![list_id, media_id],
    ).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE user_lists SET updated_at = ?1 WHERE list_id = ?2",
        params![now, list_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
async fn get_list_items(list_id: i64, state: State<'_, AppState>) -> Result<Vec<ListItem>, String> {
    let conn = state.db.lock().await;
    let mut stmt = conn
        .prepare(
            "SELECT mc.media_id, mc.title, mc.poster_path, mc.media_type, mc.release_date, li.added_at
             FROM media_cache mc
             INNER JOIN list_items li ON mc.media_id = li.media_id
             WHERE li.list_id = ?1
             ORDER BY li.added_at DESC"
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![list_id], |row| {
            Ok(ListItem {
                media_id: row.get(0)?,
                title: row.get(1)?,
                poster_path: row.get(2)?,
                media_type: row.get(3)?,
                release_date: row.get(4)?,
                added_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Create app data dir if it doesn't exist
    let app_data_dir = get_app_data_dir();
    std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");
    let db_path = app_data_dir.join("tv-time-clone.db");
    let conn = Connection::open(db_path).expect("failed to open database");
    init_db(&conn).expect("failed to initialize database");
    let db_arc = Arc::new(Mutex::new(conn));
    let db_arc_for_task = db_arc.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init()) // Add shell plugin!
        .manage(AppState {
            db: db_arc,
        })
        .setup(move |_app| {
            // Spawn background task for season updates after app is setup
            tauri::async_runtime::spawn(async move {
                start_season_update_task(db_arc_for_task).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search_tmdb_api,
            get_tmdb_videos,
            get_tmdb_similar,
            get_tmdb_recommendations,
            get_upcoming_movies,
            add_media_to_watchlist,
            remove_from_watchlist,
            mark_as_watched,
            mark_as_to_watch,
            get_watchlist_dashboard,
            get_tv_seasons,
            get_tv_season_episodes,
            get_watched_episodes,
            toggle_episode_watched,
            get_upcoming_episodes,
            get_next_episodes_to_watch,
            // New TMDB account commands
            get_tmdb_request_token,
            open_tmdb_auth_url,
            create_tmdb_session,
            get_saved_tmdb_session,
            get_tmdb_account_details,
            add_to_tmdb_favorites,
            add_to_tmdb_watchlist,
            get_user_profile,
            save_user_profile,
            toggle_favorite,
            is_favorite,
            get_favorites,
            create_list,
            rename_list,
            delete_list,
            get_lists,
            add_to_list,
            remove_from_list,
            get_list_items
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
fn main() {
    run();
}
