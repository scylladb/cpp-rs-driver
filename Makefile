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
:UuidTests.*\
:HostFilterTest.*\
:ExecutionProfileTest.*\
:DCExecutionProfileTest.*\
:DisconnectedNullStringApiArgsTest.*\
:MetricsTests.*\
:DcAwarePolicyTest.*\
:AsyncTests.*\
:VectorTests.*\
:-SchemaMetadataTest.Integration_Cassandra_RegularMetadataNotMarkedVirtual\
:SchemaMetadataTest.Integration_Cassandra_VirtualMetadata\
:HeartbeatTests.Integration_Cassandra_HeartbeatFailed\
:TimestampTests.Integration_Cassandra_MonotonicTimestampGenerator\
:ExecutionProfileTest.Integration_Cassandra_RequestTimeout\
:ExecutionProfileTest.Integration_Cassandra_RoundRobin\
:ExecutionProfileTest.Integration_Cassandra_TokenAwareRouting\
:ExecutionProfileTest.Integration_Cassandra_SpeculativeExecutionPolicy\
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
:UuidTests.*\
:HostFilterTest.*\
:ExecutionProfileTest.*\
:DCExecutionProfileTest.*\
:DisconnectedNullStringApiArgsTest.*\
:MetricsTests.*\
:DcAwarePolicyTest.*\
:AsyncTests.*\
:VectorTests.*\
:-PreparedTests.Integration_Cassandra_FailFastWhenPreparedIDChangesDuringReprepare\
:SchemaMetadataTest.Integration_Cassandra_RegularMetadataNotMarkedVirtual\
:SchemaMetadataTest.Integration_Cassandra_VirtualMetadata\
:HeartbeatTests.Integration_Cassandra_HeartbeatFailed\
:TimestampTests.Integration_Cassandra_MonotonicTimestampGenerator\
:ExecutionProfileTest.Integration_Cassandra_RequestTimeout\
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
	vector_insert_select \

	# auth <- unimplemented `cass_cluster_set_authenticator_callbacks()`
	# host_listener <- never terminates by design; loops forever listening to events.
	# logging <- unimplemented `cass_cluster_set_host_listener_callback()`
	# schema_meta <- unimplemented multiple schema-related functions
	# vector_search_ann <- needs a Vector Store instance running alongside the cluster
endif

ifndef CCM_COMMIT_ID
	export CCM_COMMIT_ID := master
endif

ifndef SCYLLA_VERSION
	SCYLLA_VERSION := release:2026.2.2
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
BUILD_DIR := $(CURRENT_DIR)build
INTEGRATION_TEST_BIN := ${BUILD_DIR}/cassandra-integration-tests
CMAKE_FLAGS ?=
CMAKE_BUILD_TYPE ?= Release
OPENSSL_WIN_VERSION ?= 1.1.1u

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

install-lcov-if-missing: update-apt-cache-if-needed
	@genhtml --version >/dev/null 2>&1 || (
		echo "lcov not found in the system, install it."
		sudo apt-get install -y lcov
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

# =============================================================================
# OpenSSL 3.0 Compatibility Verification
# =============================================================================
# Regression test for issue #455: ensures the static driver archive can link
# against OpenSSL 3.0 (our minimum supported version). Rather than rebuilding
# from source, this target uses the artifact already produced by the default
# build (which enables both shared and static). If the archive references
# symbols only available in OpenSSL >3.0 (e.g. due to build environment
# contamination), the link fails.
#
# This target is Linux/amd64-only (matches our release artifact platform).
# =============================================================================

OPENSSL_3_0_COMPAT_SYSROOT := /tmp/openssl-3.0-compat-sysroot
OPENSSL_3_0_LIBSSL_DEV_URL := https://launchpad.net/ubuntu/+archive/primary/+files/libssl-dev_3.0.2-0ubuntu1_amd64.deb
OPENSSL_3_0_LIBSSL_DEV_SHA256 := f3671a9f01aa92928db200b3d28f1acb782366882fe318a940649bd02363ceb6
OPENSSL_3_0_LIBSSL_DEV_PATH := /tmp/libssl-dev_3.0.2.deb

verify-openssl-3.0-compat:
	@echo "=== Verifying static driver links against OpenSSL 3.0 (issue #455) ==="
	rm -rf "$(OPENSSL_3_0_COMPAT_SYSROOT)"
	DESTDIR="$(OPENSSL_3_0_COMPAT_SYSROOT)" cmake --install "$(BUILD_DIR)"
	curl -fL -o "$(OPENSSL_3_0_LIBSSL_DEV_PATH)" "$(OPENSSL_3_0_LIBSSL_DEV_URL)"
	echo "$(OPENSSL_3_0_LIBSSL_DEV_SHA256)  $(OPENSSL_3_0_LIBSSL_DEV_PATH)" | sha256sum --check
	dpkg-deb -x "$(OPENSSL_3_0_LIBSSL_DEV_PATH)" "$(OPENSSL_3_0_COMPAT_SYSROOT)"
	rm -f "$(OPENSSL_3_0_COMPAT_SYSROOT)/usr/lib/x86_64-linux-gnu/libssl.so"
	rm -f "$(OPENSSL_3_0_COMPAT_SYSROOT)/usr/lib/x86_64-linux-gnu/libcrypto.so"
	PKG_CONFIG_SYSROOT_DIR="$(OPENSSL_3_0_COMPAT_SYSROOT)" \
	PKG_CONFIG_PATH="$(OPENSSL_3_0_COMPAT_SYSROOT)/usr/local/lib/x86_64-linux-gnu/pkgconfig:$(OPENSSL_3_0_COMPAT_SYSROOT)/usr/lib/x86_64-linux-gnu/pkgconfig" \
	pkg-config --libs --static scylladb_static
	cc \
		$$(PKG_CONFIG_SYSROOT_DIR="$(OPENSSL_3_0_COMPAT_SYSROOT)" \
		   PKG_CONFIG_PATH="$(OPENSSL_3_0_COMPAT_SYSROOT)/usr/local/lib/x86_64-linux-gnu/pkgconfig:$(OPENSSL_3_0_COMPAT_SYSROOT)/usr/lib/x86_64-linux-gnu/pkgconfig" \
		   pkg-config --cflags scylladb_static) \
		examples/ssl/ssl.c \
		$$(PKG_CONFIG_SYSROOT_DIR="$(OPENSSL_3_0_COMPAT_SYSROOT)" \
		   PKG_CONFIG_PATH="$(OPENSSL_3_0_COMPAT_SYSROOT)/usr/local/lib/x86_64-linux-gnu/pkgconfig:$(OPENSSL_3_0_COMPAT_SYSROOT)/usr/lib/x86_64-linux-gnu/pkgconfig" \
		   pkg-config --libs --static scylladb_static) \
		-o /tmp/openssl-3.0-compat-link-test
	@echo "=== OpenSSL 3.0 compatibility verified ==="
	rm -rf "$(OPENSSL_3_0_COMPAT_SYSROOT)" \
		"$(OPENSSL_3_0_LIBSSL_DEV_PATH)" /tmp/openssl-3.0-compat-link-test

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
	cmake -S . -B build -G "Visual Studio 17 2022" -A x64 -DCMAKE_BUILD_TYPE=$(CMAKE_BUILD_TYPE) -DOPENSSL_VERSION=$(OPENSSL_WIN_VERSION) $(CMAKE_FLAGS)
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

.PHONY: deny
deny:
	@cd ${CURRENT_DIR}/scylla-rust-wrapper
	cargo deny check

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
	@echo "Running Rust CCM integration tests on scylla ${SCYLLA_VERSION}"
	@cd ${CURRENT_DIR}/scylla-rust-wrapper
	@# These tests start real, TLS-enabled clusters via CCM and drive the
	@# driver through its C API. `SCYLLA_TEST_CLUSTER` selects the CCM version
	@# (see the scylla-ccm-bridge crate). Ignored tests (documenting not-yet
	@# implemented behavior) are intentionally not run.
	@#
	@# Prefer a fully-qualified SCYLLA_VERSION (e.g. release:2025.3.8 rather
	@# than release:2025.3): scylla-ccm re-resolves the version on every `ccm`
	@# invocation, and a partial one forces it to list an S3 bucket and sleep
	@# for a random 0-5 seconds each time, which dominates the test runtime.
	SCYLLA_TEST_CLUSTER="${SCYLLA_VERSION}" CCM_ROOT_DIR=/tmp/ccm-rust RUSTFLAGS="${FULL_RUSTFLAGS}" cargo test --test integration ccm

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
	@# The `ccm` tests require a running CCM environment and a real cluster,
	@# so they are excluded here. They are run as part of the integration
	@# test targets instead (see `run-test-integration-scylla`).
	RUSTFLAGS="${FULL_RUSTFLAGS}" cargo test -- --skip ccm

# =============================================================================
# Code coverage
# =============================================================================
# LLVM source-based coverage (`-C instrument-coverage`, set up by
# cargo-llvm-cov) of the driver's Rust implementation in scylla-rust-wrapper,
# as exercised by the test suites:
#
#   run-test-coverage-unit    The Rust unit and proxy tests, as run by
#                             `run-test-unit`. Needs no cluster.
#   run-test-coverage-scylla  Those, the C++ integration tests (both test sets
#                             `run-test-integration-scylla` runs) and the Rust
#                             CCM integration tests, against ScyllaDB
#                             SCYLLA_VERSION. This is what CI runs.
#   coverage-report           Rewrites the reports below from the profile data
#                             collected so far.
#   clean-coverage            Removes the coverage build, data and reports.
#
# The reports are ${COVERAGE_REPORT_DIR}/lcov.info, an HTML report in
# ${COVERAGE_REPORT_DIR}/html and a per-file summary in
# ${COVERAGE_REPORT_DIR}/summary.txt. Both run-test-coverage-* targets start
# from no profile data, run every suite even when an earlier one fails, write
# the reports, and only then exit non-zero, so that a failed run still leaves a
# (partial) report behind to diagnose it with.
#
# Nothing runs under valgrind: it adds nothing to what coverage measures, and
# it slows instrumented code down a lot. The C and C++ sources in src/ and
# tests/ are test harness code, compiled into the integration test binary but
# never into the driver library, so they are not measured. Neither is the Rust
# test code: scylla-rust-wrapper/tests/, scylla-rust-wrapper/src/testing/, and
# the test modules and helpers built only with cfg(test) (see coverage-report).
#
# Needs cargo-llvm-cov (0.9.1, as in CI: the targets below rely on how it sets
# up the instrumentation), rustup's llvm-tools component and lcov:
#   cargo install cargo-llvm-cov --version 0.9.1 --locked
#   rustup component add llvm-tools
#   sudo apt-get install lcov
# =============================================================================

# The coverage build has a tree of its own, so that nothing built without
# instrumentation (in build/ or scylla-rust-wrapper/target) can be reused.
COVERAGE_BUILD_DIR := $(CURRENT_DIR)build-coverage
# cargo_build() in cmake/CMakeCargo.cmake builds the library with
# CARGO_TARGET_DIR set to the CMake binary directory of scylla-rust-wrapper.
# The Rust test suites are built in that same directory, so that one
# `cargo llvm-cov clean --workspace` resets every build, and every
# instrumented process writes its profile data there (see LLVM_PROFILE_FILE).
COVERAGE_TARGET_DIR := $(COVERAGE_BUILD_DIR)/scylla-rust-wrapper
COVERAGE_REPORT_DIR := $(COVERAGE_BUILD_DIR)/llvm-cov
COVERAGE_INTEGRATION_TEST_BIN := $(COVERAGE_BUILD_DIR)/cassandra-integration-tests
# The library the integration test binary loads, through its RUNPATH.
COVERAGE_LIBRARY := $(COVERAGE_BUILD_DIR)/libscylladb.so
# The Rust test binaries, as the build in .coverage-test-unit lists them: the
# driver's unit tests, which build the driver with cfg(test), and the other
# test binaries, which link it built without.
COVERAGE_UNIT_TEST_BINS := $(COVERAGE_TARGET_DIR)/unit-test-binaries.txt
COVERAGE_RUST_TEST_BINS := $(COVERAGE_TARGET_DIR)/rust-test-binaries.txt

# Sets up the rest of the recipe for coverage: a RUSTC_WRAPPER that adds
# `-C instrument-coverage` to the rustc invocations for this crate, on top of
# whatever RUSTFLAGS the Makefile or CMake pass, and LLVM_PROFILE_FILE, which
# tells instrumented processes where to write their profile data. The output
# is assigned first so that a failing `cargo llvm-cov` fails the recipe instead
# of letting it carry on uninstrumented.
define COVERAGE_ENV
	export CARGO_TARGET_DIR="${COVERAGE_TARGET_DIR}"
	coverage_env="$$(cd "${CURRENT_DIR}scylla-rust-wrapper" && cargo llvm-cov show-env --sh)" || {
		echo "Coverage needs cargo-llvm-cov and the llvm-tools rustup component, see the \"Code coverage\" section of the Makefile." >&2
		exit 1
	}
	eval "$${coverage_env}"
	coverage_tools="$$(rustc --print sysroot)/lib/rustlib/$$(rustc -vV | sed -n 's/^host: //p')/bin"
	llvm_profdata="$${LLVM_PROFDATA:-$${coverage_tools}/llvm-profdata}"
	llvm_cov="$${LLVM_COV:-$${coverage_tools}/llvm-cov}"
endef

# Fails unless the $(1) suite left profile data in which instrumented driver
# code ran. A suite whose binaries lost their instrumentation writes no profile
# data at all, and a Rust test binary that ran no test writes profile data in
# which no function ran. The suites write their profile data under their own
# name, so the profile data that the build scripts (which are instrumented as
# well) write while being built counts for no suite. The suite's merged profile
# data goes next to it, where the next run overwrites it, rather than into a
# temporary file that a failing llvm-profdata would leave behind.
define check-coverage-profile
	profiles=("${COVERAGE_TARGET_DIR}"/$(1)-*.profraw)
	if [ ! -e "$${profiles[0]}" ]; then
		echo "The $(1) suite wrote no coverage profile data: it did not run instrumented code." >&2
		exit 1
	fi
	profdata="${COVERAGE_TARGET_DIR}/$(1).profdata"
	"$${llvm_profdata}" merge -sparse "$${profiles[@]}" -o "$${profdata}"
	functions="$$("$${llvm_profdata}" show "$${profdata}" | sed -n 's/^Total functions: //p')"
	if [ "$${functions:-0}" -eq 0 ]; then
		echo "The $(1) suite ran no instrumented code." >&2
		exit 1
	fi
	echo "The $(1) suite ran $${functions} instrumented functions."
endef

# Fails unless the gtest XML report $(1) records at least one test. gtest
# succeeds when its filter matches no test, and unlike a Rust test binary the
# integration test binary runs driver code even then (from its own setup and
# teardown), so check-coverage-profile cannot tell that case apart.
define check-gtest-ran
	tests="$$(sed -n 's/^<testsuites tests="\([0-9]*\)".*/\1/p' "$(1)")"
	if [ "$${tests:-0}" -eq 0 ]; then
		echo "No integration test ran, see $(1)." >&2
		exit 1
	fi
endef

# Fails unless every test binary in the `cargo test` output of the $(1) suite
# ran at least one test. check-coverage-profile fails a suite whose binaries
# all ran none, but not a binary that ran none next to one that ran some. Doc
# tests do not count: they are not instrumented. When no test binary in the
# output reports how many tests it ran, the check fails too: cargo printing
# its output differently would look like that, and would otherwise turn the
# check off without a word.
define check-rust-tests-ran
	sed 's/\x1b\[[0-9;]*m//g' "${COVERAGE_TARGET_DIR}/$(1)-tests.log" | awk '
		/^ *Running / { binary = $$NF; gsub(/^\(|\)$$/, "", binary); next }
		/^ *Doc-tests / { binary = ""; next }
		binary != "" && /^running [0-9]+ tests?$$/ {
			if ($$2 == 0) { print binary " ran no test." > "/dev/stderr"; failed = 1 }
			binaries++; tests += $$2; binary = ""
		}
		END {
			if (!binaries) { print "No test binary of the $(1) suite reported how many tests it ran." > "/dev/stderr"; exit 1 }
			if (!failed) printf "The $(1) suite ran %d %s in %d test %s.\n", tests, (tests == 1 ? "test" : "tests"), binaries, (binaries == 1 ? "binary" : "binaries")
			exit failed
		}'
endef

.coverage-clean: install-cargo-if-missing
	@${COVERAGE_ENV}
	cd "${CURRENT_DIR}scylla-rust-wrapper"
	@# Removes the profile data and the crate's own build artifacts, so that
	@# everything this run measures is rebuilt instrumented and run afresh.
	cargo llvm-cov clean --workspace
	rm -rf "${COVERAGE_REPORT_DIR}" "${COVERAGE_UNIT_TEST_BINS}" "${COVERAGE_RUST_TEST_BINS}" "${COVERAGE_BUILD_DIR}"/integration-tests*.xml

.coverage-test-unit: install-cargo-if-missing
	@${COVERAGE_ENV}
	echo "Running Rust unit and proxy tests with coverage"
	cd "${CURRENT_DIR}scylla-rust-wrapper"
	@# Built first, so that nothing is built while the suite's LLVM_PROFILE_FILE
	@# is set, and so that coverage-report knows which test binaries ran.
	mkdir -p "${COVERAGE_TARGET_DIR}"
	RUSTFLAGS="${FULL_RUSTFLAGS}" cargo test --no-run --message-format=json-render-diagnostics > "${COVERAGE_TARGET_DIR}/rust-tests.json"
	@# The test binaries are the executables built with the test profile.
	@# Integration tests are the targets of kind "test"; any other test binary
	@# is a unit test build of the driver's library. Both kinds exist, so a
	@# list that comes out empty means cargo's output no longer reads as
	@# expected, and coverage-report would count the wrong lines.
	sed -n '/"profile":{[^}]*"test":true/ { /"kind":\["test"\]/! s/.*"executable":"\([^"]*\)".*/\1/p }' "${COVERAGE_TARGET_DIR}/rust-tests.json" > "${COVERAGE_UNIT_TEST_BINS}"
	sed -n '/"profile":{[^}]*"test":true/ { /"kind":\["test"\]/ s/.*"executable":"\([^"]*\)".*/\1/p }' "${COVERAGE_TARGET_DIR}/rust-tests.json" > "${COVERAGE_RUST_TEST_BINS}"
	if [ ! -s "${COVERAGE_UNIT_TEST_BINS}" ] || [ ! -s "${COVERAGE_RUST_TEST_BINS}" ]; then
		echo "No unit test build of the driver, or no integration test binary, in ${COVERAGE_TARGET_DIR}/rust-tests.json." >&2
		exit 1
	fi
	@# Every test binary runs even when an earlier one fails, so that
	@# coverage-report has profile data for each of them.
	set -o pipefail
	LLVM_PROFILE_FILE="${COVERAGE_TARGET_DIR}/unit-%p-%4m.profraw" RUSTFLAGS="${FULL_RUSTFLAGS}" cargo test --no-fail-fast -- --skip ccm 2>&1 | tee "${COVERAGE_TARGET_DIR}/unit-tests.log"
	$(call check-rust-tests-ran,unit)
	$(call check-coverage-profile,unit)

.coverage-build-integration-test-bin: install-cargo-if-missing
	@${COVERAGE_ENV}
	echo "Building instrumented integration test binary to ${COVERAGE_INTEGRATION_TEST_BIN}"
	mkdir -p "${COVERAGE_BUILD_DIR}"
	cd "${COVERAGE_BUILD_DIR}"
	@# Debug, i.e. cargo's dev profile, as for the Rust tests, which also spares
	@# the build the release profile's LTO. Without the static library, which
	@# the integration test binary does not link.
	cmake -DCASS_BUILD_INTEGRATION_TESTS=ON -DCASS_BUILD_STATIC=OFF -DCMAKE_BUILD_TYPE=Debug .. && (make -j 4 || make)

.coverage-test-cpp-scylla: .prepare-environment-update-aio-max-nr
	@${COVERAGE_ENV}
	@# The second test set runs even when the first one fails, as every suite
	@# does in run-test-coverage-scylla, so that a failed run's report still
	@# covers both.
	status=0
	echo "Running integration tests on scylla ${SCYLLA_VERSION} with coverage"
	LLVM_PROFILE_FILE="${COVERAGE_TARGET_DIR}/cpp-%p-%4m.profraw" "${COVERAGE_INTEGRATION_TEST_BIN}" --scylla --version=${SCYLLA_VERSION} --category=CASSANDRA --verbose=ccm --gtest_filter="${SCYLLA_TEST_FILTER}" --gtest_output="xml:${COVERAGE_BUILD_DIR}/integration-tests.xml" || status=1
	echo "Running timeout sensitive tests on scylla ${SCYLLA_VERSION} with coverage"
	LLVM_PROFILE_FILE="${COVERAGE_TARGET_DIR}/cpp-%p-%4m.profraw" "${COVERAGE_INTEGRATION_TEST_BIN}" --scylla --version=${SCYLLA_VERSION} --category=CASSANDRA --verbose=ccm --gtest_filter="${SCYLLA_NO_VALGRIND_TEST_FILTER}" --gtest_output="xml:${COVERAGE_BUILD_DIR}/integration-tests-no-valgrind.xml" || status=1
	$(call check-gtest-ran,${COVERAGE_BUILD_DIR}/integration-tests.xml)
	$(call check-gtest-ran,${COVERAGE_BUILD_DIR}/integration-tests-no-valgrind.xml)
	$(call check-coverage-profile,cpp)
	exit $${status}

.coverage-test-ccm-scylla: .prepare-environment-update-aio-max-nr
	@${COVERAGE_ENV}
	echo "Running Rust CCM integration tests on scylla ${SCYLLA_VERSION} with coverage"
	cd "${CURRENT_DIR}scylla-rust-wrapper"
	@# The same tests, cluster setup and CCM root as in `run-test-integration-scylla`.
	set -o pipefail
	SCYLLA_TEST_CLUSTER="${SCYLLA_VERSION}" CCM_ROOT_DIR=/tmp/ccm-rust LLVM_PROFILE_FILE="${COVERAGE_TARGET_DIR}/ccm-%p-%4m.profraw" RUSTFLAGS="${FULL_RUSTFLAGS}" cargo test --test integration ccm 2>&1 | tee "${COVERAGE_TARGET_DIR}/ccm-tests.log"
	$(call check-rust-tests-ran,ccm)
	$(call check-coverage-profile,ccm)

run-test-coverage-unit: .coverage-clean
	@status=0
	${MAKE} --no-print-directory .coverage-test-unit || status=1
	${MAKE} --no-print-directory coverage-report || status=1
	exit $${status}

run-test-coverage-scylla: .coverage-clean
	@status=0
	${MAKE} --no-print-directory .coverage-test-unit || status=1
	@# Not run on a failed build, which could leave an older binary in place.
	(${MAKE} --no-print-directory .coverage-build-integration-test-bin && ${MAKE} --no-print-directory .coverage-test-cpp-scylla) || status=1
	${MAKE} --no-print-directory .coverage-test-ccm-scylla || status=1
	${MAKE} --no-print-directory coverage-report || status=1
	exit $${status}

coverage-report: install-cargo-if-missing install-lcov-if-missing
	@${COVERAGE_ENV}
	@# The binaries that ran driver code: the Rust test binaries, and the
	@# library the C++ integration tests loaded, if they ran.
	unit_tests=()
	builds=()
	if [ -s "${COVERAGE_UNIT_TEST_BINS}" ]; then
		mapfile -t unit_tests < "${COVERAGE_UNIT_TEST_BINS}"
	fi
	if [ -s "${COVERAGE_RUST_TEST_BINS}" ]; then
		mapfile -t builds < "${COVERAGE_RUST_TEST_BINS}"
	fi
	if compgen -G "${COVERAGE_TARGET_DIR}/cpp-*.profraw" >/dev/null; then
		builds+=("${COVERAGE_LIBRARY}")
	fi
	if [ "$${#builds[@]}" -eq 0 ]; then
		echo "No test suite that uses the driver built without cfg(test) has run, so there is no coverage to report." >&2
		exit 1
	fi
	@# The driver's own sources: tests/ holds test code, src/testing/ test
	@# support code built only into test builds, and the bindings bindgen
	@# generates are in the build tree.
	mapfile -t sources < <(find "${CURRENT_DIR}scylla-rust-wrapper/src" -name '*.rs' -not -path '*/src/testing/*' | sort)
	if [ "$${#sources[@]}" -eq 0 ]; then
		echo "No driver sources found in ${CURRENT_DIR}scylla-rust-wrapper/src." >&2
		exit 1
	fi
	rm -rf "${COVERAGE_REPORT_DIR}"
	mkdir -p "${COVERAGE_REPORT_DIR}/binaries"
	"$${llvm_profdata}" merge -sparse "${COVERAGE_TARGET_DIR}"/*.profraw -o "${COVERAGE_TARGET_DIR}/coverage.profdata"
	: > "${COVERAGE_TARGET_DIR}/empty.proftext"
	"$${llvm_profdata}" merge "${COVERAGE_TARGET_DIR}/empty.proftext" -o "${COVERAGE_TARGET_DIR}/empty.profdata"
	@# One export per binary, merged line by line by ci/merge_coverage.py.
	@# llvm-cov can take all the binaries at once, but then it keeps only the
	@# first binary's copy of a function whose name several binaries share, and
	@# it matches profile data to a function by name and hash. The C API
	@# functions are #[no_mangle], so they have the same name in every binary,
	@# but every binary builds the crate differently, so they have a different
	@# hash in each: a combined export would count one binary's runs of a C API
	@# function and drop the others', and report lines that only the C++
	@# integration tests reached as not covered whenever a Rust test also called
	@# that function. For the same reason, the export of a single binary leaves
	@# out every C API function that only another binary ran (llvm-cov warns
	@# that those "have mismatched data"), so each binary built without cfg(test)
	@# is also exported against no profile data at all, as the complete list of
	@# the driver's lines, none of them covered.
	@#
	@# A binary that covers no line fails the report, but only once the report
	@# is written: when a suite failed early, a binary it built may not have
	@# run, and the rest of the report is still what the failure is diagnosed
	@# with.
	status=0
	merge=()
	for binary in "$${builds[@]}" "$${unit_tests[@]}"; do
		part="${COVERAGE_REPORT_DIR}/binaries/$$(basename "$${binary}")"
		"$${llvm_cov}" export -format=lcov -instr-profile="${COVERAGE_TARGET_DIR}/coverage.profdata" "$${binary}" "$${sources[@]}" > "$${part}.info"
		if ! grep -Eq '^DA:[0-9]+,[1-9]' "$${part}.info"; then
			echo "$${binary} covers no line of the driver's sources: it did not run, or its profile data is missing." >&2
			status=1
		fi
		if printf '%s\n' "$${unit_tests[@]}" | grep -qxF "$${binary}"; then
			merge+=(--test-build "$${part}.info")
		else
			"$${llvm_cov}" export -format=lcov -instr-profile="${COVERAGE_TARGET_DIR}/empty.profdata" "$${binary}" "$${sources[@]}" > "$${part}.lines.info"
			merge+=("$${part}.info" "$${part}.lines.info")
		fi
	done
	python3 "${CURRENT_DIR}ci/merge_coverage.py" --output "${COVERAGE_REPORT_DIR}/lcov.info" --root "${CURRENT_DIR}scylla-rust-wrapper/src/" "$${merge[@]}" > "${COVERAGE_REPORT_DIR}/summary.txt"
	genhtml -q "${COVERAGE_REPORT_DIR}/lcov.info" -o "${COVERAGE_REPORT_DIR}/html"
	cat "${COVERAGE_REPORT_DIR}/summary.txt"
	exit $${status}

clean-coverage:
	rm -rf "${COVERAGE_BUILD_DIR}"

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

define MAYBE_SUDO
	if [ "$$(id -u)" -eq 0 ]; then SUDO=""; else SUDO="sudo"; fi
endef

SMOKE_TEST_DIR := packaging/smoke-test-app

# DEB package testing (Ubuntu/Debian)
test-package-deb: build-package
	@echo "=== Testing DEB packages ==="
	$(MAYBE_SUDO)
	$$SUDO rm -rf $(SMOKE_TEST_DIR)/build
	$(MAKE) -C $(SMOKE_TEST_DIR) verify-driver-dev-deb
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-dev-deb || true
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-deb || true
	$(MAKE) -C $(SMOKE_TEST_DIR) install-driver-dev-deb
	$(MAKE) -C $(SMOKE_TEST_DIR) build-package CPACK_GENERATORS=DEB
	$(MAKE) -C $(SMOKE_TEST_DIR) install-app-deb
	$(MAKE) -C $(SMOKE_TEST_DIR) test-app-package
	@echo "=== DEB package test completed successfully ==="
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-app-deb || true
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-dev-deb || true
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-deb || true

# RPM package testing (runs in Fedora container for compatibility)
test-package-rpm: build-package
	@echo "=== Testing RPM packages in Fedora container ==="
	$(MAYBE_SUDO)
	$$SUDO rm -rf $(SMOKE_TEST_DIR)/build
	docker compose -f $(SMOKE_TEST_DIR)/docker-compose.yml up -d --wait
	docker run --rm \
		-v "$(CURRENT_DIR):/workspace" \
		-w /workspace \
		--network host \
		fedora:latest \
		bash -c ' \
			set -euo pipefail; \
			dnf -y install make cmake gcc-c++ findutils rpm-build zlib-devel createrepo_c; \
			$(MAKE) -C $(SMOKE_TEST_DIR) verify-driver-dev-rpm; \
			$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-dev-rpm || true; \
			$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-rpm || true; \
			$(MAKE) -C $(SMOKE_TEST_DIR) install-driver-dev-rpm; \
			pc_file=$$(find /usr -name "scylladb.pc" 2>/dev/null | head -1); \
			if [ -n "$$pc_file" ]; then \
				pc_dir=$$(dirname "$$pc_file"); \
				export PKG_CONFIG_PATH="$${pc_dir}:$${PKG_CONFIG_PATH:-}"; \
			fi; \
			lib_dir=$$(find /usr -name "libscylladb.so*" 2>/dev/null | head -1 | xargs dirname 2>/dev/null || true); \
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
	$(MAYBE_SUDO)
	$$SUDO rm -rf $(SMOKE_TEST_DIR)/build
	dnf -y install createrepo_c
	$(MAKE) -C $(SMOKE_TEST_DIR) verify-driver-dev-rpm
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-dev-rpm || true
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-rpm || true
	$(MAKE) -C $(SMOKE_TEST_DIR) install-driver-dev-rpm
	$(MAKE) -C $(SMOKE_TEST_DIR) build-package CPACK_GENERATORS=RPM
	$(MAKE) -C $(SMOKE_TEST_DIR) install-app-rpm
	$(MAKE) -C $(SMOKE_TEST_DIR) test-app-package SCYLLA_HOST=$(SCYLLA_HOST) SKIP_DOCKER_COMPOSE=$(SKIP_DOCKER_COMPOSE)
	@echo "=== RPM package test completed successfully ==="
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-app-rpm || true
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-dev-rpm || true
	$(MAKE) -C $(SMOKE_TEST_DIR) remove-driver-rpm || true

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
	$(MAYBE_SUDO)
	$$SUDO rm -rf $(SMOKE_TEST_DIR)/build
	$(MAKE) test-package-rpm

# Windows package testing (MSI)
test-package-windows: test-package-msi

# Combined macOS package testing (PKG + DMG)
test-package-macos:
	$(MAKE) test-package-pkg
	$(MAYBE_SUDO)
	$$SUDO rm -rf $(SMOKE_TEST_DIR)/build
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
		Get-ChildItem build -Filter *.msi | Copy-Item -Destination artifacts\windows"
else ifeq ($(OS_TYPE),macos)
	@set -euo pipefail
	shopt -s nullglob
	mkdir -p artifacts/macos
	for file in build/*.pkg build/*.dmg; do
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
