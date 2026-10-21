#!/usr/bin/env bash
# ==============================================================================
# Open AG Patcher (Rust Edition) - Universal Installer for Linux & macOS
# Installs 'open-ag-patcher' and creates 'agyptch' command in system PATH.
# ==============================================================================

set -e

# --- Environment Preparation (Termux / Android / Restricted environments) ---
if [ -z "$TMPDIR" ] || [ ! -d "$TMPDIR" ]; then
    if [ -n "$PREFIX" ] && [ -d "$PREFIX/tmp" ]; then
        export TMPDIR="$PREFIX/tmp"
    elif [ -d "/data/data/com.termux/files/usr/tmp" ]; then
        export TMPDIR="/data/data/com.termux/files/usr/tmp"
    fi
fi

# --- Terminal Styling (Google Material Colors) ---
if [ -t 1 ]; then
    COLOR_RESET="\033[0m"
    COLOR_BOLD="\033[1m"
    COLOR_DIM="\033[2m"
    COLOR_BLUE="\033[38;2;66;133;244m"     # Google Blue
    COLOR_GREEN="\033[38;2;52;168;83m"     # Google Green
    COLOR_YELLOW="\033[38;2;251;188;5m"    # Google Yellow
    COLOR_RED="\033[38;2;234;67;53m"       # Google Red
    COLOR_GRAY="\033[38;2;154;160;166m"    # Material Gray
    COLOR_SURFACE="\033[38;2;248;249;250m" # Off-white
else
    COLOR_RESET=""
    COLOR_BOLD=""
    COLOR_DIM=""
    COLOR_BLUE=""
    COLOR_GREEN=""
    COLOR_YELLOW=""
    COLOR_RED=""
    COLOR_GRAY=""
    COLOR_SURFACE=""
fi

# --- Helper: Animated Spinner ---
run_animated() {
    local label="$1"
    shift
    local cmd=("$@")

    local spin_chars=('⠋' '⠙' '⠹' '⠸' '⠼' '⠴' '⠦' '⠧' '⠇' '⠏')
    local pid_file
    pid_file=$(mktemp)
    local err_file
    err_file=$(mktemp)

    ( "${cmd[@]}" ) > "$err_file" 2>&1 &
    local cmd_pid=$!

    local i=0
    while kill -0 "$cmd_pid" 2>/dev/null; do
        local spin_char="${spin_chars[i % ${#spin_chars[@]}]}"
        printf "\r  ${COLOR_BLUE}%s${COLOR_RESET}  %-48s" "$spin_char" "$label"
        i=$((i + 1))
        sleep 0.05
    done

    wait "$cmd_pid"
    local exit_code=$?

    if [ $exit_code -eq 0 ]; then
        printf "\r  ${COLOR_GREEN}✓${COLOR_RESET}  ${COLOR_BOLD}%-48s${COLOR_RESET} ${COLOR_GRAY}[Готово]${COLOR_RESET}\n" "$label"
        rm -f "$pid_file" "$err_file"
        return 0
    else
        printf "\r  ${COLOR_RED}✗${COLOR_RESET}  ${COLOR_BOLD}%-48s${COLOR_RESET} ${COLOR_RED}[Ошибка]${COLOR_RESET}\n" "$label"
        if [ -s "$err_file" ]; then
            printf "${COLOR_DIM}%s${COLOR_RESET}\n" "$(cat "$err_file")"
        fi
        rm -f "$pid_file" "$err_file"
        return $exit_code
    fi
}

# --- Header Banner ---
echo ""
echo -e "${COLOR_BLUE}┌────────────────────────────────────────────────────────────┐${COLOR_RESET}"
echo -e "${COLOR_BLUE}│${COLOR_RESET}   ${COLOR_BOLD}${COLOR_SURFACE}Open AG Patcher (Rust Edition)${COLOR_RESET}                           ${COLOR_BLUE}│${COLOR_RESET}"
echo -e "${COLOR_BLUE}│${COLOR_RESET}   ${COLOR_DIM}Универсальный установщик бинарника и команды 'agyptch'${COLOR_RESET}   ${COLOR_BLUE}│${COLOR_RESET}"
echo -e "${COLOR_BLUE}└────────────────────────────────────────────────────────────┘${COLOR_RESET}"
echo ""

# --- Step 1: Detect Platform, Architecture and Atomic/Immutable OS ---
OS_TYPE="$(uname -s)"
ARCH_TYPE="$(uname -m)"

case "$OS_TYPE" in
    Linux*)     PLATFORM="linux" ;;
    Darwin*)    PLATFORM="macos" ;;
    *)          PLATFORM="unknown" ;;
esac

case "$ARCH_TYPE" in
    x86_64*|amd64*) ARCH="x86_64" ;;
    aarch64*|arm64*) ARCH="arm64" ;;
    *)               ARCH="unknown" ;;
esac

if [ "$PLATFORM" = "unknown" ] || [ "$ARCH" = "unknown" ]; then
    echo -e "  ${COLOR_RED}✗${COLOR_RESET} Неподдерживаемая архитектура: ${OS_TYPE} / ${ARCH_TYPE}"
    exit 1
fi

# Detect Termux on Android
IS_TERMUX=0
if [ -n "$TERMUX_VERSION" ] || [ -d "/data/data/com.termux" ] || [ "$(uname -o 2>/dev/null)" = "Android" ]; then
    IS_TERMUX=1
elif [ -n "$PREFIX" ] && [ -d "$PREFIX/bin" ] && [[ "$PREFIX" == *"com.termux"* ]]; then
    IS_TERMUX=1
fi

# Detect atomic / immutable Linux (Bazzite, Fedora Silverblue, SteamOS, openSUSE MicroOS, NixOS)
IS_ATOMIC=0
ATOMIC_NAME=""
if [ "$PLATFORM" = "linux" ] && [ "$IS_TERMUX" -eq 0 ]; then
    if [ -f /run/ostree-booted ] || [ -d /sysroot/ostree ]; then
        IS_ATOMIC=1
        ATOMIC_NAME="ostree"
    fi
    if [ -f /etc/os-release ]; then
        if grep -q -E -i "(bazzite|silverblue|kinoite|steamos|microos|nixos|vanilla)" /etc/os-release 2>/dev/null; then
            IS_ATOMIC=1
            ATOMIC_NAME="$(grep -E -i "^NAME=" /etc/os-release | head -n1 | cut -d= -f2 | tr -d '"')"
        fi
    fi
fi

if [ "$IS_TERMUX" -eq 1 ]; then
    echo -e "  ${COLOR_GREEN}✓${COLOR_RESET}  Система определена: ${COLOR_BOLD}Android / Termux${COLOR_RESET} (${ARCH})"
elif [ "$IS_ATOMIC" -eq 1 ] && [ -n "$ATOMIC_NAME" ]; then
    echo -e "  ${COLOR_GREEN}✓${COLOR_RESET}  Система определена: ${COLOR_BOLD}${PLATFORM}${COLOR_RESET} (${ARCH}) [Атомарный дистрибутив: ${ATOMIC_NAME}]"
else
    echo -e "  ${COLOR_GREEN}✓${COLOR_RESET}  Система определена: ${COLOR_BOLD}${PLATFORM}${COLOR_RESET} (${ARCH})"
fi

# --- Step 2: Determine Target Paths ---
# In Termux on Android, $PREFIX/bin is the official standard PATH.
# On atomic / immutable systems, rootfs (/usr) is read-only.
# We always install safely without modifying ostree layers.
if [ "$IS_TERMUX" -eq 1 ]; then
    if [ -n "$PREFIX" ] && [ -d "$PREFIX/bin" ]; then
        BIN_DIR="$PREFIX/bin"
    elif [ -d "/data/data/com.termux/files/usr/bin" ]; then
        BIN_DIR="/data/data/com.termux/files/usr/bin"
    else
        BIN_DIR="${HOME}/.local/bin"
    fi
elif [ "$(id -u)" -eq 0 ]; then
    if [ "$IS_ATOMIC" -eq 1 ] || [ ! -w /usr/local ]; then
        if [ -n "$SUDO_USER" ]; then
            TARGET_HOME="$(getent passwd "$SUDO_USER" | cut -d: -f6)"
            BIN_DIR="${TARGET_HOME}/.local/bin"
        else
            BIN_DIR="${HOME}/.local/bin"
        fi
    else
        BIN_DIR="/usr/local/bin"
    fi
else
    BIN_DIR="${HOME}/.local/bin"
fi

mkdir -p "$BIN_DIR"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" 2>/dev/null && pwd || pwd)"
SOURCE_BIN=""

# Check if binary exists in local repo
if [ -f "${SCRIPT_DIR}/agyptch" ]; then
    SOURCE_BIN="${SCRIPT_DIR}/agyptch"
elif [ -f "${SCRIPT_DIR}/open-ag-patcher" ]; then
    SOURCE_BIN="${SCRIPT_DIR}/open-ag-patcher"
elif [ -f "${SCRIPT_DIR}/rust/target/release/agyptch" ]; then
    SOURCE_BIN="${SCRIPT_DIR}/rust/target/release/agyptch"
elif [ -f "${SCRIPT_DIR}/rust/target/release/open-ag-patcher" ]; then
    SOURCE_BIN="${SCRIPT_DIR}/rust/target/release/open-ag-patcher"
fi

# --- Step 3: Obtain Binary ---
prepare_binary() {
    local dest="$1"
    if [ -n "$SOURCE_BIN" ] && [ -f "$SOURCE_BIN" ]; then
        # Copy from local build/repo
        rm -f "$dest"
        cp "$SOURCE_BIN" "$dest"
        chmod +x "$dest"
        return 0
    fi

    # If cargo is present in PATH or ~/.cargo/bin, compile from source
    local cargo_bin=""
    if command -v cargo >/dev/null 2>&1; then
        cargo_bin="cargo"
    elif [ -f "${HOME}/.cargo/bin/cargo" ]; then
        cargo_bin="${HOME}/.cargo/bin/cargo"
    fi

    if [ -n "$cargo_bin" ] && [ -d "${SCRIPT_DIR}/rust" ]; then
        (
            cd "${SCRIPT_DIR}/rust"
            "$cargo_bin" build --release --quiet
        )
        local built_bin="${SCRIPT_DIR}/rust/target/release/agyptch"
        if [ ! -f "$built_bin" ]; then
            built_bin="${SCRIPT_DIR}/rust/target/release/open-ag-patcher"
        fi
        rm -f "$dest"
        cp "$built_bin" "$dest"
        chmod +x "$dest"
        return 0
    fi

    # Fallback: Download prebuilt binary from GitHub Release
    local download_url="https://github.com/Milordick/rust-antigravity-patcher/releases/latest/download/agyptch-${PLATFORM}-${ARCH}"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$download_url" -o "$dest" || curl -fsSL "https://github.com/Milordick/rust-antigravity-patcher/releases/latest/download/open-ag-patcher-${PLATFORM}-${ARCH}" -o "$dest"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$dest" "$download_url" || wget -qO "$dest" "https://github.com/Milordick/rust-antigravity-patcher/releases/latest/download/open-ag-patcher-${PLATFORM}-${ARCH}"
    else
        echo "Требуется curl или wget для загрузки бинарника." >&2
        return 1
    fi
    chmod +x "$dest"

    # In Termux on Android, run termux-elf-cleaner if available
    if command -v termux-elf-cleaner >/dev/null 2>&1; then
        termux-elf-cleaner "$dest" >/dev/null 2>&1 || true
    fi

    # Verify if downloaded binary executes cleanly
    if "$dest" --version >/dev/null 2>&1; then
        return 0
    fi

    # Fallback build from source if prebuilt binary fails and cargo is available
    if [ -n "$cargo_bin" ]; then
        local temp_build_dir
        temp_build_dir="$(mktemp -d)"
        if git clone --depth 1 https://github.com/Milordick/rust-antigravity-patcher.git "$temp_build_dir" >/dev/null 2>&1; then
            (
                cd "${temp_build_dir}/rust"
                "$cargo_bin" build --release --quiet
            )
            local built_bin="${temp_build_dir}/rust/target/release/agyptch"
            if [ -f "$built_bin" ]; then
                rm -f "$dest"
                cp "$built_bin" "$dest"
                chmod +x "$dest"
                rm -rf "$temp_build_dir"
                return 0
            fi
            rm -rf "$temp_build_dir"
        fi
    fi

    return 0
}

TARGET_EXEC="${BIN_DIR}/agyptch"
run_animated "Установка исполняемого файла agyptch в ${BIN_DIR}" prepare_binary "$TARGET_EXEC"

# --- Step 4: Create 'open-ag-patcher' Alias Link ---
create_alias() {
    local target_alias="${BIN_DIR}/open-ag-patcher"
    rm -f "$target_alias"
    ln -sf "${TARGET_EXEC}" "$target_alias" 2>/dev/null || cp -f "${TARGET_EXEC}" "$target_alias"
    chmod +x "$target_alias"

    # In Termux or general systems, also sync ~/.local/bin to clean up any old broken files
    if [ -d "${HOME}/.local/bin" ] && [ "${BIN_DIR}" != "${HOME}/.local/bin" ]; then
        rm -f "${HOME}/.local/bin/agyptch" "${HOME}/.local/bin/open-ag-patcher"
        ln -sf "${TARGET_EXEC}" "${HOME}/.local/bin/agyptch" 2>/dev/null || true
        ln -sf "${TARGET_EXEC}" "${HOME}/.local/bin/open-ag-patcher" 2>/dev/null || true
    fi
}
run_animated "Создание дополнительного алиаса 'open-ag-patcher'" create_alias

# --- Step 5: Configure Shell PATH ---
configure_path() {
    local path_already_set=0
    case ":$PATH:" in
        *":${BIN_DIR}:"*) path_already_set=1 ;;
    esac

    if [ $path_already_set -eq 1 ]; then
        return 0
    fi

    local export_line="export PATH=\"${BIN_DIR}:\$PATH\""
    local shell_rc_files=()

    # Detect active/available shells
    if [ -n "$BASH_VERSION" ] || [ -f "${HOME}/.bashrc" ]; then
        shell_rc_files+=("${HOME}/.bashrc")
    fi
    if [ -f "${HOME}/.zshrc" ]; then
        shell_rc_files+=("${HOME}/.zshrc")
    fi
    if [ -f "${HOME}/.profile" ]; then
        shell_rc_files+=("${HOME}/.profile")
    fi
    if [ -f "${HOME}/.config/fish/config.fish" ]; then
        local fish_line="fish_add_path ${BIN_DIR}"
        if ! grep -q "fish_add_path ${BIN_DIR}" "${HOME}/.config/fish/config.fish" 2>/dev/null; then
            echo "$fish_line" >> "${HOME}/.config/fish/config.fish"
        fi
    fi

    for rc_file in "${shell_rc_files[@]}"; do
        if [ -f "$rc_file" ]; then
            if ! grep -q "${BIN_DIR}" "$rc_file" 2>/dev/null; then
                printf "\n# Added by Open AG Patcher installer\n%s\n" "$export_line" >> "$rc_file"
            fi
        fi
    done

    export PATH="${BIN_DIR}:${PATH}"
    return 0
}
run_animated "Регистрация ${BIN_DIR} в системном окружении PATH" configure_path

# --- Step 6: Verify Installation ---
verify_installation() {
    "${BIN_DIR}/agyptch" --version >/dev/null 2>&1
}
run_animated "Проверка целостности и готовности утилиты" verify_installation

# --- Final Success Summary Card ---
VERSION_INFO="$("${BIN_DIR}/agyptch" --version 2>/dev/null || echo "v1.4.0 (Rust Edition)")"

echo ""
echo -e "${COLOR_GREEN}╭────────────────────────────────────────────────────────────╮${COLOR_RESET}"
echo -e "${COLOR_GREEN}│${COLOR_RESET}   ${COLOR_BOLD}Установка успешно завершена!${COLOR_RESET}                             ${COLOR_GREEN}│${COLOR_RESET}"
echo -e "${COLOR_GREEN}├────────────────────────────────────────────────────────────┤${COLOR_RESET}"
echo -e "${COLOR_GREEN}│${COLOR_RESET}   Версия:    ${COLOR_SURFACE}${VERSION_INFO}${COLOR_RESET}"
echo -e "${COLOR_GREEN}│${COLOR_RESET}   Команда:   ${COLOR_BOLD}${COLOR_BLUE}agyptch${COLOR_RESET} ${COLOR_DIM}(или open-ag-patcher)${COLOR_RESET}"
echo -e "${COLOR_GREEN}│${COLOR_RESET}   Путь:      ${COLOR_GRAY}${BIN_DIR}/agyptch${COLOR_RESET}"
echo -e "${COLOR_GREEN}├────────────────────────────────────────────────────────────┤${COLOR_RESET}"
echo -e "${COLOR_GREEN}│${COLOR_RESET}   ${COLOR_BOLD}Быстрый старт:${COLOR_RESET}                                           ${COLOR_GREEN}│${COLOR_RESET}"
echo -e "${COLOR_GREEN}│${COLOR_RESET}     ${COLOR_BLUE}agyptch${COLOR_RESET}              Запуск графического интерфейса (TUI)${COLOR_GREEN}│${COLOR_RESET}"
echo -e "${COLOR_GREEN}│${COLOR_RESET}     ${COLOR_BLUE}agyptch --check${COLOR_RESET}      Быстрая проверка статуса компонентов${COLOR_GREEN}│${COLOR_RESET}"
echo -e "${COLOR_GREEN}│${COLOR_RESET}     ${COLOR_BLUE}agyptch --patch-all${COLOR_RESET}  Пропатчить всё одной командой       ${COLOR_GREEN}│${COLOR_RESET}"
echo -e "${COLOR_GREEN}╰────────────────────────────────────────────────────────────╯${COLOR_RESET}"
echo ""

# Hint if current terminal hasn't reloaded PATH
case ":$PATH:" in
    *":${BIN_DIR}:"*) ;;
    *)
        echo -e "${COLOR_YELLOW}Обратите внимание:${COLOR_RESET} перезапустите терминал или выполните: ${COLOR_BOLD}export PATH=\"${BIN_DIR}:\$PATH\"${COLOR_RESET}"
        echo ""
        ;;
esac
