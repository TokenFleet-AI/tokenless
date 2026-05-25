#!/usr/bin/env bash
# tokenless-env-fix.sh — Auto-fix script for missing tool dependencies.
# Reads a JSON array of dependency entries from stdin.
# Usage: echo '[{"binary":"jq","package":"jq","manager":"rpm"}]' | tokenless-env-fix.sh fix-all

set -euo pipefail

# Detect package manager
detect_manager() {
    if command -v dnf &>/dev/null; then echo "dnf"
    elif command -v yum &>/dev/null; then echo "yum"
    elif command -v apt-get &>/dev/null; then echo "apt"
    elif command -v apk &>/dev/null; then echo "apk"
    elif command -v brew &>/dev/null; then echo "brew"
    elif command -v cargo &>/dev/null; then echo "cargo"
    else echo "unknown"
    fi
}

install_package() {
    local pkg="$1"
    local mgr="$2"
    case "$mgr" in
        dnf)   sudo dnf install -y "$pkg" 2>/dev/null || sudo yum install -y "$pkg" 2>/dev/null ;;
        yum)   sudo yum install -y "$pkg" ;;
        apt)   sudo apt-get install -y "$pkg" ;;
        apk)   sudo apk add "$pkg" ;;
        brew)  brew install "$pkg" ;;
        cargo) cargo install "$pkg" 2>/dev/null ;;
        *)     echo "  [env-fix] No known package manager for $pkg"; return 1 ;;
    esac
}

fix_dep() {
    local binary package manager
    binary=$(echo "$1" | jq -r '.binary // empty')
    package=$(echo "$1" | jq -r '.package // empty')
    manager=$(echo "$1" | jq -r '.manager // empty')

    if [[ -z "$binary" ]]; then
        return 0
    fi
    if command -v "$binary" &>/dev/null; then
        echo "  [env-fix] $binary already installed"
        return 0
    fi

    local resolved_manager
    if [[ "$manager" == "rpm" ]]; then
        resolved_manager=$(detect_manager)
    else
        resolved_manager="$manager"
    fi

    if [[ "$resolved_manager" == "unknown" ]]; then
        echo "  [env-fix] Cannot install $binary: no package manager detected"
        return 1
    fi

    echo "  [env-fix] Installing $binary via $resolved_manager..."
    install_package "$package" "$resolved_manager" && {
        if command -v "$binary" &>/dev/null; then
            echo "  [env-fix] $binary installed successfully"
        else
            echo "  [env-fix] $package installed but $binary not found on PATH"
        fi
    } || {
        echo "  [env-fix] Failed to install $binary"
        return 1
    }
}

case "${1:-}" in
    fix-all)
        deps=$(cat)
        count=$(echo "$deps" | jq length 2>/dev/null || echo 0)
        if [[ "$count" -eq 0 ]]; then
            echo "  [env-fix] No dependencies to fix"
            exit 0
        fi
        for i in $(seq 0 $((count - 1))); do
            dep=$(echo "$deps" | jq ".[$i]")
            fix_dep "$dep"
        done
        ;;
    *)
        echo "Usage: $0 fix-all"
        echo "  Reads JSON array of deps from stdin and installs missing ones."
        exit 1
        ;;
esac
