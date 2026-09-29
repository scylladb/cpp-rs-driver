#!/usr/bin/env python3

"""
Tests how `make run-test-coverage-scylla` and `make run-test-coverage-unit`
run the steps of the coverage lane: every suite runs even when an earlier one
fails (except the C++ integration tests, which need the binary their build
step makes), the report is written either way, and the target fails when any
step did. The steps are replaced by stand-ins that only record that they ran,
so this builds and runs nothing.

Run with `python3 ci/test_coverage_targets.py`.
"""

import os
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
        self.assertEqual(self.run_target("run-test-coverage-scylla"), (0, SCYLLA_STEPS))

    def test_scylla_keeps_going_after_a_failing_step(self):
        for failing in [".coverage-test-unit", ".coverage-test-cpp-scylla", ".coverage-test-ccm-scylla",
                        "coverage-report"]:
            with self.subTest(failing=failing):
                code, ran = self.run_target("run-test-coverage-scylla", [failing])
                self.assertNotEqual(code, 0)
                self.assertEqual(ran, SCYLLA_STEPS)

    def test_scylla_skips_the_cpp_tests_after_a_failing_build(self):
        code, ran = self.run_target("run-test-coverage-scylla", [".coverage-build-integration-test-bin"])
        self.assertNotEqual(code, 0)
        self.assertEqual(ran, [step for step in SCYLLA_STEPS if step != ".coverage-test-cpp-scylla"])

    def test_nothing_runs_after_a_failing_clean(self):
        for target in ["run-test-coverage-scylla", "run-test-coverage-unit"]:
            with self.subTest(target=target):
                code, ran = self.run_target(target, [".coverage-clean"])
                self.assertNotEqual(code, 0)
                self.assertEqual(ran, [".coverage-clean"])

    def test_unit_runs_every_step(self):
        self.assertEqual(self.run_target("run-test-coverage-unit"), (0, UNIT_STEPS))

    def test_unit_keeps_going_after_a_failing_step(self):
        for failing in [".coverage-test-unit", "coverage-report"]:
            with self.subTest(failing=failing):
                code, ran = self.run_target("run-test-coverage-unit", [failing])
                self.assertNotEqual(code, 0)
                self.assertEqual(ran, UNIT_STEPS)


if __name__ == "__main__":
    unittest.main()
