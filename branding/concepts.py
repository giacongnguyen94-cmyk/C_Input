"""5 phương án logo C_Input để chọn. Mỗi phương án dùng MỘT hình cho mọi cỡ (exe, taskbar, khay, giao diện).

    python branding/concepts.py        -> branding/concepts/*.png + concepts_sheet.png

Khung chung dòng C: ô đen than bo góc, viền mảnh màu nhấn, chữ "C" dày hở bên phải.
Dấu ấn riêng mỗi phương án:
  A. Móc     — đầu trên chữ C vươn thành cái móc của ơ / ư.
  B. Mũ      — dấu mũ "^" lớn đội trên chữ C (như Ĉ), con trỏ trong lòng.
  C. Dấu sắc — nét dấu sắc bay ra từ miệng chữ C, con trỏ trong lòng.
  D. Đ       — gạch ngang xuyên thân trái chữ C, gợi chữ Đ chỉ tiếng Việt có.
  E. Cĩ      — chữ i trong lòng chữ C, dấu chấm thay bằng dấu ngã "~" (ĩ).
"""
import math
import os

from PIL import Image, ImageDraw, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "concepts")

BG_TOP, BG_BOTTOM = (36, 36, 39), (11, 11, 12)
ACC_TOP, ACC_BOTTOM, BORDER = (74, 222, 160), (5, 150, 105), (16, 185, 129)
LIGHT = (236, 253, 245)
GREY_TOP, GREY_BOTTOM, GREY_LIGHT = (170, 175, 186), (100, 106, 120), (205, 209, 217)


def vgrad(S, top, bottom):
    g = Image.new("RGBA", (S, S))
    d = ImageDraw.Draw(g)
    for y in range(S):
        t = y / (S - 1)
        d.line([(0, y), (S, y)], fill=tuple(int(top[i] + (bottom[i] - top[i]) * t) for i in range(3)) + (255,))
    return g


def stroke(d, pts, w0, w1=None):
    w1 = w0 if w1 is None else w1
    n = len(pts)
    for i, (x, y) in enumerate(pts):
        r = (w0 + (w1 - w0) * i / max(1, n - 1)) / 2
        d.ellipse([x - r, y - r, x + r, y + r], fill=255)


def arc_pts(cx, cy, r, a0, a1, n=900):
    return [(cx + r * math.cos(math.radians(a)), cy + r * math.sin(math.radians(a)))
            for a in [a0 + (a1 - a0) * i / n for i in range(n + 1)]]


def bezier(p0, p1, p2, p3, n=400):
    out = []
    for i in range(n + 1):
        t = i / n
        a, b, c, e = (1 - t) ** 3, 3 * (1 - t) ** 2 * t, 3 * (1 - t) * t ** 2, t ** 3
        out.append((a * p0[0] + b * p1[0] + c * p2[0] + e * p3[0], a * p0[1] + b * p1[1] + c * p2[1] + e * p3[1]))
    return out


def caret(d, x, y0, y1, w):
    d.rounded_rectangle([x - w / 2, y0, x + w / 2, y1], radius=w / 2, fill=255)


# ------------------------------------------------------------------ các phương án
# Mỗi hàm vẽ lên hai mặt nạ: acc (màu nhấn, chuyển sắc) và light (màu sáng). Toạ độ theo S.

def concept_a(S, k, acc, light):  # Móc
    cx, cy, r, w = 0.47 * S, 0.54 * S, 0.25 * S, 0.125 * S * k
    a1 = 300
    pts = arc_pts(cx, cy, r, 40, a1)
    stroke(acc, pts, w)
    p0 = pts[-1]
    tx, ty = -math.sin(math.radians(a1)), math.cos(math.radians(a1))
    p1 = (p0[0] + tx * 0.10 * S, p0[1] + ty * 0.10 * S)
    p3 = (cx + 0.33 * S, cy - 0.35 * S)
    p2 = (p3[0] - 0.03 * S, p3[1] + 0.11 * S)
    stroke(acc, bezier(p0, p1, p2, p3), w, w * 0.55)
    caret(light, cx + 0.03 * S, cy - 0.11 * S, cy + 0.11 * S, 0.062 * S * k)


def concept_b(S, k, acc, light):  # Mũ
    cx, cy, r, w = 0.48 * S, 0.605 * S, 0.22 * S, 0.115 * S * k
    stroke(acc, arc_pts(cx, cy, r, 42, 318), w)
    hw, st = 0.165 * S, 0.068 * S * k
    peak, foot = 0.185 * S, 0.285 * S
    hx = cx + 0.01 * S
    stroke(light, [(hx - hw + (hw) * i / 200, foot - (foot - peak) * i / 200) for i in range(201)]
           + [(hx + hw * i / 200, peak + (foot - peak) * i / 200) for i in range(201)], st)
    caret(light, cx + 0.03 * S, cy - 0.10 * S, cy + 0.10 * S, 0.06 * S * k)


def concept_c(S, k, acc, light):  # Dấu sắc
    cx, cy, r, w = 0.45 * S, 0.56 * S, 0.245 * S, 0.125 * S * k
    stroke(acc, arc_pts(cx, cy, r, 45, 315), w)
    # dấu sắc: nét xiên từ dưới-trái lên trên-phải, đầu trên to, đầu dưới thon
    p0, p1 = (cx + 0.20 * S, cy - 0.25 * S), (cx + 0.33 * S, cy - 0.39 * S)
    stroke(acc, [(p0[0] + (p1[0] - p0[0]) * i / 200, p0[1] + (p1[1] - p0[1]) * i / 200) for i in range(201)],
           0.05 * S * k, 0.105 * S * k)
    caret(light, cx + 0.035 * S, cy - 0.11 * S, cy + 0.11 * S, 0.062 * S * k)


def concept_d(S, k, acc, light):  # Đ
    cx, cy, r, w = 0.5 * S, 0.5 * S, 0.27 * S, 0.13 * S * k
    stroke(acc, arc_pts(cx, cy, r, 45, 315), w)
    bh = 0.07 * S * k
    x0, x1 = cx - r - 0.085 * S, cx - r + 0.085 * S
    light.rounded_rectangle([x0, cy - bh / 2, x1, cy + bh / 2], radius=bh / 2, fill=255)
    caret(light, cx + 0.06 * S, cy - 0.11 * S, cy + 0.11 * S, 0.062 * S * k)


def concept_e(S, k, acc, light):  # Cĩ
    cx, cy, r, w = 0.47 * S, 0.55 * S, 0.27 * S, 0.12 * S * k
    stroke(acc, arc_pts(cx, cy, r, 42, 318), w)
    ix = cx + 0.045 * S
    caret(light, ix, cy - 0.02 * S, cy + 0.15 * S, 0.072 * S * k)
    # dấu ngã thay cho dấu chấm của chữ i
    tw, ta = 0.07 * S, 0.022 * S
    ty = cy - 0.095 * S
    pts = [(ix - tw + 2 * tw * i / 300, ty - ta * math.sin(2 * math.pi * i / 300)) for i in range(301)]
    stroke(acc, pts, 0.052 * S * k)


CONCEPTS = [
    ("A", "Móc ơ/ư", concept_a),
    ("B", "Mũ ^", concept_b),
    ("C", "Dấu sắc", concept_c),
    ("D", "Chữ Đ", concept_d),
    ("E", "Cĩ", concept_e),
]


def render(fn, size, on=True):
    ss = 4 if size >= 64 else 8
    S = size * ss
    k = 1.3 if size <= 20 else 1.18 if size <= 32 else 1.08 if size <= 48 else 1.0
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    tile = Image.new("L", (S, S), 0)
    ImageDraw.Draw(tile).rounded_rectangle([0, 0, S - 1, S - 1], radius=int(S * 0.225), fill=255)
    img.paste(vgrad(S, BG_TOP, BG_BOTTOM), (0, 0), tile)
    if size > 32:
        inset = int(S * 0.035)
        ImageDraw.Draw(img).rounded_rectangle(
            [inset, inset, S - 1 - inset, S - 1 - inset], radius=int(S * 0.195),
            outline=(BORDER if on else (95, 100, 112)) + (150,), width=max(1, int(S * 0.011)))
    acc = Image.new("L", (S, S), 0)
    light = Image.new("L", (S, S), 0)
    fn(S, k, ImageDraw.Draw(acc), ImageDraw.Draw(light))
    top, bottom = (ACC_TOP, ACC_BOTTOM) if on else (GREY_TOP, GREY_BOTTOM)
    img.paste(vgrad(S, top, bottom), (0, 0), acc)
    img.paste(Image.new("RGBA", (S, S), (LIGHT if on else GREY_LIGHT) + (255,)), (0, 0), light)
    return img.resize((size, size), Image.LANCZOS)


def font(sz, bold=False):
    try:
        return ImageFont.truetype("C:/Windows/Fonts/" + ("seguisb.ttf" if bold else "segoeui.ttf"), sz)
    except OSError:
        return ImageFont.load_default()


def sheet():
    col, W = 300, 300 * 5 + 40
    H = 640
    im = Image.new("RGBA", (W, H), (244, 244, 246, 255))
    d = ImageDraw.Draw(im)
    for i, (tag, name, fn) in enumerate(CONCEPTS):
        x = 20 + i * col
        d.rounded_rectangle([x + 6, 14, x + col - 6, H - 14], radius=18, fill=(255, 255, 255, 255), outline=(225, 225, 230))
        d.text((x + 24, 28), f"Phương án {tag}", font=font(26, True), fill=(20, 20, 24))
        d.text((x + 24, 62), name, font=font(19), fill=(110, 110, 120))
        big = render(fn, 240)
        im.paste(big, (x + 30, 100), big)
        # cỡ nhỏ thật: 64 / 48 / 32 / 24 / 16
        sx = x + 26
        for s in (64, 48, 32, 24, 16):
            a = render(fn, s)
            im.paste(a, (sx, 360 + (64 - s)), a)
            sx += s + 12
        # khay hệ thống: thanh taskbar tối và sáng, bật / tắt
        for j, (bg, label) in enumerate((((32, 32, 36), "Taskbar tối"), ((230, 233, 238), "Taskbar sáng"))):
            y = 448 + j * 74
            d.rounded_rectangle([x + 24, y, x + col - 24, y + 56], radius=10, fill=bg + (255,))
            for kk, on in enumerate((True, False)):
                a = render(fn, 24, on)
                im.paste(a, (x + 40 + kk * 44, y + 16), a)
                a = render(fn, 16, on)
                im.paste(a, (x + 140 + kk * 34, y + 20), a)
            d.text((x + 214, y + 18), label.split()[1], font=font(15), fill=(150, 150, 160) if j == 0 else (90, 90, 100))
    d.text((24, H - 36), "Hàng nhỏ: 64 · 48 · 32 · 24 · 16 px (cỡ thật).  Taskbar: 24 px và 16 px, bản bật (xanh) và tắt (xám).",
           font=font(15), fill=(120, 120, 130))
    return im


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    for tag, _, fn in CONCEPTS:
        render(fn, 1024).save(os.path.join(OUT, f"{tag}_1024.png"))
        render(fn, 1024, on=False).save(os.path.join(OUT, f"{tag}_off_1024.png"))
    sheet().save(os.path.join(HERE, "concepts_sheet.png"))
    print("ok")
