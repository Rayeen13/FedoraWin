from hashlib import sha256
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
    "33 MB sampled idle",
    "Power Mode",
    "./gallery.html",
    "data-runtime-shot=\"activities\"",
]:
    if required not in index:
        errors.append(f"index.html: missing required content: {required}")

# The homepage is a screenshot viewer, not a simulated product. Its images
# must use the same verified runtime file keys already published by Windows CI.
for required in (
    'data-hero-stage',
    'data-hero-preview',
    'data-hero-screenshot="activities"',
    'data-hero-screenshot="desktop"',
    'data-hero-screenshot="apps"',
    'data-hero-screenshot="quick_settings_dark"',
    'data-ci-run-link',
):
    if required not in index:
        errors.append(f"index.html: GNOME 51 screenshot showcase missing {required}")

frame_engine = (DOCS / "frame-engine.html").read_text(encoding="utf-8")
for required in ("Design only", "not yet", "without restarting Windows", "Opera GX", "System32", "FRAME_ENGINE_RFC.md"):
    if required.lower() not in frame_engine.lower():
        errors.append(f"frame-engine.html: missing design and safety disclosure: {required}")

docs_hub = (DOCS / "docs.html").read_text(encoding="utf-8")
for required in [
    "33 MB sampled CI idle",
    "Safety & reversibility",
    "Workspace foundation",
    "./architecture.html",
    "./status.html#memory",
]:
    if required not in docs_hub:
        errors.append(f"docs.html: missing required content: {required}")

gallery = (DOCS / "gallery.html").read_text(encoding="utf-8")
for key in [
    "desktop",
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


for required in ('id="real-adwaita"', 'id="dwm-diagnostics"', 'not yet wired into the production FedoraWin shell', '13 main shell views', 'assets/adwaita/adwaita-dark.png', 'assets/adwaita/adwaita-light.png'):
    if required.lower() not in gallery.lower():
        errors.append(f"gallery.html: missing verified Adwaita separation: {required}")

# Adwaita frames displayed on the homepage are original GTK-owned HWND
# captures, NOT synthetic Windows/Fluent frames or FedoraWin.exe screenshots.
# Freeze their verified PNG bytes; Pages deploy checks the same SHA-256 values.
for image_name, expected_sha in {
    "adwaita-dark.png": "e5526b58c95f32d2c59c758a7c3a6e305f5667c30bea6241016a5b6216e15264",
    "adwaita-light.png": "6bfc8f9f6b20062d110507b68ef54aeca1855ab1cc31c12dc0269cdf51f6412c",
}.items():
    filename = DOCS / "assets" / "adwaita" / image_name
    if not filename.is_file():
        errors.append(f"verified libadwaita image missing: {filename}")
    elif sha256(filename.read_bytes()).hexdigest() != expected_sha:
        errors.append(f"libadwaita image SHA-256 changed without revalidation: {image_name}")

for required in (
    'id="adwaita-frames"',
    'id="adwaita-frames-title"',
    'data-image="./assets/adwaita/adwaita-dark.png"',
    'data-image="./assets/adwaita/adwaita-light.png"',
    'https://github.com/Rayeen13/FedoraWin/actions/runs/35572296452',
    'System-wide replacement of Fluent/Windows frames is not implemented',
    'not a screenshot of the production shell',
):
    if required not in index:
        errors.append(f"index.html: verified native Adwaita boundary missing: {required}")
if 'src="./assets/runtime/native_frame.png"' in index and "libadwaita" not in index:
    errors.append("index.html: confusing native Windows DWM frame with GTK libadwaita")

for required in (".adwaita-frames__grid", ".adwaita-frames__shot:focus-visible", ".adwaita-frames__note"):
    if required not in (DOCS / "styles.css").read_text(encoding="utf-8"):
        errors.append(f"styles.css: missing accessible Adwaita evidence showcase: {required}")

# Only actual FedoraWin shell captures belong in the primary album. DWM
# diagnostics remain available in their own technical section, never as
# examples of GTK4/libadwaita chrome.
album = gallery.split('id="galleryGrid"', 1)[1].split('</div>', 1)[0]
diagnostics = gallery.split('id="dwm-diagnostics"', 1)[1]
if album.count('data-runtime-shot=') != 13:
    errors.append("gallery.html: primary shell album must contain exactly 13 real shell screenshots")
for frame_key in ("native_frame", "native_frame_light"):
    if f'data-runtime-shot="{frame_key}"' in album:
        errors.append(f"gallery.html: DWM diagnostic {frame_key} leaked into primary album")
    if f'data-runtime-shot="{frame_key}"' not in diagnostics:
        errors.append(f"gallery.html: missing separately labeled DWM diagnostic {frame_key}")
if gallery.index('id="real-adwaita"') > gallery.index('id="dwm-diagnostics"'):
    errors.append("gallery.html: genuine Adwaita evidence must appear before legacy DWM diagnostics")
if 'Legacy Windows DWM diagnostics — not Adwaita' not in gallery:
    errors.append("gallery.html: missing explicit non-Adwaita diagnostic label")

# Gallery controls must operate on the real runtime shots, not copied mock cards.
for mode in ("list", "grid", "card", "carousel"):
    if f'data-gallery-view="{mode}"' not in gallery:
        errors.append(f"gallery.html: missing {mode} view switch")
if gallery.count('data-runtime-shot=') != 15:
    errors.append("gallery.html: expected exactly 15 runtime screenshot elements")
for element_id in ("galleryGrid", "galleryPrevious", "galleryNext", "galleryPosition", "galleryCarouselControls"):
    if element_id not in parsed["gallery.html"].ids:
        errors.append(f"gallery.html: missing interactive element #{element_id}")

site_js = (DOCS / "app.js").read_text(encoding="utf-8")
site_css = (DOCS / "styles.css").read_text(encoding="utf-8")
for required in ('selectDemoTab', "demoImage.dataset.runtimeShot=key", "tab.addEventListener('click',()=>selectDemoTab(tab))"):
    if required not in site_js:
        errors.append(f"app.js: screenshot switcher missing {required}")
for required in (".desktop-demo__screen", ".desktop-demo__tab", ".latest-capture-note"):
    if required not in site_css:
        errors.append(f"styles.css: modern screenshot presentation missing {required}")

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
    "fedorawin:gallery-image-selected",
    "cards.indexOf(event.detail.entry)",
    "opener?.hidden",
    "opener?.focus",
    "touchend",
    "Escape",
):
    if required not in site_js:
        errors.append(f"app.js: unified image viewer missing {required}")
for required in ("body.viewer-open", ".gallery-card[hidden]", ".lightbox.open", "prefers-reduced-motion"):
    if required not in site_css:
        errors.append(f"styles.css: GNOME-like gallery interaction missing {required}")



# GNOME-inspired website framing must be a working, accessible site control,
# distinct from the genuine Windows runtime screenshots and their provenance.
for required in (
    "sitePanel.innerHTML",
    "siteOverview.innerHTML",
    "aria-modal",
    "siteOverview.hidden=true",
    "closeSiteOverview",
    "openSiteOverview",
    "activitiesButton.addEventListener('click',openSiteOverview)",
    "document.querySelector('#heroActivities')?.addEventListener('click',openSiteOverview)",
    "siteClock.textContent",
    "setInterval(updateSiteClock,60000)",
    "requestAnimationFrame(updatePageProgress)",
):
    if required not in site_js:
        errors.append(f"app.js: GNOME-inspired site interaction missing {required}")
for required in (
    ".site-panel",
    ".site-overview",
    ".site-overview[hidden]",
    ".site-overview__tiles",
    ".hero-dock",
    ".hero-actions__overview",
    ".gallery-cover__front",
    "@keyframes site-rise",
    "prefers-reduced-motion:reduce",
):
    if required not in site_css:
        errors.append(f"styles.css: desktop-inspired theme missing {required}")
for required in (
    'id="heroActivities"',
    'class="hero-dock"',
    'src="./assets/runtime/desktop.png"',
):
    if required not in index:
        errors.append(f"index.html: missing GNOME-like homepage element {required}")
for required in (
    'class="wrap gallery-hero__grid"',
    'gallery-cover__rear',
    'gallery-cover__middle',
    'gallery-cover__front',
    'href="#galleryGrid"',
):
    if required not in gallery:
        errors.append(f"gallery.html: missing real-capture album presentation {required}")


if errors:
    raise SystemExit("\n".join(f"ERROR: {error}" for error in errors))
print(
    f"Docs validation passed: {len(html_files)} HTML pages, links, metadata, docs hub and 15-surface, four-view gallery verified"
)
