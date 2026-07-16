
# TV Time Clone - Media Tracker

A local-first media tracker desktop app built with Tauri, Vue 3, Tailwind CSS, and SQLite, using the TMDB API!

## Features
- 🔍 Search movies and TV shows (via TMDB)
- 📝 Add items to your watchlist
- ✅ Mark items as watched
- 📅 Split view: Released & Watching + Upcoming Calendar
- 💾 Local-only data storage with SQLite
- 📦 TMDB API caching for faster access

## Getting Started

### Prerequisites
1. **Node.js**: (https://nodejs.org/)
2. **Rust**: (https://rustup.rs/)
3. **TMDB API Key**: Get a free key at (https://www.themoviedb.org/settings/api)

### Installation
1. Clone or open this project folder!
2. Install dependencies:
   ```bash
   npm install
   ```
3. Add your TMDB API Key:
   - Open `src-tauri/src/main.rs`
   - Replace `const TMDB_API_KEY: &str = "YOUR_TMDB_API_KEY";` with your actual key!

### Running the App
```bash
npm run tauri dev
```

### Building for Production
```bash
npm run tauri build
```

## Project Structure
```
tv-time/
├── src/                     # Vue 3 frontend
│   ├── App.vue             # Main dashboard component
│   ├── main.ts             # App entry point
│   ├── style.css           # Tailwind CSS styles
│   └── vite-env.d.ts       # Vite env declarations
├── src-tauri/              # Rust backend
│   ├── src/main.rs         # Main Rust file (commands, DB setup)
│   ├── Cargo.toml          # Rust dependencies
│   └── tauri.conf.json     # Tauri config
├── index.html
├── package.json
├── vite.config.ts
├── tailwind.config.js
├── postcss.config.js
└── tsconfig.json
```

## Tech Stack
- **Frontend**: Vue 3 (Composition API), Vite, Tailwind CSS
- **Backend**: Tauri (Rust), rusqlite (SQLite), reqwest (HTTP)
- **External API**: The Movie Database (TMDB)
