import importlib.util
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "links", Path(__file__).with_name("check-doc-links.py")
)
links = importlib.util.module_from_spec(spec)
spec.loader.exec_module(links)


class LinkTests(unittest.TestCase):
    def test_files_fragments_and_external_links(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "index.html").write_text('''
                <a href="other.html#hello%20world">good</a>
                <a href="https://example.org/absent">external</a>
                <a href="/tellegen/other.html#missing">bad heading</a>
                <img src="missing.svg">
                <a href="../escape.html">outside</a>
            ''')
            (root / "other.html").write_text('<h1 id="hello world">Hi</h1>')
            errors = links.check(root)
            self.assertEqual(len(errors), 3)
            self.assertTrue(any("missing heading" in e for e in errors))
            self.assertTrue(any("missing.svg" in e for e in errors))
            self.assertTrue(any("escape.html" in e for e in errors))


if __name__ == "__main__":
    unittest.main()
