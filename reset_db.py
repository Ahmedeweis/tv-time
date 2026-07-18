import sqlite3
import os

db_paths = [
    r"e:\Lan_Production\TFourthGitHub\tv-time\src-tauri\tv-time-clone-data\tv-time-clone.db",
    r"e:\Lan_Production\TFourthGitHub\tv-time\database\tv-time-clone.db",
]

tables = [
    'list_items',
    'user_lists',
    'user_favorites',
    'episodes',
    'watched_episodes',
    'seasons',
    'user_watchlist',
    'tmdb_auth',
    'media_cache',
]

for db_path in db_paths:
    if not os.path.exists(db_path):
        print(f"Skipping (not found): {db_path}")
        continue

    print(f"\n=== Resetting: {db_path} ===")
    db = sqlite3.connect(db_path)
    c = db.cursor()
    c.execute('PRAGMA foreign_keys=OFF')

    for table in tables:
        try:
            c.execute(f"DELETE FROM {table}")
            print(f"  Cleared {table}: {c.rowcount} rows deleted")
        except Exception as e:
            print(f"  Skip {table}: {e}")

    c.execute('PRAGMA foreign_keys=ON')
    db.commit()
    db.execute('VACUUM')
    db.close()
    print(f"  => Done!")

print("\nAll databases reset successfully!")
