#!/usr/bin/env python3

"""
Tests ci/merge_coverage.py, which decides what the coverage report counts: how
the lcov exports of the binaries `make coverage-report` measures are merged,
and which lines of the driver's unit test build are left out as test code.

Run with `python3 ci/test_merge_coverage.py`.
"""

import os
import subprocess
import sys
import tempfile
import unittest

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "merge_coverage.py")


def lcov(files):
    """Renders {source file: [(line, count), ...]} as the DA records of an lcov export."""
    records = []
    for source, lines in files.items():
        records.append(f"SF:{source}")
        records += [f"DA:{number},{count}" for number, count in lines]
        records.append("end_of_record")
    return "\n".join(records) + "\n"


def parse(text):
    """Returns the DA records of an lcov report as {source file: {line: count}}."""
    files = {}
    for record in text.splitlines():
        if record.startswith("SF:"):
            lines = files.setdefault(record[3:], {})
        elif record.startswith("DA:"):
            number, count = record[3:].split(",")
            lines[int(number)] = int(count)
    return files


class MergeCoverageTest(unittest.TestCase):
    def setUp(self):
        """Gives each test a directory of its own for the exports and the report."""
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.directory = directory.name

    def export(self, name, content):
        """Writes an export, lcov text or {source file: [(line, count), ...]}, and returns its path."""
        path = os.path.join(self.directory, name)
        with open(path, "w") as export:
            export.write(content if isinstance(content, str) else lcov(content))
        return path

    def run_merge(self, *exports, test_builds=(), root=None):
        """Merges the exports and returns the report and the summary the script printed."""
        output = os.path.join(self.directory, "lcov.info")
        command = [sys.executable, SCRIPT, "--output", output, *exports]
        for path in test_builds:
            command += ["--test-build", path]
        if root is not None:
            command += ["--root", root]
        result = subprocess.run(command, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        with open(output) as report:
            return report.read(), result.stdout

    def merge(self, *exports, test_builds=()):
        """Merges the exports and returns the report as {source file: {line: count}}."""
        return parse(self.run_merge(*exports, test_builds=test_builds)[0])

    def test_counts_of_builds_without_cfg_test_add_up_line_by_line(self):
        """Every line of the builds without cfg(test) is reported, with the sum of their counts."""
        first = self.export("first.info", {"/src/a.rs": [(1, 0), (2, 3)]})
        second = self.export("second.info", {"/src/a.rs": [(2, 1), (3, 0)], "/src/b.rs": [(5, 2)]})
        self.assertEqual(self.merge(first, second),
                         {"/src/a.rs": {1: 0, 2: 4, 3: 0}, "/src/b.rs": {5: 2}})

    def test_lines_only_the_unit_test_build_has_are_left_out(self):
        """The unit test build adds its counts to the other builds' lines, and no line of its own."""
        library = self.export("library.info", {"/src/a.rs": [(1, 0), (2, 0)]})
        unit_tests = self.export("unit.info", {"/src/a.rs": [(1, 5), (2, 0), (40, 9)],
                                               "/src/testing.rs": [(1, 3)]})
        self.assertEqual(self.merge(library, test_builds=[unit_tests]), {"/src/a.rs": {1: 5, 2: 0}})

    def test_unit_tests_cover_what_only_the_empty_profile_export_lists(self):
        """The unit test build's counts reach lines that only the empty profile's export lists.

        A C API function that only the unit tests call is missing from the
        exports of the other binaries that ran (llvm-cov reports it as
        mismatched data), but the export against an empty profile lists it.
        """
        ran = self.export("ran.info", {"/src/api.rs": [(10, 4)]})
        all_lines = self.export("all.info", {"/src/api.rs": [(10, 0), (20, 0), (21, 0)]})
        unit_tests = self.export("unit.info", {"/src/api.rs": [(20, 2)]})
        self.assertEqual(self.merge(ran, all_lines, test_builds=[unit_tests]),
                         {"/src/api.rs": {10: 4, 20: 2, 21: 0}})

    def test_repeated_records_add_up_and_other_records_are_ignored(self):
        """Repeated DA records of a line add up; DA checksums and all other records are ignored."""
        export = self.export("export.info", "\n".join([
            "TN:", "SF:/src/a.rs", "FN:1,f", "FNDA:1,f", "DA:1,1", "DA:1,2", "DA:3,2,a1b2c3",
            "BRDA:3,0,0,1", "LF:9", "LH:9", "end_of_record", ""]))
        self.assertEqual(self.merge(export), {"/src/a.rs": {1: 3, 3: 2}})

    def test_report_is_sorted_and_counts_lines(self):
        """The report lists files and lines in order, and each file's LF and LH counts."""
        export = self.export("export.info", {"/src/b.rs": [(7, 1)], "/src/a.rs": [(2, 0), (1, 4)]})
        report, _ = self.run_merge(export)
        self.assertEqual(report, "\n".join([
            "SF:/src/a.rs", "DA:1,4", "DA:2,0", "LF:2", "LH:1", "end_of_record",
            "SF:/src/b.rs", "DA:7,1", "LF:1", "LH:1", "end_of_record", ""]))

    def test_summary_is_per_file_relative_to_root(self):
        """The summary has a row per file, named relative to --root, and a TOTAL row."""
        export = self.export("export.info", {"/src/a.rs": [(1, 1), (2, 0)], "/src/b/c.rs": [(1, 1)]})
        _, summary = self.run_merge(export, root="/src/")
        self.assertEqual([row.split() for row in summary.splitlines()[1:]],
                         [["a.rs", "2", "1", "50.00%"], ["b/c.rs", "1", "1", "100.00%"],
                          ["TOTAL", "3", "2", "66.67%"]])

    def test_needs_a_build_without_cfg_test(self):
        """Without the export of a build without cfg(test), the script fails and writes no report."""
        unit_tests = self.export("unit.info", {"/src/a.rs": [(1, 1)]})
        output = os.path.join(self.directory, "lcov.info")
        result = subprocess.run([sys.executable, SCRIPT, "--output", output, "--test-build", unit_tests],
                                capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(os.path.exists(output))


if __name__ == "__main__":
    unittest.main()
