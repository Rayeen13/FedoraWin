from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[2]
DOCS = ROOT / "docs"
PUBLIC_PAGES = {"index.html", "docs.html", "gallery.html", "getting-started.html", "architecture.html", "status.html", "frame-engine.html"}


class Collector(HTMLParser):
    def __init__(self):
        super().__init__()
        self.refs = []
        self.ids = set()
        self.images_without_alt = []
        self.meta_description = False
        self.canonical = False
        self.favicon = False

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if attrs.get("id"):
            self.ids.add(attrs["id"])
        if tag == "a" and attrs.get("href"):
            self.refs.append(("href", attrs["href"]))
        if tag in {"img", "script"} and attrs.get("src"):
            self.refs.append(("src", attrs["src"]))
        if tag == "link" and attrs.get("href"):
            self.refs.append(("href", attrs["href"]))
            rel = attrs.get("rel", "")
            if rel == "canonical":
                self.canonical = True
            if "icon" in rel:
                self.favicon = True
        if tag == "meta" and attrs.get("name") == "description" and attrs.get("content", "").strip():
            self.meta_description = True
        if tag == "img" and "alt" not in attrs:
            self.images_without_alt.append(attrs.get("src", "<unknown>"))


errors = []
html_files = sorted(DOCS.glob("*.html"))
parsed = {}
for html_file in html_files:
    parser = Collector()
    parser.feed(html_file.read_text(encoding="utf-8"))
    parsed[html_file.name] = parser
    if parser.images_without_alt:
        errors.append(f"{html_file.name}: images missing alt: {parser.images_without_alt}")
    if html_file.name in PUBLIC_PAGES:
        if not parser.meta_description:
            errors.append(f"{html_file.name}: missing meta description")
        if not parser.canonical:
            errors.append(f"{html_file.name}: missing canonical link")
        if not parser.favicon:
            errors.append(f"{html_file.name}: missing favicon link")

for html_file in html_files:
    parser = parsed[html_file.name]
    for _, ref in parser.refs:
        if not ref or ref.startswith(("mailto:", "javascript:")):
            continue
        if ref.startswith("#"):
            if ref[1:] and ref[1:] not in parser.ids:
                errors.append(f"{html_file.name}: missing local anchor: {ref}")
            continue
        parsed_ref = urlparse(ref)
        if parsed_ref.scheme in {"http", "https"}:
            continue
        local_path = parsed_ref.path
        if local_path == "/FedoraWin" or local_path == "/FedoraWin/":
            local_path = "index.html"
        elif local_path.startswith("/FedoraWin/"):
            local_path = local_path[len("/FedoraWin/"):]
        elif local_path.startswith("/"):
            errors.append(f"{html_file.name}: unexpected site-root reference: {ref}")
            continue
        target = (html_file.parent / local_path).resolve()
        try:
            target.relative_to(DOCS.resolve())
        except ValueError:
            errors.append(f"{html_file.name}: local reference escapes docs/: {ref}")
            continue
        if parsed_ref.path and not target.exists():
            errors.append(f"{html_file.name}: missing local target: {ref}")
            continue
        if parsed_ref.fragment and target.suffix.lower() == ".html" and target.exists():
            target_parser = parsed.get(target.name)
            if target_parser and parsed_ref.fragment not in target_parser.ids:
                errors.append(f"{html_file.name}: missing anchor #{parsed_ref.fragment} in {target.name}")

index = (DOCS / "index.html").read_text(encoding="utf-8")
for required in [
    "GNOME 51 desktop.",
    "32 MB measured idle",
    "Power Mode",
    "./gallery.html",
    "data-runtime-shot=\"activities\"",
]:
    if required not in index:
        errors.append(f"index.html: missing required content: {required}")

frame_engine = (DOCS / "frame-engine.html").read_text(encoding="utf-8")
for required in ("Design only", "not yet", "without restarting Windows", "Opera GX", "System32", "FRAME_ENGINE_RFC.md"):
    if required.lower() not in frame_engine.lower():
        errors.append(f"frame-engine.html: missing design and safety disclosure: {required}")

docs_hub = (DOCS / "docs.html").read_text(encoding="utf-8")
for required in [
    "32 MB verified idle",
    "Safety & reversibility",
    "Workspace foundation",
    "./architecture.html",
    "./status.html#memory",
]:
    if required not in docs_hub:
        errors.append(f"docs.html: missing required content: {required}")

gallery = (DOCS / "gallery.html").read_text(encoding="utf-8")
for key in [
    "panel",
    "activities",
    "apps",
    "search_terminal",
    "quick_settings_dark",
    "quick_settings_light",
    "appearance",
    "power_mode",
    "date_menu",
    "native_frame",
    "native_frame_light",
]:
    if f'data-runtime-shot="{key}"' not in gallery:
        errors.append(f"gallery.html: missing runtime shot: {key}")


for required in ("id=\"real-adwaita\"", "not yet wired into the production FedoraWin shell", "the 14 images above", "assets/adwaita/adwaita-dark.png", "assets/adwaita/adwaita-light.png"):
    if required.lower() not in gallery.lower():
        errors.append(f"gallery.html: missing verified Adwaita separation: {required}")

# Gallery controls must operate on the real runtime shots, not copied mock cards.
for mode in ("list", "grid", "card", "carousel"):
    if f'data-gallery-view="{mode}"' not in gallery:
        errors.append(f"gallery.html: missing {mode} view switch")
if gallery.count('data-runtime-shot=') != 14:
    errors.append("gallery.html: expected exactly 14 runtime screenshot elements")
for element_id in ("galleryGrid", "galleryPrevious", "galleryNext", "galleryPosition", "galleryCarouselControls"):
    if element_id not in parsed["gallery.html"].ids:
        errors.append(f"gallery.html: missing interactive element #{element_id}")

site_js = (DOCS / "app.js").read_text(encoding="utf-8")
site_css = (DOCS / "styles.css").read_text(encoding="utf-8")
for required in (
    "gallery.dataset.view=view",
    "card.hidden=carousel&&index!==active",
    "localStorage.setItem(viewKey,view)",
    "ArrowLeft",
    "ArrowRight",
    "touchstart",
    "touchend",
    "data-runtime-shot",
):
    if required not in site_js:
        errors.append(f"app.js: gallery controller is missing {required}")
for required in (
    '[data-view="list"]',
    '[data-view="grid"]',
    '[data-view="card"]',
    '[data-view="carousel"]',
    "auto-fit",
    "dvh",
    "object-fit:contain",
    "prefers-reduced-motion",
):
    if required not in site_css:
        errors.append(f"styles.css: gallery layout is missing {required}")


for required_file in [
    ".nojekyll",
    "docs.html",
    "gallery.html",
    "status.html",
    "assets/favicon.svg",
    "assets/fedorawin-logo.svg",
    "robots.txt",
    "sitemap.xml",
]:
    if not (DOCS / required_file).exists():
        errors.append(f"docs/{required_file} is missing")


# The site keeps one FedoraWin identity on every public page; the generated
# Windows captures must remain the sole source for production shell images.
for page_name in PUBLIC_PAGES:
    source = (DOCS / page_name).read_text(encoding="utf-8")
    if 'src="./assets/fedorawin-logo.svg"' not in source:
        errors.append(f"{page_name}: FedoraWin identity missing")
for required in (
    "lightbox-previous",
    "lightbox-next",
    "lightbox-position",
    "show(active+(event.key==='ArrowRight'?1:-1))",
    "image.src=entry.dataset.image",
    "opener?.focus",
    "touchend",
    "Escape",
):
    if required not in site_js:
        errors.append(f"app.js: unified image viewer missing {required}")
for required in ("body.viewer-open", ".gallery-card[hidden]", ".lightbox.open", "prefers-reduced-motion"):
    if required not in site_css:
        errors.append(f"styles.css: GNOME-like gallery interaction missing {required}")


if errors:
    raise SystemExit("\n".join(f"ERROR: {error}" for error in errors))
print(
    f"Docs validation passed: {len(html_files)} HTML pages, links, metadata, docs hub and 14-surface, four-view gallery verified"
)
