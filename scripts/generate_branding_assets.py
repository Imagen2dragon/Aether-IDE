#!/usr/bin/env python3
"""
AetherIDE — Automated Brand Asset Generator
Generates full PNG icon resolutions, multi-resolution Windows .ico files,
and Inno Setup wizard bitmaps from source mathematical vector specifications.
"""

import os
from PIL import Image, ImageDraw, ImageFont

ROOT_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
BRANDING_DIR = os.path.join(ROOT_DIR, "branding")
PNG_DIR = os.path.join(BRANDING_DIR, "png")
WIN_DIR = os.path.join(BRANDING_DIR, "windows")
WEBSITE_ASSETS = os.path.join(ROOT_DIR, "website", "assets")

os.makedirs(PNG_DIR, exist_ok=True)
os.makedirs(WIN_DIR, exist_ok=True)
os.makedirs(WEBSITE_ASSETS, exist_ok=True)


def draw_aether_symbol(size=1024, is_installer=False):
    """Draw high-resolution AetherIDE icon with anti-aliasing."""
    # Render at 2x scale for supersampling
    scale = 2
    render_size = size * scale
    im = Image.new("RGBA", (render_size, render_size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(im)

    pad = int(render_size * 0.05)
    r = int(render_size * 0.20)

    # 1. Outer squircle background: Deep navy (#0F172A)
    # Rounded rectangle coordinates
    x0, y0, x1, y1 = pad, pad, render_size - pad, render_size - pad
    draw.rounded_rectangle([x0, y0, x1, y1], radius=r, fill=(15, 23, 42, 255))

    # Border gradient simulation: dual concentric strokes
    stroke_w = int(render_size * 0.025)
    draw.rounded_rectangle([x0, y0, x1, y1], radius=r, outline=(56, 189, 248, 255), width=stroke_w)

    # 2. Coordinate mapping for cybernetic 'A'
    cx = render_size // 2
    apex_y = int(render_size * 0.20)
    base_y = int(render_size * 0.80)
    inner_apex_y = int(render_size * 0.48)
    
    left_outer_x = int(render_size * 0.22)
    left_inner_x = int(render_size * 0.35)
    right_inner_x = int(render_size * 0.65)
    right_outer_x = int(render_size * 0.78)

    # Left Wing (Electric Cyan #38BDF8)
    draw.polygon([
        (cx, apex_y),
        (left_outer_x, base_y),
        (left_inner_x, base_y),
        (cx, inner_apex_y),
    ], fill=(56, 189, 248, 255))

    # Right Wing (Indigo #6366F1)
    draw.polygon([
        (cx, apex_y),
        (right_outer_x, base_y),
        (right_inner_x, base_y),
        (cx, inner_apex_y),
    ], fill=(99, 102, 241, 255))

    # Central Crossbar Bridge
    bridge_y0 = int(render_size * 0.62)
    bridge_y1 = int(render_size * 0.69)
    draw.polygon([
        (int(render_size * 0.38), bridge_y0),
        (int(render_size * 0.62), bridge_y0),
        (int(render_size * 0.59), bridge_y1),
        (int(render_size * 0.41), bridge_y1),
    ], fill=(56, 189, 248, 255))

    # Center Synthesis Core Prism (White #FFFFFF)
    prism_h = int(render_size * 0.12)
    prism_w = int(render_size * 0.06)
    prism_cy = int(render_size * 0.52)
    draw.polygon([
        (cx, prism_cy - prism_h),
        (cx + prism_w, prism_cy),
        (cx, prism_cy + prism_h),
        (cx - prism_w, prism_cy),
    ], fill=(255, 255, 255, 255))

    # Apex Focus Spark
    spark_r = int(render_size * 0.035)
    draw.ellipse([cx - spark_r, apex_y - spark_r, cx + spark_r, apex_y + spark_r], fill=(56, 189, 248, 255))
    inner_spark = int(spark_r * 0.5)
    draw.ellipse([cx - inner_spark, apex_y - inner_spark, cx + inner_spark, apex_y + inner_spark], fill=(255, 255, 255, 255))

    # If installer icon, draw small setup down-arrow / package badge at bottom right
    if is_installer:
        badge_cx = int(render_size * 0.76)
        badge_cy = int(render_size * 0.76)
        badge_r = int(render_size * 0.16)
        draw.ellipse([badge_cx - badge_r, badge_cy - badge_r, badge_cx + badge_r, badge_cy + badge_r], fill=(30, 41, 59, 255), outline=(34, 197, 94, 255), width=int(badge_r * 0.15))
        # Downward arrow
        arrow_w = int(badge_r * 0.5)
        arrow_h = int(badge_r * 0.55)
        draw.polygon([
            (badge_cx - arrow_w, badge_cy - int(arrow_h * 0.3)),
            (badge_cx + arrow_w, badge_cy - int(arrow_h * 0.3)),
            (badge_cx, badge_cy + int(arrow_h * 0.6)),
        ], fill=(34, 197, 94, 255))

    # Downsample with Lanczos anti-aliasing
    final_im = im.resize((size, size), Image.Resampling.LANCZOS)
    return final_im


def generate_all():
    print("Generating AetherIDE Brand Assets...")

    # 1. Generate master 512x512 image
    master_512 = draw_aether_symbol(512, is_installer=False)
    master_installer_512 = draw_aether_symbol(512, is_installer=True)

    # 2. Generate PNG family: 16, 24, 32, 48, 64, 128, 256, 512
    sizes = [16, 24, 32, 48, 64, 128, 256, 512]
    png_images = {}
    for sz in sizes:
        resized = master_512.resize((sz, sz), Image.Resampling.LANCZOS)
        out_path = os.path.join(PNG_DIR, f"aetheride-icon-{sz}.png")
        resized.save(out_path, format="PNG")
        png_images[sz] = resized
        print(f"  [PNG] Saved {out_path} ({sz}x{sz})")

    # Also save favicon.png to website/assets
    fav_path = os.path.join(WEBSITE_ASSETS, "favicon.png")
    png_images[32].save(fav_path, format="PNG")
    print(f"  [Favicon] Saved {fav_path}")

    # 3. Generate Windows .ico files
    ico_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    
    # aetheride.ico (multi-resolution embedded application icon)
    app_ico_path = os.path.join(WIN_DIR, "aetheride.ico")
    master_512.save(app_ico_path, format="ICO", sizes=ico_sizes)
    print(f"  [ICO] Generated {app_ico_path} with {len(ico_sizes)} resolutions")

    # aetheride-installer.ico (installer executable icon)
    installer_ico_path = os.path.join(WIN_DIR, "aetheride-installer.ico")
    master_installer_512.save(installer_ico_path, format="ICO", sizes=ico_sizes)
    print(f"  [ICO] Generated {installer_ico_path} with {len(ico_sizes)} resolutions")

    # Also copy aetheride.ico to website/assets/favicon.ico
    web_ico_path = os.path.join(WEBSITE_ASSETS, "favicon.ico")
    master_512.save(web_ico_path, format="ICO", sizes=[(16, 16), (32, 32), (48, 48)])
    print(f"  [Favicon] Generated {web_ico_path}")

    # 4. Generate Inno Setup Wizard BMPs (24-bit RGB BMP)
    # aetheride-wizard.bmp: 164 x 314
    wizard_w, wizard_h = 164, 314
    wiz_im = Image.new("RGB", (wizard_w, wizard_h), (15, 23, 42))
    wiz_draw = ImageDraw.Draw(wiz_im)

    # Gradient background
    for y in range(wizard_h):
        factor = y / wizard_h
        r = int(15 + factor * 15)  # 15 -> 30
        g = int(23 + factor * 18)  # 23 -> 41
        b = int(42 + factor * 17)  # 42 -> 59
        wiz_draw.line([(0, y), (wizard_w, y)], fill=(r, g, b))

    # Draw vertical accent line on right edge
    wiz_draw.line([(wizard_w - 2, 0), (wizard_w - 2, wizard_h)], fill=(56, 189, 248))
    wiz_draw.line([(wizard_w - 1, 0), (wizard_w - 1, wizard_h)], fill=(99, 102, 241))

    # Center AetherIDE symbol in top region
    sym_size = 96
    sym_x = (wizard_w - sym_size) // 2
    sym_y = 48
    sym_icon = master_512.resize((sym_size, sym_size), Image.Resampling.LANCZOS)
    wiz_im.paste(sym_icon, (sym_x, sym_y), sym_icon)

    # Wordmark text simulation
    # "AETHERIDE"
    wiz_draw.text((wizard_w // 2, 168), "AetherIDE", fill=(248, 250, 252), anchor="mm")
    wiz_draw.text((wizard_w // 2, 186), "BETA.2", fill=(56, 189, 248), anchor="mm")
    wiz_draw.text((wizard_w // 2, 220), "AI-Native", fill=(148, 163, 184), anchor="mm")
    wiz_draw.text((wizard_w // 2, 236), "Engineering", fill=(148, 163, 184), anchor="mm")
    wiz_draw.text((wizard_w // 2, 252), "Environment", fill=(148, 163, 184), anchor="mm")

    wiz_bmp_path = os.path.join(WIN_DIR, "aetheride-wizard.bmp")
    wiz_im.save(wiz_bmp_path, format="BMP")
    print(f"  [BMP] Generated Inno Setup WizardImage: {wiz_bmp_path} ({wizard_w}x{wizard_h})")

    # aetheride-wizard-small.bmp: 55 x 55
    small_w, small_h = 55, 55
    small_im = Image.new("RGB", (small_w, small_h), (15, 23, 42))
    small_icon = master_512.resize((48, 48), Image.Resampling.LANCZOS)
    small_im.paste(small_icon, (3, 3), small_icon)
    small_bmp_path = os.path.join(WIN_DIR, "aetheride-wizard-small.bmp")
    small_im.save(small_bmp_path, format="BMP")
    print(f"  [BMP] Generated Inno Setup WizardSmallImage: {small_bmp_path} ({small_w}x{small_h})")

    print("\nAll Brand Assets Generated Successfully!")


if __name__ == "__main__":
    generate_all()
