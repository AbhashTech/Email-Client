# 🚀 AT-mail-rs: High-Performance, Lightweight Native Email Client in Rust

[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)
[![Memory](https://img.shields.io/badge/memory-%3C45%20MB%20RAM-brightgreen.svg)]()
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey.svg)]()
[![Wayland](https://img.shields.io/badge/display-Wayland%20%7C%20X11-purple.svg)]()
[![Security](https://img.shields.io/badge/security-PGP%20%2F%20OpenPGP-success.svg)]()

**AT-mail-rs** is an ultra-fast, modern, native cross-platform desktop email client built from the ground up in **pure Rust**. It delivers a clean, responsive desktop user experience with **exceptionally low memory usage (< 45MB resident memory vs 500MB+ for web/Electron-based clients)**, instant sub-100ms cold startup time, and GPU-accelerated rendering.

---

## ⚡ Quick Start & Automated Setup

AT-mail-rs includes an automated, cross-distro setup script (`setup.sh`) that detects your operating system, installs all necessary system dependencies, checks the Rust toolchain, and compiles the application.

```bash
# 1. Clone repository
git clone git@github.com:AbhashTech/Email-Client.git
cd Email-Application

# 2. Run automated setup & launch
./setup.sh --run
```

### 🛠️ Setup Script CLI Options (`./setup.sh`)

| Command / Option | Description |
|---|---|
| `./setup.sh` | Full automated setup: checks dependencies, verifies Rust, and builds the release binary. |
| `./setup.sh -y --install` | Unattended setup: installs system packages, builds release, and installs to `~/.local/bin` + creates desktop launcher. |
| `./setup.sh --run` | Builds the project and immediately launches AT-mail-rs. |
| `./setup.sh --deps` | Installs system distribution dependencies only (APT, Pacman, DNF, Zypper, APK, XBPS, Homebrew). |
| `./setup.sh --check` | Validates environment and Rust toolchain without installing packages or building. |
| `./setup.sh --dev` | Compiles in debug mode instead of release. |
| `./setup.sh --uninstall` | Removes installed binary and desktop entry from the user system. |

---

## ⌨️ Keyboard Shortcuts & Power Navigation

AT-mail-rs provides first-class, Vim-inspired keyboard navigation and an omnipresent Command Palette for high-efficiency workflows.

| Shortcut | Action | Scope / Context |
|---|---|---|
| <kbd>Ctrl</kbd> + <kbd>K</kbd> / <kbd>Cmd</kbd> + <kbd>K</kbd> | **Open Command Palette** (Fuzzy command launcher) | Global |
| <kbd>Ctrl</kbd> + <kbd>P</kbd> | **Open Command Palette** (Alternative hotkey) | Global |
| <kbd>Ctrl</kbd> + <kbd>,</kbd> / <kbd>Cmd</kbd> + <kbd>,</kbd> | **Open Settings & Preferences** | Global |
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
- **Async IMAP/SMTP Pipeline**: Full RFC 2047 envelope parsing (`mailparse`), configurable date-window syncing (7d, 14d, 30d, 45d, 60d, 90d, 1y, Custom Days, All Time), and safe connection pooling.
- **OS-Native Keyring Integration**: Secure credential storage via Linux Secret Service, macOS Keychain, and Windows Credential Manager (`keyring-rs`). Passwords are never stored in plaintext.

---

### 🎨 Dynamic Multi-Theme Engine & Real-Time OS Theme Detection
The entire interface dynamically adapts across light and dark color schemes, updating cards, sidebars, borders, badges, and reading surfaces in real-time.

Switch between handcrafted visual themes via `⚙ Preferences -> 🎨 Appearance` or the `Ctrl+K` Command Palette:
1. **System Auto (Follow OS)**: Automatically tracks your operating system's dark/light mode preference in real-time, displaying **Dark Slate** in dark mode and **Clean Daylight** in light mode.
2. **Gruvbox Auto (System)**: Automatically switches between **Gruvbox Retro Dark** & **Gruvbox Retro Light** based on your OS preference.
3. **Dark Slate (Default)**: Sleek charcoal surface with Google Blue accents (`#121418`).
4. **Gruvbox Retro Dark**: Warm retro groove dark palette with amber and gold accents (`#282828`).
5. **Gruvbox Retro Light**: Warm retro groove light parchment palette with ochre accents (`#fbf1c7`).
6. **Catppuccin Mocha**: Soothing pastel dark palette with lavender highlights (`#1e1e2e`).
7. **Nord Arctic**: Crisp north-bluish dark aesthetic (`#2e3440`).
8. **Solarized Dark**: Precision low-contrast cyan and warm-green dark palette (`#002b36`).
9. **OLED Pure Black**: True `#000000` pitch-black mode for OLED displays and maximum battery savings.
10. **Clean Daylight**: Modern, crisp daylight theme for well-lit environments (`#f5f7fa`).
11. **🎨 Custom Theme Creator**: Full interactive color pickers for App Background, Sidebar, Reading Pane, Cards, Accents, Borders, and Text. Custom themes are saved as standalone JSON files in the OS config folder (`~/.config/at-mail-rs/themes/<theme_name>.json`) and can be exported, imported, and applied on the fly.

#### 🌓 Real-Time OS Appearance Integration:
- **Linux (Wayland & X11)**: Queries the **XDG Desktop Portal** and GNOME `gsettings get org.gnome.desktop.interface color-scheme` (`'prefer-dark'` vs `'prefer-light'`), with fallback to `$GTK_THEME`.
- **macOS**: Queries Apple interface style (`defaults read -g AppleInterfaceStyle`).
- **Windows**: Checks the user personalize registry setting `AppsUseLightTheme`.
- **GPU Render Loop**: Automatically hot-reloads styling and repaints the GPU frame buffer without requiring an application restart.

---

### 🔍 SQLite FTS5 Full-Text Offline Search & Smart Tokens
- **🚀 Instant Full-Text FTS5 Engine**: Embedded SQLite `FTS5` virtual table with **BM25 relevance ranking** indexing subjects, recipient addresses, snippet previews, and full email body contents offline for instant sub-millisecond search across hundreds of thousands of emails.
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

### 💤 Snooze & Remind Later
- **Convenient Presets (`[💤 Snooze ▾]`)**: Temporarily hide incoming emails from your inbox until a chosen time:
  - `⏰ Later Today (+3 hours)`
  - `🌅 Tomorrow Morning (9:00 AM)`
  - `🌆 Tomorrow Evening (6:00 PM)`
  - `📅 Next Week (Monday 9:00 AM)`
- **`💤 Snoozed` Virtual Mailbox**: Dedicated smart view in the left sidebar to browse and manage all active snoozed emails.
- **Visual Snooze Warning Banner**: Informative indicator on snoozed emails with a 1-click `[Unsnooze]` action to immediately restore emails to your Inbox.
- **Automated Background Dispatcher**: Background queue engine monitors due timestamps and automatically restores emails to your Inbox with a live toast notification.

---

### 💬 Conversation Threading View & Per-Message Toolbars
- **Smart Conversation Hierarchy**: Resolves and groups related email exchanges using `In-Reply-To`, `Message-ID`, and cleaned subject roots into unified chronological threads.
- **Conversational Bubble Timeline**: Displays multi-message discussions as interactive bubble cards with sender avatar badges, timestamps, recipients, and connector lines.
- **Full-Card Clickable Accordion**: The entire header area of any thread card is interactive with hover feedback and a pointing hand cursor, allowing 1-click toggling anywhere to expand or collapse.
- **Crisp Custom Vector Chevrons**: Features custom-rendered vector chevrons (`⌃` for Collapse, `⌄` for Expand) in styled pill badges with theme-aware accent colors.
- **Per-Message Action Toolbar**: Every individual message in a thread provides its own complete inline action bar (`[↩ Reply]`, `[📝 Text]`, `[👥 Reply All]`, `[➡ Forward]`, `[✉ Read/Unread]`, `[💤 Snooze]`, `[📁 Move]`, `[📤 Export]`, `[🌐 Web View]`, `[↗ Browser]`, `[🗑 Delete]`) targeting that specific email in the thread.

---

### 🔒 End-to-End Encryption & Cryptographic Signatures (PGP / OpenPGP)
- **RSA-2048 & AES-256-GCM Cryptography**: Complete zero-knowledge cryptographic pipeline for message privacy and authenticity.
- **Armored Keypair Generator & Keychain**: Generate 2048-bit PGP keypairs directly within Preferences with SHA-256 fingerprinting, clipboard export, and `.asc` export.
- **Recipient Public Key Address Book**: Import and manage contact public keys with instant validation and SQLite persistence.
- **One-Click Composer Toggles**: `[🔒 Encrypt (PGP)]` and `[✍ Sign (PGP)]` checkboxes in composer toolbar for seamless message encryption and RSA cleartext signing.
- **Automatic Security Badges & Decryption**: Incoming PGP messages and signatures display dedicated security alert banners in the reading pane with automatic key resolution and integrity verification.

---

### 📝 Composer, Drafts, Scheduled Send & Undo Send
- **🪟 Resizable & Dynamic Height Composer**: Fully resizable modal window with an expanding multiline editor that dynamically scales and grows with your window height.
- **🖋️ Visible Attached Signature Card & Live Preview**: Displays the currently attached signature name with a live preview of the signature text below the editor. Includes 1-click controls to change signature or detach it from the email on the fly.
- **💬 Quoted Original Message Editor (Replying & Forwarding)**: When replying or forwarding, the previous email is displayed in a dedicated quoted message card with a `[✓ Include in reply]` checkbox, `[▲ Hide / ▼ Show Quote]` toggle, and full inline editing capability to trim or edit the quote before sending.
- **💾 Save as Draft (`💾 Save Draft`)**: Persist in-progress emails (recipients, format, subject, body, signature) to local SQLite storage. Re-open and edit drafts directly from the reading pane (`[✏ Edit Draft]`).
- **⏰ Send Later / Scheduled Outbox (`⏰ Send Later ▾`)**: Schedule delivery with convenient presets (`In 15m`, `In 1h`, `In 3h`, `Tomorrow 9 AM`, `Tomorrow 6 PM`) or custom date/time. The background dispatcher automatically transmits emails when due via SMTP.
- **⚡ Markdown Compose Mode with Live Preview**: Side-by-side split screen for drafting emails in Markdown with real-time rendered HTML preview. Includes formatting action buttons for Bold, Italic, Headings, Lists, Blockquotes, and Code Blocks.
- **🌐 HTML & Plain Text Modes**: Full support for HTML rich text and Text-Only drafting modes.
- **📎 Attachment Drag-and-Drop (`[📎 Attach]`)**: Drag files directly from your OS file manager into the composer window or use the file picker. Attached files display interactive chip pills with filenames, formatted sizes (KB/MB), and 1-click removal.
- **📦 MIME Multipart Encoding**: Automatic base64 attachment serialization and RFC 2046 `multipart/mixed` structure generation with automatic MIME type discovery.
- **↩ 5-Second Undo Send**: Outgoing emails enter a 5-second safety buffer with a floating countdown bar (`[ ↩ Undo Send ]` / `[ ⚡ Send Now ]`). Clicking Undo immediately aborts transmission and restores the full draft into the composer.
- **🖋️ Default & Custom Signatures (Create & Edit)**: Edit and manage account-specific or global signatures with HTML sanitization.
- **📋 Quick Templates & Snippets (Create & Edit)**: Edit and manage reusable response templates with quick shortcut triggers (e.g. `/meeting`, `/followup`).

---

### 📤 Offline Outbox Auto-Retry Queue
- **Resilient Offline Sending**: If network connectivity drops or SMTP transmission fails during immediate or scheduled dispatch, emails are automatically safely queued into the offline Outbox.
- **Exponential Backoff Dispatcher**: Background worker periodically polls the queue and re-attempts delivery with increasing backoff intervals (`30s` -> `1m` -> `2m` -> `5m` -> `15m` -> `30m`).
- **`📤 Outbox (Retry)` Smart View**: Dedicated mailbox in the sidebar and top toolbar badge showing queued items, retry counts, timestamps, and error diagnostics.
- **Auto-Recovery on Reconnect**: Seamlessly transmits all pending messages as soon as internet connection is restored with zero data loss.

---

### 🪟 Window Controls, Wayland/Hyprland & System Tray Integration
- **Crisp Titlebar Controls**: Font-safe Minimize (`[−]`), Maximize/Restore (`[◻ / ⧉]`), and Close (`[×]`) buttons built directly into the top navigation bar.
- **Wayland / Tiling WM Support**: Fully compatible with Wayland compositors (Hyprland, Sway, GNOME, KDE) with opaque frame buffers and clean surface teardown on exit.
- **System Tray (StatusNotifierItem DBus)**:
  - Click tray icon to toggle application visibility.
  - Context menu actions: `Show/Hide Window`, `✉ Compose Email`, `🔄 Sync All Mail`, and `Quit`.
- **Customizable Close Action (`Settings -> General & Storage`)**: Choose between *Quit Application Completely* (recommended for Wayland & Tiling WMs) and *Minimize to System Tray* to keep background sync running.
- **Zero GUI Thread Blocking**: Queue counters and background checks are cached and throttled to eliminate SQLite lock contention and ensure 60+ FPS responsiveness.

---

### 🛡️ Privacy Shield & Security
- **🛡️ Remote Image Blocker**: Automatically suppresses external `http://` / `https://` tracking pixels and remote images to safeguard IP privacy and prevent email tracking. Users can load images on demand with one click (`[ 🖼 Load Images ]`).
- **⚠️ Anti-Phishing Link Safety Detector**: Identifies deceptive hyperlinks where the visible display text (e.g. `paypal.com`) points to a different destination domain (`evil-site.com`). Highlights suspicious links with warning badges (`⚠️ [Deceptive Link!]`) and detailed tooltips.

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
├── setup.sh                 # Cross-platform automated setup & installer
├── crates/
│   ├── email-core/          # Domain models, errors, event definitions (SyncCommand / SyncEvent), PGP crypto
│   ├── email-keychain/      # Native OS Keyring abstraction layer (Secret Service / Keychain / Credential Manager)
│   ├── email-storage/       # SQLite storage engine with connection pooling (r2d2), WAL mode, FTS5 search, and schema migrations
│   ├── email-smtp/          # Asynchronous SMTP client (lettre + tokio-rustls)
│   ├── email-sync/          # IMAP synchronization actor (async-imap + mailparse) with IDLE and background worker
│   ├── email-html/          # HTML AST sanitizer, Markdown compiler, phishing detector, and plain-text extractor
│   └── email-ui/            # Native desktop GUI (eframe / egui / wry) with 3-pane layout, themes, command palette, and settings
```

---

## 🛠️ Manual Build & System Prerequisites

If you prefer to install dependencies and build manually without `setup.sh`:

### 1. Distribution Package Managers

#### **Ubuntu / Debian / Linux Mint / Pop!_OS**
```bash
sudo apt update
sudo apt install -y \
  build-essential \
  pkg-config \
  cmake \
  libssl-dev \
  libdbus-1-dev \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libxkbcommon-dev \
  libwayland-dev \
  libx11-dev \
  libxcb-render0-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libgl1-mesa-dev
```

#### **Arch Linux / Manjaro / EndeavourOS**
```bash
sudo pacman -Syu --needed \
  base-devel \
  pkgconf \
  cmake \
  openssl \
  dbus \
  gtk3 \
  webkit2gtk-4.1 \
  libxkbcommon \
  wayland \
  libx11 \
  libxcb \
  mesa \
  vulkan-icd-loader
```

#### **Fedora / RHEL 9+ / Rocky Linux / AlmaLinux / CentOS Stream**
```bash
sudo dnf check-update
sudo dnf install -y \
  gcc \
  gcc-c++ \
  make \
  pkgconf-pkg-config \
  cmake \
  openssl-devel \
  dbus-devel \
  gtk3-devel \
  webkit2gtk4.1-devel \
  libxkbcommon-devel \
  wayland-devel \
  libX11-devel \
  libxcb-devel \
  mesa-libGL-devel \
  vulkan-loader-devel
```

#### **openSUSE (Tumbleweed & Leap)**
```bash
sudo zypper refresh
sudo zypper install -y \
  patterns-devel-base-devel_basis \
  pkg-config \
  cmake \
  libopenssl-devel \
  dbus-1-devel \
  gtk3-devel \
  webkit2gtk3-devel \
  libxkbcommon-devel \
  wayland-devel \
  libX11-devel \
  libxcb-devel \
  Mesa-libGL-devel
```

#### **Alpine Linux**
```bash
sudo apk update
sudo apk add \
  build-base \
  pkgconf \
  cmake \
  openssl-dev \
  dbus-dev \
  gtk+3.0-dev \
  webkit2gtk-4.1-dev \
  libxkbcommon-dev \
  wayland-dev \
  libx11-dev \
  libxcb-dev \
  mesa-dev
```

#### **Void Linux**
```bash
sudo xbps-install -Syu \
  base-devel \
  pkg-config \
  cmake \
  openssl-devel \
  dbus-devel \
  gtk+3-devel \
  webkit2gtk-4.1-devel \
  libxkbcommon-devel \
  wayland-devel \
  libX11-devel \
  libxcb-devel \
  MesaLib-devel
```

#### **macOS (Homebrew)**
```bash
brew install pkg-config cmake openssl dbus
```

---

### 2. Compile & Run

```bash
# Check workspace compilation
cargo check --workspace

# Run automated tests
cargo test --workspace

# Build optimized release binary
cargo build --release

# Run client
cargo run --release --bin email-ui
```

The compiled binary will be located at:
`target/release/email-ui`

---

## 🚀 First-Time Account Setup

1. Launch the application (`./setup.sh --run` or `cargo run --release --bin email-ui`).
2. Click **`+ Add Account`** in the sidebar or navigate to `⚙ Settings -> 📬 Accounts`.
3. Select your provider preset (e.g. Gmail, Outlook, Yahoo, or Custom IMAP/SMTP).
4. Enter your Display Name, Email Address, and App-Specific Password / Token.
5. Click **`⚡ Test Connection`** to verify server connectivity.
6. Click **`💾 Save & Sync`**.
7. Click **`🔄 Sync All`** to synchronize your mailbox!

---

## 📄 License

Dual-licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
