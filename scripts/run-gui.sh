#!/usr/bin/env bash
# scripts/run-gui.sh
#
# Launches a GUI on the headless display, wrapped in a DBus session.
#
# The wrapper is what makes the file dialogs work. Both apps open files through `rfd`
# (src/bin/*/widgets.rs), and rfd 0.17 defaults to its xdg-portal backend; with no portal reachable
# the dialog fails *silently* -- clicking the button does nothing at all, with no error and no
# panic. Running through this script rather than a bare `cargo run` is what keeps that from being
# rediscovered as an application bug.
#
# Usage:
#   bash scripts/run-gui.sh                          # cargo run --bin ev_cost_recovery
#   bash scripts/run-gui.sh --bin ev_peak_gui        # the older app
#   bash scripts/run-gui.sh --release --bin ev_cost_recovery
set -e

if [ $# -eq 0 ]; then
    set -- --bin ev_cost_recovery
fi

exec dbus-run-session -- cargo run "$@"
