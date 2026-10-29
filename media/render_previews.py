"""Regenerate the README comparison images (requires Pillow and FiraCode Nerd Font)."""

import json
import re
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


ROOT = Path(__file__).resolve().parent.parent
MEDIA = Path(__file__).resolve().parent
SCALE = 2
WIDTH, HEIGHT = 2048, 690

FONT_DIRS = [
    Path.home() / "AppData/Local/Microsoft/Windows/Fonts",
    Path("C:/Windows/Fonts"),
    Path.home() / ".local/share/fonts",
    Path("/usr/share/fonts/truetype"),
]


def font_path(bold=False):
    name = "FiraCodeNerdFontMono-Bold.ttf" if bold else "FiraCodeNerdFontMono-Regular.ttf"
    for directory in FONT_DIRS:
        matches = list(directory.rglob(name)) if directory.exists() else []
        if matches:
            return matches[0]
    raise FileNotFoundError(f"Install FiraCode Nerd Font ({name}) to regenerate previews")


FONT_REGULAR = font_path()
FONT_BOLD = font_path(True)


def font(size, bold=False):
    return ImageFont.truetype(str(FONT_BOLD if bold else FONT_REGULAR), size * SCALE)


BG = "#080F1B"
PANEL = "#0D1626"
BORDER = "#344663"
MUTED = "#A3B9D1"
WHITE = "#E5EEFA"
BLUE = "#7DAAEE"
YELLOW = "#EABF69"
GREEN = "#84CEB0"

theme = json.loads((ROOT / "themes/material.json").read_text(encoding="utf-8"))


def theme_icon(name, folder=False):
    kind = "folder" if folder else "file"
    rules = theme["rules"]
    exact = next((r for r in reversed(rules) if r["kind"] == kind and r["pattern"].lower() == name.lower()), None)
    if not exact and not folder:
        extensions = [r for r in rules if r["kind"] == "extension" and name.lower().endswith("." + r["pattern"].lower())]
        exact = max(extensions, key=lambda r: len(r["pattern"]), default=None)
    return (exact or {"icon": theme["fallbacks"][kind]})["icon"]


def pt(x, y):
    return round(x * SCALE), round(y * SCALE)


def text(draw, x, y, value, size=26, color=WHITE, bold=False):
    draw.text(pt(x, y), value, font=font(size, bold), fill=color, anchor="lt")


def base(title, subtitle):
    image = Image.new("RGB", pt(WIDTH, HEIGHT), BG)
    draw = ImageDraw.Draw(image)
    text(draw, 38, 24, title, 30, WHITE, True)
    text(draw, 38, 66, subtitle, 19, MUTED)
    for x, label, tag in ((28, "default", "Stock"), (1040, "Glyfuse", "Material theme")):
        draw.rounded_rectangle((*pt(x, 116), *pt(x + 980, 647)), radius=18 * SCALE, fill=PANEL, outline=BORDER, width=2 * SCALE)
        draw.ellipse((*pt(x + 22, 148), *pt(x + 35, 161)), fill=GREEN)
        text(draw, x + 47, 139, label, 27, WHITE, True)
        tag_width = draw.textlength(tag, font=font(19)) / SCALE
        text(draw, x + 951 - tag_width, 144, tag, 19, MUTED)
        draw.line((*pt(x + 21, 185), *pt(x + 959, 185)), fill=BORDER, width=2 * SCALE)
    return image, draw


names = ["integrations", "media", "src", "themes", "Cargo.lock", "Cargo.toml", "LICENSE", "README.md", "THIRD_PARTY_LICENSES.md"]
stock_glyphs = ["", "", "", "", "", "", "", "", ""]
stock_colors = [BLUE, BLUE, BLUE, BLUE, WHITE, YELLOW, WHITE, YELLOW, WHITE]


def render_eza():
    image, draw = base("Same directory. Different file icons.", "eza  ·  default icons compared with Glyfuse's Material theme")
    for index, name in enumerate(names):
        y = 222 + index * 42
        is_folder = index < 4
        file_color = BLUE if is_folder else YELLOW if name in ("Cargo.toml", "README.md") else WHITE
        for x, glyfuse in ((28, False), (1040, True)):
            if glyfuse:
                icon = theme_icon(name, is_folder)
                glyph, icon_color = icon["glyph"], icon["color"]
            else:
                glyph, icon_color = stock_glyphs[index], stock_colors[index]
            text(draw, x + 38, y, glyph, 27, icon_color)
            text(draw, x + 73, y, name, 28, file_color, is_folder or name in ("Cargo.toml", "README.md"))
    image.resize((WIDTH, HEIGHT), Image.Resampling.LANCZOS).save(MEDIA / "eza-comparison.png", optimize=True)


TOKEN = re.compile(r"\s+|[A-Za-z_][A-Za-z_0-9]*|[^\w\s]", re.ASCII)


def rust_line(draw, x, y, line):
    for match in TOKEN.finditer(line):
        token = match.group(0)
        color = ("#59C7EF" if token in {"mod", "use"} else
                 "#A7D46F" if token in {"eza", "tui", "anyhow", "glyfuse", "std", "env", "io"} else
                 "#C5A4ED" if token in {"Context", "Result", "Kind", "Resolver", "Theme", "Write", "IsTerminal"} else
                 "#E8B870" if token in {"bail", "use_theme", "self"} else WHITE)
        draw.text(pt(x, y), token, font=font(24), fill=color, anchor="la")
        x += draw.textlength(token, font=font(24)) / SCALE


def render_bat():
    image, draw = base("Same file. One extra icon.", "bat  ·  default header compared with the optional Glyfuse patch")
    lines = (ROOT / "src/main.rs").read_text(encoding="utf-8").splitlines()[:8]
    for x, glyfuse in ((28, False), (1040, True)):
        text(draw, x + 38, 214, "File:", 25, MUTED)
        icon_x = x + 132
        if glyfuse:
            icon = theme_icon("main.rs")
            text(draw, icon_x, 214, icon["glyph"], 25, icon["color"])
            icon_x += 34
        text(draw, icon_x, 214, "src/main.rs", 25, WHITE)
        draw.line((*pt(x + 38, 256), *pt(x + 940, 256)), fill=BORDER, width=SCALE)
        draw.line((*pt(x + 99, 270), *pt(x + 99, 619)), fill=BORDER, width=SCALE)
        for index, line in enumerate(lines):
            y = 280 + index * 41
            text(draw, x + 46, y, str(index + 1).rjust(2), 23, "#73869F")
            rust_line(draw, x + 120, y, line)
    image.resize((WIDTH, HEIGHT), Image.Resampling.LANCZOS).save(MEDIA / "bat-comparison.png", optimize=True)


if __name__ == "__main__":
    render_eza()
    render_bat()
