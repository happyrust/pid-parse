# -*- coding: utf-8 -*-
r"""At what size does GDI draw a character the selected font does not carry?

The reference plots set a character their label's font cannot draw -- the `℃`
of `L=35℃`, the A2 sheet's `动火点`, all named in Arial Narrow -- in NSimSun
at `Tf 9.6`: an em of 20 device units (1/150 in) where Arial Narrow itself is
on a 24-unit cell. `pid-plot/src/write/borrowed.rs` letters those runs at 20
units, and this is the measurement the 20 rests on. See
`docs/analysis/2026-11-11-a-borrowed-glyph-is-set-at-the-named-fonts-character-height.md`.

It asks this machine's GDI, so it reads what the plotter read only on a
Windows whose font association is the plotter's: a Chinese system, where
`HKLM\SYSTEM\CurrentControlSet\Control\FontAssoc` gives the ANSI character
set an associated font (`Associated Charset`: `ANSI(00) = YES`;
`Associated DefaultFonts`: `FontPackage = 新宋体`). Everything is drawn on a
memory DC; nothing is written anywhere.

Three readings:

  python -X utf8 tools/gdi_font_association.py bitmap [--face "Arial Narrow"] [--cells 24,22,23,25,27,29]

    Select the face by cell height (a positive `lfHeight`, the way the
    plotter sizes it), draw `动` and `℃` through it with `TextOutW`, and find
    the em at which 新宋体 / 宋体 selected directly draw the identical
    bitmap. For Arial Narrow at 24 that em is 20 -- the font's *character
    height*, `tmHeight - tmInternalLeading` -- and not the 24 of its cell nor
    the 21.195 of its em. `TextOutA` on the same character draws something
    else, so the plotter went through the wide-character call.

  python -X utf8 tools/gdi_font_association.py sweep <face> <first cell> <last cell> [--vdmx <font file>]

    One row per cell height: the font's TEXTMETRIC, its character height,
    and the advance `GetTextExtentPoint32W` gives `动` and `℃`. The character
    height is the font's own device metric, not a proportion of the cell, so
    no other tier can be worked out from the one the reference plots show:
    Arial Narrow answers 17, 19, 21 and 24 on cells of 22, 23, 25 and 29, and
    Arial answers 21 on 24.

    With `--vdmx` the same heights are read a second way, with no GDI in it:
    out of the font file's own `VDMX` table, whose 1:1 group lists, for each
    em in pixels (`yPelHeight`), the `yMax` and `yMin` the hinted font
    reaches. The cell GDI realizes is the tallest `yMax - yMin` that fits the
    cell asked for, and the character height is that row's `yPelHeight`. A
    row that disagrees with what GDI measured is marked.

  python -X utf8 tools/gdi_font_association.py reference

    The two readings `write.rs`'s `REFERENCE_CAP_PX` holds for each face --
    the mean top of `ADMXZ` and `tmAscent` at `lfHeight = 72` -- measured
    for the faces the table carries and for NSimSun, whose row was added
    with the borrowed lettering. It reads what SimSun reads, 49.2 / 62.
"""
import ctypes
import struct
import sys
from ctypes import wintypes

gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)

LF_FACESIZE = 32
ANSI_CHARSET = 0
GB2312_CHARSET = 134
NONANTIALIASED_QUALITY = 3
TA_BASELINE = 24
WHITENESS = 0x00FF0062
GGO_METRICS = 0
GDI_ERROR = 0xFFFFFFFF
WIDTH, HEIGHT = 240, 160

# What the reference plots borrow: a Han character and the degree sign.
PROBES = ("动", "℃")
# The faces GDI may be drawing them in, by either name.
LENDERS = ("NSimSun", "SimSun", "新宋体", "宋体")


class LOGFONTW(ctypes.Structure):
    _fields_ = [
        ("lfHeight", wintypes.LONG),
        ("lfWidth", wintypes.LONG),
        ("lfEscapement", wintypes.LONG),
        ("lfOrientation", wintypes.LONG),
        ("lfWeight", wintypes.LONG),
        ("lfItalic", wintypes.BYTE),
        ("lfUnderline", wintypes.BYTE),
        ("lfStrikeOut", wintypes.BYTE),
        ("lfCharSet", wintypes.BYTE),
        ("lfOutPrecision", wintypes.BYTE),
        ("lfClipPrecision", wintypes.BYTE),
        ("lfQuality", wintypes.BYTE),
        ("lfPitchAndFamily", wintypes.BYTE),
        ("lfFaceName", wintypes.WCHAR * LF_FACESIZE),
    ]


class TEXTMETRICW(ctypes.Structure):
    _fields_ = [
        ("tmHeight", wintypes.LONG),
        ("tmAscent", wintypes.LONG),
        ("tmDescent", wintypes.LONG),
        ("tmInternalLeading", wintypes.LONG),
        ("tmExternalLeading", wintypes.LONG),
        ("tmAveCharWidth", wintypes.LONG),
        ("tmMaxCharWidth", wintypes.LONG),
        ("tmWeight", wintypes.LONG),
        ("tmOverhang", wintypes.LONG),
        ("tmDigitizedAspectX", wintypes.LONG),
        ("tmDigitizedAspectY", wintypes.LONG),
        ("tmFirstChar", wintypes.WCHAR),
        ("tmLastChar", wintypes.WCHAR),
        ("tmDefaultChar", wintypes.WCHAR),
        ("tmBreakChar", wintypes.WCHAR),
        ("tmItalic", wintypes.BYTE),
        ("tmUnderlined", wintypes.BYTE),
        ("tmStruckOut", wintypes.BYTE),
        ("tmPitchAndFamily", wintypes.BYTE),
        ("tmCharSet", wintypes.BYTE),
    ]


class SIZE(ctypes.Structure):
    _fields_ = [("cx", wintypes.LONG), ("cy", wintypes.LONG)]


class POINT(ctypes.Structure):
    _fields_ = [("x", wintypes.LONG), ("y", wintypes.LONG)]


class GLYPHMETRICS(ctypes.Structure):
    _fields_ = [
        ("gmBlackBoxX", wintypes.UINT),
        ("gmBlackBoxY", wintypes.UINT),
        ("gmptGlyphOrigin", POINT),
        ("gmCellIncX", ctypes.c_short),
        ("gmCellIncY", ctypes.c_short),
    ]


class FIXED(ctypes.Structure):
    _fields_ = [("fract", wintypes.WORD), ("value", ctypes.c_short)]


class MAT2(ctypes.Structure):
    _fields_ = [("eM11", FIXED), ("eM12", FIXED), ("eM21", FIXED), ("eM22", FIXED)]


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [
        ("biSize", wintypes.DWORD),
        ("biWidth", wintypes.LONG),
        ("biHeight", wintypes.LONG),
        ("biPlanes", wintypes.WORD),
        ("biBitCount", wintypes.WORD),
        ("biCompression", wintypes.DWORD),
        ("biSizeImage", wintypes.DWORD),
        ("biXPelsPerMeter", wintypes.LONG),
        ("biYPelsPerMeter", wintypes.LONG),
        ("biClrUsed", wintypes.DWORD),
        ("biClrImportant", wintypes.DWORD),
    ]


gdi32.CreateCompatibleDC.restype = wintypes.HDC
gdi32.CreateCompatibleDC.argtypes = [wintypes.HDC]
gdi32.CreateFontIndirectW.restype = wintypes.HFONT
gdi32.CreateFontIndirectW.argtypes = [ctypes.POINTER(LOGFONTW)]
gdi32.SelectObject.restype = wintypes.HGDIOBJ
gdi32.SelectObject.argtypes = [wintypes.HDC, wintypes.HGDIOBJ]
gdi32.DeleteObject.argtypes = [wintypes.HGDIOBJ]
gdi32.GetTextMetricsW.argtypes = [wintypes.HDC, ctypes.POINTER(TEXTMETRICW)]
gdi32.GetTextFaceW.argtypes = [wintypes.HDC, ctypes.c_int, wintypes.LPWSTR]
gdi32.GetTextExtentPoint32W.argtypes = [wintypes.HDC, wintypes.LPCWSTR, ctypes.c_int, ctypes.POINTER(SIZE)]
gdi32.GetTextExtentPoint32A.argtypes = [wintypes.HDC, ctypes.c_char_p, ctypes.c_int, ctypes.POINTER(SIZE)]
gdi32.TextOutW.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int, wintypes.LPCWSTR, ctypes.c_int]
gdi32.TextOutA.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int, ctypes.c_char_p, ctypes.c_int]
gdi32.CreateDIBSection.restype = wintypes.HBITMAP
gdi32.CreateDIBSection.argtypes = [
    wintypes.HDC,
    ctypes.c_void_p,
    wintypes.UINT,
    ctypes.POINTER(ctypes.c_void_p),
    wintypes.HANDLE,
    wintypes.DWORD,
]
gdi32.PatBlt.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wintypes.DWORD]
gdi32.SetBkMode.argtypes = [wintypes.HDC, ctypes.c_int]
gdi32.SetTextColor.argtypes = [wintypes.HDC, wintypes.COLORREF]
gdi32.SetTextAlign.argtypes = [wintypes.HDC, wintypes.UINT]
gdi32.GetGlyphIndicesW.argtypes = [
    wintypes.HDC,
    wintypes.LPCWSTR,
    ctypes.c_int,
    ctypes.POINTER(wintypes.WORD),
    wintypes.DWORD,
]
gdi32.GetGlyphOutlineW.restype = wintypes.DWORD
gdi32.GetGlyphOutlineW.argtypes = [
    wintypes.HDC,
    wintypes.UINT,
    wintypes.UINT,
    ctypes.POINTER(GLYPHMETRICS),
    wintypes.DWORD,
    ctypes.c_void_p,
    ctypes.POINTER(MAT2),
]


class Canvas:
    """A memory DC over a 32-bit DIB, with one font selected at a time."""

    def __init__(self):
        self.hdc = gdi32.CreateCompatibleDC(None)
        header = BITMAPINFOHEADER()
        header.biSize = ctypes.sizeof(BITMAPINFOHEADER)
        header.biWidth = WIDTH
        header.biHeight = -HEIGHT
        header.biPlanes = 1
        header.biBitCount = 32
        self.bits = ctypes.c_void_p()
        self.bitmap = gdi32.CreateDIBSection(self.hdc, ctypes.byref(header), 0, ctypes.byref(self.bits), None, 0)
        gdi32.SelectObject(self.hdc, self.bitmap)
        gdi32.SetBkMode(self.hdc, 1)
        gdi32.SetTextColor(self.hdc, 0)
        gdi32.SetTextAlign(self.hdc, TA_BASELINE)
        self.font = None

    def select(self, face, height, charset):
        """Select `face`: a positive `height` is a cell, a negative one an em."""
        logfont = LOGFONTW()
        logfont.lfHeight = height
        logfont.lfWeight = 400
        logfont.lfCharSet = charset
        logfont.lfQuality = NONANTIALIASED_QUALITY
        logfont.lfFaceName = face
        font = gdi32.CreateFontIndirectW(ctypes.byref(logfont))
        gdi32.SelectObject(self.hdc, font)
        if self.font:
            gdi32.DeleteObject(self.font)
        self.font = font

    def metrics(self):
        metric = TEXTMETRICW()
        gdi32.GetTextMetricsW(self.hdc, ctypes.byref(metric))
        name = ctypes.create_unicode_buffer(64)
        gdi32.GetTextFaceW(self.hdc, 64, name)
        return metric, name.value

    def carries(self, char):
        """Whether the selected font itself has a glyph for `char`."""
        out = (wintypes.WORD * 1)()
        gdi32.GetGlyphIndicesW(self.hdc, char, 1, out, 1)
        return out[0] != 0xFFFF

    def advance_w(self, text):
        size = SIZE()
        gdi32.GetTextExtentPoint32W(self.hdc, text, len(text), ctypes.byref(size))
        return size.cx

    def advance_a(self, text):
        raw = text.encode("gbk")
        size = SIZE()
        gdi32.GetTextExtentPoint32A(self.hdc, raw, len(raw), ctypes.byref(size))
        return size.cx

    def draw(self, text, ansi=False):
        """The pixels `text` inks, as (x, y) from its origin on the baseline."""
        gdi32.PatBlt(self.hdc, 0, 0, WIDTH, HEIGHT, WHITENESS)
        if ansi:
            raw = text.encode("gbk")
            gdi32.TextOutA(self.hdc, 40, 100, raw, len(raw))
        else:
            gdi32.TextOutW(self.hdc, 40, 100, text, len(text))
        buffer = (ctypes.c_uint32 * (WIDTH * HEIGHT)).from_address(self.bits.value)
        ink = set()
        for y in range(HEIGHT):
            row = y * WIDTH
            for x in range(WIDTH):
                if buffer[row + x] & 0x00FFFFFF != 0x00FFFFFF:
                    ink.add((x - 40, y - 100))
        return frozenset(ink)

    def cap_top(self, letter):
        """`gmptGlyphOrigin.y` of `letter`: how far its top is above the baseline."""
        glyph = GLYPHMETRICS()
        identity = MAT2(FIXED(0, 1), FIXED(0, 0), FIXED(0, 0), FIXED(0, 1))
        got = gdi32.GetGlyphOutlineW(self.hdc, ord(letter), GGO_METRICS, ctypes.byref(glyph), 0, None, ctypes.byref(identity))
        return None if got == GDI_ERROR else glyph.gmptGlyphOrigin.y


def box(ink):
    if not ink:
        return "no ink"
    xs = [x for x, _ in ink]
    ys = [y for _, y in ink]
    return f"x {min(xs)}..{max(xs)} y {min(ys)}..{max(ys)}, {len(ink)} px"


def bitmap(face, cells):
    canvas = Canvas()
    for cell in cells:
        canvas.select(face, cell, ANSI_CHARSET)
        metric, name = canvas.metrics()
        character = metric.tmHeight - metric.tmInternalLeading
        print(
            f"== {face} asked for a cell of {cell}: selected {name!r}, tmHeight {metric.tmHeight} "
            f"tmAscent {metric.tmAscent} tmDescent {metric.tmDescent} tmInternalLeading {metric.tmInternalLeading}; "
            f"character height {character}"
        )
        for probe in PROBES:
            canvas.select(face, cell, ANSI_CHARSET)
            carried = canvas.carries(probe)
            wide = canvas.draw(probe)
            narrow = canvas.draw(probe, ansi=True)
            advances = (canvas.advance_w(probe), canvas.advance_a(probe))
            same = []
            for lender in LENDERS:
                for em in range(6, 80):
                    canvas.select(lender, -em, GB2312_CHARSET)
                    if wide and canvas.draw(probe) == wide:
                        same.append(f"{lender} at an em of {em}")
            print(
                f"   {probe}: the font carries it: {carried}; advance through TextOutW {advances[0]}, "
                f"through TextOutA {advances[1]}; ink through TextOutW [{box(wide)}], through TextOutA "
                f"[{box(narrow)}]"
            )
            print(f"      the TextOutW bitmap is that of: {', '.join(same) if same else 'no lender at any em'}")


def vdmx_rows(path):
    """The 1:1 group of a font file's `VDMX` table: (yPelHeight, yMax, yMin) rows."""
    data = open(path, "rb").read()
    tables = {}
    for index in range(struct.unpack(">H", data[4:6])[0]):
        tag, _, offset, length = struct.unpack(">4sIII", data[12 + 16 * index : 28 + 16 * index])
        tables[tag] = (offset, length)
    if b"VDMX" not in tables:
        raise SystemExit(f"{path} carries no VDMX table")
    start, _ = tables[b"VDMX"]
    _, _, ratios = struct.unpack(">HHH", data[start : start + 6])
    group = None
    for index in range(ratios):
        _, x_ratio, y_first, y_last = struct.unpack(">BBBB", data[start + 6 + 4 * index : start + 10 + 4 * index])
        # A device with square pixels: the 1:1 record, or the record for all.
        square = x_ratio == 1 and y_first <= 1 <= y_last
        if square or (x_ratio, y_first, y_last) == (0, 0, 0):
            at = start + 6 + 4 * ratios + 2 * index
            group = start + struct.unpack(">H", data[at : at + 2])[0]
            break
    if group is None:
        raise SystemExit(f"{path}: its VDMX table has no group for square pixels")
    count = struct.unpack(">H", data[group : group + 2])[0]
    return [struct.unpack(">Hhh", data[group + 4 + 6 * index : group + 10 + 6 * index]) for index in range(count)]


def vdmx_character_heights(rows, cell):
    """The `yPelHeight` of every row whose `yMax - yMin` is the tallest that fits `cell`."""
    fitting = [y_max - y_min for _, y_max, y_min in rows if y_max - y_min <= cell]
    if not fitting:
        return []
    tallest = max(fitting)
    return [pel for pel, y_max, y_min in rows if y_max - y_min == tallest]


def sweep(face, first, last, font_file=None):
    canvas = Canvas()
    rows = vdmx_rows(font_file) if font_file else None
    table = " | VDMX yPelHeight" if rows else ""
    print(f"{face}: cell | tmHeight tmAscent tmDescent tmInternalLeading | character height | advance of 动, of ℃{table}")
    apart = 0
    unexplained = 0
    below = 0
    for cell in range(first, last + 1):
        canvas.select(face, cell, ANSI_CHARSET)
        metric, _ = canvas.metrics()
        character = metric.tmHeight - metric.tmInternalLeading
        advances = [canvas.advance_w(probe) for probe in PROBES]
        # A full-width glyph of the associated font advances by its em, which
        # is the character height taken up to an even number.
        expected = character + character % 2
        note = "" if all(advance == expected for advance in advances) else "   <-- not the character height made even"
        apart += bool(note)
        from_table = ""
        if rows:
            heights = vdmx_character_heights(rows, cell)
            from_table = f" | {'/'.join(str(height) for height in heights) or '-'}"
            if not heights:
                from_table += "   (shorter than the table's first row)"
                below += 1
            elif character not in heights:
                from_table += "   <-- not what GDI measured"
                unexplained += 1
        print(
            f"  {cell:3d} | {metric.tmHeight:3d} {metric.tmAscent:3d} {metric.tmDescent:3d} "
            f"{metric.tmInternalLeading:3d} | {character:3d} | {advances[0]:3d} {advances[1]:3d}{from_table}{note}"
        )
    print(f"{face}: {last - first + 1} cells, {apart} where the advance is not the character height made even")
    if rows:
        print(
            f"{face}: {below} cells shorter than the VDMX table's first row, and {unexplained} of the other "
            f"{last - first + 1 - below} where the table does not give the character height GDI measured"
        )


def reference():
    canvas = Canvas()
    for face, charset in (
        ("SimSun", GB2312_CHARSET),
        ("NSimSun", GB2312_CHARSET),
        ("FangSong", GB2312_CHARSET),
        ("Arial", ANSI_CHARSET),
        ("Arial Narrow", ANSI_CHARSET),
        ("SimSun-ExtB", GB2312_CHARSET),
    ):
        canvas.select(face, 72, charset)
        metric, name = canvas.metrics()
        tops = [canvas.cap_top(letter) for letter in "ADMXZ"]
        read = [top for top in tops if top is not None]
        mean = sum(read) / len(read) if read else float("nan")
        cap = min(mean, metric.tmAscent - metric.tmInternalLeading)
        print(
            f"{face:<13} selected {name!r}: tmAscent {metric.tmAscent} tmDescent {metric.tmDescent} "
            f"tmInternalLeading {metric.tmInternalLeading}; tops of ADMXZ {tops} -> cap {cap:.2f}, ascent {metric.tmAscent}"
        )


def main():
    args = sys.argv[1:]
    mode = args.pop(0) if args else ""
    if mode == "bitmap":
        face, cells = "Arial Narrow", [24]
        while args:
            flag = args.pop(0)
            if flag == "--face":
                face = args.pop(0)
            elif flag == "--cells":
                cells = [int(value) for value in args.pop(0).split(",")]
            else:
                raise SystemExit(__doc__)
        bitmap(face, cells)
    elif mode == "sweep" and len(args) == 3:
        sweep(args[0], int(args[1]), int(args[2]))
    elif mode == "sweep" and len(args) == 5 and args[3] == "--vdmx":
        sweep(args[0], int(args[1]), int(args[2]), args[4])
    elif mode == "reference" and not args:
        reference()
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
