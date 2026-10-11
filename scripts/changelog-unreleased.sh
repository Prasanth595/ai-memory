#!/usr/bin/env bash
# Print only the ## [Unreleased] section from the root CHANGELOG.md.
#
# Usage: scripts/changelog-unreleased.sh [path-to-CHANGELOG.md]
#
# Handy for agents and reviewers who need entry style without attaching the
# full changelog or archived release files.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHANGELOG="${1:-$REPO_ROOT/CHANGELOG.md}"

if [[ ! -f "$CHANGELOG" ]]; then
    echo "changelog-unreleased: $CHANGELOG not found" >&2
    exit 2
fi

awk '
    /^## \[Unreleased\]/ { found = 1; print; next }
    found && /^## \[[0-9]/ { exit }
    found { print }
' "$CHANGELOG"
