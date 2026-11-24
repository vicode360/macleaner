# Macleaner

A lightweight, native macOS menu bar utility to inspect and safely reclaim **System Data**, developer caches, and bloated application directories without permanently deleting files.

Built with **Tauri v2**, **Rust** (`jwalk`, `rayon`, `trash`, `window-vibrancy`), and **TypeScript + Vite**.

---

## Features

- **Native macOS Menu Bar Popover:** Runs as an accessory app (`ActivationPolicy::Accessory`, no Dock icon) with native `NSVisualEffectMaterial::Popover` vibrancy, automatic Light/Dark mode support, and auto-dismiss on focus loss.
- **Three-Tab Workflow:**
  - **Summary:** Live disk health telemetry, proportional storage bar, parallel scan progress, and quick access to the largest folders.
  - **Items:** Directories sorted by size with risk filters (`Low`, `Medium`, `High`), top file extensions, concise explanations, and expandable **subfolder inspection** (e.g., separating `steamapps/common` from `steamapps/shadercache` and `appcache`).
  - **Settings:** Runtime language switcher (**Português**, **English**, **Español**), minimum folder size threshold (MB), and AI engine configuration.
- **Fast & Safe Scanning:**
  - Parallel traversal with `jwalk` and `rayon` across `~/Library/Caches`, `~/Library/Application Support`, `~/Library/Developer`, `~/.cache`, and common developer targets (`.cargo/registry`, `.npm`, `.ollama/models`, `Library/pnpm/store`, `Library/Logs`).
  - Instant deterministic classification via embedded rules (`src-tauri/rules/known.toml`).
  - Proactive **SIP** and **TCC** path filtering; `PermissionDenied` errors are handled silently.
- **Reversible Removal:** Never uses `rm -rf` or `remove_dir_all`. Every selected folder or subfolder is moved to the **macOS Trash** using the `trash` crate so files can always be restored.
- **On-Scan Hybrid AI:** Runs strictly during a scan for unmapped folders using real subfolder and extension telemetry—supporting local **Ollama** (auto-detecting installed models, disabling reasoning overhead with `think: false`, and unloading from RAM via `keep_alive: 0`) or an optional HTTPS OpenAI-compatible API, with a conservative fallback when offline.
- **Update Check:** Header **Update** button checks GitHub Releases against the installed version (`v0.1.0`) and alerts when a newer release is published.

---

## Requirements

### Running the App
- **macOS 12+** (Apple Silicon `aarch64` builds provided in Releases; Intel `x86_64` supported when built from source).
- *(Optional)* **Ollama** running locally on `http://127.0.0.1:11434` with any pulled model (e.g., `ollama pull qwen3:8b` or `llama3.2:3b`), or an HTTPS OpenAI-compatible API key. Without an AI provider, Macleaner operates with local deterministic rules and conservative classification.

### Building from Source
- **macOS** with **Xcode Command Line Tools**:
  ```bash
  xcode-select --install
  ```
- **Rust** (`1.90+`) via [rustup](https://rustup.rs/):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **Node.js** (`20+`) and `npm`.

---

## Getting Started

```bash
git clone https://github.com/vicode360/macleaner.git
cd macleaner
npm install
```

### Development
```bash
npm run tauri dev
```
The Macleaner icon appears in the macOS menu bar. Left-click to toggle the popover; right-click to quit (**Quit Macleaner**).

### Production Build
```bash
npm run tauri build
```
Output bundles (`.app` and `.dmg`) are generated in `src-tauri/target/release/bundle/`.

### Verification
```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

---

## Project Structure

```text
macleaner/
├── index.html                  # Popover HTML entry (380x480)
├── src/
│   ├── main.ts                 # Native 3-tab UI, Lucide icons, and Tauri IPC calls
│   ├── i18n.ts                 # Localization (PT-BR, EN, ES) and Intl formatting
│   ├── styles.css              # Apple HIG / Aqua-inspired Light & Dark tokens
│   └── types.ts                # 1:1 TypeScript definitions matching Rust Serde structs
└── src-tauri/
    ├── rules/known.toml        # Embedded macOS cleanup rules and SIP/TCC denylist
    ├── capabilities/default.json
    ├── tauri.conf.json
    └── src/
        ├── lib.rs              # Accessory policy, TrayIcon, Positioner, and Vibrancy
        ├── rules.rs            # Embedded rule engine and SIP/TCC path protection
        ├── scanner.rs          # Parallel jwalk + rayon scanner, sysinfo disk health, and Trash moves
        ├── ai.rs               # Loopback Ollama / HTTPS external API client with conservative fallback
        └── commands.rs         # Tauri IPC commands, settings persistence, and release checker
```

---

## Privacy & Safety Guarantees

- **Zero permanent deletion:** Removal uses only `trash::delete` to move items to the macOS Trash.
- **Metadata-only AI prompts:** Only directory names, sanitized relative paths (`~/...`), sizes, file counts, ages, and extension totals are sent—never file contents.
- **Localhost enforcement:** Ollama endpoints are restricted to loopback HTTP (`127.0.0.1`, `localhost`, `::1`); external API endpoints require HTTPS and redact API keys in debug logs.
