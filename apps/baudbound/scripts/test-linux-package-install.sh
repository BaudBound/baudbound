#!/usr/bin/env bash

set -euo pipefail

format="${1:-}"
package_directory="${2:-}"
expected_version="${3:-}"
package_name="baud-bound"
user_data_directory="${HOME}/.local/share/BaudBound/runner"
user_data_marker="${user_data_directory}/package-preservation-test"

fail() {
    printf 'Linux package installation test: %s\n' "$1" >&2
    exit 1
}

[[ -d "$package_directory" ]] || fail "package directory does not exist"
[[ "$expected_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]] \
    || fail "expected version is invalid"

find_package() {
    local suffix="$1"
    mapfile -t matches < <(find "$package_directory" -maxdepth 1 -type f -name "*$suffix" -print)
    [[ "${#matches[@]}" -eq 1 ]] \
        || fail "expected exactly one $suffix package, found ${#matches[@]}"
    printf '%s' "${matches[0]}"
}

verify_installed_files() {
    [[ -x /usr/bin/baudbound ]] || fail "installed baudbound command is missing"
    [[ -f /usr/share/applications/BaudBound.desktop ]] \
        || fail "installed desktop entry is missing"
    [[ -f /usr/share/icons/hicolor/128x128/apps/baudbound.png ]] \
        || fail "installed application icon is missing"
    version_output="$(baudbound --version)"
    [[ "$version_output" == *"$expected_version"* ]] \
        || fail "installed command reported an unexpected version: $version_output"
}

verify_removed_files() {
    [[ ! -e /usr/bin/baudbound ]] || fail "package removal left /usr/bin/baudbound behind"
    [[ ! -e /usr/share/applications/BaudBound.desktop ]] \
        || fail "package removal left the desktop entry behind"
    [[ -f "$user_data_marker" ]] \
        || fail "package removal deleted runner user data"
}

mkdir -p "$user_data_directory"
printf 'preserve runner data\n' > "$user_data_marker"

case "$format" in
    deb)
        package_path="$(find_package .deb)"
        apt-get update
        apt-get install --yes "$package_path"
        installed_version="$(dpkg-query --show --showformat='${Version}' "$package_name")"
        [[ "$installed_version" == "$expected_version" ]] \
            || fail "Debian package database reported version $installed_version"
        verify_installed_files
        apt-get remove --yes "$package_name"
        package_status="$(
            dpkg-query --show --showformat='${db:Status-Abbrev}' "$package_name" 2>/dev/null || true
        )"
        [[ "$package_status" != ii* ]] || fail "Debian package remains installed after removal"
        verify_removed_files
        ;;
    rpm)
        package_path="$(find_package .rpm)"
        dnf install --assumeyes "$package_path"
        installed_version="$(rpm -q --queryformat '%{VERSION}' "$package_name")"
        [[ "$installed_version" == "$expected_version" ]] \
            || fail "RPM package database reported version $installed_version"
        verify_installed_files
        dnf remove --assumeyes "$package_name"
        rpm -q "$package_name" >/dev/null 2>&1 \
            && fail "RPM package remains installed after removal"
        verify_removed_files
        ;;
    *) fail "format must be deb or rpm" ;;
esac

printf 'Linux %s package installation and removal test passed.\n' "$format"
