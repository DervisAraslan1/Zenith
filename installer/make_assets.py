"""Draws Zenith's icon and installer artwork.

The mark: a night-sky squircle with a glowing horizon arc at the bottom
and, at the very top, a dark glass pill holding a bright star. The zenith
is the highest point of the sky, where the notch sits on the screen.
Everything is drawn in code at 4x and downsampled, so there is no source
file to keep in sync. Run from the repo root after changing it:

    python installer/make_assets.py

Writes assets/icon/zenith.ico, assets/icon/zenith-<size>.png and the
installer's wizard images in installer/art/.
"""

import math
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent.parent
ICON_DIR = ROOT / "assets" / "icon"
ART_DIR = ROOT / "installer" / "art"

SKY_TOP = (8, 10, 30)
SKY_BOTTOM = (34, 52, 140)
HORIZON = (255, 168, 92)
HORIZON_HOT = (255, 226, 170)
PILL_TOP = (36, 38, 54)
PILL_BOTTOM = (6, 7, 14)
STAR = (255, 240, 200)
STAR_GLOW = (255, 196, 110)
SS = 4  # supersampling factor


def squircle_mask(size: int, inset: float, n: float = 5.0) -> Image.Image:
    """Superellipse |x|^n + |y|^n <= 1, the Windows 11 tile shape."""
    big = size * SS
    mask = Image.new("L", (big, big), 0)
    px = mask.load()
    r = big / 2 - inset * SS
    c = big / 2
    for y in range(big):
        dy = abs((y + 0.5 - c) / r)
        if dy >= 1:
            continue
        half = r * (1 - dy**n) ** (1 / n)
        x0 = max(0, int(c - half))
        x1 = min(big, int(c + half) + 1)
        for x in range(x0, x1):
            px[x, y] = 255
    return mask


def vertical_gradient(w: int, h: int, top, bottom) -> Image.Image:
    grad = Image.new("RGB", (1, h))
    for y in range(h):
        t = y / max(1, h - 1)
        grad.putpixel((0, y), tuple(round(a + (b - a) * t) for a, b in zip(top, bottom)))
    return grad.resize((w, h))


def star_points(cx: float, cy: float, r_out: float, r_in: float):
    """A four-pointed sparkle."""
    pts = []
    for i in range(8):
        a = -math.pi / 2 + i * math.pi / 4
        r = r_out if i % 2 == 0 else r_in
        pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
    return pts


def pill_layer(big: int, box, star: bool) -> Image.Image:
    """Dark glass pill with a hairline highlight and a star in the middle."""
    x0, y0, x1, y1 = box
    h = y1 - y0
    layer = Image.new("RGBA", (big, big), (0, 0, 0, 0))

    shadow = Image.new("L", (big, big), 0)
    ImageDraw.Draw(shadow).rounded_rectangle(
        (x0, y0 + h * 0.18, x1, y1 + h * 0.18), radius=h / 2, fill=120
    )
    shadow = shadow.filter(ImageFilter.GaussianBlur(h * 0.22))
    layer.paste((2, 3, 12, 255), (0, 0), shadow)

    mask = Image.new("L", (big, big), 0)
    ImageDraw.Draw(mask).rounded_rectangle(box, radius=h / 2, fill=255)
    grad = Image.new("RGB", (big, big))
    grad.paste(vertical_gradient(big, int(h) + 1, PILL_TOP, PILL_BOTTOM), (0, int(y0)))
    layer.paste(grad, (0, 0), mask)

    edge = max(1.0, h * 0.06)
    shifted = Image.new("L", (big, big), 0)
    ImageDraw.Draw(shifted).rounded_rectangle(
        (x0, y0 + edge, x1, y1 + edge), radius=h / 2, fill=255
    )
    rim = ImageChops.subtract(mask, shifted).point(lambda v: int(v * 0.32))
    layer.paste((255, 255, 255, 255), (0, 0), rim)

    if star:
        cx = (x0 + x1) / 2
        cy = (y0 + y1) / 2
        glow = Image.new("L", (big, big), 0)
        g = h * 0.75
        ImageDraw.Draw(glow).ellipse((cx - g, cy - g, cx + g, cy + g), fill=150)
        glow = glow.filter(ImageFilter.GaussianBlur(h * 0.35))
        layer.paste(STAR_GLOW + (255,), (0, 0), ImageChops.multiply(glow, mask))
        ImageDraw.Draw(layer).polygon(
            star_points(cx, cy, h * 0.36, h * 0.09), fill=STAR + (255,)
        )
    return layer


def icon(size: int) -> Image.Image:
    big = size * SS
    small = size <= 24
    inset = 0 if small else size * 0.035
    tile_mask = squircle_mask(size, inset)

    tile = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    tile.paste(vertical_gradient(big, big, SKY_TOP, SKY_BOTTOM), (0, 0), tile_mask)

    # Horizon: the top of a huge planet-like disc, lit along its rim.
    cx, r = big / 2, big * 0.95
    cy = big * (0.80 if small else 0.86) + r
    disc = Image.new("L", (big, big), 0)
    ImageDraw.Draw(disc).ellipse((cx - r, cy - r, cx + r, cy + r), fill=255)
    glow = disc.filter(ImageFilter.GaussianBlur(big * 0.10)).point(lambda v: min(255, int(v * 1.6)))
    tile.paste(
        HORIZON + (255,),
        (0, 0),
        ImageChops.multiply(glow, tile_mask).point(lambda v: int(v * 0.75)),
    )
    tile.paste((6, 8, 22, 255), (0, 0), ImageChops.multiply(disc, tile_mask))
    rim_w = max(SS * 1.2, big * 0.012)
    inner = Image.new("L", (big, big), 0)
    ImageDraw.Draw(inner).ellipse(
        (cx - r + rim_w, cy - r + rim_w, cx + r - rim_w, cy + r - rim_w), fill=255
    )
    rim = ImageChops.subtract(disc, inner)
    tile.paste(HORIZON_HOT + (255,), (0, 0), ImageChops.multiply(rim, tile_mask))

    if not small:
        # A few faint stars in the sky.
        draw = ImageDraw.Draw(tile)
        for sx, sy, sr, a in (
            (0.22, 0.52, 0.010, 150),
            (0.78, 0.46, 0.012, 170),
            (0.66, 0.64, 0.008, 120),
            (0.30, 0.70, 0.007, 110),
            (0.85, 0.66, 0.007, 100),
        ):
            draw.ellipse(
                (big * (sx - sr), big * (sy - sr), big * (sx + sr), big * (sy + sr)),
                fill=(255, 255, 255, a),
            )
        # Glassy sheen and a hairline rim so the tile holds its edge.
        sheen = Image.new("L", (big, big), 0)
        ImageDraw.Draw(sheen).ellipse((-big * 0.3, -big * 0.8, big * 1.3, big * 0.36), fill=22)
        sheen = sheen.filter(ImageFilter.GaussianBlur(big * 0.04))
        tile.paste((255, 255, 255, 255), (0, 0), ImageChops.multiply(sheen, tile_mask))
        inner_t = squircle_mask(size, inset + 0.6 * (size / 64))
        trim = ImageChops.subtract(tile_mask, inner_t).point(lambda v: int(v * 0.20))
        tile.paste((255, 255, 255, 255), (0, 0), trim)

    pw = 0.62 if small else 0.52
    ph = pw * (15 / 36)
    py = 0.30 if small else 0.27
    box = (big * (0.5 - pw / 2), big * (py - ph / 2), big * (0.5 + pw / 2), big * (py + ph / 2))
    tile.alpha_composite(pill_layer(big, box, star=size >= 16))
    tile.putalpha(ImageChops.multiply(tile.getchannel("A"), tile_mask))
    return tile.resize((size, size), Image.LANCZOS)


def wizard_large(w: int, h: int) -> Image.Image:
    """Left panel of the welcome and finish pages."""
    img = vertical_gradient(w, h, (6, 8, 26), (18, 26, 72)).convert("RGBA")
    glow = Image.new("L", (w, h), 0)
    ImageDraw.Draw(glow).ellipse((-w * 0.6, h * 0.80, w * 1.6, h * 1.4), fill=150)
    glow = glow.filter(ImageFilter.GaussianBlur(w * 0.20))
    img.paste(HORIZON + (255,), (0, 0), glow)
    s = int(w * 0.58)
    mark = icon(256).resize((s, s), Image.LANCZOS)
    my = int(h * 0.36 - s / 2)
    img.alpha_composite(mark, ((w - s) // 2, my))

    draw = ImageDraw.Draw(img)
    try:
        font = ImageFont.truetype("segoeuib.ttf", round(w * 0.16))
    except OSError:
        font = ImageFont.load_default(round(w * 0.16))
    draw.text((w / 2, my + s + w * 0.10), "Zenith", font=font, fill=(255, 255, 255), anchor="mt")
    return img.convert("RGB")


def main() -> None:
    ICON_DIR.mkdir(parents=True, exist_ok=True)
    ART_DIR.mkdir(parents=True, exist_ok=True)

    sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256]
    frames = {s: icon(s) for s in sizes}
    for s in (16, 32, 48, 256):
        frames[s].save(ICON_DIR / f"zenith-{s}.png")
    icon(512).save(ICON_DIR / "zenith-512.png")
    # Pillow writes each size from its own frame when they are appended.
    frames[256].save(
        ICON_DIR / "zenith.ico",
        sizes=[(s, s) for s in sizes],
        append_images=[frames[s] for s in sizes if s != 256],
    )

    # Inno Setup picks the bitmap closest to the screen's DPI.
    for scale, suffix in ((1.0, "100"), (1.25, "125"), (1.5, "150"), (2.0, "200")):
        wizard_large(round(164 * scale), round(314 * scale)).save(ART_DIR / f"wizard-{suffix}.png")
        # Transparent, so it sits on the light and the dark wizard alike.
        icon(round(55 * scale)).save(ART_DIR / f"wizard-small-{suffix}.png")
    print(f"wrote {ICON_DIR} and {ART_DIR}")


if __name__ == "__main__":
    main()
