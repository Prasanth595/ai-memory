#!/usr/bin/env bash
# Fail if a change adds or edits entries inside an already-released
# CHANGELOG section or modifies an existing archived release file.
#
# `bin/release` renames `## [Unreleased]` to `## [X.Y.Z]`. A branch opened
# before that release still carries a CHANGELOG diff anchored at the old
# `[Unreleased]` line numbers, and git merges it into whatever section now
# occupies them — no conflict, no warning. The entry then claims a change
# shipped in a release that did not contain it, and the release that *did*
# ship it lists nothing.
#
# Three times in one week: #491 into [1.32.0], #502 into [1.32.2], #517 into
# [1.33.0]. Each found by hand, twice only because an unrelated merge
# conflict forced someone to look at the file.
#
# Usage: check-changelog-frozen.sh [base-ref]   (default: origin/main)
#
# Compares the merge base of <base-ref> and HEAD against HEAD, and rejects
# any changed CHANGELOG line that sits below the `## [Unreleased]` heading.
# Deliberate historical corrections on the default branch are unaffected:
# this asks what a branch changes relative to where it forked, not what the
# file looks like versus a tag.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

BASE_REF="${1:-origin/main}"
git rev-parse --verify "$BASE_REF" >/dev/null 2>&1 || {
    echo "check-changelog-frozen: base ref '$BASE_REF' not found; skipping."
    exit 0
}

# Compare the released half of the file directly against the base branch,
# rather than diffing line ranges since the merge base.
#
# The line-range form fired falsely on any branch that merged the base after a
# release: from the old merge base, the whole new `## [X.Y.Z]` section reads as
# lines this branch touched, even though it arrived through the merge. The
# question that matters is simpler — does this branch's copy of an
# already-released section differ from the base branch's copy?
#
# A section the base has and this branch does not is fine (rotation or a stale
# branch, not a rewrite). Every released version section still present in HEAD
# must match the base copy byte for byte.

if git show "$BASE_REF:CHANGELOG.md" >/dev/null 2>&1; then
    if ! python3 - "$REPO_ROOT" "$BASE_REF" <<'PY'
import subprocess
import sys
from pathlib import Path

repo_root = Path(sys.argv[1])
base_ref = sys.argv[2]
sys.path.insert(0, str(repo_root / "scripts"))
from changelog_lib import parse_changelog

def git_show(ref: str, path: str) -> str:
    return subprocess.check_output(
        ["git", "show", f"{ref}:{path}"],
        cwd=repo_root,
        text=True,
    )

try:
    base_text = git_show(base_ref, "CHANGELOG.md")
except subprocess.CalledProcessError:
    sys.exit(0)

head_text = (repo_root / "CHANGELOG.md").read_text()
_, _, base_sections, _ = parse_changelog(base_text)
_, _, head_sections, _ = parse_changelog(head_text)
base_map = dict(base_sections)

status = 0
for version, chunk in head_sections:
    base_chunk = base_map.get(version)
    if base_chunk is None:
        continue
    if chunk != base_chunk:
        print(
            f"error: released section [{version}] in CHANGELOG.md was modified.",
            file=sys.stderr,
        )
        status = 1

if status:
    print(file=sys.stderr)
    print(
        "Released sections are frozen. An entry for unreleased work belongs under\n"
        "`## [Unreleased]`.\n\n"
        "This usually means the branch was opened before a release was cut, and git\n"
        "merged the entry into the section that now occupies those lines. Move it up\n"
        "to `[Unreleased]`; the content does not change, only the section.",
        file=sys.stderr,
    )
sys.exit(status)
PY
    then
        exit 1
    fi
    echo "CHANGELOG.md: no released section was modified."
else
    echo "check-changelog-frozen: base has no CHANGELOG.md yet."
fi

# Archived per-version files are immutable once created. New files (rotation on
# release) are allowed; edits to an existing archive are not.
archive_status=0
while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    if git cat-file -e "$BASE_REF:$path" 2>/dev/null; then
        base_content="$(git show "$BASE_REF:$path")"
        head_content="$(git show "HEAD:$path")"
        if [[ "$base_content" != "$head_content" ]]; then
            echo "error: archived changelog file was modified: $path" >&2
            archive_status=1
        fi
    fi
done < <(git diff --name-only "$BASE_REF"...HEAD -- docs/changelog/ 2>/dev/null || true)

if [[ $archive_status -ne 0 ]]; then
    cat >&2 <<'EOF'

Files under docs/changelog/ are frozen after creation. Fix typos only with an
explicit maintainer ack; do not edit them in ordinary feature PRs.
EOF
    exit 1
fi

echo "docs/changelog/: no existing archive file was modified."
