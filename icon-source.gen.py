#!/usr/bin/env python3
"""Generate the Runebook app icon source (1024×1024 PNG).

A warm-charcoal rounded tile with the Elder Futhark *raidho* rune (ᚱ — the "R"
rune, on-theme for Rune-book) in the ember-orange accent, plus a small bookmark
ribbon nodding to "book". Drawn at 4× and downsampled for clean anti-aliasing.
Run, then `npx tauri icon icon-source.png -o src-tauri/icons`.
"""
from PIL import Image, ImageDraw

S = 1024
SS = 4  # supersample
W = S * SS

img = Image.new("RGBA", (W, W), (0, 0, 0, 0))
d = ImageDraw.Draw(img)

def px(v):  # 1024-space → supersampled
    return v * SS

# Rounded-square tile background (warm charcoal), full-bleed with soft corners.
BG = (33, 27, 22, 255)       # #211b16
BG2 = (46, 37, 29, 255)      # subtle top sheen
EMBER = (232, 93, 4, 255)    # #e85d04 accent
EMBER_DIM = (180, 70, 6, 255)
radius = px(220)
d.rounded_rectangle([0, 0, W, W], radius=radius, fill=BG)
# faint lighter band near the top for a touch of depth
d.rounded_rectangle([0, 0, W, px(360)], radius=radius, fill=BG2)
d.rectangle([0, px(180), W, px(360)], fill=BG2)

# Bookmark ribbon (the "book" nod): a thin vertical strap near the right edge.
rx = px(720)
d.rectangle([rx, px(150), rx + px(96), px(560)], fill=EMBER_DIM)
# notched tail
d.polygon(
    [(rx, px(560)), (rx + px(96), px(560)), (rx + px(96), px(640)),
     (rx + px(48), px(592)), (rx, px(640))],
    fill=EMBER_DIM,
)

# Raidho rune ᚱ (reads as R): vertical stem + top bowl + diagonal leg.
stroke = px(96)
pts_stem = [(px(360), px(300)), (px(360), px(760))]
pts_bowl = [(px(360), px(300)), (px(636), px(410)), (px(360), px(520))]
pts_leg = [(px(360), px(515)), (px(666), px(760))]

def stroke_path(points, color, width):
    d.line(points, fill=color, width=width, joint="curve")
    r = width // 2
    for (x, y) in points:  # round caps
        d.ellipse([x - r, y - r, x + r, y + r], fill=color)

stroke_path(pts_stem, EMBER, stroke)
stroke_path(pts_bowl, EMBER, stroke)
stroke_path(pts_leg, EMBER, stroke)

img = img.resize((S, S), Image.LANCZOS)
img.save("icon-source.png")
print("wrote icon-source.png", img.size)
