"""When was each plot made, and when was the drawing beside it last saved?

    python tools/display_order/plot_and_file_dates.py <corpus>

For every `<name>.pid` of the corpus with a `<name>.pdf` beside it, prints the
plot's `/CreationDate` and `/Producer` as its document information states
them, and the time the drawing's compound file was last written: the
modification time of its root storage and the "last saved" time of its
summary information. A plot made after the file was saved may show a later
state of the drawing than the file holds; this is where the two dates are
read. Needs `olefile`. Read-only.

On the eleven acceptance sheets (2026-11-12): every plot was made on
2026-10-12 or 2026-10-14 by "Amyuni PDF Converter", and every drawing was
last saved between 2026-06-22 and 2026-07-06 — `一级闪蒸分离器A`, `B`,
`低温分离器A` and `换热器CD` within 63 seconds of one another on 2026-07-06.
"""

import glob
import os
import re
import sys

import olefile


def information(pdf: bytes, key: bytes):
    """One entry of a PDF's document information, as text."""
    literal = re.search(rb"/" + key + rb"\s*\(([^)]*)\)", pdf)
    if literal:
        return literal.group(1).decode("latin-1", "replace")
    hexadecimal = re.search(rb"/" + key + rb"\s*<([0-9A-Fa-f]+)>", pdf)
    if hexadecimal:
        raw = bytes.fromhex(hexadecimal.group(1).decode())
        if raw[:2] in (b"\xfe\xff", b"\xff\xfe"):
            return raw.decode("utf-16", "replace")
        return raw.decode("latin-1", "replace")
    return None


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    corpus = sys.argv[1]
    read = 0
    for plot in sorted(glob.glob(os.path.join(corpus, "*.pdf"))):
        name = os.path.splitext(os.path.basename(plot))[0]
        drawing = os.path.join(corpus, name + ".pid")
        if not os.path.isfile(drawing):
            continue
        with open(plot, "rb") as handle:
            pdf = handle.read()
        ole = olefile.OleFileIO(drawing)
        try:
            written = ole.root.getmtime()
            try:
                saved = ole.get_metadata().last_saved_time
            except Exception as error:  # a drawing with no summary information
                saved = f"no summary information ({error})"
        finally:
            ole.close()
        print(
            f"{name} | plot made {information(pdf, b'CreationDate')} by "
            f"{information(pdf, b'Producer')} | drawing's root storage written {written} | "
            f"summary says last saved {saved}"
        )
        read += 1
    print(f"{read} drawing(s) with a plot beside them")
    return 0


if __name__ == "__main__":
    sys.exit(main())
