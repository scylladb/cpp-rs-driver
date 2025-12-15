SHELL := /bin/bash
.ONESHELL:

MAKEFILE_PATH := $(abspath $(dir $(abspath $(lastword $(MAKEFILE_LIST)))))
BUILD_DIR ?= ${MAKEFILE_PATH}/build
CMAKE_FLAGS ?=
BUILD_TYPE ?= Release
CMAKE_GENERATOR ?=
INSTALL_PREFIX ?=
INSTALL_TARGET ?= /

CMAKE_GENERATOR_FLAG :=
ifneq ($(strip $(CMAKE_GENERATOR)),)
CMAKE_GENERATOR_FLAG := -G $(CMAKE_GENERATOR)
endif

CMAKE_INSTALL_PREFIX_FLAG :=
ifneq ($(strip $(INSTALL_PREFIX)),)
CMAKE_INSTALL_PREFIX_FLAG := -DCMAKE_INSTALL_PREFIX=$(INSTALL_PREFIX)
endif

UNAME_S := $(shell uname -s)
ifeq ($(OS),Windows_NT)
    OS_TYPE = windows
else ifeq ($(UNAME_S),Darwin)
	OS_TYPE = macos
else
	OS_TYPE = linux
endif

.prepare-docker-macos:
	@if ! docker --version 2>&1 1>/dev/null; then \
		brew install colima docker;\
		colima start;\
		docker context use colima;\
  	fi

ifeq ($(OS_TYPE),macos)
CPACK_GENERATORS ?= productbuild DragNDrop

.prepare-for-test: .prepare-docker-macos
else ifeq ($(OS_TYPE),windows)
CPACK_GENERATORS ?= WIX

.prepare-for-test:
else
CPACK_GENERATORS ?= DEB RPM

.prepare-for-test:
endif

define INSTALL_PACKAGE_SCRIPT
	set -euo pipefail
	if [ "$${#packages[@]}" -eq 0 ]; then
		echo "No ${package_name} package found"
		exit 1
	fi
	if [ "$${#packages[@]}" -gt 1 ]; then
		echo "Found more than one file for ${package_name}: $${packages[@]}"
		exit 1
	fi
	package=$${packages[0]}
	if [[ "$$package" == *".deb" ]]; then
		sudo dpkg -i "$$package";
	elif [[ "$$package" == *".pkg" ]]; then
		sudo installer -pkg "$$pkg" -target $(INSTALL_TARGET)
	elif [[ "$$package" == *".rpm" ]]; then
		if command -v dnf >/dev/null 2>&1; then
			sudo dnf -y install "$$package"
		elif command -v yum >/dev/null 2>&1; then
			sudo yum -y install "$$package"
		elif command -v zypper >/dev/null 2>&1; then
			sudo zypper --non-interactive install "$$package"
		else
			sudo rpm -Uvh "$$package"
		fi
	else
		echo "$$package has unknown package type"
	fi
endef

define REMOVE_PACKAGE_SCRIPT
	set -euo pipefail
	if [ "$${#packages[@]}" -eq 0 ]; then
		echo "No ${package_name} package found"
		exit 1
	fi
	if [ "$${#packages[@]}" -gt 1 ]; then
		echo "Found more than one file for ${package_name}: $${packages[@]}"
		exit 1
	fi
	package=$${packages[0]}
	if [[ "$$package" == *".deb" ]]; then
		# For Debian/Ubuntu systems
		pkg_name=$$(dpkg-deb -f "$$package" Package)
		sudo dpkg -r "$$pkg_name"
	elif [[ "$$package" == *".pkg" ]]; then
		# macOS packages don't have standard uninstallers
		echo "Uninstalling macOS .pkg is not standardized; manual cleanup may be required"
	elif [[ "$$package" == *".rpm" ]]; then
		# For RHEL/Fedora/SUSE systems
		pkg_name=$$(rpm -qp --queryformat '%{NAME}' "$$package" 2>/dev/null || true)
		if [ -z "$$pkg_name" ]; then
			echo "Could not determine RPM package name for $$package"
			exit 1
		fi
		if command -v dnf >/dev/null 2>&1; then
			sudo dnf -y remove "$$pkg_name"
		elif command -v yum >/dev/null 2>&1; then
			sudo yum -y remove "$$pkg_name"
		elif command -v zypper >/dev/null 2>&1; then
			sudo zypper --non-interactive remove "$$pkg_name"
		else
			sudo rpm -e "$$pkg_name"
		fi
	else
		echo "$$package has unknown package type"
	fi
endef

build-package:
	cmake -S . -B $(BUILD_DIR) $(CMAKE_GENERATOR_FLAG) -DCMAKE_BUILD_TYPE=$(BUILD_TYPE) $(CMAKE_FLAGS) $(CMAKE_INSTALL_PREFIX_FLAG)
	cmake --build $(BUILD_DIR) --config $(BUILD_TYPE)
	for generator in $(CPACK_GENERATORS); do \
		(cd $(BUILD_DIR) && cpack -G $$generator -C $(BUILD_TYPE));\
	done

install-app-pkg:
	@shopt -s nullglob
	packages=($(BUILD_DIR)/scylla-cpp-driver-smoke-app-*.pkg)
	package_name="some-test-app PKG"
	${INSTALL_PACKAGE_SCRIPT}

install-app-deb:
	@shopt -s nullglob
	packages=($(BUILD_DIR)/scylla-cpp-driver-smoke-app*.deb)
	package_name="some-test-app DEB"
	${INSTALL_PACKAGE_SCRIPT}

install-app-rpm:
	@shopt -s nullglob
	packages=($(BUILD_DIR)/scylla-cpp-driver-smoke-app*.rpm)
	package_name="some-test-app RPM"
	${INSTALL_PACKAGE_SCRIPT}

install-driver-dev-deb:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/scylla_cpp_driver-dev_*.deb)
	package_name="scylla-rs-driver dev DEB"
	${INSTALL_PACKAGE_SCRIPT}

install-driver-deb:
	@shopt -s nullglob;
	packages=(${MAKEFILE_PATH}/../../build/scylla_cpp_driver_*.deb)
	package_name="scylla-rs-driver dev DEB"
	${INSTALL_PACKAGE_SCRIPT}

install-driver-dev-rpm:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/scylla_cpp_driver-devel_*.rpm)
	package_name="scylla-rs-driver dev RPM"
	${INSTALL_PACKAGE_SCRIPT}

install-driver-rpm:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/scylla_cpp_driver_*.rpm)
	package_name="scylla-rs-driver RPM"
	${INSTALL_PACKAGE_SCRIPT}

install-driver-dev-pkg:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/scylla_cpp_driver-dev_*_macos.pkg)
	package_name="scylla-rs-driver dev PKG"
	${INSTALL_PACKAGE_SCRIPT}

install-driver-pkg:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/scylla_cpp_driver_*_macos.pkg)
	package_name="scylla-rs-driver PKG"
	${INSTALL_PACKAGE_SCRIPT}

ifeq ($(OS_TYPE),macos)
install-driver: install-driver-pkg
install-driver-dev: install-driver-dev-pkg
install-app: install-app-pkg

else ifeq ($(OS_TYPE),windows)
install-driver:
	echo "NOT IMPLEMENTED"
	exit 1
install-driver-dev:
	echo "NOT IMPLEMENTED"
	exit 1
install-app:
	echo "NOT IMPLEMENTED"
	exit 1

else ifeq ($(shell command -v dpkg >/dev/null 2>&1 && echo yes),yes)
install-driver: install-driver-deb
install-driver-dev: install-driver-dev-deb
install-app: install-app-deb
else
install-driver: install-driver-rpm
install-driver-dev: install-driver-dev-rpm
install-app: install-app-rpm
endif

remove-app-pkg:
	@shopt -s nullglob
	packages=($(BUILD_DIR)/scylla-cpp-driver-smoke-app-*.pkg)
	package_name="some-test-app PKG"
	${REMOVE_PACKAGE_SCRIPT}

remove-app-deb:
	@shopt -s nullglob
	packages=($(BUILD_DIR)/scylla-cpp-driver-smoke-app*.deb)
	package_name="some-test-app DEB"
	${REMOVE_PACKAGE_SCRIPT}

remove-app-rpm:
	@shopt -s nullglob
	packages=($(BUILD_DIR)/scylla-cpp-driver-smoke-app*.rpm)
	package_name="some-test-app RPM"
	${REMOVE_PACKAGE_SCRIPT}

remove-driver-dev-deb:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/*-dev*.deb)
	package_name="scylla-rs-driver dev DEB"
	${REMOVE_PACKAGE_SCRIPT}

remove-driver-deb:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/*scylla_cpp_driver_*.deb)
	package_name="scylla-rs-driver DEB"
	${REMOVE_PACKAGE_SCRIPT}

remove-driver-dev-rpm:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/*-cpp_driver-dev.rpm)
	package_name="scylla-rs-driver dev RPM"
	${REMOVE_PACKAGE_SCRIPT}

remove-driver-rpm:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/*-cpp_driver.rpm)
	package_name="scylla-rs-driver RPM"
	${REMOVE_PACKAGE_SCRIPT}

remove-driver-dev-pkg:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/*-scylla_cpp_driver-dev.pkg)
	package_name="scylla-rs-driver dev PKG"
	${REMOVE_PACKAGE_SCRIPT}

remove-driver-pkg:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/*-scylla_cpp_driver.pkg)
	package_name="scylla-rs-driver PKG"
	${REMOVE_PACKAGE_SCRIPT}

ifeq ($(OS_TYPE),macos)
remove-driver: remove-driver-pkg
remove-driver-dev: remove-driver-dev-pkg
remove-app: remove-app-pkg

else ifeq ($(OS_TYPE),windows)
remove-driver:
	echo "NOT IMPLEMENTED"
	exit 1
remove-driver-dev:
	echo "NOT IMPLEMENTED"
	exit 1
remove-app:
	echo "NOT IMPLEMENTED"
	exit 1

else ifeq ($(shell command -v dpkg >/dev/null 2>&1 && echo yes),yes)
remove-driver: remove-driver-deb
remove-driver-dev: remove-driver-dev-deb
remove-app: remove-app-deb
else
remove-driver: remove-driver-rpm
remove-driver-dev: remove-driver-dev-rpm
remove-app: remove-app-rpm
endif

test-app-package: .prepare-for-test
	@cleanup() {\
		sudo docker compose -f ${MAKEFILE_PATH}/docker-compose.yml down --remove-orphans;\
	};\
	trap cleanup EXIT;\
	sudo docker compose -f ${MAKEFILE_PATH}/docker-compose.yml up -d --wait;\
	/usr/bin/scylla-cpp-driver-smoke-test 172.44.0.2
