# 🚀 AT-mail-rs: High-Performance, Lightweight Native Email Client in Rust

**AT-mail-rs** is an ultra-fast, modern, native cross-platform desktop email client built from the ground up in **pure Rust**. It delivers a clean, responsive desktop user experience with **exceptionally low memory usage (< 45MB resident memory vs 500MB+ for web/Electron-based clients)**, instant sub-100ms cold startup time, and GPU-accelerated rendering.

---

## ⌨️ Keyboard Shortcuts & Power Navigation

AT-mail-rs provides first-class, Vim-inspired keyboard navigation and an omnipresent Command Palette for high-efficiency workflows.

| Shortcut | Action | Scope / Context |
|---|---|---|
| <kbd>Ctrl</kbd> + <kbd>K</kbd> / <kbd>Cmd</kbd> + <kbd>K</kbd> | **Open Command Palette** (Fuzzy command launcher) | Global |
| <kbd>Ctrl</kbd> + <kbd>P</kbd> | **Open Command Palette** (Alternative hotkey) | Global |
| <kbd>/</kbd> | **Focus Search Bar** | Global |
| <kbd>j</kbd> or <kbd>↓</kbd> | **Next Email** | Message List |
| <kbd>k</kbd> or <kbd>↑</kbd> | **Previous Email** | Message List |
| <kbd>x</kbd> | **Toggle Checkbox Selection** (Multi-select) | Message List |
| <kbd>Ctrl</kbd> / <kbd>Cmd</kbd> + Click | **Toggle Individual Selection** | Message List |
| <kbd>Shift</kbd> + Click | **Select Contiguous Range** | Message List |
| <kbd>c</kbd> | **Compose New Email** | Global |
| <kbd>r</kbd> | **Reply to Email** (HTML default) | Message View / List |
| <kbd>a</kbd> | **Reply All** | Message View / List |
| <kbd>f</kbd> | **Forward Email** | Message View / List |
| <kbd>s</kbd> | **Toggle Star / Flag** | Message View / List |
| <kbd>u</kbd> | **Toggle Read / Unread** | Message View / List |
| <kbd>Del</kbd> / <kbd>Backspace</kbd> | **Delete Selected Email(s)** | Message View / List |
| <kbd>Esc</kbd> | **Close Active Modal / Dismiss Palette** | Modals & Overlays |

---

## ✨ Key Features

### ⚡ Performance & Core Architecture
- **Ultra-Low Memory Footprint**: Built in **pure Rust** with GPU-accelerated **`egui`** (`eframe`) — zero Electron, zero Chromium runtime overhead. Startup time < 100ms; idle resident memory footprint < 45 MB.
- **Embedded SQLite WAL Engine**: Full MIME messages, HTML bodies, and plain-text alternatives are indexed and cached locally with Write-Ahead Logging (WAL) for 0ms latency when opening emails.
- **Async IMAP/SMTP Pipeline**: Full RFC 2047 envelope parsing (`mailparse`), configurable date-window syncing (7d, 14d, 30d, 90d, 1y, All Time), and safe connection pooling.
- **OS-Native Keyring Integration**: Secure credential storage via Linux Secret Service, macOS Keychain, and Windows Credential Manager (`keyring-rs`). Passwords are never stored in plaintext.

---

### 🔍 Command Palette & Smart Search
- **⚡ Command Palette (`Ctrl+K` / `Cmd+K`)**: Modal command launcher with fuzzy filtering, categorical grouping (Navigation, Folders, Actions, Message, Themes), and keyboard arrow navigation.
- **🏷️ Quick Filter Chips**: Tactile chip buttons (`[All]`, `[✉ Unread]`, `[★ Starred]`, `[📎 Files]`) above the message list with active visual indicators.
- **🔎 Structured Search Tokens**: Search query parser supporting structured search tokens alongside free-text queries:
  - `from:alice@work.com`
  - `to:team@company.com`
  - `subject:financial report`
  - `has:attachment`
  - `is:unread` / `is:read`
  - `is:starred` / `is:flagged`

---

### 📝 Composer & Undo Send
- **⚡ Markdown Compose Mode with Live Preview**: Side-by-side split screen for drafting emails in Markdown with real-time rendered HTML preview. Includes formatting action buttons for Bold, Italic, Headings, Lists, Blockquotes, and Code Blocks.
- **🌐 HTML & Plain Text Modes**: Full support for HTML rich text and Text-Only drafting modes.
- **↩ 5-Second Undo Send**: Outgoing emails enter a 5-second safety buffer with a floating countdown bar (`[ ↩ Undo Send ]` / `[ ⚡ Send Now ]`). Clicking Undo immediately aborts transmission and restores the full draft into the composer.
- **🖋️ Default & Custom Signatures**: Automatically attaches account-specific or global signatures with HTML sanitization.
- **📋 Boilerplate Templates**: Reusable response snippets with dynamic variable substitutions (`{{name}}`, `{{sender}}`, `{{date}}`).

---

### 🛡️ Privacy Shield & Security
- **🛡️ Remote Image Blocker**: Automatically suppresses external `http://` / `https://` tracking pixels and remote images to safeguard IP privacy and prevent email tracking. Users can load images on demand with one click (`[ 🖼 Load Images ]`).
- **⚠️ Anti-Phishing Link Safety Detector**: Identifies deceptive hyperlinks where the visible display text (e.g. `paypal.com`) points to a different destination domain (`evil-site.com`). Highlights suspicious links with warning badges (`⚠️ [Deceptive Link!]`) and detailed tooltips.

---

### 🎨 Multi-Theme Engine & Custom Theme Creator
Switch between handcrafted visual themes via `⚙ Preferences -> 🎨 Appearance` or the `Ctrl+K` Command Palette:
1. **Gruvbox Retro Dark**: Warm retro groove dark palette with amber and gold accents (`#282828`).
2. **Gruvbox Retro Light**: Warm retro groove light parchment palette with ochre accents (`#fbf1c7`).
3. **Gruvbox Auto (System)**: Automatically switches between Gruvbox Dark & Light based on your operating system's dark mode preference.
4. **Dark Slate (Default)**: Sleek charcoal surface with Google Blue accents.
5. **Catppuccin Mocha**: Soothing pastel dark palette with lavender highlights.
6. **Nord Arctic**: Crisp north-bluish dark aesthetic.
7. **Solarized Dark**: Precision low-contrast cyan and warm-green dark palette.
8. **OLED Pure Black**: True `#000000` pitch-black mode for OLED displays and battery savings.
9. **Clean Daylight**: Modern, crisp daylight theme for well-lit environments.
10. **🎨 Custom Theme Creator**: Full interactive color pickers for App Background, Sidebar, Reading Pane, Cards, Accents, Borders, and Text. Custom themes are saved as standalone JSON files in the OS config folder (`~/.config/at-mail-rs/themes/<theme_name>.json`) and can be exported, imported, and applied on the fly.

---

### 📁 Storage Path Inspector & Data Relocation
- **Path Inspection**: View the exact live filepaths for the SQLite database (`email_client.db`), OS configuration directory (`~/.config/at-mail-rs/`), and custom themes.
- **📁 Relocate / Move Data Directory**: 1-click migration tool in *Settings -> General & Storage* that safely copies the database and WAL files to any chosen custom directory (e.g. secondary SSD or encrypted vault) and updates the application configuration pointer.

---

### 💾 Complete Application Backup & Restore
- **🔒 Privacy First**: Exports complete configuration packages while strictly **omitting passwords and keyring secrets**.
- **Portable JSON Backup**: Exports and imports Email Account endpoints, Custom Themes, Quick Templates, Signatures, and Preferences via *Settings -> 💾 Backup & Restore*.

---

### 📤 1-Click Message Export
Export any email directly from the reading pane toolbar (`📤 Export ▾`):
- **`📄 Markdown (.md)`**: Export with YAML frontmatter headers (Subject, From, To, Cc, Date) and formatted body text.
- **`🌐 HTML Document (.html)`**: Standalone, styled HTML document viewable in any browser.
- **`✉ Raw EML (.eml)`**: Standard RFC-822 formatted message file compatible with all standard email clients.

---

### ⏱️ Flexible Download & Sync Windows
Select how many days of emails to synchronize during account setup or in settings:
- **7 Days**, **14 Days**, **30 Days**, **45 Days**, **60 Days**, **90 Days**, **365 Days (1 Year)**, **Custom Days...** (numeric stepper for any custom duration), or **All History**.

---

### 📦 Multi-Select, Batch Actions & Drag-and-Drop
- **Multi-Select**: Select emails using row checkboxes `[✓]`, `Ctrl / Cmd + Click`, or `Shift + Click` range selection.
- **Batch Actions Bar**: Perform bulk operations across selected items: Batch Delete (`🗑 Delete`), Batch Move (`📁 Move ▾`), Batch Mark Read/Unread (`✉ Read` / `✉ Unread`), and Batch Star (`★ Star`).
- **Drag-and-Drop**: Drag individual or batch-selected emails directly onto sidebar folders with live mouse payload count badges (`📁 Moving N emails...`).

---

## 🏗️ Architecture & Workspace Structure

```
Email-Application/
├── Cargo.toml               # Workspace manifest
├── crates/
│   ├── email-core/          # Domain models, errors, event definitions (SyncCommand / SyncEvent)
│   ├── email-keychain/      # Native OS Keyring abstraction layer
│   ├── email-storage/       # SQLite storage engine with connection pooling (r2d2), WAL mode, and structured search
│   ├── email-smtp/          # Asynchronous SMTP client (lettre + tokio-rustls)
│   ├── email-sync/          # IMAP synchronization actor (async-imap + mailparse)
│   ├── email-html/          # HTML AST sanitizer, Markdown compiler, phishing detector, and plain-text extractor
│   └── email-ui/            # Native desktop GUI (eframe / egui) with 3-pane layout, themes, command palette, and settings
```

---

## 🛠️ Prerequisites & Build Instructions

### 1. System Dependencies (Linux)

Ensure you have the required development libraries installed:

```bash
# Ubuntu / Debian
sudo apt update
sudo apt install build-essential pkg-config libssl-dev libdbus-1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev
```

### 2. Build & Test the Project

```bash
# Clone repository
git clone git@github.com:AbhashTech/Email-Client.git
cd Email-Application

# Check workspace compilation
cargo check --workspace

# Run automated tests
cargo test --workspace

# Build optimized release binary
cargo build --release
```

The compiled binary will be located at:
`target/release/email-ui`

---

## 🚀 Running the Client

```bash
cargo run --release --bin email-ui
```

### First-Time Setup:
1. Launch the application.
2. Click **`+ Add Account`** in the sidebar or from `⚙ Settings -> 📬 Accounts`.
3. Select your provider preset (e.g. Zoho Mail, Gmail, Outlook, Hostinger, or Custom IMAP/SMTP).
4. Enter your email and app-specific password.
5. Click **`Save & Connect`**.
6. Click **`🔄 Sync All`** to synchronize your mailbox!

---

## 📄 License

Dual-licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
