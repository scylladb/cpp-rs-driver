# Installation

## Packages

Pre-built packages are available for:
- Rocky Linux 9,
- Fedora 41 and 42,
- Ubuntu 22.04 LTS and 24.04 LTS.

### RHEL/Rocky

To install the dependencies:

```bash
sudo dnf -y install libuv openssl
```

Install the runtime library for your platform:

```bash
sudo dnf install -y ./scylla_cpp_driver_<VERSION>_x86_64.rpm
```

When developing against the driver you'll also want to install the development package:

```bash
sudo dnf install -y ./scylla_cpp_driver_<VERSION>_x86_64.rpm ./scylla_cpp_driver-devel_<VERSION>_x86_64.rpm
```

### Ubuntu/Debian

Ubuntu's apt-get will resolve and install the dependencies by itself:

```bash
sudo apt-get update
sudo apt-get install -y ./scylla_cpp_driver_<VERSION>_amd64.deb
```

When developing against the driver you'll also want to install the runtime and
development packages:

```bash
sudo apt-get install -y ./scylla_cpp_driver_<VERSION>_amd64.deb ./scylla_cpp_driver-dev_<VERSION>_amd64.deb
```

## Building

If pre-built packages are not available for your platform or architecture,
you will need to build the driver from source. Directions for building and
installing the ScyllaDB C/C++ Driver can be found [here](building.md).
