#!/usr/bin/env bash
# Generate BUILD_INFO metadata file capturing the build environment.
# Usage: ci/generate-build-info.sh <output-path>
#
# The script degrades gracefully: fields that cannot be determined are
# printed as "unknown". If bash is not available, CMake will skip this
# step without failing the build (see CMakeLists.txt).

set -euo pipefail

OUTPUT="${1:?Usage: $0 <output-path>}"
BUILD_DIR="$(cd "$(dirname "$OUTPUT")" && pwd)"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SOURCE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# --- Helpers ---

cmake_cache_get() {
    local key="$1"
    local cache="$BUILD_DIR/CMakeCache.txt"
    if [ -f "$cache" ]; then
        grep "^${key}:" "$cache" 2>/dev/null | cut -d= -f2- || echo "unknown"
    else
        echo "unknown"
    fi
}

# --- Driver version ---

VERSION=$(cd "$SOURCE_DIR" && git describe --tags --always 2>/dev/null || echo "unknown")

# --- Platform ---

ARCH=$(uname -m 2>/dev/null || echo "unknown")
KERNEL=$(uname -r 2>/dev/null || echo "unknown")

case "${OSTYPE:-}" in
    linux-gnu*|linux*)
        if [ -f /etc/os-release ]; then
            OS=$(. /etc/os-release && echo "${PRETTY_NAME:-unknown}")
        else
            OS="Linux (unknown distro)"
        fi
        GLIBC=$(ldd --version 2>/dev/null | head -1 | grep -oE '[0-9]+\.[0-9]+$' || echo "unknown")
        ;;
    darwin*)
        OS="macOS $(sw_vers -productVersion 2>/dev/null || echo unknown)"
        GLIBC="N/A"
        ;;
    msys*|cygwin*|mingw*)
        OS="Windows"
        GLIBC="N/A"
        ;;
    *)
        OS="unknown (${OSTYPE:-not set})"
        GLIBC="unknown"
        ;;
esac

# --- Toolchain ---

RUST_VERSION=$(rustc --version 2>/dev/null || echo "unknown")
CARGO_VERSION=$(cargo --version 2>/dev/null || echo "unknown")
CMAKE_VERSION_STR=$(cmake --version 2>/dev/null | head -1 || echo "unknown")
CC_VERSION=$(${CC:-cc} --version 2>/dev/null | head -1 || echo "unknown")

# --- OpenSSL (from openssl-sys build script output) ---

OPENSSL_SYS_OUTPUT=""
# The output file is at target/<profile>/build/openssl-sys-<hash>/output
# Search within the cargo target directory used by this build.
CARGO_TARGET_DIR=$(cmake_cache_get "CARGO_TARGET_DIR")
if [ "$CARGO_TARGET_DIR" = "unknown" ] || [ ! -d "$CARGO_TARGET_DIR" ]; then
    # Fallback: search in the standard location
    CARGO_TARGET_DIR="$SOURCE_DIR/scylla-rust-wrapper/target"
fi
if [ -d "$CARGO_TARGET_DIR" ]; then
    OPENSSL_SYS_OUTPUT=$(find "$CARGO_TARGET_DIR" -path "*/openssl-sys-*/output" -type f 2>/dev/null | head -1)
fi

if [ -n "$OPENSSL_SYS_OUTPUT" ] && [ -f "$OPENSSL_SYS_OUTPUT" ]; then
    OPENSSL_VERSION_HEX=$(grep "^cargo:version_number=" "$OPENSSL_SYS_OUTPUT" | cut -d= -f2 || echo "")
    OPENSSL_INCLUDE=$(grep "^cargo:include=" "$OPENSSL_SYS_OUTPUT" | cut -d= -f2 || echo "unknown")
    OSSLCONF_FLAGS=$(grep "^cargo:rustc-cfg=osslconf=" "$OPENSSL_SYS_OUTPUT" | sed 's/cargo:rustc-cfg=osslconf="\(.*\)"/\1/' | paste -sd ' ' || echo "none")

    # Quoting from openssl docs:
    # > A build script can be used to detect the OpenSSL or LibreSSL version at compile time if needed. The `openssl-sys`
    # > crate propagates the version via the `DEP_OPENSSL_VERSION_NUMBER` and `DEP_OPENSSL_LIBRESSL_VERSION_NUMBER`
    # > environment variables to build scripts. The version format is a hex-encoding of the OpenSSL release version:
    # > `0xMNNFFPPS`. For example, version 1.0.2g’s encoding is `0x1_00_02_07_0`.
    #
    # However, OpenSSL 3.x changed the displaying, while encoding is still the same.
    # - In version 1.2.3.c, 3 was Fix and c was Patch.
    # - In version 3.2.1, 3 is Patch and Fix is not present (0).
    # We're only interested in OpenSSL 3+, so we can assume the version is in the Major.Minor.Patch format.
    if [ -n "$OPENSSL_VERSION_HEX" ]; then
        dec=$((16#$OPENSSL_VERSION_HEX))
        major=$(( (dec >> 28) & 0xF ))
        minor=$(( (dec >> 20) & 0xFF ))
        patch=$(( (dec >> 4) & 0xFF ))
        OPENSSL_VERSION="${major}.${minor}.${patch}"
    else
        OPENSSL_VERSION="unknown"
    fi
else
    OPENSSL_VERSION=$(pkg-config --modversion openssl 2>/dev/null || echo "unknown")
    OPENSSL_INCLUDE=$(pkg-config --variable=includedir openssl 2>/dev/null || echo "unknown")
    OSSLCONF_FLAGS="unknown (openssl-sys build output not found)"
fi

# --- Build configuration (from Cargo.toml) ---

CARGO_TOML="$SOURCE_DIR/scylla-rust-wrapper/Cargo.toml"
if [ -f "$CARGO_TOML" ]; then
    LTO=$(grep "^lto" "$CARGO_TOML" | head -1 | sed 's/.*=[ ]*//' | tr -d '"' || echo "unknown")
    PANIC=$(grep "^panic" "$CARGO_TOML" | head -1 | sed 's/.*=[ ]*//' | tr -d '"' || echo "unknown")
else
    LTO="unknown"
    PANIC="unknown"
fi

# --- CMake options ---

CMAKE_BUILD_TYPE=$(cmake_cache_get "CMAKE_BUILD_TYPE")
CMAKE_INSTALL_PREFIX=$(cmake_cache_get "CMAKE_INSTALL_PREFIX")
CASS_BUILD_SHARED=$(cmake_cache_get "CASS_BUILD_SHARED")
CASS_BUILD_STATIC=$(cmake_cache_get "CASS_BUILD_STATIC")

# --- Write output ---

cat > "$OUTPUT" <<EOF
Build Information for scylla-cpp-driver $VERSION

Platform
--------
OS:             $OS
Architecture:   $ARCH
Kernel:         $KERNEL
glibc:          $GLIBC

Toolchain
---------
Rust:           $RUST_VERSION
Cargo:          $CARGO_VERSION
CMake:          $CMAKE_VERSION_STR
CC:             $CC_VERSION

OpenSSL (detected by openssl-sys at build time)
------------------------------------------------
Version:        $OPENSSL_VERSION
Include path:   $OPENSSL_INCLUDE
Disabled (osslconf): ${OSSLCONF_FLAGS:-none}

Build Configuration
-------------------
CMAKE_BUILD_TYPE:       $CMAKE_BUILD_TYPE
CMAKE_INSTALL_PREFIX:   $CMAKE_INSTALL_PREFIX
CASS_BUILD_SHARED:      $CASS_BUILD_SHARED
CASS_BUILD_STATIC:      $CASS_BUILD_STATIC
LTO:                    $LTO
Panic strategy:         $PANIC

Static Library Compatibility Notes
----------------------------------
The static archive (libscylladb_static.a) requires consumers to
provide OpenSSL >= 3.0 at link time. The minimum OpenSSL version
is determined by the build-time detection performed by the
openssl-sys crate.

Consumers linking on systems with a different OpenSSL configuration
(e.g. different osslconf flags) may encounter undefined symbol errors
for conditionally-compiled functions.
EOF

echo "Generated: $OUTPUT"
