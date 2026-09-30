#!/usr/bin/env python3

"""
Tests how `make run-test-coverage-scylla` and `make run-test-coverage-unit`
run the steps of the coverage lane: every suite runs even when an earlier one
fails (except the C++ integration tests, which need the binary their build
step makes), the report is written either way, and the target fails when any
step did. The steps are replaced by stand-ins that only record that they ran,
so this builds and runs nothing. Also tests check-rust-tests-ran, which fails
a Rust suite in which a test binary ran no test, on `cargo test` output as CI
prints it.

Run with `python3 ci/test_coverage_targets.py`.
"""

import os
import re
import subprocess
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

SCYLLA_STEPS = [".coverage-clean", ".coverage-test-unit", ".coverage-build-integration-test-bin",
                ".coverage-test-cpp-scylla", ".coverage-test-ccm-scylla", "coverage-report"]
UNIT_STEPS = [".coverage-clean", ".coverage-test-unit", "coverage-report"]
# The steps' prerequisites, which would install packages and set up the host.
SETUP = ["install-cargo-if-missing", "install-lcov-if-missing", "update-apt-cache-if-needed",
         ".prepare-environment-update-aio-max-nr"]

# Read after the Makefile, so that these recipes replace its own (make warns
# about each). A step appends its name to $STUB_LOG, and fails when $STUB_FAIL
# names it.
STUBS = f"""
{" ".join(SCYLLA_STEPS)}:
\t@echo $@ >> "$$STUB_LOG"; case " $$STUB_FAIL " in *" $@ "*) exit 1;; esac

{" ".join(SETUP)}:
\t@:
"""

# The status lines `cargo test` prints before each test binary and before the
# doc tests, colored as in CI, where CARGO_TERM_COLOR is always.
RUNNING = "\x1b[1m\x1b[92m     Running\x1b[0m tests/integration/main.rs ({})"
DOC_TESTS = "\x1b[1m\x1b[92m   Doc-tests\x1b[0m scylladb"


def libtest_output(tests):
    """Renders the lines a test binary prints around its tests when it runs that many."""
    return ["", f"running {tests} test{'' if tests == 1 else 's'}", "", "test result: ok.", ""]


def cargo_test_output(binaries, doc_tests=(4, 5)):
    """Renders `cargo test` output for [(executable, number of tests), ...] and blocks of doc tests."""
    lines = []
    for executable, tests in binaries:
        lines += [RUNNING.format(executable), *libtest_output(tests)]
    lines.append(DOC_TESTS)
    for tests in doc_tests:
        lines += libtest_output(tests)
    return "\n".join(lines) + "\n"


class CoverageTargetsTest(unittest.TestCase):
    def run_target(self, target, failing=()):
        """Returns the exit code of `make target` and the steps it ran, in order."""
        with tempfile.TemporaryDirectory() as directory:
            stubs = os.path.join(directory, "stubs.mk")
            with open(stubs, "w") as out:
                out.write(STUBS)
            log = os.path.join(directory, "steps")
            make = ["make", "--no-print-directory", "-f", "Makefile", "-f", stubs]
            env = dict(os.environ, STUB_LOG=log, STUB_FAIL=" ".join(failing))
            # The targets run most steps through ${MAKE}, which has to read the
            # stand-ins as well.
            result = subprocess.run([*make, "MAKE=" + " ".join(make), target],
                                    cwd=ROOT, env=env, capture_output=True, text=True)
            ran = []
            if os.path.exists(log):
                with open(log) as steps:
                    ran = steps.read().split()
        return result.returncode, ran

    def test_scylla_runs_every_step(self):
        """With no step failing, run-test-coverage-scylla runs every step, in order, and succeeds."""
        self.assertEqual(self.run_target("run-test-coverage-scylla"), (0, SCYLLA_STEPS))

    def test_scylla_keeps_going_after_a_failing_step(self):
        """A failing suite or report fails run-test-coverage-scylla, but only after every step ran."""
        for failing in [".coverage-test-unit", ".coverage-test-cpp-scylla", ".coverage-test-ccm-scylla",
                        "coverage-report"]:
            with self.subTest(failing=failing):
                code, ran = self.run_target("run-test-coverage-scylla", [failing])
                self.assertNotEqual(code, 0)
                self.assertEqual(ran, SCYLLA_STEPS)

    def test_scylla_skips_the_cpp_tests_after_a_failing_build(self):
        """A failed build skips only the C++ tests, which need its binary, and fails the target."""
        code, ran = self.run_target("run-test-coverage-scylla", [".coverage-build-integration-test-bin"])
        self.assertNotEqual(code, 0)
        self.assertEqual(ran, [step for step in SCYLLA_STEPS if step != ".coverage-test-cpp-scylla"])

    def test_nothing_runs_after_a_failing_clean(self):
        """Both targets stop at a failing clean step: they start from no profile data, or not at all."""
        for target in ["run-test-coverage-scylla", "run-test-coverage-unit"]:
            with self.subTest(target=target):
                code, ran = self.run_target(target, [".coverage-clean"])
                self.assertNotEqual(code, 0)
                self.assertEqual(ran, [".coverage-clean"])

    def test_unit_runs_every_step(self):
        """With no step failing, run-test-coverage-unit runs every step, in order, and succeeds."""
        self.assertEqual(self.run_target("run-test-coverage-unit"), (0, UNIT_STEPS))

    def test_unit_keeps_going_after_a_failing_step(self):
        """A failing unit suite or report fails run-test-coverage-unit, but only after every step ran."""
        for failing in [".coverage-test-unit", "coverage-report"]:
            with self.subTest(failing=failing):
                code, ran = self.run_target("run-test-coverage-unit", [failing])
                self.assertNotEqual(code, 0)
                self.assertEqual(ran, UNIT_STEPS)


class RustTestsRanTest(unittest.TestCase):
    def check(self, output):
        """Returns the exit code, stdout and stderr of check-rust-tests-ran on that output."""
        with tempfile.TemporaryDirectory() as directory:
            with open(os.path.join(directory, "unit-tests.log"), "w") as log:
                log.write(output)
            check = os.path.join(directory, "check.mk")
            with open(check, "w") as out:
                out.write(".check:\n\t$(call check-rust-tests-ran,unit)\n")
            result = subprocess.run(["make", "--no-print-directory", "-s", "-f", "Makefile", "-f", check,
                                     "COVERAGE_TARGET_DIR=" + directory, ".check"],
                                    cwd=ROOT, capture_output=True, text=True)
        return result.returncode, result.stdout, result.stderr

    def test_counts_the_tests_of_every_binary(self):
        """Passes when every binary ran tests, whether cargo colors its output or not, and says so."""
        output = cargo_test_output([("/deps/scylladb-1", 53), ("/deps/integration-2", 7)])
        for colors in [True, False]:
            with self.subTest(colors=colors):
                code, out, _ = self.check(output if colors else re.sub(r"\x1b\[[0-9;]*m", "", output))
                self.assertEqual((code, out), (0, "The unit suite ran 60 tests in 2 test binaries.\n"))

    def test_names_a_binary_that_ran_no_test(self):
        """Fails when a binary ran no test, even next to one that ran some, and names that binary."""
        output = cargo_test_output([("/deps/scylladb-1", 53), ("/deps/integration-2", 0)])
        code, out, err = self.check(output)
        self.assertNotEqual(code, 0)
        self.assertEqual(out, "")
        self.assertIn("/deps/integration-2 ran no test.", err)
        self.assertNotIn("scylladb-1", err)

    def test_doc_tests_do_not_count(self):
        """Passes when only the doc tests ran none: they are not instrumented."""
        code, out, _ = self.check(cargo_test_output([("/deps/integration-2", 1)], doc_tests=(0,)))
        self.assertEqual((code, out), (0, "The unit suite ran 1 test in 1 test binary.\n"))

    def test_gives_no_doc_test_count_to_a_binary_without_one(self):
        """A binary with no test count, as with a custom harness, gets none from the doc tests."""
        output = cargo_test_output([("/deps/scylladb-1", 53)])
        custom = RUNNING.format("/deps/custom-3") + "\ncustom harness output\n"
        code, out, _ = self.check(output.replace(DOC_TESTS, custom + DOC_TESTS))
        self.assertEqual((code, out), (0, "The unit suite ran 53 tests in 1 test binary.\n"))

    def test_fails_on_output_it_cannot_read(self):
        """Fails when no test binary reports how many tests it ran, rather than pass unchecked."""
        unknown = cargo_test_output([("/deps/integration-2", 0)]).replace("Running", "Executing")
        for case, output in [("empty", ""), ("unknown", unknown)]:
            with self.subTest(case=case):
                code, _, err = self.check(output)
                self.assertNotEqual(code, 0)
                self.assertIn("No test binary of the unit suite reported how many tests it ran.", err)


if __name__ == "__main__":
    unittest.main()
