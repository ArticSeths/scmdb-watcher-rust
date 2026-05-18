# SCMDB Watcher

A native desktop application that tails Star Citizen's `Game.log`, parses mission and blueprint events, and pushes them live to [scmdb.net](https://scmdb.net) via Server-Sent Events.

Built with **Tauri v2** (Rust backend) and a vanilla HTML/CSS/JS frontend. Compiles to a single lightweight binary for Windows, Linux, and macOS.

## Why migrate from Python to Tauri?

The original watcher was a ~450-line Python script using Flask. It worked well but had friction:

| Problem with Python version | How Tauri solves it |
|---|---|
| Requires Python 3.10+ installed | Single `.exe` / `.app` — no runtime needed |
| `install.bat` + virtualenv setup | Double-click to run, zero setup |
| Console window must stay open | Native window with system tray, runs in background |
| No UI — text-only console | Full dashboard: active missions, live logs, settings |
| Platform-specific batch files | Cross-platform binary from one codebase |
| Unsigned `.exe` from PyInstaller triggers antivirus | Tauri produces clean native binaries, can be code-signed |

## Features

- **Live watcher** — tails `Game.log` with rotation detection, parses mission lifecycle and blueprint events
- **SSE server** — `127.0.0.1:23456` with `/ping`, `/state`, `/events` endpoints (100% retrocompatible with scmdb.net)
- **Dashboard** — active missions with live timers, recent events feed
- **Log viewer** — real-time event stream with auto-scroll
- **Import tool** — scan `logbackups/` directory, export completed missions + blueprints as JSON
- **Settings** — configurable log path, port, dev mode, auto-start
- **System tray** — minimize to tray on close, right-click menu (Show/Quit), left-click restores window
- **Cross-platform** — Windows, Linux, macOS from a single codebase

## Architecture

```
┌─────────────────────────────────────────────┐
│  Tauri v2 App                               │
│  ┌───────────────────────────────────────┐  │
│  │  Frontend (Vanilla HTML/CSS/JS)       │  │
│  │  Dashboard | Logs | Import | Settings │  │
│  └───────────────────────────────────────┘  │
│  ┌───────────────────────────────────────┐  │
│  │  Rust Backend (tokio async runtime)   │  │
│  │  LogTailer → Parser → EventBus       │  │
│  │  Axum SSE Server (port 23456)         │  │
│  │  Import Engine | Config persistence   │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
         │ SSE on 127.0.0.1:23456
         ▼
   scmdb.net (browser tab)
```

### Backend modules (`src-tauri/src/`)

| Module | Purpose |
|--------|---------|
| `watcher/patterns.rs` | 4 shared compiled regex patterns (mission markers, accept, end, blueprint) |
| `watcher/state.rs` | Mission state machine (guid map, active missions, lifecycle correlation) |
| `watcher/parser.rs` | Timestamp regex + `process_line()` event extraction (uses patterns) |
| `watcher/tailer.rs` | Async file polling (200ms), rotation detection, seek-based resume |
| `watcher/bus.rs` | `tokio::sync::broadcast` channel for fan-out to UI + SSE clients |
| `server/sse_server.rs` | Axum HTTP server: `/ping`, `/state`, `/events` (SSE stream) |
| `importer/mod.rs` | Batch scan of logbackups, dedup by GUID, JSON export |
| `config.rs` | `AppConfig` struct, JSON persistence in OS config directory |
| `commands.rs` | Tauri IPC commands (bridge between frontend and backend) |
| `main.rs` | App setup, system tray, window events, auto-start |

### Frontend (`src/`)

| File | Purpose |
|------|---------|
| `index.html` | App shell with sidebar navigation and 4 tab panels |
| `styles.css` | Dark theme with CSS variables, responsive layout |
| `app.js` | Tauri IPC calls, event listeners, DOM rendering |

## Requirements

### Development

- **Rust** 1.70+ (with `cargo`)
- **Node.js** 18+ (for Tauri CLI)
- Platform-specific dependencies:
  - **Windows**: Visual Studio Build Tools (C++ workload)
  - **Linux**: `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`
  - **macOS**: Xcode Command Line Tools

### End users

Nothing. The compiled binary is self-contained.

## Development

```bash
# Install Tauri CLI (first time)
npm install

# Run in development mode (hot-reload on Rust changes)
npm run tauri dev

# Build release binary
npm run tauri build
```

The release binary will be in `src-tauri/target/release/` and installers in `src-tauri/target/release/bundle/`.

## Configuration

Settings are stored in the OS config directory:

- **Windows**: `%APPDATA%\scmdb-watcher\config.json`
- **Linux**: `~/.config/scmdb-watcher/config.json`
- **macOS**: `~/Library/Application Support/scmdb-watcher/config.json`

Default config:

```json
{
  "log_path": "C:\\Program Files\\Roberts Space Industries\\StarCitizen\\LIVE\\Game.log",
  "port": 23456,
  "dev_mode": false,
  "dev_origins": [],
  "auto_start_watcher": true,
  "custom_origins": []
}
```

## API (SSE Server)

The embedded server runs on `127.0.0.1:23456` (localhost only, no LAN exposure).

| Endpoint | Description |
|----------|-------------|
| `GET /ping` | `{"status": "ok", "version": "0.2.0"}` |
| `GET /state` | `{"active": [...]}` — current active missions |
| `GET /events` | SSE stream — real-time events (same format as Python version) |

CORS is restricted to `https://scmdb.net` and `https://www.scmdb.net`. Dev mode adds `localhost:5173` and `localhost:3000`.

## Event types (SSE)

| Type | Trigger |
|------|---------|
| `mission_start` | Contract accepted |
| `mission_complete` | Mission completed successfully |
| `mission_ended` | Mission abandoned/failed/disconnected |
| `blueprint_received` | Blueprint drop (correlated to recent mission) |
| `session_reset` | Log rotation detected (new game session) |
| `state_snapshot` | Sent on SSE connect with current active missions |

## License

Same as the original SCMDB Log Watcher project.
