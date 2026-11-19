"""Sinh crates/cinput-charset/src/tables.rs từ bảng mã của UniKey 3.62 (vnconv/data.cpp).

    python tools/gen_tables.py <đường dẫn tới Uk362/vnconv/data.cpp> crates/cinput-charset/src/tables.rs

Mã nguồn UniKey 3.62 (GPL, tác giả Phạm Kim Long) không kèm trong repo; tải tại unikey.org.

Mỗi bảng mã cũ được xuất thành chuỗi ký tự cho từng chữ cái tiếng Việt, đúng như Windows biểu diễn
văn bản bảng mã đó trong clipboard: mỗi byte là một ký tự theo trang mã Windows-1252 (riêng CP1258
giải mã theo trang mã 1258 — chữ gốc + dấu thanh tổ hợp).
"""
import re
import sys

BS = chr(92)
src = open(sys.argv[1], 'rb').read().decode('latin-1')
N = 213          # số mục trong mỗi bảng UniKey
LETTERS = 186    # 186 mục đầu là chữ cái; phần sau là ký hiệu Windows (đã xử lý qua trang mã)


def block(start, ends):
    i = src.index(start)
    j = min(src.index(e, i + len(start)) for e in ends if src.find(e, i + len(start)) != -1)
    return src[i + len(start):j]


def parse_items(body):
    body = re.sub(r'//[^\n]*', '', body)
    items = []
    for m in re.finditer(r"'(" + BS * 2 + r".|.)'|0x[0-9A-Fa-f]+|\b\d+\b", body, re.S):
        t = m.group()
        if t.startswith("'"):
            c = m.group(1)
            items.append(ord(c[1]) if c.startswith(BS) else ord(c))
        else:
            items.append(int(t, 0))
    assert len(items) >= N, len(items)
    return items[:N]


def byte_char(b, codepage):
    try:
        return bytes([b]).decode(codepage)
    except UnicodeDecodeError:
        return chr(b)


SINGLE_ENDS = ['//VPS', '//VISCII', '// BKHCM1', '//Vietware-F', '};']
DOUBLE_ENDS = ['//BKHCM2', '//VIETWARE-X', '};']
unicode = parse_items(block('UnicodeChar UnicodeTable[TOTAL_VNCHARS] =', ['};']))
tables = [
    ('tcvn3', 'single', block('// TCVN3', SINGLE_ENDS), 'cp1252'),
    ('vps', 'single', block('//VPS', SINGLE_ENDS), 'cp1252'),
    ('viscii', 'single', block('//VISCII', SINGLE_ENDS), 'cp1252'),
    ('bkhcm1', 'single', block('// BKHCM1', SINGLE_ENDS), 'cp1252'),
    ('vietware_f', 'single', block('//Vietware-F', SINGLE_ENDS), 'cp1252'),
    ('vni', 'double', block('//VNIWIN', DOUBLE_ENDS), 'cp1252'),
    ('bkhcm2', 'double', block('//BKHCM2', DOUBLE_ENDS), 'cp1252'),
    ('vietware_x', 'double', block('//VIETWARE-X', DOUBLE_ENDS), 'cp1252'),
    ('cp1258', 'double', block('//Windows CP 1258', ['//Windows CP1258 - with', '};']), 'cp1258'),
]


def rust_str(s):
    out = []
    for ch in s:
        o = ord(ch)
        if ch == '"' or ch == BS:
            out.append(BS + ch)
        elif o < 0x20 or 0x7f <= o < 0xa0:
            out.append(BS + 'u{%X}' % o)
        else:
            out.append(ch)
    return '"' + ''.join(out) + '"'


lines = [
    "// Sinh tự động bằng tools/gen_tables.py từ bảng mã của UniKey 3.62 (vnconv/data.cpp). Không sửa tay.",
    "",
    f"/// {LETTERS} chữ cái tiếng Việt (hoa, thường xen kẽ), cùng thứ tự với các bảng bên dưới.",
    f"pub(crate) const UNICODE: [char; {LETTERS}] = [",
]
for i in range(0, LETTERS, 12):
    lines.append("    " + " ".join(f"'{chr(u)}'," for u in unicode[i:min(i + 12, LETTERS)]))
lines.append("];")
lines.append("")
lines.append("/// Mỗi bảng mã cũ: chuỗi biểu diễn từng chữ cái (\"\" = bảng mã không có chữ đó).")
lines.append(f"pub(crate) const LEGACY: [(&str, [&str; {LETTERS}]); {len(tables)}] = [")
for name, kind, body, cp in tables:
    items = parse_items(body)
    reps = []
    for v in items[:LETTERS]:
        if v == 0:
            reps.append('')
        elif kind == 'single' or v < 0x100:
            reps.append(byte_char(v & 0xFF, cp))
        else:
            reps.append(byte_char(v & 0xFF, cp) + byte_char(v >> 8, cp))
    lines.append(f'    ("{name}", [')
    for i in range(0, LETTERS, 12):
        lines.append("        " + " ".join(rust_str(r) + "," for r in reps[i:i + 12]))
    lines.append("    ]),")
lines.append("];")
open(sys.argv[2], 'w', encoding='utf-8', newline='\n').write("\n".join(lines) + "\n")
print("ok:", ", ".join(t[0] for t in tables))
