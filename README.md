# 🚀 AT-mail-rs: High-Performance, Lightweight Native Email Client in Rust

**AT-mail-rs** is an ultra-fast, modern, native cross-platform desktop email client built from the ground up in **pure Rust**. It delivers a clean, responsive desktop user experience with **exceptionally low memory usage (< 45MB resident memory vs 500MB+ for web/Electron-based clients)**, instant sub-100ms cold startup time, and GPU-accelerated rendering.

---

## ✨ Key Features

- ⚡ **Ultra-Low Memory Footprint & High Performance**:
  - Built with native **Rust** and GPU-accelerated **`egui`** (`eframe`) — zero Electron, zero Chromium runtime overhead.
  - Startup time < 100ms; idle resident memory footprint < 45 MB.

- 🖥️ **True 3-Pane Resizable Layout**:
  - **Sidebar Panel**: Resizable panel (180px–320px) displaying unified Smart Views (All Inboxes, Unread, Starred, Sent, Drafts, Trash) and collapsible account folder trees.
  - **Message List Panel**: Resizable panel (260px–600px) with virtualized scrolling, sender avatar pills, unread indicators, and live multi-attribute search.
  - **Reading Pane**: Full-width reading container with responsive email header cards, rendered HTML bodies, attachments, and quick action bar (Reply, Reply All, Forward, Star, Mark Read/Unread, Delete).

- 📬 **Full IMAP & SMTP Engine**:
  - **MIME & Envelope Decoding**: Automatic RFC 2047 and MIME parsing (`mailparse`) resolving subjects, senders, recipients, and timestamps.
  - **Smart Date-Window Sync**: Configurable sync windows (Last 7 Days, 14 Days, 30 Days, 90 Days, 1 Year, or All Time).
  - **Full Offline Caching**: Full MIME messages, HTML bodies, and plain-text alternatives are stored locally in SQLite with WAL mode for 0ms latency when opening emails.
  - **Per-Folder Sync Selection**: Choose exactly which remote folders to synchronize via `⚙ Settings -> 📬 Accounts`.
  - **Safe Connection Management**: Serialized connection pool with explicit `LOGOUT` lifecycle to prevent account lockout on multi-connection mail providers (Zoho, Hostinger, Gmail, Outlook).

- ✉️ **Modern Composer & Smart Reply**:
  - **HTML Email by Default**: Default rich HTML email formatting (`🌐 HTML (Default)`) with clean MIME multipart alternative generation and plain-text fallbacks.
  - **Format Mode Switcher**: Easily toggle between HTML and Text-Only (`📝 Plain Text`) with one click in the composer toolbar.
  - **Rich Formatting Toolbar**: Quick actions for Bold, Italic, Link insertion, Bulleted lists, and Blockquotes.
  - **Text-Only Reply Action**: Dedicated `📝 Text Reply` button in the reading pane to reply in clean plain text with converted quoted context.
  - **Automatic Default Signatures**: Default signatures (account-specific or global) are automatically attached to new emails and replies, with support for rich HTML signatures, HTML sanitization, and toolbar dropdown selector.

- 🎨 **HTML Email Rendering & Inline Images**:
  - Native AST HTML sanitizer that removes conditional comments (`<!--[if ...]>`), XML declarations, and `<style>` blocks.
  - Automatic `cid:xxx` inline MIME attachment resolution with GPU image rendering (PNG, JPEG, WebP, SVG).
  - One-click attachment downloading: clicking attachments opens a native system file save dialog.

- ⚙️ **Rich Preferences & Customization**:
  - **Account Manager**: Add, edit, test connections, and delete IMAP/SMTP accounts.
  - **Signatures**: Create, edit, and assign custom default signatures with preview and automatic HTML sanitization.
  - **Quick Templates**: Manage boilerplate snippets with variable substitution (`{{name}}`, `{{sender}}`, `{{date}}`).
  - **General Telemetry**: Live SQLite WAL database stats, resident memory footprint telemetry, and system tray integration.

- 🔒 **Security & Credentials**:
  - OS-level secure storage integration using Linux Secret Service / macOS Keychain / Windows Credential Manager (`keyring-rs`). Passwords are never stored in plaintext.

- 🔔 **System Tray Integration**:
  - Integrated DBus StatusNotifierItem (`ksni`) with live unread badge counters, minimize-to-tray, and quick compose actions.

---

## 🏗️ Architecture & Workspace Structure

```
Email-Application/
├── Cargo.toml               # Workspace manifest
├── crates/
│   ├── email-core/          # Domain models, errors, event definitions (SyncCommand / SyncEvent)
│   ├── email-keychain/      # Native OS Keyring abstraction layer
│   ├── email-storage/       # SQLite storage engine with connection pooling (r2d2) and WAL mode
│   ├── email-smtp/          # Asynchronous SMTP client (lettre + tokio-rustls)
│   ├── email-sync/          # IMAP synchronization actor (async-imap + mailparse)
│   ├── email-html/          # HTML AST sanitizer, parser, and plain-text extractor
│   └── email-ui/            # Native desktop GUI (eframe / egui) with 3-pane layout and settings
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

### 2. Build the Project

```bash
# Clone repository
git clone <repo-url>
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
