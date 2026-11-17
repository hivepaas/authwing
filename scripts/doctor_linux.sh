#!/bin/bash
#
# Which of the libraries a Linux build needs are here, and which are missing
# (see docs/platforms.md, "Toolchain"). Run by `make doctor`.
#
#   scripts/doctor_linux.sh
#
# Each library is asked for by its pkg-config name, as the crates that link it
# do; PAM has no pkg-config file, so its header is looked for instead. What is
# missing is named at the end as the apt-get line that installs it.
#
# Exit status: 0 when everything is here, 1 when something is missing.

set -uo pipefail

# pkg-config name:Debian package. The crate that wants each one is in
# docs/platforms.md.
LIBRARIES=(
	"fontconfig:libfontconfig-dev"
	"freetype2:libfreetype-dev"
	"wayland-client:libwayland-dev"
	"xkbcommon:libxkbcommon-dev"
	"xkbcommon-x11:libxkbcommon-x11-dev"
	"x11-xcb:libx11-xcb-dev"
	"xcb:libxcb1-dev"
	"xcb-xkb:libxcb-xkb-dev"
	"xcb-shape:libxcb-shape0-dev"
	"xcb-xfixes:libxcb-xfixes0-dev"
	"vulkan:libvulkan-dev"
	"dbus-1:libdbus-1-dev"
	"libzstd:libzstd-dev"
	"alsa:libasound2-dev"
)

missing=()

say() { printf "  %-16s %s\n" "$1" "$2"; }

# A compiler and pkg-config itself, before anything is asked of them.
if command -v cc >/dev/null 2>&1; then
	say cc "$(cc --version | head -1)"
else
	say cc "missing"
	missing+=("build-essential")
fi

if command -v pkg-config >/dev/null 2>&1; then
	say pkg-config "$(pkg-config --version)"
else
	say pkg-config "missing: the libraries below cannot be looked for without it"
	missing+=("pkg-config")
	printf "\n  On Debian or Ubuntu: sudo apt-get install %s\n" "${missing[*]}"
	exit 1
fi

for entry in "${LIBRARIES[@]}"; do
	name=${entry%%:*}
	package=${entry#*:}
	if version=$(pkg-config --modversion "$name" 2>/dev/null); then
		say "$name" "$version"
	else
		say "$name" "missing"
		missing+=("$package")
	fi
done

# PAM ships no pkg-config file: its header says whether it is here.
if [ -e /usr/include/security/pam_appl.h ]; then
	say pam "/usr/include/security/pam_appl.h"
else
	say pam "missing"
	missing+=("libpam0g-dev")
fi

if [ ${#missing[@]} -eq 0 ]; then
	exit 0
fi

printf "\n  On Debian or Ubuntu: sudo apt-get install %s\n" "${missing[*]}"
printf "  Another distribution: the same libraries under its own names.\n"
exit 1
