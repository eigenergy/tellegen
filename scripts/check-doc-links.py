#!/usr/bin/env python3
"""Check file and fragment links in the built book, without network requests."""

import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit


class Page(HTMLParser):
    def __init__(self, text):
        super().__init__()
        self.ids = set()
        self.links = []
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if "id" in attrs:
            self.ids.add(attrs["id"])
        if tag == "a" and "name" in attrs:
            self.ids.add(attrs["name"])
        key = "href" if tag in ("a", "link") else "src"
        if key in attrs:
            self.links.append(attrs[key])


def check(root):
    root = root.resolve()
    pages = {p.resolve(): Page(p.read_text()) for p in root.rglob("*.html")}
    errors = []
    for path, page in pages.items():
        for link in page.links:
            url = urlsplit(link)
            if url.scheme or url.netloc or not link:
                continue
            name = unquote(url.path)
            if name.startswith("/tellegen/"):
                target = root / name.removeprefix("/tellegen/")
            elif name.startswith("/"):
                target = root / name.lstrip("/")
            else:
                target = path.parent / name if name else path
            target = target.resolve()
            if target.is_dir():
                target /= "index.html"
            if not target.is_relative_to(root) or not target.exists():
                errors.append(f"{path.relative_to(root)}: missing file {link}")
            elif url.fragment and target in pages:
                if unquote(url.fragment) not in pages[target].ids:
                    errors.append(f"{path.relative_to(root)}: missing heading {link}")
    return errors


if __name__ == "__main__":
    problems = check(Path(sys.argv[1] if len(sys.argv) > 1 else "docs/book"))
    for problem in sorted(set(problems)):
        print(problem, file=sys.stderr)
    if problems:
        sys.exit(1)
    print("Book links and headings checked.")
