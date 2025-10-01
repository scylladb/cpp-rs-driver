SHELL := /bin/bash

BUILD_DIR ?= build
BUILD_TYPE ?= Release
CMAKE_GENERATOR ?=
INSTALL_PREFIX ?=
CPACK_GENERATORS ?= productbuild DragNDrop
INSTALL_TARGET ?= /

CMAKE_GENERATOR_FLAG :=
ifneq ($(strip $(CMAKE_GENERATOR)),)
CMAKE_GENERATOR_FLAG := -G $(CMAKE_GENERATOR)
endif

CMAKE_INSTALL_PREFIX_FLAG :=
ifneq ($(strip $(INSTALL_PREFIX)),)
CMAKE_INSTALL_PREFIX_FLAG := -DCMAKE_INSTALL_PREFIX=$(INSTALL_PREFIX)
endif

.PHONY: package
package:
	cmake -S . -B $(BUILD_DIR) $(CMAKE_GENERATOR_FLAG) -DCMAKE_BUILD_TYPE=$(BUILD_TYPE) $(CMAKE_INSTALL_PREFIX_FLAG)
	cmake --build $(BUILD_DIR) --config $(BUILD_TYPE)
	for generator in $(CPACK_GENERATORS); do \
		(cd $(BUILD_DIR) && cpack -G $$generator -C $(BUILD_TYPE)); \
	done

.PHONY: install-pkg
install-pkg:
	set -euo pipefail; \
	shopt -s nullglob; \
	packages=($(BUILD_DIR)/scylla-cpp-driver-smoke-app-*-macos.pkg); \
	if [ "$${#packages[@]}" -eq 0 ]; then \
		echo "No smoke-test productbuild packages produced"; \
		exit 1; \
	fi; \
	for pkg in "$${packages[@]}"; do \
		sudo installer -pkg "$$pkg" -target $(INSTALL_TARGET); \
	done

.PHONY: install-deb
install-deb:
	set -euo pipefail; \
	shopt -s nullglob; \
	packages=($(BUILD_DIR)/*.deb); \
	if [ "$${#packages[@]}" -eq 0 ]; then \
		echo "No smoke-test DEB packages produced"; \
		exit 1; \
	fi; \
	sudo dpkg -i "$${packages[@]}"; \
	sudo apt-get install -f -y
