#!/usr/bin/env bash
# ==============================================================================
# 🚀 AT-mail-rs: Automated Setup, Dependency Installer & Build Script
# ==============================================================================
# Supports: Ubuntu, Debian, Pop!_OS, Linux Mint, Arch Linux, Manjaro,
#           EndeavourOS, Fedora, RHEL, Rocky Linux, openSUSE, Alpine, Void,
#           NixOS, Gentoo, and macOS (Homebrew).
# ==============================================================================

set -euo pipefail

# --- Color Formatting ---
BOLD='\033[1m'
DIM='\033[2m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
RESET='\033[0m'

# --- Project Paths ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="email-ui"
APP_NAME="AT-mail-rs"
DESKTOP_ENTRY_NAME="at-mail-rs.desktop"
INSTALL_PREFIX="${HOME}/.local"
BIN_INSTALL_DIR="${INSTALL_PREFIX}/bin"
DESKTOP_INSTALL_DIR="${HOME}/.local/share/applications"
ICON_INSTALL_DIR="${HOME}/.local/share/icons/hicolor/scalable/apps"

# --- Script Flags ---
FLAG_INSTALL_DEPS=false
FLAG_BUILD_RELEASE=true
FLAG_RUN=false
FLAG_INSTALL_DESKTOP=false
FLAG_UNINSTALL=false
FLAG_CHECK_ONLY=false
FLAG_DEV=false
FLAG_YES=false

# --- Helper Functions ---
log_info() {
    echo -e "${BLUE}${BOLD}[INFO]${RESET} $*"
}

log_success() {
    echo -e "${GREEN}${BOLD}[SUCCESS]${RESET} $*"
}

log_warn() {
    echo -e "${YELLOW}${BOLD}[WARNING]${RESET} $*"
}

log_error() {
    echo -e "${RED}${BOLD}[ERROR]${RESET} $*"
}

log_step() {
    echo -e "\n${CYAN}${BOLD}==>${RESET} ${BOLD}$*${RESET}"
}

print_banner() {
    echo -e "${MAGENTA}${BOLD}"
    cat << "EOF"
   ___ _____       __  __       _ _                    
  / _ \_   \     |  \/  |     (_) |      _ __ ___     
 / /_\ \/ /\_____| |\/| |_____| | |_____| '__/ __|    
/ /_\\/ /  |_____| |  | |_____| | |_____| |  \__ \    
\____/\/         |_|  |_|     |_|_|     |_|  |___/    
EOF
    echo -e "${RESET}${CYAN}  High-Performance, Lightweight Native Rust Email Client${RESET}"
    echo -e "${DIM}  Zero Electron • Pure Rust • GPU Accelerated • <45MB RAM${RESET}\n"
}

print_usage() {
    cat << EOF
Usage: $(basename "$0") [OPTIONS]

Options:
  -h, --help           Show this help message and exit
  -y, --yes            Automatic yes to dependency installation prompts
  --deps               Install system dependencies only and exit
  --build              Compile release binary (target/release/email-ui)
  --dev                Compile in debug mode instead of release
  --run                Build and immediately launch AT-mail-rs
  --install            Build and install binary to ~/.local/bin + create desktop entry
  --uninstall          Remove installed binary and desktop entry from user system
  --check              Check if required dependencies and Rust toolchain are present

Examples:
  ./setup.sh                 # Full automated setup (deps, build)
  ./setup.sh -y --install    # Unattended setup, build release & install desktop app
  ./setup.sh --run           # Build and run immediately
  ./setup.sh --deps          # Only install system distribution dependencies
EOF
}

# --- Parse Command Line Arguments ---
parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help)
                print_usage
                exit 0
                ;;
            -y|--yes)
                FLAG_YES=true
                shift
                ;;
            --deps)
                FLAG_INSTALL_DEPS=true
                FLAG_BUILD_RELEASE=false
                shift
                ;;
            --build)
                FLAG_BUILD_RELEASE=true
                shift
                ;;
            --dev)
                FLAG_DEV=true
                shift
                ;;
            --run)
                FLAG_RUN=true
                FLAG_BUILD_RELEASE=true
                shift
                ;;
            --install)
                FLAG_INSTALL_DESKTOP=true
                FLAG_BUILD_RELEASE=true
                shift
                ;;
            --uninstall)
                FLAG_UNINSTALL=true
                shift
                ;;
            --check)
                FLAG_CHECK_ONLY=true
                shift
                ;;
            *)
                log_error "Unknown option: $1"
                print_usage
                exit 1
                ;;
        esac
    done
}

# --- Detect OS & Distribution ---
detect_os() {
    OS="unknown"
    DISTRO="unknown"

    if [[ "$OSTYPE" == "darwin"* ]]; then
        OS="macos"
        DISTRO="macos"
    elif [[ "$OSTYPE" == "linux-gnu"* ]] || [[ -f /etc/os-release ]]; then
        OS="linux"
        if [[ -f /etc/os-release ]]; then
            # Source os-release
            # shellcheck disable=SC1091
            source /etc/os-release
            DISTRO="${ID:-unknown}"
            DISTRO_LIKE="${ID_LIKE:-}"
        fi
    fi

    log_info "Detected Platform: ${BOLD}${OS}${RESET} (Distribution/Family: ${BOLD}${DISTRO}${RESET})"
}

# --- Ask Confirmation ---
ask_confirm() {
    local prompt="$1"
    if [[ "$FLAG_YES" == true ]]; then
        return 0
    fi
    echo -en "${YELLOW}${prompt} [Y/n]: ${RESET}"
    read -r response
    case "$response" in
        [yY][eE][sS]|[yY]|"")
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

# --- Check & Install System Dependencies ---
install_dependencies() {
    log_step "Checking & Installing System Dependencies..."

    if [[ "$OS" == "macos" ]]; then
        if ! command -v brew &> /dev/null; then
            log_error "Homebrew is required on macOS. Please install Homebrew from https://brew.sh/"
            exit 1
        fi
        log_info "Installing macOS dependencies with Homebrew..."
        brew install pkg-config cmake openssl dbus
        return 0
    fi

    local install_cmd=""
    local packages=()

    case "$DISTRO" in
        ubuntu|debian|pop|mint|elementary|zorin|kali)
            install_cmd="sudo apt-get update && sudo apt-get install -y"
            packages=(
                build-essential
                pkg-config
                cmake
                libssl-dev
                libdbus-1-dev
                libgtk-3-dev
                libwebkit2gtk-4.1-dev
                libxkbcommon-dev
                libwayland-dev
                libx11-dev
                libxcb-render0-dev
                libxcb-shape0-dev
                libxcb-xfixes0-dev
                libgl1-mesa-dev
            )
            ;;

        arch|manjaro|endeavouros|artix|garuda)
            install_cmd="sudo pacman -Syu --needed --noconfirm"
            packages=(
                base-devel
                pkgconf
                cmake
                openssl
                dbus
                gtk3
                webkit2gtk-4.1
                libxkbcommon
                wayland
                libx11
                libxcb
                mesa
                vulkan-icd-loader
            )
            ;;

        fedora|rhel|rocky|almalinux|centos)
            install_cmd="sudo dnf install -y"
            packages=(
                gcc
                gcc-c++
                make
                pkgconf-pkg-config
                cmake
                openssl-devel
                dbus-devel
                gtk3-devel
                webkit2gtk4.1-devel
                libxkbcommon-devel
                wayland-devel
                libX11-devel
                libxcb-devel
                mesa-libGL-devel
                vulkan-loader-devel
            )
            ;;

        opensuse*|suse|sles)
            install_cmd="sudo zypper install -y"
            packages=(
                patterns-devel-base-devel_basis
                pkg-config
                cmake
                libopenssl-devel
                dbus-1-devel
                gtk3-devel
                webkit2gtk3-devel
                libxkbcommon-devel
                wayland-devel
                libX11-devel
                libxcb-devel
                Mesa-libGL-devel
            )
            ;;

        alpine)
            install_cmd="sudo apk add"
            packages=(
                build-base
                pkgconf
                cmake
                openssl-dev
                dbus-dev
                gtk+3.0-dev
                webkit2gtk-4.1-dev
                libxkbcommon-dev
                wayland-dev
                libx11-dev
                libxcb-dev
                mesa-dev
            )
            ;;

        void)
            install_cmd="sudo xbps-install -Sy"
            packages=(
                base-devel
                pkg-config
                cmake
                openssl-devel
                dbus-devel
                gtk+3-devel
                webkit2gtk-4.1-devel
                libxkbcommon-devel
                wayland-devel
                libX11-devel
                libxcb-devel
                MesaLib-devel
            )
            ;;

        gentoo)
            install_cmd="sudo emerge --ask=n --noreplace"
            packages=(
                sys-devel/base-devel
                dev-util/pkgconf
                dev-build/cmake
                dev-libs/openssl
                sys-apps/dbus
                x11-libs/gtk+:3
                net-libs/webkit-gtk:4.1
                x11-libs/libxkbcommon
                dev-libs/wayland
                x11-libs/libX11
                x11-libs/libxcb
                media-libs/mesa
            )
            ;;

        nixos)
            log_info "NixOS detected. Use the provided flake.nix / shell.nix or nix-shell."
            return 0
            ;;

        *)
            # Fallback to ID_LIKE checks
            if [[ "${DISTRO_LIKE:-}" == *"debian"* ]] || [[ "${DISTRO_LIKE:-}" == *"ubuntu"* ]]; then
                install_cmd="sudo apt-get update && sudo apt-get install -y"
                packages=(build-essential pkg-config cmake libssl-dev libdbus-1-dev libgtk-3-dev libwebkit2gtk-4.1-dev libxkbcommon-dev libwayland-dev libx11-dev libgl1-mesa-dev)
            elif [[ "${DISTRO_LIKE:-}" == *"arch"* ]]; then
                install_cmd="sudo pacman -Syu --needed --noconfirm"
                packages=(base-devel pkgconf cmake openssl dbus gtk3 webkit2gtk-4.1 libxkbcommon wayland libx11 libxcb mesa)
            elif [[ "${DISTRO_LIKE:-}" == *"fedora"* ]] || [[ "${DISTRO_LIKE:-}" == *"rhel"* ]]; then
                install_cmd="sudo dnf install -y"
                packages=(gcc gcc-c++ make pkgconf-pkg-config cmake openssl-devel dbus-devel gtk3-devel webkit2gtk4.1-devel libxkbcommon-devel wayland-devel libX11-devel mesa-libGL-devel)
            else
                log_warn "Unrecognized Linux distribution '${DISTRO}'."
                log_warn "Please ensure you have: C/C++ compiler, cmake, pkg-config, openssl, dbus, gtk3, webkit2gtk-4.1, libxkbcommon, wayland, and mesa libraries installed."
                return 0
            fi
            ;;
    esac

    if ask_confirm "Proceed to install system dependencies via '${install_cmd}'?"; then
        log_info "Executing: ${install_cmd} ${packages[*]}"
        eval "${install_cmd} ${packages[*]}"
        log_success "System dependencies installed successfully."
    else
        log_warn "Skipped system dependency installation. Build may fail if packages are missing."
    fi
}

# --- Check & Install Rust Toolchain ---
check_rust() {
    log_step "Checking Rust Toolchain..."

    if command -v cargo &> /dev/null && command -v rustc &> /dev/null; then
        local rust_ver
        rust_ver="$(rustc --version)"
        log_success "Rust is installed: ${BOLD}${rust_ver}${RESET}"
    else
        log_warn "Rust toolchain was not found in PATH."
        if ask_confirm "Would you like to install Rust automatically via rustup.rs?"; then
            log_info "Installing Rust via official rustup script..."
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            # shellcheck disable=SC1090
            source "$HOME/.cargo/env"
            log_success "Rust installed successfully: $(rustc --version)"
        else
            log_error "Rust is required to build ${APP_NAME}. Please install Rust from https://rustup.rs"
            exit 1
        fi
    fi
}

# --- Build Application ---
build_project() {
    log_step "Building ${APP_NAME}..."
    cd "$SCRIPT_DIR"

    local build_mode="--release"
    local target_dir="target/release"

    if [[ "$FLAG_DEV" == true ]]; then
        build_mode=""
        target_dir="target/debug"
        log_info "Building in ${BOLD}DEBUG${RESET} mode..."
    else
        log_info "Building in ${BOLD}OPTIMIZED RELEASE${RESET} mode..."
    fi

    if [[ -n "$build_mode" ]]; then
        cargo build --release --bin "$BIN_NAME"
    else
        cargo build --bin "$BIN_NAME"
    fi

    local binary_path="${SCRIPT_DIR}/${target_dir}/${BIN_NAME}"
    if [[ -f "$binary_path" ]]; then
        log_success "Build complete! Binary located at: ${BOLD}${binary_path}${RESET}"
    else
        log_error "Build completed but binary was not found at ${binary_path}"
        exit 1
    fi
}

# --- Desktop Entry & Icon Installation ---
install_desktop_entry() {
    log_step "Installing Desktop Integration & Binary..."

    local target_binary="${SCRIPT_DIR}/target/release/${BIN_NAME}"
    if [[ "$FLAG_DEV" == true ]]; then
        target_binary="${SCRIPT_DIR}/target/debug/${BIN_NAME}"
    fi

    if [[ ! -f "$target_binary" ]]; then
        log_error "Compiled binary not found at ${target_binary}. Building first..."
        build_project
    fi

    # Create destination directories
    mkdir -p "$BIN_INSTALL_DIR"
    mkdir -p "$DESKTOP_INSTALL_DIR"
    mkdir -p "$ICON_INSTALL_DIR"

    # Install binary
    log_info "Copying binary to ${BIN_INSTALL_DIR}/${BIN_NAME}..."
    cp -f "$target_binary" "${BIN_INSTALL_DIR}/${BIN_NAME}"
    chmod +x "${BIN_INSTALL_DIR}/${BIN_NAME}"

    # Generate vector SVG app icon if none exists
    local icon_path="${ICON_INSTALL_DIR}/at-mail-rs.svg"
    cat << "EOF" > "$icon_path"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128" width="128" height="128">
  <defs>
    <linearGradient id="grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" style="stop-color:#4285f4;stop-opacity:1" />
      <stop offset="100%" style="stop-color:#1a73e8;stop-opacity:1" />
    </linearGradient>
    <filter id="shadow" x="-20%" y="-20%" width="140%" height="140%">
      <feDropShadow dx="0" dy="4" stdDeviation="6" flood-opacity="0.3"/>
    </filter>
  </defs>
  <rect x="8" y="8" width="112" height="112" rx="24" fill="url(#grad)" filter="url(#shadow)"/>
  <path d="M24 38 L64 68 L104 38" fill="none" stroke="#ffffff" stroke-width="8" stroke-linecap="round" stroke-linejoin="round"/>
  <rect x="24" y="38" width="80" height="54" rx="8" fill="none" stroke="#ffffff" stroke-width="7"/>
  <circle cx="94" cy="42" r="7" fill="#fb923c"/>
</svg>
EOF

    # Generate .desktop file
    local desktop_path="${DESKTOP_INSTALL_DIR}/${DESKTOP_ENTRY_NAME}"
    cat << EOF > "$desktop_path"
[Desktop Entry]
Name=AT-mail-rs
GenericName=Email Client
Comment=High-Performance, Lightweight Native Rust Desktop Email Client
Exec=${BIN_INSTALL_DIR}/${BIN_NAME} %u
Icon=at-mail-rs
Terminal=false
Type=Application
Categories=Network;Email;Office;
MimeType=x-scheme-handler/mailto;message/rfc822;
Keywords=email;mail;imap;smtp;rust;pgp;fast;client;
StartupNotify=true
StartupWMClass=email-ui
EOF

    chmod +x "$desktop_path"

    # Update desktop database if tool is available
    if command -v update-desktop-database &> /dev/null; then
        update-desktop-database "$DESKTOP_INSTALL_DIR" 2>/dev/null || true
    fi

    log_success "Installed successfully!"
    echo -e "  • Executable:     ${BOLD}${BIN_INSTALL_DIR}/${BIN_NAME}${RESET}"
    echo -e "  • Desktop Entry:  ${BOLD}${desktop_path}${RESET}"
    echo -e "  • App Icon:       ${BOLD}${icon_path}${RESET}"
    echo -e "\n${GREEN}Make sure '${BIN_INSTALL_DIR}' is in your \$PATH.${RESET}"
}

# --- Uninstall Desktop Integration ---
uninstall() {
    log_step "Uninstalling ${APP_NAME}..."

    rm -f "${BIN_INSTALL_DIR}/${BIN_NAME}"
    rm -f "${DESKTOP_INSTALL_DIR}/${DESKTOP_ENTRY_NAME}"
    rm -f "${ICON_INSTALL_DIR}/at-mail-rs.svg"

    if command -v update-desktop-database &> /dev/null; then
        update-desktop-database "$DESKTOP_INSTALL_DIR" 2>/dev/null || true
    fi

    log_success "${APP_NAME} has been removed from ${INSTALL_PREFIX}."
}

# --- Launch Application ---
run_application() {
    log_step "Launching ${APP_NAME}..."
    cd "$SCRIPT_DIR"
    if [[ "$FLAG_DEV" == true ]]; then
        cargo run --bin "$BIN_NAME"
    else
        cargo run --release --bin "$BIN_NAME"
    fi
}

# --- Main Execution Flow ---
main() {
    print_banner
    parse_args "$@"

    if [[ "$FLAG_UNINSTALL" == true ]]; then
        uninstall
        exit 0
    fi

    detect_os

    if [[ "$FLAG_CHECK_ONLY" == true ]]; then
        check_rust
        log_success "Check completed."
        exit 0
    fi

    if [[ "$FLAG_INSTALL_DEPS" == true ]]; then
        install_dependencies
        exit 0
    fi

    # Default flow: install deps if missing/needed, check Rust, then build
    install_dependencies
    check_rust

    if [[ "$FLAG_BUILD_RELEASE" == true ]]; then
        build_project
    fi

    if [[ "$FLAG_INSTALL_DESKTOP" == true ]]; then
        install_desktop_entry
    fi

    if [[ "$FLAG_RUN" == true ]]; then
        run_application
    else
        echo -e "\n${GREEN}${BOLD}🎉 Setup & Build Completed Successfully!${RESET}"
        echo -e "\nTo run ${APP_NAME}, execute:"
        echo -e "  ${CYAN}./setup.sh --run${RESET}   ${DIM}# or: cargo run --release --bin email-ui${RESET}\n"
    fi
}

main "$@"
