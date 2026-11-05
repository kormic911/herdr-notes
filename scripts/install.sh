#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "${HERDR_NOTES_BUILD_FROM_SOURCE:-}" == 1 ]]; then
  cd "$root"
  exec cargo build --release --locked
fi
version="$(sed -n 's/^version = "\([0-9]*\.[0-9]*\.[0-9]*\)"\r\{0,1\}$/\1/p' "$root/herdr-plugin.toml")"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Cannot determine release version.' >&2; exit 1; }
case "$(uname -s)/$(uname -m)" in
  Linux/x86_64) target=x86_64-unknown-linux-musl ;;
  Linux/aarch64|Linux/arm64) target=aarch64-unknown-linux-musl ;;
  Darwin/x86_64) target=x86_64-apple-darwin ;;
  Darwin/arm64) target=aarch64-apple-darwin ;;
  *) echo 'Unsupported platform. Set HERDR_NOTES_BUILD_FROM_SOURCE=1 to build with Rust.' >&2; exit 1 ;;
esac
asset="herdr-notes-v$version-$target"
base="https://github.com/kormic911/herdr-notes/releases/download/v$version"
destination="$root/target/release"
mkdir -p "$destination"
temp="$(mktemp -d "$destination/.download-XXXXXXXX")"
trap 'rm -rf -- "$temp"' EXIT
curl --fail --location --retry 3 --proto '=https' --tlsv1.2 "$base/$asset" -o "$temp/binary"
curl --fail --location --retry 3 --proto '=https' --tlsv1.2 "$base/$asset.sha256" -o "$temp/checksum"
expected="$(tr -d '\r\n' < "$temp/checksum")"
[[ "$expected" =~ ^[a-fA-F0-9]{64}$ ]] || { echo 'Invalid release checksum.' >&2; exit 1; }
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$temp/binary" | cut -d ' ' -f 1)"
else
  actual="$(shasum -a 256 "$temp/binary" | cut -d ' ' -f 1)"
fi
[[ "$actual" == "$expected" ]] || { echo 'Release checksum mismatch. Binary was not installed.' >&2; exit 1; }
chmod 755 "$temp/binary"
mv -f "$temp/binary" "$destination/herdr-notes"
echo "Installed herdr-notes v$version ($target); Rust is not required."
