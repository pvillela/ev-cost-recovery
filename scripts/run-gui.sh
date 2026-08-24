#!/usr/bin/env bash
# scripts/run-gui.sh
#
# Launches the desktop app on the headless display, wrapped in a DBus session.
#
# The wrapper is what makes the file dialogs work. The app opens files through `rfd`
# (src/bin/ev_cost_recovery/widgets.rs), and rfd 0.17 defaults to its xdg-portal backend; with no
# portal reachable the dialog fails *silently* -- clicking a picker does nothing at all, with no
# error and no panic. Running through this script rather than a bare `cargo run` is what keeps that
# from being rediscovered as an application bug.
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

exec dbus-run-session -- cargo run "$@"
