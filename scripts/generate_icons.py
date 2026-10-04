"""Generate high-resolution Windows icons and installer bitmaps for FileViewer.
Matches the aesthetic defined in assets/header.svg (Precise by Construction).
"""

from PIL import Image, ImageDraw, ImageFont
import math
import os

def render_icon(size=1024):
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Base squircle / rounded rectangle
    margin = size * 0.05
    radius = size * 0.22
    rect = [margin, margin, size - margin, size - margin]

    # Background gradient: Dark obsidian / charcoal
    # We will draw a smooth rounded rectangle
    bg = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    bg_draw = ImageDraw.Draw(bg)
    bg_draw.rounded_rectangle(rect, radius=radius, fill=(18, 18, 22, 255), outline=(60, 60, 68, 255), width=int(size * 0.015))

    # Inner subtle glow / border
    inner_margin = margin + size * 0.015
    inner_radius = radius - size * 0.015
    bg_draw.rounded_rectangle([inner_margin, inner_margin, size - inner_margin, size - inner_margin],
                              radius=inner_radius, outline=(38, 38, 44, 180), width=int(size * 0.008))

    img = Image.alpha_composite(img, bg)
    draw = ImageDraw.Draw(img)

    # Coordinates from SVG:
    # Bounding box in SVG: X from 521 to 679 (width 158), Y from 67 to 177 (height 110)
    # Center: (600, 122)
    svg_cx = 600.0
    svg_cy = 122.0
    scale = (size * 0.58) / 158.0  # fills ~58% of icon with symbol
    target_cx = size / 2.0
    target_cy = size / 2.0

    def t(x, y):
        nx = target_cx + (x - svg_cx) * scale
        ny = target_cy + (y - svg_cy) * scale
        return (nx, ny)

    # Draw corner brackets (stroke width ~ 1.5 in SVG -> scaled)
    corner_stroke = max(2, int(1.8 * scale))
    bracket_color = (130, 130, 138, 255)

    # Top-left corner
    draw.line([t(521, 93), t(521, 67), t(547, 67)], fill=bracket_color, width=corner_stroke, joint="miter")
    # Top-right corner
    draw.line([t(653, 67), t(679, 67), t(679, 93)], fill=bracket_color, width=corner_stroke, joint="miter")
    # Bottom-left corner
    draw.line([t(521, 151), t(521, 177), t(547, 177)], fill=bracket_color, width=corner_stroke, joint="miter")
    # Bottom-right corner
    draw.line([t(653, 177), t(679, 177), t(679, 151)], fill=bracket_color, width=corner_stroke, joint="miter")

    # Monogram 'F'
    # Vertical spine: 549, 86 to 562, 158
    f_stem = [t(549, 86), t(562, 86), t(562, 158), t(549, 158)]
    # Top arm: 562, 86 -> 615, 86 -> 605, 99 -> 562, 99
    f_top = [t(562, 86), t(615, 86), t(605, 99), t(562, 99)]
    # Mid arm: 562, 116 -> 600, 116 -> 590, 129 -> 562, 129
    f_mid = [t(562, 116), t(600, 116), t(590, 129), t(562, 129)]

    # Draw F polygons in crisp white/off-white
    draw.polygon(f_stem, fill=(245, 245, 248, 255))
    draw.polygon(f_top, fill=(245, 245, 248, 255))
    draw.polygon(f_mid, fill=(230, 230, 235, 255))

    # Spine tree view
    spine_color = (155, 155, 165, 255)
    spine_width = max(2, int(1.7 * scale))

    # Vertical trunk
    draw.line([t(635, 100), t(635, 144)], fill=spine_color, width=spine_width)
    # Three horizontal branches
    draw.line([t(635, 100), t(643, 100)], fill=spine_color, width=spine_width)
    draw.line([t(635, 122), t(643, 122)], fill=spine_color, width=spine_width)
    draw.line([t(635, 144), t(643, 144)], fill=spine_color, width=spine_width)

    # 3 node circles
    node_r = 3.6 * scale
    for ny in [100, 122, 144]:
        cx, cy = t(649, ny)
        draw.ellipse([cx - node_r, cy - node_r, cx + node_r, cy + node_r], fill=(180, 185, 195, 255))

    return img

def create_installer_sidebar(width=164, height=314):
    # Standard Inno Setup wizard image is 164x314 (or 2x 328x628)
    # High DPI 328x628 works well
    w = 328
    h = 628
    img = Image.new("RGB", (w, h), (18, 18, 22))
    draw = ImageDraw.Draw(img)

    # Subtle diagonal background gradient or grid lines
    for y in range(0, h, 20):
        draw.line([(0, y), (w, y)], fill=(24, 24, 30), width=1)
    for x in range(0, w, 20):
        draw.line([(x, 0), (x, h)], fill=(24, 24, 30), width=1)

    # Place FileViewer icon centered on upper-middle
    icon = render_icon(size=180)
    img.paste(icon, ((w - 180) // 2, 130), icon)

    # Draw separator line
    draw.line([(40, 360), (w - 40, 360)], fill=(60, 60, 70), width=2)

    # Downscale smoothly to standard 164x314
    return img.resize((width, height), Image.Resampling.LANCZOS)

def create_installer_small(width=55, height=55):
    # Standard Inno Setup small wizard header image is 55x55
    icon = render_icon(size=256)
    bg = Image.new("RGB", (width, height), (240, 240, 240))
    # resize icon to fit
    scaled_icon = icon.resize((48, 48), Image.Resampling.LANCZOS)
    bg.paste(scaled_icon, ((width - 48) // 2, (height - 48) // 2), scaled_icon)
    return bg

def main():
    os.makedirs("assets/installer", exist_ok=True)

    master = render_icon(1024)
    master.save("assets/icon.png", format="PNG")
    master.save("assets/installer/app-icon.png", format="PNG")

    # Generate multi-size ICO
    icon_sizes = [(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (24, 24), (16, 16)]
    master.save("assets/icon.ico", format="ICO", sizes=icon_sizes)
    master.save("assets/installer/setup-icon.ico", format="ICO", sizes=icon_sizes)

    # Installer wizard images (Inno Setup requires BMP)
    wizard_large = create_installer_sidebar(164, 314)
    wizard_large.save("assets/installer/wizard-large.bmp", format="BMP")

    wizard_small = create_installer_small(55, 55)
    wizard_small.save("assets/installer/wizard-small.bmp", format="BMP")

    # Also update assets/icons for cross-platform Linux & desktop use
    icons_dir = "assets/icons"
    os.makedirs(icons_dir, exist_ok=True)
    for sz in [16, 32, 48, 64, 128, 256, 512]:
        resized = master.resize((sz, sz), Image.Resampling.LANCZOS)
        resized.save(os.path.join(icons_dir, f"{sz}.png"), format="PNG")

    print("Icons and installer bitmaps successfully generated!")

if __name__ == "__main__":
    main()
