"""Logo C_Input (phương án B — "Mũ ^"): một hình duy nhất cho exe, taskbar, khay hệ thống và giao diện.

    python branding/make_logo.py

Chữ "C" dày hở phải đội dấu mũ lớn như "Ĉ", trong lòng là vạch con trỏ đang gõ.
Hình vẽ nằm ở branding/concepts.py (concept_b); file này chỉ xuất ra các cỡ cần dùng:
  - branding/logo_*.png, logo.svg-tương đương ở 1024px
  - app/res/logo.png   (giao diện + icon cửa sổ trên taskbar)
  - app/res/app.ico    (icon file exe)
  - app/res/tray_on.ico / tray_off.ico (khay hệ thống: xanh = tiếng Việt, xám = tiếng Anh)
Mọi cỡ cùng một hình; cỡ nhỏ chỉ làm nét dày hơn và bỏ viền mảnh.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from concepts import concept_b, render  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
APP_RES = os.path.join(HERE, "..", "app", "res")
ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]


def save_ico(path, on):
    frames = [render(concept_b, s, on) for s in ICO_SIZES]
    frames[-1].save(path, format="ICO", sizes=[(s, s) for s in ICO_SIZES], append_images=frames[:-1])


if __name__ == "__main__":
    os.makedirs(APP_RES, exist_ok=True)
    render(concept_b, 1024).save(os.path.join(HERE, "logo_1024.png"))
    render(concept_b, 1024, on=False).save(os.path.join(HERE, "logo_off_1024.png"))
    for s in (16, 24, 32, 48, 64, 128, 256, 512):
        render(concept_b, s).save(os.path.join(HERE, f"logo_{s}.png"))
    render(concept_b, 256).save(os.path.join(APP_RES, "logo.png"))
    save_ico(os.path.join(APP_RES, "app.ico"), True)
    save_ico(os.path.join(APP_RES, "tray_on.ico"), True)
    save_ico(os.path.join(APP_RES, "tray_off.ico"), False)
    print("written:", HERE, "+", os.path.normpath(APP_RES))
