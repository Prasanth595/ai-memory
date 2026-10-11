#!/usr/bin/env python3
"""Parse, rotate, and rebuild the root CHANGELOG.md with per-version archives."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# Released version headings only (not [Unreleased]).
VERSION_HEADING = re.compile(r"^## \[([0-9][^\]]*)\](?: - [^\n]+)?\n", re.MULTILINE)
COMPARE_FOOTER_START = re.compile(r"^\[Unreleased\]:", re.MULTILINE)
COMPARE_LINE = re.compile(r"^\[([^\]]+)\]:[^\n]*\n")

ARCHIVE_POINTER = (
    "\nOlder released versions are archived under "
    "[docs/changelog/](docs/changelog/README.md).\n"
)
ARCHIVE_POINTER_LINE = ARCHIVE_POINTER.strip()

DEFAULT_KEEP_RELEASED = 2


def _strip_archive_pointer(versions_text: str) -> str:
    """Remove the rolling-window archive pointer from the versions block."""
    marker = ARCHIVE_POINTER_LINE
    if marker not in versions_text:
        return versions_text
    without = versions_text.replace(ARCHIVE_POINTER, "\n")
    without = without.replace(marker + "\n", "\n").replace(marker, "")
    return without.rstrip("\n") + ("\n" if without else "")


def parse_changelog(text: str) -> tuple[str, str, list[tuple[str, str]], str]:
    """Return preamble, unreleased block, version sections (newest first), footer."""
    unreleased_m = re.search(r"^## \[Unreleased\]\n", text, re.MULTILINE)
    if not unreleased_m:
        raise ValueError("CHANGELOG.md is missing ## [Unreleased]")

    preamble = text[: unreleased_m.start()]
    rest = text[unreleased_m.start() :]

    first_version = VERSION_HEADING.search(rest)
    if first_version:
        unreleased_block = rest[: first_version.start()]
        versions_text = rest[first_version.start() :]
    else:
        unreleased_block = rest
        versions_text = ""

    footer_m = COMPARE_FOOTER_START.search(versions_text)
    if footer_m:
        versions_only = versions_text[: footer_m.start()].rstrip("\n")
        footer = versions_text[footer_m.start() :]
        if not versions_only.endswith("\n") and versions_only:
            versions_only += "\n"
    else:
        versions_only = versions_text.rstrip("\n")
        footer = ""
        if versions_only and not versions_only.endswith("\n"):
            versions_only += "\n"

    versions_only = _strip_archive_pointer(versions_only)

    sections: list[tuple[str, str]] = []
    matches = list(VERSION_HEADING.finditer(versions_only))
    for i, m in enumerate(matches):
        version = m.group(1)
        end = matches[i + 1].start() if i + 1 < len(matches) else len(versions_only)
        chunk = versions_only[m.start() : end].rstrip("\n") + "\n"
        sections.append((version, chunk))

    return preamble, unreleased_block, sections, footer


def parse_compare_footer(footer: str) -> dict[str, str]:
    links: dict[str, str] = {}
    for line in footer.splitlines(keepends=True):
        m = COMPARE_LINE.match(line)
        if m:
            links[m.group(1)] = line
    return links


def rebuild_changelog(
    preamble: str,
    unreleased_block: str,
    sections: list[tuple[str, str]],
    footer_links: dict[str, str],
    *,
    include_archive_pointer: bool,
) -> str:
    parts = [preamble.rstrip("\n") + "\n\n", unreleased_block.rstrip("\n") + "\n\n"]
    for _, chunk in sections:
        parts.append(chunk.rstrip("\n") + "\n\n")

    if include_archive_pointer and ARCHIVE_POINTER.strip() not in "".join(parts):
        parts.append(ARCHIVE_POINTER)

    ordered_keys = ["Unreleased"] + [ver for ver, _ in sections]
    footer_lines = []
    for key in ordered_keys:
        if key in footer_links:
            footer_lines.append(footer_links[key].rstrip("\n"))
    parts.append("\n".join(footer_lines) + ("\n" if footer_lines else ""))
    return "".join(parts)


def refresh_archive_readme(archive_dir: Path) -> None:
    versions = [
        p.stem for p in archive_dir.glob("*.md") if p.name != "README.md"
    ]
    if versions:
        write_archive_readme(archive_dir, versions)


def rotate_once(
    changelog_path: Path,
    archive_dir: Path,
    keep_released: int = DEFAULT_KEEP_RELEASED,
    *,
    dry_run: bool = False,
) -> bool:
    """Archive the oldest in-window release if count > keep_released. Returns True if rotated."""
    text = changelog_path.read_text()
    preamble, unreleased, sections, footer = parse_changelog(text)
    if len(sections) <= keep_released:
        return False

    version, chunk = sections[-1]
    archive_path = archive_dir / f"{version}.md"
    if archive_path.exists():
        raise FileExistsError(f"archive already exists: {archive_path}")

    links = parse_compare_footer(footer)
    if version not in links:
        print(
            f"warning: no compare link for [{version}] in CHANGELOG footer",
            file=sys.stderr,
        )

    if not dry_run:
        archive_dir.mkdir(parents=True, exist_ok=True)
        archive_path.write_text(chunk)
        del links[version]
        new_sections = sections[:-1]
        new_text = rebuild_changelog(
            preamble,
            unreleased,
            new_sections,
            links,
            include_archive_pointer=True,
        )
        changelog_path.write_text(new_text)
        refresh_archive_readme(archive_dir)

    return True


def rotate_all(
    changelog_path: Path,
    archive_dir: Path,
    keep_released: int = DEFAULT_KEEP_RELEASED,
    *,
    dry_run: bool = False,
) -> int:
    count = 0
    while rotate_once(
        changelog_path, archive_dir, keep_released, dry_run=dry_run
    ):
        count += 1
    return count


def migrate_to_archive(
    changelog_path: Path,
    archive_dir: Path,
    keep_released: int = DEFAULT_KEEP_RELEASED,
    *,
    dry_run: bool = False,
) -> list[str]:
    """One-time: write every released section beyond the window to docs/changelog/."""
    text = changelog_path.read_text()
    preamble, unreleased, sections, footer = parse_changelog(text)
    if len(sections) <= keep_released:
        return []

    to_archive = sections[keep_released:]
    archived: list[str] = []
    links = parse_compare_footer(footer)

    if not dry_run:
        archive_dir.mkdir(parents=True, exist_ok=True)

    for version, chunk in to_archive:
        archive_path = archive_dir / f"{version}.md"
        if archive_path.exists():
            raise FileExistsError(f"archive already exists: {archive_path}")
        if not dry_run:
            archive_path.write_text(chunk)
        archived.append(version)
        links.pop(version, None)

    kept = sections[:keep_released]
    if not dry_run:
        new_text = rebuild_changelog(
            preamble,
            unreleased,
            kept,
            links,
            include_archive_pointer=True,
        )
        changelog_path.write_text(new_text)

    return archived


def write_archive_readme(archive_dir: Path, versions: list[str]) -> None:
    """Write docs/changelog/README.md listing archived versions (newest first)."""
    lines = [
        "# Changelog archive",
        "",
        "Released versions rotated out of the active [`CHANGELOG.md`](../CHANGELOG.md) "
        "live here as immutable markdown, one file per semver.",
        "",
        "Contributors add entries only under `## [Unreleased]` in the root changelog. "
        "`bin/release` moves that block into a new version section and rotates older "
        "releases into this directory. See [CONTRIBUTING.md](../CONTRIBUTING.md) "
        '("CHANGELOG is a merge gate").',
        "",
        "To read an older release: open the file below, `git show vX.Y.Z`, or the "
        "matching [GitHub release](https://github.com/akitaonrails/ai-memory/releases).",
        "",
        "## Archived versions",
        "",
    ]
    for version in sorted(versions, key=_version_key, reverse=True):
        lines.append(f"- [{version}]({version}.md)")
    lines.append("")
    archive_dir.mkdir(parents=True, exist_ok=True)
    (archive_dir / "README.md").write_text("\n".join(lines))


def _version_key(version: str) -> tuple:
    """Sort key for semver strings (best-effort; pre-release sorts after patch)."""
    parts = re.split(r"[.-]", version)
    nums: list[int | str] = []
    for p in parts:
        if p.isdigit():
            nums.append(int(p))
        else:
            nums.append(p)
    return tuple(nums)


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--changelog",
        type=Path,
        default=repo_root / "CHANGELOG.md",
    )
    parser.add_argument(
        "--archive-dir",
        type=Path,
        default=repo_root / "docs" / "changelog",
    )
    parser.add_argument(
        "--keep",
        type=int,
        default=DEFAULT_KEEP_RELEASED,
        help=f"released sections to keep in CHANGELOG.md (default {DEFAULT_KEEP_RELEASED})",
    )
    parser.add_argument("--dry-run", action="store_true")
    sub = parser.add_subparsers(dest="cmd", required=True)

    sub.add_parser("rotate", help="rotate until only --keep released sections remain")
    sub.add_parser(
        "migrate",
        help="one-time archive of all sections beyond --keep and refresh README",
    )

    args = parser.parse_args()
    if args.cmd == "rotate":
        n = rotate_all(
            args.changelog,
            args.archive_dir,
            args.keep,
            dry_run=args.dry_run,
        )
        print(f"rotated {n} release section(s)")
        return 0

    if args.cmd == "migrate":
        archived = migrate_to_archive(
            args.changelog,
            args.archive_dir,
            args.keep,
            dry_run=args.dry_run,
        )
        if not args.dry_run and archived:
            all_versions = [
                p.stem for p in args.archive_dir.glob("*.md") if p.name != "README.md"
            ]
            write_archive_readme(args.archive_dir, all_versions)
        print(f"archived {len(archived)} release section(s)")
        return 0

    return 1


if __name__ == "__main__":
    raise SystemExit(main())
