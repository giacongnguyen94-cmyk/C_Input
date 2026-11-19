import re, sys

BS = chr(92)
src = open(sys.argv[1], 'rb').read().decode('latin-1')


def block(start_pat, end_pat):
    i = src.index(start_pat)
    j = src.index(end_pat, i)
    return src[i:j]


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
    return items


tc = parse_items(block("// TCVN3", "//VPS").split('{', 2)[2])
vni = parse_items(block("//VNIWIN", "//BKHCM2"))
uni = parse_items(block("UnicodeChar UnicodeTable", "};").split('=', 1)[1])
print(len(tc), len(vni), len(uni), file=sys.stderr)
assert len(tc) == 213 and len(vni) == 213 and len(uni) == 213
out = [
    "// Sinh tự động từ bảng mã của UniKey 3.62 (vnconv/data.cpp). Không sửa tay.",
    "// Mỗi dòng: (Unicode, byte TCVN3 (0 = không có), mã VNI Windows (byte cao = dấu, byte thấp = chữ gốc))",
    "pub(crate) const VN_TABLE: [(u32, u8, u16); 213] = [",
]
for u, t, v in zip(uni, tc, vni):
    out.append(f"    (0x{u:04X}, 0x{t:02X}, 0x{v:04X}),")
out.append("];")
open(sys.argv[2], 'w', encoding='utf-8', newline='\n').write("\n".join(out) + "\n")
