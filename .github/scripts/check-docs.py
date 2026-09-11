from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[2]
DOCS = ROOT / "docs"

class Collector(HTMLParser):
    def __init__(self):
        super().__init__()
        self.refs = []
        self.images_without_alt = []
    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "a" and attrs.get("href"):
            self.refs.append(("href", attrs["href"]))
        if tag in {"img", "script"}:
            key = "src"
            if attrs.get(key):
                self.refs.append((key, attrs[key]))
        if tag == "link" and attrs.get("href"):
            self.refs.append(("href", attrs["href"]))
        if tag == "img" and "alt" not in attrs:
            self.images_without_alt.append(attrs.get("src", "<unknown>"))

errors = []
html_files = sorted(DOCS.glob("*.html"))
if not html_files:
    errors.append("No documentation HTML files found.")

for html_file in html_files:
    parser = Collector()
    parser.feed(html_file.read_text(encoding="utf-8"))
    if parser.images_without_alt:
        errors.append(f"{html_file.name}: images missing alt: {parser.images_without_alt}")
    for _, ref in parser.refs:
        if not ref or ref.startswith(("#", "mailto:", "javascript:")):
            continue
        parsed = urlparse(ref)
        if parsed.scheme in {"http", "https"}:
            continue
        target = (html_file.parent / parsed.path).resolve()
        try:
            target.relative_to(DOCS.resolve())
        except ValueError:
            errors.append(f"{html_file.name}: local reference escapes docs/: {ref}")
            continue
        if parsed.path and not target.exists():
            errors.append(f"{html_file.name}: missing local target: {ref}")

index = (DOCS / "index.html").read_text(encoding="utf-8")
for required in [
    "Alpha preview",
    "Showcase-ready, not stability-ready.",
    "Native HWNDs",
    "No system DLL patches",
    "assets/screenshots/quick-settings-dark.webp",
]:
    if required not in index:
        errors.append(f"index.html: missing required content: {required}")

if not (DOCS / ".nojekyll").exists():
    errors.append("docs/.nojekyll is missing")

if errors:
    raise SystemExit("\n".join(f"ERROR: {e}" for e in errors))

print(f"Docs validation passed: {len(html_files)} HTML pages")
