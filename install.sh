#!/usr/bin/env bash
set -e

DEST="$HOME/.gemini/config/skills/slide-kit"
mkdir -p "$DEST"
cp -R "$(dirname "$0")/skills/slide-kit/"* "$DEST/"
echo "Successfully installed slide-kit skill to $DEST"
