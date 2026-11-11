# -*- coding: utf-8 -*-
r"""The text objects of a plotted PDF page, with every number as the file writes it.

A PDF viewer and a text extractor both hide what a plot driver wrote: they
join adjacent text objects into words and hand back floats. This reads the
content stream itself, one record per `Tj` / `TJ`, with the text state in
force and every operand kept as the string the file holds -- a text matrix
written `0.797` is seen as `0.797`. It is what the readings in
`docs/analysis/2026-11-11-a-string-is-one-text-object-for-each-script-run.md`
and `docs/analysis/2026-11-11-the-driver-states-a-font-transform-in-256ths.md`
were taken with.

Needs PyMuPDF (`pip install pymupdf`). It only reads.

  python -X utf8 tools/pdf_text_objects.py shows <pdf> [--grep TEXT] [--scaled] [--limit N]

    Every text object of the first page: face, `Tf`, the text matrix as
    written, `Tc`, and the text. `--scaled` keeps the ones whose matrix is
    not `1 0 0 1`, `--grep` the ones whose text contains TEXT.

  python -X utf8 tools/pdf_text_objects.py scripts <pdf> [<pdf> ...] [--cuts]

    How many text objects hold Latin letters, Han characters and full-width
    forms, alone and together, and how many hold none of the three. The
    reference plots never hold two of the three together. `--cuts` lists the
    places where one text object ends and the next continues the same line.

  python -X utf8 tools/pdf_text_objects.py matrices <pdf> [<pdf> ...]

    Every distinct (face, `Tf`, text-matrix scale) the pages write, how many
    text objects carry it, and the two scales times 256. The reference plots
    write whole 256ths.

  python -X utf8 tools/pdf_text_objects.py near <pdf> <x mm> <y mm> [radius mm]

    The text objects that start within the radius of a point (paper
    millimetres from the lower-left corner): where each starts, how far its
    glyphs advance, and the gap from the end of the one before it.
"""
import collections
import math
import os
import re
import sys

import pymupdf

PT_MM = 25.4 / 72.0

WHITE = b" \t\r\n\x0c\x00"
DELIMS = b"()<>[]{}/%"


def tokenize(data):
    """Yield (kind, value): 'num' (raw string), 'name', 'str' (bytes), 'op', '[' , ']'."""
    at = 0
    size = len(data)
    while at < size:
        byte = data[at : at + 1]
        if byte in (b" ", b"\t", b"\r", b"\n", b"\x0c", b"\x00"):
            at += 1
            continue
        if byte == b"%":
            while at < size and data[at : at + 1] not in (b"\r", b"\n"):
                at += 1
            continue
        if byte == b"(":
            depth = 1
            at += 1
            out = bytearray()
            while at < size and depth:
                ch = data[at : at + 1]
                if ch == b"\\":
                    nxt = data[at + 1 : at + 2]
                    table = {b"n": b"\n", b"r": b"\r", b"t": b"\t", b"b": b"\b", b"f": b"\f", b"(": b"(", b")": b")", b"\\": b"\\"}
                    if nxt in table:
                        out += table[nxt]
                        at += 2
                    elif nxt.isdigit():
                        digits = re.match(rb"[0-7]{1,3}", data[at + 1 : at + 4]).group(0)
                        out.append(int(digits, 8) & 0xFF)
                        at += 1 + len(digits)
                    elif nxt in (b"\r", b"\n"):
                        at += 2
                        if nxt == b"\r" and data[at : at + 1] == b"\n":
                            at += 1
                    else:
                        out += nxt
                        at += 2
                    continue
                if ch == b"(":
                    depth += 1
                elif ch == b")":
                    depth -= 1
                    if depth == 0:
                        at += 1
                        break
                out += ch
                at += 1
            yield ("str", bytes(out))
            continue
        if byte == b"<":
            if data[at : at + 2] == b"<<":
                yield ("op", "<<")
                at += 2
                continue
            end = data.index(b">", at)
            digits = re.sub(rb"\s", b"", data[at + 1 : end])
            if len(digits) % 2:
                digits += b"0"
            yield ("str", bytes.fromhex(digits.decode("ascii")))
            at = end + 1
            continue
        if byte == b">":
            if data[at : at + 2] == b">>":
                yield ("op", ">>")
                at += 2
            else:
                at += 1
            continue
        if byte == b"[":
            yield ("[", None)
            at += 1
            continue
        if byte == b"]":
            yield ("]", None)
            at += 1
            continue
        if byte == b"/":
            end = at + 1
            while end < size and data[end : end + 1] not in WHITE and data[end : end + 1] not in DELIMS:
                end += 1
            yield ("name", data[at + 1 : end].decode("latin-1"))
            at = end
            continue
        end = at
        while end < size and data[end : end + 1] not in WHITE and data[end : end + 1] not in DELIMS:
            end += 1
        if end == at:
            at += 1
            continue
        word = data[at:end].decode("latin-1")
        at = end
        if re.fullmatch(r"[+-]?(\d+\.?\d*|\.\d+)", word):
            yield ("num", word)
        else:
            yield ("op", word)


def multiply(m, n):
    """m × n for PDF matrices [a b c d e f] (row vectors: apply m first)."""
    a, b, c, d, e, f = m
    A, B, C, D, E, F = n
    return [a * A + b * C, a * B + b * D, c * A + d * C, c * B + d * D, e * A + f * C + E, e * B + f * D + F]


def parse_cmap(stream):
    """code -> unicode string, from a ToUnicode CMap."""
    table = {}
    text = stream.decode("latin-1")
    for block in re.findall(r"beginbfchar(.*?)endbfchar", text, re.S):
        for source, target in re.findall(r"<([0-9A-Fa-f]+)>\s*<([0-9A-Fa-f]+)>", block):
            table[int(source, 16)] = bytes.fromhex(target).decode("utf-16-be", errors="replace")
    for block in re.findall(r"beginbfrange(.*?)endbfrange", text, re.S):
        for first, last, rest in re.findall(r"<([0-9A-Fa-f]+)>\s*<([0-9A-Fa-f]+)>\s*(\[.*?\]|<[0-9A-Fa-f]+>)", block, re.S):
            low, high = int(first, 16), int(last, 16)
            if rest.startswith("["):
                targets = re.findall(r"<([0-9A-Fa-f]+)>", rest)
                for offset, target in enumerate(targets):
                    table[low + offset] = bytes.fromhex(target).decode("utf-16-be", errors="replace")
            else:
                base = int(rest[1:-1], 16)
                for code in range(low, high + 1):
                    table[code] = chr(base + code - low)
    return table


class Font:
    def __init__(self, doc, xref, basefont, kind, encoding):
        self.basefont = basefont.split("+")[-1]
        self.kind = kind
        self.two_byte = kind == "Type0"
        self.cmap = {}
        key = doc.xref_get_key(xref, "ToUnicode")
        if key[0] == "xref":
            target = int(key[1].split()[0])
            self.cmap = parse_cmap(doc.xref_stream(target))
        self.widths = {}
        self.default_width = 1000.0
        if self.two_byte:
            descendants = doc.xref_get_key(xref, "DescendantFonts")
            match = re.search(r"(\d+)\s+0\s+R", descendants[1])
            if match:
                descendant = int(match.group(1))
                dw = doc.xref_get_key(descendant, "DW")
                if dw[0] in ("int", "float"):
                    self.default_width = float(dw[1])
                w = doc.xref_get_key(descendant, "W")
                source = w[1]
                if w[0] == "xref":
                    source = doc.xref_object(int(w[1].split()[0]))
                self._parse_w(source)
        else:
            first = doc.xref_get_key(xref, "FirstChar")
            widths = doc.xref_get_key(xref, "Widths")
            source = widths[1]
            if widths[0] == "xref":
                source = doc.xref_object(int(widths[1].split()[0]))
            if first[0] == "int" and source:
                start = int(first[1])
                for offset, value in enumerate(re.findall(r"[-+]?\d*\.?\d+", source)):
                    self.widths[start + offset] = float(value)

    def _parse_w(self, source):
        tokens = re.findall(r"\[|\]|[-+]?\d*\.?\d+", source or "")
        at = 0
        # Skip the outer bracket.
        if tokens and tokens[0] == "[":
            at = 1
        while at < len(tokens) and tokens[at] != "]":
            first = int(float(tokens[at]))
            if tokens[at + 1] == "[":
                at += 2
                code = first
                while tokens[at] != "]":
                    self.widths[code] = float(tokens[at])
                    code += 1
                    at += 1
                at += 1
            else:
                last = int(float(tokens[at + 1]))
                width = float(tokens[at + 2])
                for code in range(first, last + 1):
                    self.widths[code] = width
                at += 3

    def codes(self, raw):
        if self.two_byte:
            return [int.from_bytes(raw[i : i + 2], "big") for i in range(0, len(raw) - 1, 2)]
        return list(raw)

    def text(self, codes):
        out = []
        for code in codes:
            if code in self.cmap:
                out.append(self.cmap[code])
            elif not self.two_byte:
                out.append(bytes([code]).decode("cp1252", errors="replace"))
            else:
                out.append("\ufffd")
        return "".join(out)

    def width(self, code):
        return self.widths.get(code, self.default_width)


def page_fonts(doc, page):
    fonts = {}
    for xref, _ext, kind, basefont, name, encoding, *_ in page.get_fonts(full=True):
        try:
            fonts[name] = Font(doc, xref, basefont, kind, encoding)
        except Exception as error:  # a font this reader cannot take apart
            print(f"   font {name} ({basefont}): {error}", file=sys.stderr)
    return fonts


def shows(pdf_path):
    """One dict per text-showing operator of the first page."""
    doc = pymupdf.open(pdf_path)
    page = doc[0]
    fonts = page_fonts(doc, page)
    data = page.read_contents()
    height = page.mediabox.height
    ctm = [1, 0, 0, 1, 0, 0]
    stack = []
    operands = []
    state = {"font": None, "size": "0", "tc": "0", "tw": "0", "tz": "100", "tl": 0.0}
    tm = [1, 0, 0, 1, 0, 0]
    tlm = [1, 0, 0, 1, 0, 0]
    tm_raw = None
    array = None
    sequence = 0
    out = []

    def number(value):
        return float(value)

    def show(parts):
        nonlocal tm, sequence
        font = fonts.get(state["font"])
        size = number(state["size"])
        tc = number(state["tc"])
        tw = number(state["tw"])
        th = number(state["tz"]) / 100.0
        start = list(tm)
        codes_all = []
        advance = 0.0
        last_width = 0.0
        gaps = []
        for part in parts:
            if isinstance(part, bytes):
                codes = font.codes(part) if font else list(part)
                for code in codes:
                    width = (font.width(code) if font else 500.0) / 1000.0 * size
                    step = (width + tc + (tw if code == 32 and not (font and font.two_byte) else 0.0)) * th
                    gaps.append(step)
                    advance += step
                    last_width = width * th
                    codes_all.append(code)
            else:
                advance -= part / 1000.0 * size * th
        placed = multiply(start, ctm)
        record = {
            "seq": sequence,
            "font": font.basefont if font else state["font"],
            "resource": state["font"],
            "size_raw": state["size"],
            "tc_raw": state["tc"],
            "tz_raw": state["tz"],
            "tm_raw": tm_raw,
            "tm": start,
            "ctm": list(ctm),
            "matrix": placed,
            "text": font.text(codes_all) if font else "",
            "codes": codes_all,
            "glyphs": len(codes_all),
            # In text space, before the text matrix: the run with and without
            # the letter-spacing after its last glyph.
            "advance_text": advance,
            "ink_text": advance - (tc * th if codes_all else 0.0),
            "last_width_text": last_width,
            "page_height": height,
        }
        out.append(record)
        sequence += 1
        tm = multiply([1, 0, 0, 1, advance, 0], tm)

    for kind, value in tokenize(data):
        if kind == "[":
            array = []
            continue
        if kind == "]":
            operands.append(("array", array))
            array = None
            continue
        if kind in ("num", "str", "name"):
            if array is not None:
                array.append(number(value) if kind == "num" else value)
            else:
                operands.append((kind, value))
            continue
        op = value
        nums = [v for k, v in operands if k == "num"]
        if op == "q":
            stack.append(list(ctm))
        elif op == "Q":
            if stack:
                ctm = stack.pop()
        elif op == "cm" and len(nums) == 6:
            ctm = multiply([number(v) for v in nums], ctm)
        elif op == "BT":
            tm = [1, 0, 0, 1, 0, 0]
            tlm = [1, 0, 0, 1, 0, 0]
            tm_raw = None
        elif op == "Tf":
            names = [v for k, v in operands if k == "name"]
            if names and nums:
                state["font"] = names[-1]
                state["size"] = nums[-1]
        elif op == "Tc" and nums:
            state["tc"] = nums[-1]
        elif op == "Tw" and nums:
            state["tw"] = nums[-1]
        elif op == "Tz" and nums:
            state["tz"] = nums[-1]
        elif op == "TL" and nums:
            state["tl"] = number(nums[-1])
        elif op == "Tm" and len(nums) == 6:
            tm = [number(v) for v in nums]
            tlm = list(tm)
            tm_raw = list(nums)
        elif op in ("Td", "TD") and len(nums) == 2:
            tx, ty = number(nums[0]), number(nums[1])
            if op == "TD":
                state["tl"] = -ty
            tlm = multiply([1, 0, 0, 1, tx, ty], tlm)
            tm = list(tlm)
        elif op == "T*":
            tlm = multiply([1, 0, 0, 1, 0, -state["tl"]], tlm)
            tm = list(tlm)
        elif op == "Tj":
            strings = [v for k, v in operands if k == "str"]
            if strings:
                show([strings[-1]])
        elif op == "TJ":
            arrays = [v for k, v in operands if k == "array"]
            if arrays:
                show(arrays[-1])
        elif op in ("'", '"'):
            tlm = multiply([1, 0, 0, 1, 0, -state["tl"]], tlm)
            tm = list(tlm)
            strings = [v for k, v in operands if k == "str"]
            if strings:
                show([strings[-1]])
        operands = []
    doc.close()
    return out


def command_shows(args):
    grep = None
    limit = None
    scaled = False
    if "--grep" in args:
        at = args.index("--grep")
        grep = args[at + 1]
        del args[at : at + 2]
    if "--limit" in args:
        at = args.index("--limit")
        limit = int(args[at + 1])
        del args[at : at + 2]
    if "--scaled" in args:
        scaled = True
        args.remove("--scaled")
    shown = 0
    for record in shows(args[0]):
        if grep is not None and grep not in record["text"]:
            continue
        raw = record["tm_raw"] or ["?"] * 6
        if scaled and raw[:4] == ["1", "0", "0", "1"]:
            continue
        print(
            f"{record['seq']:5d} {record['font']:<14} Tf {record['size_raw']:>7} Tm [{' '.join(raw)}] Tc {record['tc_raw']:>7}"
            f" | {record['glyphs']:2d} glyphs {record['text']!r}"
        )
        shown += 1
        if limit and shown >= limit:
            break


def script_of(ch):
    """The kind of character that starts a script run: 'L', 'H', 'W', or '' for one that starts none."""
    code = ord(ch)
    if ("A" <= ch <= "Z") or ("a" <= ch <= "z"):
        return "L"
    if 0x3400 <= code <= 0x4DBF or 0x4E00 <= code <= 0x9FFF or 0xF900 <= code <= 0xFAFF or 0x20000 <= code <= 0x323AF:
        return "H"
    if 0xFF01 <= code <= 0xFF60 or 0xFFE0 <= code <= 0xFFE6:
        return "W"
    return ""


def command_scripts(args):
    cuts_wanted = "--cuts" in args
    pdfs = [a for a in args if not a.startswith("--")]
    held = collections.Counter()
    names = {"L": "Latin letters", "H": "Han", "W": "full-width forms", "": "none of the three"}
    total = 0
    for pdf in pdfs:
        records = shows(pdf)
        total += len(records)
        page = collections.Counter()
        cuts = []
        last = None
        for record in records:
            kinds = "".join(sorted({script_of(c) for c in record["text"]} - {""}))
            page[kinds] += 1
            m = record["matrix"]
            sx = math.hypot(m[0], m[1]) or 1.0
            ux, uy = m[0] / sx, m[1] / sx
            x, y = m[4], m[5]
            if last is not None:
                lrec, lx, ly, lux, luy, lend = last
                along = (x - lx) * lux + (y - ly) * luy
                across = -(x - lx) * luy + (y - ly) * lux
                size = float(record["size_raw"])
                if abs(across) < 0.05 and abs(ux - lux) < 1e-6 and abs(uy - luy) < 1e-6 and -0.6 * size < along - lend < 2.0 * size:
                    cuts.append((lrec, record, along - lend))
            last = (record, x, y, ux, uy, record["advance_text"] * sx)
        held.update(page)
        together = sum(n for kinds, n in page.items() if len(kinds) > 1)
        print(f"== {os.path.basename(pdf)}: {len(records)} text objects; {together} hold two of the three kinds together; {len(cuts)} continue the line of the one before")
        if cuts_wanted:
            seen = collections.OrderedDict()
            for left, right, gap in cuts:
                key = (left["font"], left["size_raw"], left["text"], right["text"])
                seen.setdefault(key, [0, gap, left, right])[0] += 1
            for (font, size, ltext, rtext), (count, gap, left, right) in seen.items():
                print(f"   x{count} {font}@{size} {ltext!r} | {rtext!r}  gap {gap * PT_MM:+.3f} mm, Tc {left['tc_raw']} / {right['tc_raw']}")
    print(f"total over {len(pdfs)} page(s): {total} text objects")
    for kinds, count in sorted(held.items(), key=lambda item: (len(item[0]), item[0])):
        label = " and ".join(names[k] for k in kinds) if kinds else names[""]
        print(f"   {count:>6}  {label}")


def command_matrices(args):
    combos = collections.Counter()
    examples = {}
    for pdf in args:
        for record in shows(pdf):
            raw = record["tm_raw"]
            if raw is None:
                continue
            key = (record["font"], record["size_raw"], *raw[:4])
            combos[key] += 1
            examples.setdefault(key, record["text"])
    print("face | Tf | a b c d | text objects | scale along x 256, across x 256 | example")
    for key, count in sorted(combos.items(), key=lambda item: (item[0][0], float(item[0][1]), item[0][2:])):
        font, size, a, b, c, d = key
        along = math.hypot(float(a), float(b))
        across = math.hypot(float(c), float(d))
        plain = abs(along - 1) < 1e-9 and abs(across - 1) < 1e-9
        note = "" if plain else f"{along * 256:8.3f} {across * 256:8.3f}"
        print(f"{font:<13} {size:>6} | {a:>7} {b:>7} {c:>7} {d:>7} | {count:5d} | {note:<18} | {examples[key][:24]!r}")


def command_near(args):
    pdf, x_mm, y_mm = args[0], float(args[1]), float(args[2])
    radius = float(args[3]) if len(args) > 3 else 8.0
    last_end = None
    for record in shows(pdf):
        m = record["matrix"]
        sx = math.hypot(m[0], m[1])
        x, y = m[4] * PT_MM, m[5] * PT_MM
        if math.hypot(x - x_mm, y - y_mm) > radius:
            continue
        ux, uy = (m[0] / sx, m[1] / sx) if sx else (1.0, 0.0)
        advance = record["advance_text"] * sx * PT_MM
        ink = record["ink_text"] * sx * PT_MM
        gap = ""
        if last_end is not None:
            along = (x - last_end[0]) * ux + (y - last_end[1]) * uy
            across = -(x - last_end[0]) * uy + (y - last_end[1]) * ux
            gap = f"gap {along:+.4f} mm ({along / PT_MM:+.3f} pt), across {across:+.4f} mm"
        raw = record["tm_raw"] or ["?"] * 6
        print(
            f"{record['seq']:5d} {record['font']:<13} Tf {record['size_raw']:>6} Tc {record['tc_raw']:>8} Tm [{' '.join(raw[:4])}]"
            f" | start ({x:9.4f}, {y:9.4f}) advance {advance:8.4f} ink {ink:8.4f} mm | {record['text']!r} {gap}"
        )
        last_end = (x + ux * ink, y + uy * ink)


COMMANDS = {
    "shows": command_shows,
    "scripts": command_scripts,
    "matrices": command_matrices,
    "near": command_near,
}


def main():
    if len(sys.argv) < 3 or sys.argv[1] not in COMMANDS:
        print(__doc__)
        sys.exit(2)
    COMMANDS[sys.argv[1]](sys.argv[2:])


if __name__ == "__main__":
    main()
