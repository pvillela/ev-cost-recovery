#!/usr/bin/env bash
# scripts/run-gui.sh
#
# Launches the desktop app wrapped in an explicit DBus session.
#
# A fallback, not the usual way in. The app opens files through `rfd`
# (src/bin/ev_cost_recovery/widgets.rs), and rfd 0.17 defaults to its xdg-portal backend, which
# needs a session bus. With the portal packages this devcontainer installs, the GTK stack starts one
# itself when none is set and it exits with the app, so a bare `cargo run --bin ev_cost_recovery`
# works and this script is unnecessary. Verified by killing every dbus-daemon and running the binary
# directly: the chooser opened.
#
# What it is for is an environment without those packages, where the dialog fails *silently* --
# clicking a picker does nothing at all, with no error and no panic, which is easy to misdiagnose as
# an application bug. See docs/session/Devcontainer_GUI_Options.md.
#
# Any arguments given are passed to `cargo run` instead of the default.
#
# Usage:
#   bash scripts/run-gui.sh                          # cargo run --bin ev_cost_recovery
#   bash scripts/run-gui.sh --release --bin ev_cost_recovery
set -e

if [ $# -eq 0 ]; then
    set -- --bin ev_cost_recovery
fi

# Said out loud, not just left in the header. Someone reaching for this script has usually been told
# to, and the thing worth knowing is that they probably do not need it.
cat >&2 <<'NOTE'
run-gui.sh: launching under an explicit DBus session.

  This is a fallback. Where a desktop portal is installed -- as it is in this devcontainer -- the
  GTK stack starts a session bus by itself and `cargo run --bin ev_cost_recovery` works on its own.

  Use this script where no portal is reachable. There, the file pickers fail silently: clicking
  "Choose..." does nothing at all, with no error and no panic.

  See docs/session/Devcontainer_GUI_Options.md.

NOTE

exec dbus-run-session -- cargo run "$@"
