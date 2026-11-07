#!/usr/bin/env python3
"""
Renders the Horizon Android PRO signature 'H' icon mark to crisp PNG.
Features:
- Material 3 squircle plate with subtle ambient glow and border
- Crisp porcelain-white left pillar (#F8FAFC)
- Radiant electric violet right pillar (#9A86FF -> #7B52FF)
- Dynamic slanted horizon bridge sweeping from left to right pillar
- Full anti-aliasing via signed distance fields (SDF)
"""
import math
import struct
import zlib
from pathlib import Path

def render_horizon_icon(width=1024, height=1024):
    cx = width * 0.5
    cy = height * 0.5

    logo_size = width * 0.62
    ox = cx - logo_size * 0.5
    oy = cy - logo_size * 0.5

    tile_half_w = width * 0.43
    tile_half_h = height * 0.43
    tile_r = width * 0.21

    pixels = bytearray(width * height * 4)

    def sd_rounded_box(px, py, hw, hh, r):
        qx = abs(px) - hw + r
        qy = abs(py) - hh + r
        return min(max(qx, qy), 0.0) + math.hypot(max(qx, 0.0), max(qy, 0.0)) - r

    def sd_seg(px, py, ax, ay, bx, by):
        pax, pay = px - ax, py - ay
        bax, bay = bx - ax, by - ay
        t = max(0.0, min(1.0, (pax * bax + pay * bay) / (bax * bax + bay * bay)))
        dx, dy = pax - bax * t, pay - bay * t
        return math.hypot(dx, dy)

    u_to_x = lambda u: ox + u * logo_size
    v_to_y = lambda v: oy + v * logo_size

    l_ax, l_ay = u_to_x(0.23), v_to_y(0.18)
    l_bx, l_by = u_to_x(0.23), v_to_y(0.82)
    l_r = 0.092 * logo_size

    r_ax, r_ay = u_to_x(0.77), v_to_y(0.18)
    r_bx, r_by = u_to_x(0.77), v_to_y(0.82)
    r_r = 0.092 * logo_size

    b_ax, b_ay = u_to_x(0.23), v_to_y(0.56)
    b_bx, b_by = u_to_x(0.77), v_to_y(0.42)
    b_r = 0.086 * logo_size

    glow_radius = width * 0.05
    shadow_offset = width * 0.008

    idx = 0
    for y in range(height):
        py = y + 0.5
        dy_center = py - cy
        v_grad = y / height
        for x in range(width):
            px = x + 0.5
            dx_center = px - cx

            d_tile = sd_rounded_box(dx_center, dy_center, tile_half_w, tile_half_h, tile_r)

            alpha_glow = 0.0
            if d_tile > 0:
                if d_tile < glow_radius:
                    alpha_glow = ((1.0 - d_tile / glow_radius) ** 2) * 0.35

            tile_alpha = max(0.0, min(1.0, 0.5 - d_tile))

            if tile_alpha <= 0.0 and alpha_glow <= 0.0:
                idx += 4
                continue

            pr = int(23 - 9 * v_grad)
            pg = int(25 - 10 * v_grad)
            pb = int(38 - 16 * v_grad)
            pa = int(255 * tile_alpha)

            stroke_d = abs(d_tile + 1.2)
            if stroke_d < 1.8:
                s_factor = (1.0 - stroke_d / 1.8) * 0.55 * tile_alpha
                pr = int(pr * (1 - s_factor) + 72 * s_factor)
                pg = int(pg * (1 - s_factor) + 75 * s_factor)
                pb = int(pb * (1 - s_factor) + 115 * s_factor)

            d_left = sd_seg(px, py, l_ax, l_ay, l_bx, l_by) - l_r
            d_right = sd_seg(px, py, r_ax, r_ay, r_bx, r_by) - r_r
            d_bridge = sd_seg(px, py, b_ax, b_ay, b_bx, b_by) - b_r

            a_left = max(0.0, min(1.0, 0.5 - d_left))
            a_right = max(0.0, min(1.0, 0.5 - d_right))
            a_bridge = max(0.0, min(1.0, 0.5 - d_bridge))

            d_h_shadow = min(
                sd_seg(px, py - shadow_offset, l_ax, l_ay, l_bx, l_by) - l_r,
                sd_seg(px, py - shadow_offset, r_ax, r_ay, r_bx, r_by) - r_r,
                sd_seg(px, py - shadow_offset, b_ax, b_ay, b_bx, b_by) - b_r,
            )
            if d_h_shadow < 18.0 and (a_left < 0.99 and a_right < 0.99 and a_bridge < 0.99):
                sh_a = ((1.0 - max(0.0, d_h_shadow) / 18.0) ** 2) * 0.45 * tile_alpha
                pr = int(pr * (1.0 - sh_a))
                pg = int(pg * (1.0 - sh_a))
                pb = int(pb * (1.0 - sh_a))

            ar = int(163 - 49 * v_grad)
            ag = int(143 - 72 * v_grad)
            ab = 255

            if a_right > 0.0:
                f = a_right * tile_alpha
                pr = int(pr * (1 - f) + ar * f)
                pg = int(pg * (1 - f) + ag * f)
                pb = int(pb * (1 - f) + ab * f)

            if a_bridge > 0.0:
                f = a_bridge * tile_alpha
                br = int(168 - 35 * v_grad)
                bg = int(148 - 60 * v_grad)
                bb = 255
                pr = int(pr * (1 - f) + br * f)
                pg = int(pg * (1 - f) + bg * f)
                pb = int(pb * (1 - f) + bb * f)

            if a_left > 0.0:
                f = a_left * tile_alpha
                lr = int(248 - 18 * v_grad)
                lg = int(249 - 16 * v_grad)
                lb = int(252 - 10 * v_grad)
                pr = int(pr * (1 - f) + lr * f)
                pg = int(pg * (1 - f) + lg * f)
                pb = int(pb * (1 - f) + lb * f)

            if tile_alpha < 1.0 and alpha_glow > 0.0:
                glow_r, glow_g, glow_b = 154, 134, 255
                ga = alpha_glow * (1.0 - tile_alpha)
                pr = int(pr * tile_alpha + glow_r * ga)
                pg = int(pg * tile_alpha + glow_g * ga)
                pb = int(pb * tile_alpha + glow_b * ga)
                pa = int(pa + 255 * ga)

            pixels[idx] = pr
            pixels[idx + 1] = pg
            pixels[idx + 2] = pb
            pixels[idx + 3] = min(255, pa)
            idx += 4

    raw = bytearray()
    for row in range(height):
        raw.append(0)
        raw.extend(pixels[row * width * 4 : (row + 1) * width * 4])
    compressed = zlib.compress(bytes(raw), 9)

    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    header = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return header + chunk(b"IHDR", ihdr) + chunk(b"IDAT", compressed) + chunk(b"IEND", b"")

if __name__ == "__main__":
    out_dir = Path("packaging/android/res/drawable-nodpi")
    out_path = out_dir / "launcher_icon.png"
    png_data = render_horizon_icon(1024, 1024)
    out_path.write_bytes(png_data)
    print(f"Generated {out_path} ({len(png_data)} bytes)")
