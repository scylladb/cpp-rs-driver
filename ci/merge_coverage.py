#!/usr/bin/env python3

"""
Merges the lcov exports of the binaries `make coverage-report` measures into
one lcov report, and prints a per-file summary of it.

The coverage of each binary is exported on its own (the Makefile explains why)
and merged here, line by line: a line is covered when any binary covered it.
The exports of the driver's own unit test build (--test-build) are the
exception. That build is compiled with cfg(test), so it also contains the test
modules and test helpers inside the driver's source files, which no file
filter can leave out. Its lines count only where a build without cfg(test)
has them too: a line only the unit test build has is test code, and is
dropped.
"""

import argparse
import sys


def read_lcov(path):
    """Returns the DA records of an lcov file as {source file: {line: count}}."""
    files = {}
    lines = None
    with open(path) as report:
        for record in report:
            if record.startswith("SF:"):
                lines = files.setdefault(record[3:].rstrip("\n"), {})
            elif record.startswith("DA:"):
                number, count = (int(field) for field in record[3:].split(",")[:2])
                lines[number] = lines.get(number, 0) + count
            elif record.startswith("end_of_record"):
                lines = None
    return files


def main():
    """Merges the exports given on the command line into --output and prints its per-file summary."""
    parser = argparse.ArgumentParser(description=__doc__.strip().splitlines()[0])
    parser.add_argument("exports", nargs="+", metavar="LCOV",
                        help="export of a binary that links the driver built without cfg(test)")
    parser.add_argument("--test-build", action="append", default=[], metavar="LCOV",
                        help="export of the driver's unit test build")
    parser.add_argument("--output", required=True, help="where to write the merged lcov report")
    parser.add_argument("--root", default="", help="prefix to leave out of file names in the summary")
    args = parser.parse_args()

    merged = {}
    for path in args.exports:
        for source, lines in read_lcov(path).items():
            known = merged.setdefault(source, {})
            for number, count in lines.items():
                known[number] = known.get(number, 0) + count
    for path in args.test_build:
        for source, lines in read_lcov(path).items():
            known = merged.get(source, {})
            for number, count in lines.items():
                if number in known:
                    known[number] += count

    with open(args.output, "w") as output:
        for source in sorted(merged):
            lines = merged[source]
            output.write(f"SF:{source}\n")
            for number in sorted(lines):
                output.write(f"DA:{number},{lines[number]}\n")
            output.write(f"LF:{len(lines)}\n")
            output.write(f"LH:{sum(1 for count in lines.values() if count)}\n")
            output.write("end_of_record\n")

    rows = []
    for source in sorted(merged):
        lines = merged[source]
        name = source[len(args.root):] if source.startswith(args.root) else source
        rows.append((name, len(lines), sum(1 for count in lines.values() if count)))
    rows.append(("TOTAL", sum(row[1] for row in rows), sum(row[2] for row in rows)))
    print(f"{'File':<32} {'Lines':>8} {'Covered':>8} {'Cover':>8}")
    for name, total, covered in rows:
        percent = f"{100 * covered / total:.2f}%" if total else "-"
        print(f"{name:<32} {total:8d} {covered:8d} {percent:>8}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
