#!/usr/bin/env python3
"""Unit tests for scripts/changelog_lib.py."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from changelog_lib import (  # noqa: E402
    ARCHIVE_POINTER_LINE,
    migrate_to_archive,
    parse_changelog,
    rebuild_changelog,
    rotate_all,
)


SAMPLE = """\
# Changelog

Preamble.

## [Unreleased]

### Added
- Unreleased item.

## [1.1.0] - 2026-01-02

### Added
- One.

## [1.0.0] - 2026-01-01

### Added
- Zero.

[Unreleased]: https://example.com/unreleased
[1.1.0]: https://example.com/1.1.0
[1.0.0]: https://example.com/1.0.0
"""


class ChangelogLibTests(unittest.TestCase):
    def test_parse_sections_newest_first(self) -> None:
        preamble, unreleased, sections, footer = parse_changelog(SAMPLE)
        self.assertIn("Preamble", preamble)
        self.assertIn("Unreleased item", unreleased)
        self.assertEqual([v for v, _ in sections], ["1.1.0", "1.0.0"])
        self.assertIn("[Unreleased]:", footer)

    def test_migrate_and_rotate(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            changelog = root / "CHANGELOG.md"
            archive = root / "docs" / "changelog"
            changelog.write_text(SAMPLE)

            archived = migrate_to_archive(changelog, archive, keep_released=1)
            self.assertEqual(archived, ["1.0.0"])
            self.assertTrue((archive / "1.0.0.md").is_file())
            text = changelog.read_text()
            self.assertIn("## [1.1.0]", text)
            self.assertNotIn("## [1.0.0]", text)
            self.assertNotIn("[1.0.0]:", text)

            # Simulate a release adding 1.2.0 (append manually).
            changelog.write_text(
                text.replace(
                    "## [Unreleased]\n\n",
                    "## [Unreleased]\n\n### Added\n- New.\n\n## [1.2.0] - 2026-02-01\n\n"
                    "### Added\n- Two.\n\n",
                ).replace(
                    "[Unreleased]:",
                    "[Unreleased]: https://example.com/u\n[1.2.0]: https://example.com/1.2.0\n",
                )
            )
            n = rotate_all(changelog, archive, keep_released=1)
            self.assertEqual(n, 1)
            self.assertTrue((archive / "1.1.0.md").is_file())
            self.assertNotIn("## [1.1.0]", changelog.read_text())

    def test_archive_pointer_not_in_released_section(self) -> None:
        text = SAMPLE.replace(
            "\n[Unreleased]:",
            f"\n{ARCHIVE_POINTER_LINE}\n\n[Unreleased]:",
        )
        _, _, sections, _ = parse_changelog(text)
        for version, chunk in sections:
            self.assertNotIn("Older released versions", chunk, version)

    def test_rebuild_preserves_compare_order(self) -> None:
        _, unreleased, sections, footer = parse_changelog(SAMPLE)
        links = {"Unreleased": "[Unreleased]: x\n", "1.1.0": "[1.1.0]: y\n"}
        out = rebuild_changelog(
            "# H\n\n",
            unreleased,
            sections[:1],
            links,
            include_archive_pointer=False,
        )
        self.assertLess(out.index("[Unreleased]:"), out.index("[1.1.0]:"))


if __name__ == "__main__":
    unittest.main()
