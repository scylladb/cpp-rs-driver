EMPTY :=
SPACE := ${EMPTY} ${EMPTY}
.ONESHELL:

SHELL := bash
.SHELLFLAGS := -ec
ifeq ($(OS),Windows_NT)
    SHELL := pwsh.exe
    .SHELLFLAGS := -NoProfile -Command $$ErrorActionPreference = 'Stop';
endif

UNAME_S := $(shell uname -s)
ifeq ($(OS),Windows_NT)
    OS_TYPE := windows
else ifeq ($(UNAME_S),Darwin)
    OS_TYPE := macos
else
    OS_TYPE := linux
endif

ifndef SCYLLA_TEST_FILTER
SCYLLA_TEST_FILTER := $(subst ${SPACE},${EMPTY},ClusterTests.*\
:BasicsTests.*\
:BasicsNoTabletsTests.*\
:ConfigTests.*\
:NullStringApiArgsTest.*\
:ConsistencyTwoNodeClusterTests.*\
:ConsistencyThreeNodeClusterTests.*\
:SerialConsistencyTests.*\
:HeartbeatTests.*\
:PreparedTests.*\
:StatementNoClusterTests.*\
:StatementTests.*\
:NamedParametersTests.*\
:CassandraTypes/CassandraTypesTests/*.Integration_Cassandra_*\
:ControlConnectionTests.*\
:BatchSingleNodeClusterTests*:BatchCounterSingleNodeClusterTests*:BatchCounterThreeNodeClusterTests*\
:ErrorTests.*\
:SslNoClusterTests*:SslNoSslOnClusterTests*\
:SchemaMetadataTest.*\
:TracingTests.*\
:ByNameTests.*\
:CompressionTests.*\
:LatencyAwarePolicyTest.*\
:LoggingTests.*\
:PreparedMetadataTests.*\
:UseKeyspaceCaseSensitiveTests.*\
:ServerSideFailureTests.*\
:ServerSideFailureThreeNodeTests.*\
:TimestampTests.*\
:HostFilterTest.*\
:ExecutionProfileTest.*\
:DCExecutionProfileTest.*\
:DisconnectedNullStringApiArgsTest.*\
:MetricsTests.*\
:DcAwarePolicyTest.*\
:AsyncTests.*\
:-SchemaMetadataTest.Integration_Cassandra_RegularMetadataNotMarkedVirtual\
:SchemaMetadataTest.Integration_Cassandra_VirtualMetadata\
:HeartbeatTests.Integration_Cassandra_HeartbeatFailed\
:ExecutionProfileTest.Integration_Cassandra_RoundRobin\
:ExecutionProfileTest.Integration_Cassandra_TokenAwareRouting\
:ExecutionProfileTest.Integration_Cassandra_SpeculativeExecutionPolicy\
:ControlConnectionTests.Integration_Cassandra_TopologyChange\
:ControlConnectionTests.Integration_Cassandra_FullOutage\
:ControlConnectionTests.Integration_Cassandra_TerminatedUsingMultipleIoThreadsWithError\
:ServerSideFailureTests.Integration_Cassandra_ErrorFunctionFailure\
:ServerSideFailureTests.Integration_Cassandra_ErrorFunctionAlreadyExists\
:MetricsTests.Integration_Cassandra_SpeculativeExecutionRequests\
:*NoCompactEnabledConnection\
:PreparedMetadataTests.Integration_Cassandra_AlterProperlyUpdatesColumnCount)
endif

ifndef SCYLLA_NO_VALGRIND_TEST_FILTER
SCYLLA_NO_VALGRIND_TEST_FILTER := $(subst ${SPACE},${EMPTY},AsyncTests.Integration_Cassandra_Simple\
:HeartbeatTests.Integration_Cassandra_HeartbeatFailed)
endif

ifndef CASSANDRA_TEST_FILTER
CASSANDRA_TEST_FILTER := $(subst ${SPACE},${EMPTY},ClusterTests.*\
:BasicsTests.*\
:BasicsNoTabletsTests.*\
:ConfigTests.*\
:NullStringApiArgsTest.*\
:ConsistencyTwoNodeClusterTests.*\
:ConsistencyThreeNodeClusterTests.*\
:SerialConsistencyTests.*\
:HeartbeatTests.*\
:PreparedTests.*\
:StatementNoClusterTests.*\
:StatementTests.*\
:NamedParametersTests.*\
:CassandraTypes/CassandraTypesTests/*.Integration_Cassandra_*\
:ControlConnectionTests.*\
:ErrorTests.*\
:SslClientAuthenticationTests*:SslNoClusterTests*:SslNoSslOnClusterTests*:SslTests*\
:SchemaMetadataTest.*\
:TracingTests.*\
:ByNameTests.*\
:CompressionTests.*\
:LatencyAwarePolicyTest.*\
:LoggingTests.*\
:PreparedMetadataTests.*\
:UseKeyspaceCaseSensitiveTests.*\
:ServerSideFailureTests.*\
:ServerSideFailureThreeNodeTests.*\
:TimestampTests.*\
:HostFilterTest.*\
:ExecutionProfileTest.*\
:DCExecutionProfileTest.*\
:DisconnectedNullStringApiArgsTest.*\
:MetricsTests.*\
:DcAwarePolicyTest.*\
:AsyncTests.*\
:-PreparedTests.Integration_Cassandra_FailFastWhenPreparedIDChangesDuringReprepare\
:SchemaMetadataTest.Integration_Cassandra_RegularMetadataNotMarkedVirtual\
:SchemaMetadataTest.Integration_Cassandra_VirtualMetadata\
:HeartbeatTests.Integration_Cassandra_HeartbeatFailed\
:ExecutionProfileTest.Integration_Cassandra_RoundRobin\
:ExecutionProfileTest.Integration_Cassandra_TokenAwareRouting\
:ExecutionProfileTest.Integration_Cassandra_SpeculativeExecutionPolicy\
:ControlConnectionTests.Integration_Cassandra_TopologyChange\
:ControlConnectionTests.Integration_Cassandra_FullOutage\
:ControlConnectionTests.Integration_Cassandra_TerminatedUsingMultipleIoThreadsWithError\
:ServerSideFailureTests.Integration_Cassandra_ErrorFunctionFailure\
:ServerSideFailureTests.Integration_Cassandra_ErrorFunctionAlreadyExists\
:SslTests.Integration_Cassandra_ReconnectAfterClusterCrashAndRestart\
:MetricsTests.Integration_Cassandra_SpeculativeExecutionRequests\
:*NoCompactEnabledConnection\
:PreparedMetadataTests.Integration_Cassandra_AlterProperlyUpdatesColumnCount)
endif

ifndef CASSANDRA_NO_VALGRIND_TEST_FILTER
CASSANDRA_NO_VALGRIND_TEST_FILTER := $(subst ${SPACE},${EMPTY},AsyncTests.Integration_Cassandra_Simple\
:HeartbeatTests.Integration_Cassandra_HeartbeatFailed)
endif

ifndef SCYLLA_EXAMPLES_URI
# In sync with the docker compose file.
SCYLLA_EXAMPLES_URI := 172.43.0.2
endif

ifndef SCYLLA_EXAMPLES_TO_RUN
SCYLLA_EXAMPLES_TO_RUN := \
    async \
	basic \
	batch \
	bind_by_name \
	callbacks \
	collections \
	concurrent_executions \
	date_time \
	duration \
	execution_profiles \
	maps \
	named_parameters \
	paging \
	perf \
	prepared \
	simple \
	ssl \
	tracing \
	tuple \
	udt \
	uuids \

	# auth <- unimplemented `cass_cluster_set_authenticator_callbacks()`
	# host_listener <- never terminates by design; loops forever listening to events.
	# logging <- unimplemented `cass_cluster_set_host_listener_callback()`
	# schema_meta <- unimplemented multiple schema-related functions
endif

ifndef CCM_COMMIT_ID
	export CCM_COMMIT_ID := master
endif

ifndef SCYLLA_VERSION
	SCYLLA_VERSION := release:2025.3
endif

ifndef CASSANDRA_VERSION
	CASSANDRA_VERSION := 3.11.17
endif

# RUSTFLAGS are normally specified in .cargo/config.toml, but those do not include
# the integration testing flag ("cpp_integration_testing"), to prevent CMake from
# including testing stuff when building the main library.
# This constant is used to store the full set of RUSTFLAGS that should be used
# for running integration tests, as well as running lints on conditionally compiled
# code related to integration testing.
FULL_RUSTFLAGS := --cfg scylla_unstable --cfg cpp_integration_testing

CURRENT_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
BUILD_DIR := "${CURRENT_DIR}build"
INTEGRATION_TEST_BIN := ${BUILD_DIR}/cassandra-integration-tests
CMAKE_FLAGS ?=
CMAKE_BUILD_TYPE ?= Release

ifeq ($(OS_TYPE),macos)
  CMAKE_INSTALL_PREFIX ?= /usr/local
else
  CMAKE_INSTALL_PREFIX ?= /usr
endif

ifeq ($(OS_TYPE),macos)
  CPACK_GENERATORS ?= DragNDrop productbuild
else ifeq ($(OS_TYPE),windows)
  CPACK_GENERATORS ?= WIX
else
  CPACK_GENERATORS ?= DEB RPM
endif

clean:
	rm -rf "${BUILD_DIR}"

update-apt-cache-if-needed:
	@# It searches for a file that is at most one day old.
	@# If there is no such file, executes apt update.
	@sudo find /var/cache/apt -type f -mtime -1 2>/dev/null | grep -c "" 2>/dev/null | grep 0 >/dev/null 2>&1 || (
		echo "Apt cache is outdated, update it."
		sudo apt-get update || true
	)

install-cargo-if-missing: update-apt-cache-if-needed
	@cargo --version >/dev/null 2>&1 || (
		echo "Cargo not found in the system, install it."
		sudo apt-get install -y cargo
	)

install-valgrind-if-missing: update-apt-cache-if-needed
	@valgrind --version >/dev/null 2>&1 || (
		echo "Valgrind not found in the system, install it."
		sudo apt install -y valgrind
	)

install-clang-format-if-missing: update-apt-cache-if-needed
	@clang-format --version >/dev/null 2>&1 || (
		echo "clang-format not found in the system, install it."
		sudo apt install -y clang-format
	)

install-ccm-if-missing:
	@ccm list >/dev/null 2>&1 || (
		echo "CCM not found in the system, install it."
		pip3 install --user https://github.com/scylladb/scylla-ccm/archive/${CCM_COMMIT_ID}.zip
	)

install-ccm:
	@pip3 install --user https://github.com/scylladb/scylla-ccm/archive/${CCM_COMMIT_ID}.zip

install-java8-if-missing:
	@dpkg -l openjdk-8-jre >/dev/null 2>&1 && exit 0
	@echo "Java 8 not found in the system, install it"
	@sudo apt install -y openjdk-8-jre

install-build-dependencies: update-apt-cache-if-needed
	@sudo apt-get install -y libssl1.1 libuv1-dev libkrb5-dev libc6-dbg

# Alias for backward compatibility
install-bin-dependencies: install-build-dependencies

build-integration-test-bin:
	@echo "Building integration test binary to ${INTEGRATION_TEST_BIN}"
	@mkdir "${BUILD_DIR}" >/dev/null 2>&1 || true
	@cd "${BUILD_DIR}"
	cmake -DCASS_BUILD_INTEGRATION_TESTS=ON -DCMAKE_BUILD_TYPE=Release .. && (make -j 4 || make)

build-integration-test-bin-if-missing:
	@[ -f "${INTEGRATION_TEST_BIN}" ] && exit 0
	@echo "Integration test binary not found at ${INTEGRATION_TEST_BIN}, building it"
	@mkdir "${BUILD_DIR}" >/dev/null 2>&1 || true
	@cd "${BUILD_DIR}"
	cmake -DCASS_BUILD_INTEGRATION_TESTS=ON -DCMAKE_BUILD_TYPE=Release .. && (make -j 4 || make)

build-examples:
	@echo "Building examples to ${EXAMPLES_DIR}"
	@mkdir "${BUILD_DIR}" >/dev/null 2>&1 || true
	@cd "${BUILD_DIR}"
	cmake -DCASS_BUILD_INTEGRATION_TESTS=off -DCASS_BUILD_EXAMPLES=on -DCMAKE_BUILD_TYPE=Release .. && (make -j 4 || make)

.ubuntu-package-install-dependencies: update-apt-cache-if-needed
	sudo apt-get install -y rpm ninja-build pkg-config

.fedora-package-install-dependencies:
	sudo dnf install -y rpm-build ninja-build pkgconf-pkg-config

.package-build-prepare-ubuntu:
	@missing=""
	for bin in ninja rpmbuild pkg-config; do
		if ! command -v $$bin >/dev/null 2>&1; then
			missing="$$missing $$bin"
		fi
	done
	if [ -n "$$missing" ]; then
		$(MAKE) .ubuntu-package-install-dependencies
	fi

.package-build-prepare-fedora:
	@missing=""
	for bin in ninja rpmbuild pkg-config; do
		if ! command -v $$bin >/dev/null 2>&1; then
			missing="$$missing $$bin"
		fi
	done
	if [ -n "$$missing" ]; then
		$(MAKE) .fedora-package-install-dependencies
	fi

.package-build-prepare-windows-openssl:
	@pwsh -NoProfile -Command "if (-not (choco list --local-only --exact openssl.light | Select-String '^openssl.light$$')) { choco install openssl.light --no-progress -y }"

.package-build-prepare-windows-pkgconfiglite:
	@pwsh -NoProfile -Command "if (-not (choco list --local-only --exact pkgconfiglite | Select-String '^pkgconfiglite$$')) { choco install pkgconfiglite --no-progress -y --allow-empty-checksums }"

.package-build-prepare-windows: .package-build-prepare-windows-openssl .package-build-prepare-windows-pkgconfiglite

# Detect Linux distribution type (debian/ubuntu vs fedora/rhel)
LINUX_DISTRO_FAMILY :=
ifeq ($(OS_TYPE),linux)
  ifneq ($(wildcard /etc/os-release),)
    DISTRO_ID := $(shell grep "^ID=" /etc/os-release 2>/dev/null | cut -d= -f2 | tr -d '"')
    DISTRO_ID_LIKE := $(shell grep "^ID_LIKE=" /etc/os-release 2>/dev/null | cut -d= -f2 | tr -d '"')
    ifneq ($(filter fedora rhel centos rocky almalinux,$(DISTRO_ID) $(DISTRO_ID_LIKE)),)
      LINUX_DISTRO_FAMILY := fedora
    else
      LINUX_DISTRO_FAMILY := debian
    endif
  else
    # Default to debian if /etc/os-release doesn't exist
    LINUX_DISTRO_FAMILY := debian
  endif
endif

ifeq ($(OS_TYPE),macos)
.package-build-prepare:
else ifeq ($(OS_TYPE),windows)
.package-build-prepare: .package-build-prepare-windows
else ifeq ($(LINUX_DISTRO_FAMILY),fedora)
.package-build-prepare: .package-build-prepare-fedora
else
.package-build-prepare: .package-build-prepare-ubuntu
endif

.package-configure: .package-build-prepare
ifeq ($(OS_TYPE),windows)
	cmake -S . -B build -G "Visual Studio 17 2022" -A x64 -DCMAKE_BUILD_TYPE=$(CMAKE_BUILD_TYPE) -DOPENSSL_VERSION=1.1.1u $(CMAKE_FLAGS)
else
	cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=$(CMAKE_BUILD_TYPE) -DCMAKE_INSTALL_PREFIX=$(CMAKE_INSTALL_PREFIX) $(CMAKE_FLAGS)
endif

build-driver: .package-configure
ifeq ($(OS_TYPE),windows)
	@pwsh -NoProfile -Command "$$opensslVersion = ((Select-String -Path 'build\\CMakeCache.txt' -Pattern '^OPENSSL_VERSION:STRING=' | Select-Object -First 1).Line -split '=', 2)[1]; $$opensslTarget = \"openssl-$${opensslVersion}-library\"; cmake --build build --config $(CMAKE_BUILD_TYPE) --target $$opensslTarget; $$env:OPENSSL_DIR = (Resolve-Path 'build\\libs\\openssl').Path; $$env:OPENSSL_INCLUDE_DIR = \"$$env:OPENSSL_DIR\\include\"; $$env:OPENSSL_LIB_DIR = \"$$env:OPENSSL_DIR\\lib\"; cmake --build build --config $(CMAKE_BUILD_TYPE)"
else
	cmake --build build --config $(CMAKE_BUILD_TYPE)
endif

build-package: build-driver
ifeq ($(OS_TYPE),windows)
	@pwsh -NoProfile -Command "Push-Location build; foreach ($$gen in '$(CPACK_GENERATORS)'.Split(' ', [System.StringSplitOptions]::RemoveEmptyEntries)) { cpack -G $$gen -C $(CMAKE_BUILD_TYPE) }; Pop-Location"
else
	@cd build
	for gen in $(CPACK_GENERATORS); do
		if [ "$${gen}" = "productbuild" ] && [ "$(OS_TYPE)" = "macos" ]; then
			cmake -DCPACK_BUILD_DIR="$$PWD" -DCPACK_BUILD_CONFIG="$(CMAKE_BUILD_TYPE)" -P ../cmake/RunMacProductbuild.cmake
		else
			cpack -G $${gen} -C $(CMAKE_BUILD_TYPE)
		fi
	done
endif

update-rust-tooling:
	@echo "Run rustup update"
	@rustup update stable

check-cargo: install-cargo-if-missing
	@echo "Running \"cargo check\" in ./scylla-rust-wrapper"
	@cd ${CURRENT_DIR}/scylla-rust-wrapper
	cargo check --all-targets

fix-cargo:
	@echo "Running \"cargo fix --verbose --all\" in ./scylla-rust-wrapper"
	@cd ${CURRENT_DIR}/scylla-rust-wrapper
	cargo fix --verbose --all

check-cargo-clippy: install-cargo-if-missing
	@echo "Running \"cargo clippy --verbose --all-targets -- -D warnings -Aclippy::uninlined_format_args\" in ./scylla-rust-wrapper"
	@cd ${CURRENT_DIR}/scylla-rust-wrapper
	RUSTFLAGS="${FULL_RUSTFLAGS}" cargo clippy --verbose --all-targets -- -D warnings -Aclippy::uninlined_format_args

fix-cargo-clippy: install-cargo-if-missing
	@echo "Running \"cargo clippy --verbose --all-targets --fix -- -D warnings -Aclippy::uninlined_format_args\" in ./scylla-rust-wrapper"
	@cd ${CURRENT_DIR}/scylla-rust-wrapper
	cargo clippy --verbose --all-targets --fix -- -D warnings -Aclippy::uninlined_format_args

check-cargo-fmt: install-cargo-if-missing
	@echo "Running \"cargo fmt --verbose --all -- --check\" in ./scylla-rust-wrapper"
	@cd ${CURRENT_DIR}/scylla-rust-wrapper
	cargo fmt --verbose --all -- --check

fix-cargo-fmt: install-cargo-if-missing
	@echo "Running \"cargo fmt --verbose --all\" in ./scylla-rust-wrapper"
	@cd ${CURRENT_DIR}/scylla-rust-wrapper
	cargo fmt --verbose --all

check-clang-format: install-clang-format-if-missing
	@echo "Running \"clang-format --dry-run\" on all files in ./src"
	@find src -regextype posix-egrep -regex '.*\.(cpp|hpp|c|h)' -not -path 'src/third_party/*' | xargs clang-format --dry-run

fix-clang-format: install-clang-format-if-missing
	@echo "Running \"clang-format -i\" on all files in ./src"
	@find src -regextype posix-egrep -regex '.*\.(cpp|hpp|c|h)' -not -path 'src/third_party/*' | xargs clang-format -i

check: check-clang-format check-cargo check-cargo-clippy check-cargo-fmt

fix: fix-clang-format fix-cargo fix-cargo-clippy fix-cargo-fmt

.prepare-environment-update-aio-max-nr:
	@if (( $$(< /proc/sys/fs/aio-max-nr) < 2097152 )); then
		echo 2097152 | sudo tee /proc/sys/fs/aio-max-nr >/dev/null
	fi

.prepare-environment-install-libc:
	@dpkg -l libc6-dbg >/dev/null 2>&1 || sudo apt-get install -y libc6-dbg

prepare-integration-test: .prepare-environment-install-libc update-apt-cache-if-needed install-valgrind-if-missing install-cargo-if-missing

download-ccm-scylla-image: install-ccm-if-missing
	@echo "Downloading scylla ${SCYLLA_VERSION} CCM image"
	@rm -rf /tmp/download-scylla.ccm || true
	@mkdir /tmp/download-scylla.ccm || true
	@ccm create ccm_1 -i 127.0.1. -n 3:0 -v "${SCYLLA_VERSION}" --scylla --config-dir=/tmp/download-scylla.ccm
	@rm -rf /tmp/download-scylla.ccm

download-ccm-cassandra-image: install-ccm-if-missing
	@echo "Downloading cassandra ${CASSANDRA_VERSION} CCM image"
	@rm -rf /tmp/download-cassandra.ccm || true
	@mkdir /tmp/download-cassandra.ccm || true
	@ccm create ccm_1 -i 127.0.1. -n 3:0 -v "${CASSANDRA_VERSION}" --config-dir=/tmp/download-cassandra.ccm
	@rm -rf /tmp/download-cassandra.ccm

run-test-integration-scylla: .prepare-environment-update-aio-max-nr
ifdef DONT_REBUILD_INTEGRATION_BIN
run-test-integration-scylla: build-integration-test-bin-if-missing
else
run-test-integration-scylla: build-integration-test-bin
endif
	@echo "Running integration tests on scylla ${SCYLLA_VERSION}"
	valgrind --error-exitcode=123 --leak-check=full --errors-for-leak-kinds=definite build/cassandra-integration-tests --scylla --version=${SCYLLA_VERSION} --category=CASSANDRA --verbose=ccm --gtest_filter="${SCYLLA_TEST_FILTER}"
	@echo "Running timeout sensitive tests on scylla ${SCYLLA_VERSION}"
	build/cassandra-integration-tests --scylla --version=${SCYLLA_VERSION} --category=CASSANDRA --verbose=ccm --gtest_filter="${SCYLLA_NO_VALGRIND_TEST_FILTER}"

run-test-integration-cassandra: install-java8-if-missing
ifdef DONT_REBUILD_INTEGRATION_BIN
run-test-integration-cassandra: build-integration-test-bin-if-missing
else
run-test-integration-cassandra: build-integration-test-bin
endif
	@echo "Running integration tests on cassandra ${CASSANDRA_VERSION}"
	valgrind --error-exitcode=123 --leak-check=full --errors-for-leak-kinds=definite build/cassandra-integration-tests --version=${CASSANDRA_VERSION} --category=CASSANDRA --verbose=ccm --gtest_filter="${CASSANDRA_TEST_FILTER}"
	@echo "Running timeout sensitive tests on cassandra ${CASSANDRA_VERSION}"
	build/cassandra-integration-tests --version=${CASSANDRA_VERSION} --category=CASSANDRA --verbose=ccm --gtest_filter="${CASSANDRA_NO_VALGRIND_TEST_FILTER}"

run-test-unit: install-cargo-if-missing
	@cd ${CURRENT_DIR}/scylla-rust-wrapper
	RUSTFLAGS="${FULL_RUSTFLAGS}" cargo test

# Currently not used.
CQLSH := cqlsh

run-examples-scylla: build-examples
	@sudo sh -c "echo 2097152 >> /proc/sys/fs/aio-max-nr"
	@# Keep `SCYLLA_EXAMPLES_URI` in sync with the `scylla` service in `docker-compose.yml`.
	@docker compose -f tests/examples_cluster/docker-compose.yml up -d --wait

	@# Instead of using cqlsh, which would impose another dependency on the system,
	@# we use a special example `drop_examples_keyspace` to drop the `examples` keyspace.
	@# CQLSH_HOST=${SCYLLA_EXAMPLES_URI} ${CQLSH} -e "DROP KEYSPACE IF EXISTS EXAMPLES"; \

	@echo "Running examples on scylla ${SCYLLA_VERSION}"
	@for example in ${SCYLLA_EXAMPLES_TO_RUN}; do
		echo -e "\nRunning example: $${example}"
		build/examples/drop_examples_keyspace/drop_examples_keyspace ${SCYLLA_EXAMPLES_URI} || exit 1
		build/examples/$${example}/$${example} ${SCYLLA_EXAMPLES_URI} || {
		    echo "Example \`$${example}\` has failed!"
			docker compose -f tests/examples_cluster/docker-compose.yml down
			exit 42
		}
	done
	docker compose -f tests/examples_cluster/docker-compose.yml down --remove-orphans

.windows-setup-wix:
ifeq ($(OS_TYPE),windows)
	@pwsh -NoProfile -Command " \
		$$wixPath = 'C:\\Program Files (x86)\\WiX Toolset v3.11\\bin'; \
		if (Test-Path $$wixPath) { \
			$$currentPath = [Environment]::GetEnvironmentVariable('PATH', 'Process'); \
			if ($$currentPath -notlike \"*$$wixPath*\") { \
				[Environment]::SetEnvironmentVariable('PATH', \"$$wixPath;$$currentPath\", 'Process'); \
			}; \
			if ($$env:GITHUB_PATH) { \
				Add-Content -Path $$env:GITHUB_PATH -Value $$wixPath; \
			} \
		}"
endif

# =============================================================================
# Package Testing Targets
# =============================================================================
# These targets provide end-to-end testing of built packages by installing
# the driver packages, building a smoke-test app that links against them,
# installing and running the smoke-test app.
#
# Usage:
#   make test-package              # Test default package format(s) for current OS
#   make test-package-deb          # Test DEB packages (Linux)
#   make test-package-rpm          # Test RPM packages (Linux, uses Fedora container via Docker)
#   make test-package-rpm-native   # Test RPM packages (native Fedora, for CI runners)
#   make test-package-pkg          # Test PKG packages (macOS)
#   make test-package-dmg          # Test DMG packages (macOS)
#   make test-package-msi          # Test MSI packages (Windows)
# =============================================================================

SMOKE_TEST_DIR := packaging/smoke-test-app

# DEB package testing (Ubuntu/Debian)
test-package-deb: build-package
	@echo "=== Testing DEB packages ==="
	$(MAKE) -C $(SMOKE_TEST_DIR) install-driver-dev-deb
	$(MAKE) -C $(SMOKE_TEST_DIR) install-driver-deb
	$(MAKE) -C $(SMOKE_TEST_DIR) build-package CPACK_GENERATORS=DEB
	$(MAKE) -C $(SMOKE_TEST_DIR) install-app-deb
	$(MAKE) -C $(SMOKE_TEST_DIR) test-app-package
	@echo "=== DEB package test completed successfully ==="
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-app-deb || true
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-deb || true
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-dev-deb || true

# RPM package testing (runs in Fedora container for compatibility)
test-package-rpm: build-package
	@echo "=== Testing RPM packages in Fedora container ==="
	docker compose -f $(SMOKE_TEST_DIR)/docker-compose.yml up -d --wait
	docker run --rm \
		-v "$(CURRENT_DIR):/workspace" \
		-w /workspace \
		--network host \
		fedora:latest \
		bash -c ' \
			set -euo pipefail; \
			dnf -y install make cmake gcc-c++ findutils rpm-build; \
			$(MAKE) -C $(SMOKE_TEST_DIR) install-driver-dev-rpm; \
			$(MAKE) -C $(SMOKE_TEST_DIR) install-driver-rpm; \
			pc_file=$$(find /usr -name "scylla-cpp-driver.pc" 2>/dev/null | head -1); \
			if [ -n "$$pc_file" ]; then \
				pc_dir=$$(dirname "$$pc_file"); \
				export PKG_CONFIG_PATH="$${pc_dir}:$${PKG_CONFIG_PATH:-}"; \
			fi; \
			lib_dir=$$(find /usr -name "libscylla-cpp-driver.so*" 2>/dev/null | head -1 | xargs dirname 2>/dev/null || true); \
			if [ -n "$$lib_dir" ]; then \
				export LD_LIBRARY_PATH="$${lib_dir}:$${LD_LIBRARY_PATH:-}"; \
			fi; \
			$(MAKE) -C $(SMOKE_TEST_DIR) build-package CPACK_GENERATORS=RPM; \
			$(MAKE) -C $(SMOKE_TEST_DIR) install-app-rpm; \
			smoke_bin=$$(find /usr -name "scylla-cpp-driver-smoke-test" -type f 2>/dev/null | head -1); \
			if [ -z "$$smoke_bin" ]; then \
				echo "ERROR: smoke-test binary not found"; \
				exit 1; \
			fi; \
			"$$smoke_bin" 127.0.0.1 \
		'
	@echo "=== RPM package test completed successfully ==="
	docker compose -f $(SMOKE_TEST_DIR)/docker-compose.yml down --remove-orphans || true

# RPM package testing (native, for running directly in Fedora environment)
# Use SCYLLA_HOST to specify the ScyllaDB host (default: 127.0.0.1)
# Use SKIP_DOCKER_COMPOSE=1 to skip starting ScyllaDB via docker-compose (for CI with service containers)
SCYLLA_HOST ?= 127.0.0.1
SKIP_DOCKER_COMPOSE ?=
test-package-rpm-native: build-package
	@echo "=== Testing RPM packages (native) ==="
	$(MAKE) -C $(SMOKE_TEST_DIR) install-driver-dev-rpm
	$(MAKE) -C $(SMOKE_TEST_DIR) install-driver-rpm
	$(MAKE) -C $(SMOKE_TEST_DIR) build-package CPACK_GENERATORS=RPM
	$(MAKE) -C $(SMOKE_TEST_DIR) install-app-rpm
	$(MAKE) -C $(SMOKE_TEST_DIR) test-app-package SCYLLA_HOST=$(SCYLLA_HOST) SKIP_DOCKER_COMPOSE=$(SKIP_DOCKER_COMPOSE)
	@echo "=== RPM package test completed successfully ==="
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-app-rpm || true
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-rpm || true
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-dev-rpm || true

# macOS PKG package testing
test-package-pkg: build-package
	@echo "=== Testing PKG packages ==="
	$(MAKE) -C $(SMOKE_TEST_DIR) install-driver-dev-pkg
	$(MAKE) -C $(SMOKE_TEST_DIR) install-driver-pkg
	$(MAKE) -C $(SMOKE_TEST_DIR) build-package CPACK_GENERATORS=productbuild
	$(MAKE) -C $(SMOKE_TEST_DIR) install-app-pkg
	$(MAKE) -C $(SMOKE_TEST_DIR) test-app-package
	@echo "=== PKG package test completed successfully ==="
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-app-pkg || true
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-pkg || true
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-dev-pkg || true

# macOS DMG package testing
test-package-dmg: build-package
	@echo "=== Testing DMG packages ==="
	$(MAKE) -C $(SMOKE_TEST_DIR) install-driver-dev-dmg
	$(MAKE) -C $(SMOKE_TEST_DIR) install-driver-dmg
	$(MAKE) -C $(SMOKE_TEST_DIR) build-package CPACK_GENERATORS=DragNDrop
	$(MAKE) -C $(SMOKE_TEST_DIR) install-app-dmg
	$(MAKE) -C $(SMOKE_TEST_DIR) test-app-package
	@echo "=== DMG package test completed successfully ==="
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-app-dmg || true
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-dmg || true
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-dev-dmg || true

# Windows MSI package testing
test-package-msi: .windows-setup-wix build-package
	@echo "=== Testing MSI packages ==="
	$(MAKE) -C $(SMOKE_TEST_DIR) install-driver-dev
	$(MAKE) -C $(SMOKE_TEST_DIR) install-driver
	$(MAKE) -C $(SMOKE_TEST_DIR) build-package
	$(MAKE) -C $(SMOKE_TEST_DIR) install-app
	$(MAKE) -C $(SMOKE_TEST_DIR) test-app-package
	@echo "=== MSI package test completed successfully ==="

# Combined Linux package testing (DEB + RPM)
test-package-linux:
	$(MAKE) test-package-deb
	rm -rf $(SMOKE_TEST_DIR)/build
	$(MAKE) test-package-rpm

# Windows package testing (MSI)
test-package-windows: test-package-msi

# Combined macOS package testing (PKG + DMG)
test-package-macos:
	$(MAKE) test-package-pkg
	rm -rf $(SMOKE_TEST_DIR)/build
	$(MAKE) test-package-dmg

# OS-specific default test-package target
ifeq ($(OS_TYPE),macos)
test-package: test-package-macos
else ifeq ($(OS_TYPE),windows)
test-package: test-package-windows
else
test-package: test-package-linux
endif

# Collect built packages into artifacts directory
collect-package-artifacts:
ifeq ($(OS_TYPE),windows)
	@pwsh -NoProfile -Command " \
		New-Item -ItemType Directory -Path artifacts\windows -Force | Out-Null; \
		Get-ChildItem build -Filter *.msi | Copy-Item -Destination artifacts\windows; \
		Get-ChildItem $(SMOKE_TEST_DIR)/build -Filter *.msi -ErrorAction SilentlyContinue | Copy-Item -Destination artifacts\windows"
else ifeq ($(OS_TYPE),macos)
	@set -euo pipefail
	shopt -s nullglob
	mkdir -p artifacts/macos
	for file in build/*.pkg build/*.dmg $(SMOKE_TEST_DIR)/build/*.pkg $(SMOKE_TEST_DIR)/build/*.dmg; do
		cp "$$file" artifacts/macos/
	done
else
	@set -euo pipefail
	shopt -s nullglob
	mkdir -p artifacts/linux
	for file in build/*.deb build/*.rpm; do
		cp "$$file" artifacts/linux/
	done
endif

# Download artifacts from GitHub Actions (requires RUN_ID, uses GH_TOKEN or GITHUB_TOKEN)
download-package-artifacts:
ifndef RUN_ID
	$(error RUN_ID is required for downloading artifacts)
endif
	@set -euo pipefail
	mkdir -p packages
	gh run download $(RUN_ID) --dir packages

# Upload packages to GitHub Release (requires TAG_NAME, uses GH_TOKEN or GITHUB_TOKEN)
upload-packages-to-release:
ifndef TAG_NAME
	$(error TAG_NAME is required for uploading to release)
endif
	@set -euo pipefail
	shopt -s nullglob
	for file in packages/*/*; do
		echo "Uploading $$file"
		gh release upload "$(TAG_NAME)" "$$file" --clobber
	done
