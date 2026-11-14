"""Every ComplexString (0x0021) and Group (0x007B) record on a record chain: how it closes.

Both records are a member count, a scale and one 14-byte entry a member, and then a closing.
This reads every such record of every drawing, template and symbol under the roots given --
straight off the compound files, not through the parser -- and prints what stands in each from
its count on: the sub-type word, the bytes ahead of the count, the scale, and what follows the
last entry. It is the evidence for the rule the ComplexString decoder holds a record to (a
closing of `00 02 00 00`, or of `00 03 00 00` and one u32 a member), and for holding a Group to
none: see docs/analysis/2026-11-12-a-complex-string-closes-in-one-of-two-ways.md.

    python tools/association_records_census.py [root ...]        (needs `olefile`)

With no root: `test-file`. Each distinct file (by SHA-1) and each distinct chain stream is read
once. Streams walked: every `Sheet*`, `PSMcluster0` and `StyleCluster`, at any depth, that
opens with the chain magic; `.pid` / `.igr` / `.sym` files and the members of `RefData~*.zip`
and `PlantData~*.zip` archives.

Read on 2026-11-12 over `test-file`, both `pid-test-data` and the share (885 documents): 34
ComplexStrings, all live, sub-type word 0, scale 1 or 2 -- 8 on sheets, closing `00 02 00 00`
(3, 5, 8 and 11 members), and 26 in the PSMcluster0 of a symbol cache, closing `00 03 00 00`
and a zero u32 a member (3, 4 and 8 members) -- and none in a style table. 273 Groups: 14 on
sheets, closing `00 02 00 00`, scale 1 or 2; 259 in style tables, 64 of them at scale 0 and 2
closing `00 00 00 00`.
"""
import hashlib
import io
import os
import struct
import sys
import zipfile
from collections import Counter

import olefile

MAGIC = 0x6C90F544


def opens_a_record(word):
    code = word & 0x3FFF
    return (word & 0x4000) == 0 and 1 <= code <= 0x0118 and code != 0x00F8


def walk(data):
    """(offset, type word, payload length) of every record the chain reads; whether it reached the end."""
    out = []
    at = 8
    while at < len(data):
        if at + 6 > len(data):
            return out, False
        word, length = struct.unpack_from("<HI", data, at)
        if not opens_a_record(word) or length == 0 or at + 6 + length > len(data):
            return out, False
        out.append((at, word, length))
        at += 6 + length
    return out, True


def files_under(root):
    for base, dirs, names in os.walk(root):
        dirs[:] = [d for d in dirs if d not in ("plot", "target", ".git")]
        for name in sorted(names):
            path = os.path.join(base, name)
            lower = name.lower()
            if lower.endswith((".pid", ".igr", ".sym")):
                try:
                    with open(path, "rb") as handle:
                        yield path, handle.read()
                except OSError:
                    pass
            elif lower.endswith(".zip") and (name.startswith("RefData~") or name.startswith("PlantData~")):
                try:
                    with zipfile.ZipFile(path) as archive:
                        for info in archive.infolist():
                            member = info.filename.lower()
                            if member.endswith((".pid", ".igr", ".sym")):
                                yield path + "!" + info.filename, archive.read(info)
                except (OSError, zipfile.BadZipFile):
                    pass


def main(roots):
    seen_files = set()
    seen_streams = set()
    documents = 0
    chains = Counter()
    shapes = Counter()
    examples = {}
    rule = Counter()
    rows = []
    for root in roots:
        for label, data in files_under(root):
            digest = hashlib.sha1(data).digest()
            if digest in seen_files:
                continue
            seen_files.add(digest)
            try:
                ole = olefile.OleFileIO(io.BytesIO(data))
            except Exception:
                continue
            documents += 1
            for entry in ole.listdir():
                name = "/".join(entry)
                leaf = entry[-1]
                is_sheet = leaf.startswith("Sheet")
                is_style = leaf == "StyleCluster"
                if not (is_sheet or is_style or leaf == "PSMcluster0"):
                    continue
                try:
                    stream = ole.openstream(entry).read()
                except Exception:
                    continue
                if len(stream) < 8 or struct.unpack_from("<I", stream, 0)[0] != MAGIC:
                    continue
                key = hashlib.sha1(stream).digest()
                if key in seen_streams:
                    continue
                seen_streams.add(key)
                records, whole = walk(stream)
                kind = ("sheet" if is_sheet else "style" if is_style else "cluster") + ("" if len(entry) == 1 else " (nested)")
                chains[(kind, whole)] += 1
                for at, word, length in records:
                    code = word & 0x3FFF
                    if code not in (0x0021, 0x007B):
                        continue
                    payload = stream[at + 6:at + 6 + length]
                    live = (word & 0x8000) == 0
                    count_at, members_at = (21, 33) if code == 0x0021 else (16, 28)
                    if length < members_at:
                        shapes[(code, kind, live, "shorter than its member table's offset")] += 1
                        continue
                    count = struct.unpack_from("<I", payload, count_at)[0]
                    scale = struct.unpack_from("<d", payload, count_at + 4)[0]
                    after = members_at + 14 * count
                    trailing = payload[after:] if after <= length else None
                    sub_type = struct.unpack_from("<H", payload, 12)[0]
                    head = payload[18:21].hex() if code == 0x0021 else payload[12:16].hex()
                    if trailing is None:
                        trailer = "members overrun the payload"
                    else:
                        version = trailing[:4].hex()
                        rest = trailing[4:]
                        if len(rest) == 0:
                            trailer = "%s" % version
                        elif len(rest) == 4 * count and not any(rest):
                            trailer = "%s + a zero u32 a member" % version
                        elif len(rest) == 4 * count:
                            trailer = "%s + a u32 a member, not all zero" % version
                        else:
                            trailer = "%s + %d bytes for %d members" % (version, len(rest), count)
                    words = Counter()
                    if trailing is not None:
                        for index in range(count):
                            words[struct.unpack_from("<H", payload, members_at + 14 * index + 12)[0]] += 1
                    shape = (code, kind, live, "sub-type %#06x" % sub_type, "head %s" % head, "scale %r" % scale, trailer)
                    shapes[shape] += 1
                    examples.setdefault(shape, "%s %s +%d (count %d, payload %d)" % (label, name, at, count, length))
                    rows.append((code, kind, live, count, scale, trailing, sub_type, words, label, name, at))
    print("documents %d; chain streams by kind and whether the walk reaches the end: %s" % (documents, dict(chains)))
    for shape, n in sorted(shapes.items(), key=lambda item: (item[0][0], item[0][1], str(item[0]))):
        print("%5d  %s" % (n, "  ".join(str(part) if not isinstance(part, int) else "%#06x" % part for part in shape)))
        print("         e.g. %s" % examples.get(shape, ""))
    # The candidate rules, held against every record.
    for code, kind, live, count, scale, trailing, sub_type, words, label, name, at in rows:
        which = "string" if code == 0x0021 else "group"
        rule[(which, "records")] += 1
        if live:
            rule[(which, "live")] += 1
        ok_trailer = trailing is not None and (
            (len(trailing) == 4 and trailing[0] == 0 and trailing[2:4] == b"\x00\x00")
            or (len(trailing) == 4 + 4 * count and trailing[0] == 0 and trailing[2:4] == b"\x00\x00")
        )
        rule[(which, "trailer is four bytes, or four and a u32 a member")] += ok_trailer
        rule[(which, "scale is a positive normal number")] += (scale > 2.3e-308 and scale < 1e300)
        rule[(which, "scale is 1 or 2")] += scale in (1.0, 2.0)
        rule[(which, "sub-type word is zero")] += sub_type == 0
        rule[(which, "count is 1..=64")] += 1 <= count <= 64
        for word, n in words.items():
            rule[(which, "member word %#06x" % word)] += n
    for key in sorted(rule):
        print("%-8s %-58s %d" % (key[0], key[1], rule[key]))
    counts = Counter((code, count) for code, _, _, count, *_ in rows)
    print("member counts: %s" % sorted(counts.items()))


if __name__ == "__main__":
    main(sys.argv[1:] or ["test-file"])
