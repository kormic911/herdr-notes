#!/bin/sh
# Install the Notes shortcut in the Herdr configuration.
case "$0" in
  */*) dir=${0%/*} ;;
  *) dir=. ;;
esac
exec "$dir/../target/release/herdr-notes" --install-keybindings
