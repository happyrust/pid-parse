#!/usr/bin/env python3
# -*- coding: utf-8 -*-
r"""What the root crate's tests did not check, counted.

A test of `pid-parse` that leaves a check unmade for want of data — a corpus
that is not in the repository, a fixture a checkout does not hold — says so in
one line that starts `skipping: ` and passes. `cargo test` captures what a
passing test prints, so a plain run shows none of them; this reads a run made
with `-- --nocapture` and counts them.

    python tools/soft_skips.py <log>
        Read the log of `cargo test … -- --nocapture`.

    python tools/soft_skips.py --run [--cargo "rustup run stable cargo"]
                               [--by-test] [--log PATH] [-- <cargo test arguments>]
        Run the root crate's tests with `--nocapture` (the library, the
        binaries, the integration tests and the examples; not the benches,
        whose harness takes no test flags), keep the output in PATH
        (`target/soft-skips.log` unless given) and read it. The exit code is
        cargo's: a skip fails nothing, a failing test still does.

`--by-test` names the test each line came from. That needs the tests of a
binary to run one after another (`--test-threads 1`, which `--run` then
passes; a log given by path has to have been made that way), so it is slower.
Without it the lines are counted and listed. A line counts when it starts
`skipping: `, or when nothing stands in front of that but the harness's own
`test <name> ... ` — of the test that printed it and, where tests run side by
side, of others. The words inside a sentence do not count.

The spelling is the convention, see "Soft skips" in `AGENTS.md`;
`tests/soft_skip_spelling.rs` refuses the two the tests used beside it. A test
that leaves early without the line is not counted here and is a defect.
"""
import io
import os
import re
import subprocess
import sys

SKIP = re.compile(r"^(?:test \S+ \.\.\. )*(skipping: .*)$")
RESULT = re.compile(r"^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored")
RUNNING = re.compile(r"^\s+Running (?:unittests )?(\S+) \(")
TEST = re.compile(r"^test (\S+) \.\.\. ?(.*)$")
GLUED = re.compile(r"test [A-Za-z0-9_:]+ \.\.\. ")
WIDTH = 150


def read(path):
    return io.open(path, encoding="utf-8", errors="replace").read().replace("\r\n", "\n").split("\n")


def census(lines, by_test):
    binaries = passed = failed = ignored = 0
    skips = []  # (binary, test or None, text)
    binary, test = "?", None
    for line in lines:
        running = RUNNING.match(line)
        if running:
            binary, test = running.group(1).replace("\\", "/"), None
            continue
        result = RESULT.match(line)
        if result:
            binaries += 1
            passed += int(result.group(1))
            failed += int(result.group(2))
            ignored += int(result.group(3))
            test = None
            continue
        started = TEST.match(line)
        if started:
            test = started.group(1)
            line = started.group(2)
            if line in ("ok", "FAILED") or line.startswith("ignored"):
                test = None
                continue
        elif line in ("ok", "FAILED"):
            test = None
            continue
        found = SKIP.match(line)
        if found:
            # Where tests run side by side, the harness's `test <name> ... `
            # of another test can land on the same line; the notice ends
            # where that begins.
            text = GLUED.split(found.group(1), maxsplit=1)[0].rstrip()
            skips.append((binary, test if by_test else None, text))

    out = []
    out.append(
        "soft skips of the root crate's tests: %d line(s) starting `skipping: ` "
        "(%d test binaries, %d passed, %d failed, %d ignored)"
        % (len(skips), binaries, passed, failed, ignored)
    )
    if by_test:
        tests = []
        for binary, test, _text in skips:
            if (binary, test) not in tests:
                tests.append((binary, test))
        out.append("%d test(s) printed one:" % len(tests))
        last = None
        for binary, test in tests:
            if binary != last:
                out.append("  " + binary)
                last = binary
            texts = [text for b, t, text in skips if (b, t) == (binary, test)]
            out.append("    %s: %d line(s)" % (test or "(outside a test)", len(texts)))
            distinct = []
            for text in texts:
                if text not in distinct:
                    distinct.append(text)
            for text in distinct:
                times = texts.count(text)
                out.append("        %s%s" % ("x%d " % times if times > 1 else "", text[:WIDTH]))
    else:
        tally = {}
        order = []
        for _binary, _test, text in skips:
            if text not in tally:
                tally[text] = 0
                order.append(text)
            tally[text] += 1
        for text in sorted(order, key=lambda text: (-tally[text], text)):
            out.append("  %3d  %s" % (tally[text], text[:WIDTH]))
    return out, failed


def main(argv):
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    by_test = "--by-test" in argv
    if "--run" not in argv:
        paths = [arg for arg in argv if not arg.startswith("--")]
        if len(paths) != 1:
            sys.stderr.write(__doc__)
            return 2
        out, _failed = census(read(paths[0]), by_test)
        print("\n".join(out))
        return 0

    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    extra = []
    if "--" in argv:
        cut = argv.index("--")
        argv, extra = argv[:cut], argv[cut + 1 :]
    cargo = ["cargo"]
    if "--cargo" in argv:
        cargo = argv[argv.index("--cargo") + 1].split()
    log = os.path.join(root, "target", "soft-skips.log")
    if "--log" in argv:
        log = argv[argv.index("--log") + 1]
    os.makedirs(os.path.dirname(os.path.abspath(log)), exist_ok=True)
    command = cargo + ["test", "--locked", "-p", "pid-parse", "--lib", "--bins", "--tests", "--examples"]
    command += extra + ["--", "--nocapture"]
    if by_test:
        command += ["--test-threads", "1"]
    with io.open(log, "wb") as sink:
        code = subprocess.call(command, cwd=root, stdout=sink, stderr=subprocess.STDOUT)
    out, _failed = census(read(log), by_test)
    print("\n".join(out))
    print("log: %s; cargo exited %d" % (log, code))
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
