from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from scripts.check_doc_links import collect_anchors, validate_paths


class LinkValidationTests(unittest.TestCase):
    def test_collect_anchors_matches_github_style(self):
        text = "# Choosing a Mode\n\n## Active vs. Potential Findings\n"
        self.assertEqual(
            collect_anchors(text),
            {"choosing-a-mode", "active-vs-potential-findings"},
        )

    def test_collect_anchors_suffixes_collisions_globally(self):
        text = "# Foo\n\n# Foo\n\n# Foo-1\n"
        self.assertEqual(collect_anchors(text), {"foo", "foo-1", "foo-1-1"})

    def test_missing_relative_target_is_reported(self):
        with TemporaryDirectory() as raw:
            root = Path(raw)
            readme = root / "README.md"
            readme.write_text("[Missing](docs/missing.md)\n", encoding="utf-8")
            self.assertIn("docs/missing.md", "\n".join(validate_paths([readme])))

    def test_existing_target_and_anchor_pass(self):
        with TemporaryDirectory() as raw:
            root = Path(raw)
            target = root / "guide.md"
            target.write_text("# Quick Start\n", encoding="utf-8")
            readme = root / "README.md"
            readme.write_text("[Start](guide.md#quick-start)\n", encoding="utf-8")
            self.assertEqual(validate_paths([readme]), [])
