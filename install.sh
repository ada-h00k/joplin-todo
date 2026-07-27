#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 joplin-todo contributors
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: Cargo is not installed or not available in PATH." >&2
    echo "Install Rust and Cargo, then run this script again." >&2
    exit 1
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cargo install --path "$SCRIPT_DIR" --force

echo
echo "Installed the command as: todo"
echo "Run: todo --help"
