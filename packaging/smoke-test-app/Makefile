SHELL := bash
.ONESHELL:

MAKEFILE_PATH := $(abspath $(dir $(abspath $(lastword $(MAKEFILE_LIST)))))
BUILD_DIR ?= ${MAKEFILE_PATH}/build
CMAKE_FLAGS ?=
BUILD_TYPE ?= Release
CMAKE_GENERATOR ?=
INSTALL_PREFIX ?=
INSTALL_TARGET ?= /
DRIVER_BUILD_DIR ?= ${MAKEFILE_PATH}/../../build
DRIVER_INSTALL_PATH ?= C:/Program Files/ScyllaDB/Scylla CPP Driver
SMOKE_TEST_INSTALL_PATH ?= C:/Program Files/ScyllaDB/Scylla CPP Driver Smoke Test

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

ifeq ($(OS_TYPE),macos)
CPACK_GENERATORS ?= productbuild DragNDrop
else ifeq ($(OS_TYPE),windows)
CPACK_GENERATORS ?= WIX
else
CPACK_GENERATORS ?= DEB RPM
endif

define VALIDATE_SINGLE_PACKAGE
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
endef

# Use sudo only when not running as root (which is unnecessary e.g., inside containers that run as root)
define MAYBE_SUDO
	if [ "$$(id -u)" -eq 0 ]; then SUDO=""; else SUDO="sudo"; fi
endef

define INSTALL_PACKAGE_SCRIPT
	${VALIDATE_SINGLE_PACKAGE}
	${MAYBE_SUDO}
	if [[ "$$package" == *".deb" ]]; then
		$$SUDO dpkg -i "$$package";
	elif [[ "$$package" == *".pkg" ]]; then
		$$SUDO installer -pkg "$$package" -target $(INSTALL_TARGET)
	elif [[ "$$package" == *".rpm" ]]; then
		if command -v dnf >/dev/null 2>&1; then
			$$SUDO dnf -y install "$$package"
		elif command -v yum >/dev/null 2>&1; then
			$$SUDO yum -y install "$$package"
		elif command -v zypper >/dev/null 2>&1; then
			$$SUDO zypper --non-interactive install "$$package"
		else
			$$SUDO rpm -Uvh "$$package"
		fi
	else
		echo "$$package has unknown package type"
	fi
endef

define REMOVE_PACKAGE_SCRIPT
	${VALIDATE_SINGLE_PACKAGE}
	${MAYBE_SUDO}
	if [[ "$$package" == *".deb" ]]; then
		# For Debian/Ubuntu systems
		pkg_name=$$(dpkg-deb -f "$$package" Package)
		$$SUDO dpkg -r "$$pkg_name"
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
			$$SUDO dnf -y remove "$$pkg_name"
		elif command -v yum >/dev/null 2>&1; then
			$$SUDO yum -y remove "$$pkg_name"
		elif command -v zypper >/dev/null 2>&1; then
			$$SUDO zypper --non-interactive remove "$$pkg_name"
		else
			$$SUDO rpm -e "$$pkg_name"
		fi
	else
		echo "$$package has unknown package type"
	fi
endef

ifeq ($(OS_TYPE),windows)
build-package:
	@pwsh.exe -NoProfile -Command "\$$ErrorActionPreference = 'Stop'; \$$env:PKG_CONFIG_PATH = '$(DRIVER_INSTALL_PATH)/lib/pkgconfig'; cmake -S . -B '$(BUILD_DIR)' -G 'Visual Studio 17 2022' -A x64 -DCMAKE_BUILD_TYPE=$(BUILD_TYPE) $(CMAKE_FLAGS); cmake --build '$(BUILD_DIR)' --config $(BUILD_TYPE); Push-Location '$(BUILD_DIR)'; foreach (\$$gen in '$(CPACK_GENERATORS)'.Split(' ', [System.StringSplitOptions]::RemoveEmptyEntries)) { cpack -G \$$gen -C $(BUILD_TYPE) }; Pop-Location"
else
build-package:
	cmake -S . -B $(BUILD_DIR) $(CMAKE_GENERATOR_FLAG) -DCMAKE_BUILD_TYPE=$(BUILD_TYPE) $(CMAKE_FLAGS) $(CMAKE_INSTALL_PREFIX_FLAG)
	cmake --build $(BUILD_DIR) --config $(BUILD_TYPE)
	for generator in $(CPACK_GENERATORS); do \
		(cd $(BUILD_DIR) && cpack -G $$generator -C $(BUILD_TYPE));\
	done
endif

.prepare-docker-macos:
	@if ! docker --version 2>&1 1>/dev/null; then \
		brew install colima docker docker-compose;\
		if ! colima status >/dev/null 2>&1; then\
			colima start --cpu 4 --memory 6 --disk 20;\
		fi;\
		docker context use colima;\
	elif ! docker compose version >/dev/null 2>&1 && ! command -v docker-compose >/dev/null 2>&1; then \
		brew install docker-compose;\
  	fi

.prepare-docker-windows:
ifeq ($(OS_TYPE),windows)
	@pwsh -NoProfile -Command " \
		$$ErrorActionPreference = 'Stop'; \
		$$dockerService = Get-Service -Name docker -ErrorAction SilentlyContinue; \
		if (-not $$dockerService) { \
			Install-PackageProvider -Name NuGet -MinimumVersion 2.8.5.201 -Force; \
			Install-Module -Name DockerMsftProvider -Repository PSGallery -Force; \
			Install-Package -Name docker -ProviderName DockerMsftProvider -Force; \
		}; \
		try { \
			Start-Service docker -ErrorAction Stop; \
		} catch { \
			Write-Warning \"Docker service failed to start: $$($$_.Exception.Message)\"; \
		}"
endif

ifeq ($(OS_TYPE),macos)
.prepare-for-test: .prepare-docker-macos
else ifeq ($(OS_TYPE),windows)
.prepare-for-test: .prepare-docker-windows
else
.prepare-for-test:
endif

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

install-app-msi:
	@pwsh.exe -NoProfile -Command "\$$ErrorActionPreference = 'Stop'; \$$packages = Get-ChildItem '$(BUILD_DIR)' -Filter *.msi; if (-not \$$packages) { throw 'No smoke-test MSI packages produced' }; foreach (\$$pkg in \$$packages) { \$$process = Start-Process msiexec.exe -ArgumentList \"/i \`\"\$$(\$$pkg.FullName)\`\" /qn /norestart\" -Wait -PassThru; if (\$$process.ExitCode -ne 0) { throw \"Failed to install smoke-test package \$$(\$$pkg.Name): exit code \$$(\$$process.ExitCode)\" } }"

install-driver-dev-deb:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/scylla_cpp_driver-dev_*.deb)
	package_name="scylla-rs-driver dev DEB"
	${INSTALL_PACKAGE_SCRIPT}

install-driver-deb:
	@shopt -s nullglob;
	packages=(${MAKEFILE_PATH}/../../build/scylla_cpp_driver_*.deb)
	package_name="scylla-rs-driver DEB"
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

install-driver-dev-dmg:
	@shopt -s nullglob
	dmg_files=(${MAKEFILE_PATH}/../../build/scylla_cpp_driver_*_macos-scylla_cpp_driver-dev.dmg)
	if [ "$${#dmg_files[@]}" -eq 0 ]; then echo "No driver dev DMG found"; exit 1; fi
	dmg="$${dmg_files[0]}"
	mount_point=$$(mktemp -d)
	hdiutil attach "$$dmg" -mountpoint "$$mount_point" -nobrowse
	sudo mkdir -p /usr/local/include /usr/local/lib/pkgconfig
	sudo cp -R "$$mount_point"/include/* /usr/local/include/ 2>/dev/null || true
	sudo cp -R "$$mount_point"/lib/* /usr/local/lib/ 2>/dev/null || true
	hdiutil detach "$$mount_point"
	rm -rf "$$mount_point"

install-driver-dmg:
	@shopt -s nullglob
	dmg_files=(${MAKEFILE_PATH}/../../build/scylla_cpp_driver_*_macos-scylla_cpp_driver.dmg)
	if [ "$${#dmg_files[@]}" -eq 0 ]; then echo "No driver DMG found"; exit 1; fi
	dmg="$${dmg_files[0]}"
	mount_point=$$(mktemp -d)
	hdiutil attach "$$dmg" -mountpoint "$$mount_point" -nobrowse
	sudo mkdir -p /usr/local/lib
	sudo cp -R "$$mount_point"/lib/* /usr/local/lib/ 2>/dev/null || true
	hdiutil detach "$$mount_point"
	rm -rf "$$mount_point"

install-app-dmg:
	@shopt -s nullglob
	dmg_files=($(BUILD_DIR)/scylla-cpp-driver-smoke-app-*-macos.dmg)
	if [ "$${#dmg_files[@]}" -eq 0 ]; then echo "No smoke-app DMG found"; exit 1; fi
	dmg="$${dmg_files[0]}"
	mount_point=$$(mktemp -d)
	hdiutil attach "$$dmg" -mountpoint "$$mount_point" -nobrowse
	sudo mkdir -p /usr/local/bin
	sudo cp -R "$$mount_point"/bin/* /usr/local/bin/ 2>/dev/null || true
	hdiutil detach "$$mount_point"
	rm -rf "$$mount_point"

install-driver-msi:
	@pwsh.exe -NoProfile -Command "\$$ErrorActionPreference = 'Stop'; \$$packages = Get-ChildItem '$(DRIVER_BUILD_DIR)' -Filter *.msi; if (-not \$$packages) { throw 'No driver MSI packages produced' }; Write-Host \"Installing \$$(\$$packages.Count) MSI package(s):\"; foreach (\$$pkg in \$$packages) { Write-Host \"  - \$$(\$$pkg.Name)\" }; foreach (\$$pkg in \$$packages) { \$$process = Start-Process msiexec.exe -ArgumentList \"/i \`\"\$$(\$$pkg.FullName)\`\" /qn /norestart\" -Wait -PassThru; if (\$$process.ExitCode -ne 0) { throw \"Failed to install driver package \$$(\$$pkg.Name): exit code \$$(\$$process.ExitCode)\" } }"

install-driver-dev-msi: install-driver-msi

verify-driver-dev-msi:
	@pwsh.exe -NoProfile -Command "\$$ErrorActionPreference = 'Stop'; \$$installPath = '$(DRIVER_INSTALL_PATH)'; \$$headerPath = Join-Path \$$installPath 'include\cassandra.h'; if (-not (Test-Path \$$headerPath)) { throw 'ERROR: cassandra.h header not found at \$$headerPath - dev package may not be installed' }; \$$pkgConfigPath = Join-Path \$$installPath 'lib\pkgconfig\scylla-cpp-driver.pc'; if (-not (Test-Path \$$pkgConfigPath)) { throw 'ERROR: scylla-cpp-driver.pc not found at \$$pkgConfigPath - dev package may not be installed' }; Write-Host 'Dev package verification successful'"

ifeq ($(OS_TYPE),macos)
install-driver: install-driver-pkg
install-driver-dev: install-driver-dev-pkg
install-app: install-app-pkg

else ifeq ($(OS_TYPE),windows)
install-driver: install-driver-msi
install-driver-dev: install-driver-dev-msi verify-driver-dev-msi
install-app: install-app-msi

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
	packages=(${MAKEFILE_PATH}/../../build/scylla_cpp_driver-devel_*.rpm)
	package_name="scylla-rs-driver dev RPM"
	${REMOVE_PACKAGE_SCRIPT}

remove-driver-rpm:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/*-cpp_driver.rpm)
	package_name="scylla-rs-driver RPM"
	${REMOVE_PACKAGE_SCRIPT}

remove-driver-dev-pkg:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/scylla_cpp_driver-dev_*.pkg)
	package_name="scylla-rs-driver dev PKG"
	${REMOVE_PACKAGE_SCRIPT}

remove-driver-pkg:
	@shopt -s nullglob
	packages=(${MAKEFILE_PATH}/../../build/*-scylla_cpp_driver.pkg)
	package_name="scylla-rs-driver PKG"
	${REMOVE_PACKAGE_SCRIPT}

remove-driver-dev-dmg:
	sudo rm -f /usr/local/include/cassandra.h 2>/dev/null || true
	sudo rm -f /usr/local/lib/pkgconfig/scylla-cpp-driver.pc 2>/dev/null || true

remove-driver-dmg:
	sudo rm -f /usr/local/lib/libscylla-cpp-driver.* 2>/dev/null || true

remove-app-dmg:
	sudo rm -f /usr/local/bin/scylla-cpp-driver-smoke-test 2>/dev/null || true

remove-app-msi:
	@pwsh.exe -NoProfile -Command "\$$ErrorActionPreference = 'Stop'; \$$productName = 'Scylla CPP Driver Smoke Test'; \$$product = Get-CimInstance -ClassName Win32_Product | Where-Object { \$$_.Name -like \"*\$$productName*\" }; if (\$$product) { \$$product | ForEach-Object { \$$_.Uninstall() | Out-Null }; Write-Host \"Uninstalled \$$productName\" } else { Write-Host \"\$$productName not found - nothing to uninstall\" }"

remove-driver-msi:
	@pwsh.exe -NoProfile -Command "\$$ErrorActionPreference = 'Stop'; \$$productName = 'Scylla CPP Driver'; \$$product = Get-CimInstance -ClassName Win32_Product | Where-Object { \$$_.Name -like \"*\$$productName*\" -and \$$_.Name -notlike '*Smoke*' }; if (\$$product) { \$$product | ForEach-Object { \$$_.Uninstall() | Out-Null }; Write-Host \"Uninstalled \$$productName\" } else { Write-Host \"\$$productName not found - nothing to uninstall\" }"

remove-driver-dev-msi: remove-driver-msi

ifeq ($(OS_TYPE),macos)
remove-driver: remove-driver-pkg
remove-driver-dev: remove-driver-dev-pkg
remove-app: remove-app-pkg

else ifeq ($(OS_TYPE),windows)
remove-driver: remove-driver-msi
remove-driver-dev: remove-driver-dev-msi
remove-app: remove-app-msi

else ifeq ($(shell command -v dpkg >/dev/null 2>&1 && echo yes),yes)
remove-driver: remove-driver-deb
remove-driver-dev: remove-driver-dev-deb
remove-app: remove-app-deb
else
remove-driver: remove-driver-rpm
remove-driver-dev: remove-driver-dev-rpm
remove-app: remove-app-rpm
endif

# SCYLLA_HOST: Host to connect to for smoke tests (default: 127.0.0.1)
# SKIP_DOCKER_COMPOSE: Set to 1 to skip docker-compose (use with external ScyllaDB)
SCYLLA_HOST ?= 127.0.0.1
SKIP_DOCKER_COMPOSE ?=

ifeq ($(OS_TYPE),windows)
test-app-package: .prepare-for-test
	@pwsh.exe -NoProfile -Command "\$$ErrorActionPreference = 'Stop'; \$$composeFile = '${MAKEFILE_PATH}/../../tests/examples_cluster/docker-compose.yml'; function Cleanup { docker compose -f \$$composeFile down --remove-orphans 2>\$$null | Out-Null }; try { \$$dockerService = Get-Service -Name docker -ErrorAction SilentlyContinue; if (\$$dockerService -and \$$dockerService.Status -ne 'Running') { Start-Service docker }; docker compose -f \$$composeFile up -d --wait; \$$smokePath = '$(SMOKE_TEST_INSTALL_PATH)\bin\scylla-cpp-driver-smoke-test.exe'; if (-not (Test-Path \$$smokePath)) { throw 'Smoke-test binary not found at \$$smokePath' }; & \$$smokePath 172.43.0.2 } finally { Cleanup }"
else
test-app-package: .prepare-for-test
	@smoke_bin="$$(command -v scylla-cpp-driver-smoke-test || true)";\
	if [ -z "$$smoke_bin" ] && [ "$(OS_TYPE)" = "macos" ] && command -v pkgutil >/dev/null 2>&1; then\
		pkg_id="com.scylladb.cpp-rs-driver.smoke-app";\
		pkg_ids="$$pkg_id $$(pkgutil --pkgs 2>/dev/null | awk '/scylla/ && /smoke/ {print}')";\
		for candidate in $$pkg_ids; do\
			pkg_path="$$(pkgutil --files "$$candidate" 2>/dev/null | awk '/scylla-cpp-driver-smoke-test$$/ {print "/"$$0; exit}')";\
			if [ -n "$$pkg_path" ] && [ -x "$$pkg_path" ]; then\
				smoke_bin="$$pkg_path";\
				break;\
			fi;\
		done;\
	fi;\
	if [ -z "$$smoke_bin" ] && [ "$(OS_TYPE)" = "macos" ]; then\
		for candidate in /usr/local/bin/scylla-cpp-driver-smoke-test /opt/homebrew/bin/scylla-cpp-driver-smoke-test /usr/bin/scylla-cpp-driver-smoke-test /usr/local/sbin/scylla-cpp-driver-smoke-test /opt/homebrew/sbin/scylla-cpp-driver-smoke-test; do\
			if [ -x "$$candidate" ]; then\
				smoke_bin="$$candidate";\
				break;\
			fi;\
		done;\
	fi;\
	if [ -z "$$smoke_bin" ]; then\
		echo "scylla-cpp-driver-smoke-test not found in PATH";\
		exit 1;\
	fi;\
	if [ -n "$(SKIP_DOCKER_COMPOSE)" ]; then\
		"$$smoke_bin" $(SCYLLA_HOST);\
	else\
		dc="";\
		if docker compose version >/dev/null 2>&1; then\
			dc="docker compose";\
		elif command -v docker-compose >/dev/null 2>&1; then\
			dc="docker-compose";\
		else\
			echo "docker compose is not available";\
			exit 1;\
		fi;\
		cleanup() {\
			if [ -n "$$dc" ]; then\
				sudo $$dc -f ${MAKEFILE_PATH}/docker-compose.yml down --remove-orphans;\
			fi;\
		};\
		trap cleanup EXIT;\
		sudo $$dc -f ${MAKEFILE_PATH}/docker-compose.yml up -d --wait;\
		"$$smoke_bin" $(SCYLLA_HOST);\
	fi
endif
