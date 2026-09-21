#!/usr/bin/env bash
# .devcontainer/start-wayland.sh
#
# Brings up the headless Wayland session the GUI renders onto: a session bus, a compositor, and
# the activation environment that lets the portal find the compositor.
#
# Wired to `postStartCommand` rather than `postCreateCommand` so it runs again after a plain
# container restart, not only after a rebuild -- otherwise the session quietly disappears the
# first time the container is stopped and started.
set -e

# Every daemon logs here rather than to the caller's stdout. A backgrounded child inherits
# postStartCommand's pipe, and holding that pipe open makes the devcontainer's startup step appear
# to hang long after this script has exited.
LOG=/tmp/wayland-startup.log

# Required, not cosmetic. The Wayland socket, sway's IPC socket and the session bus are all bound
# inside it, and Vulkan initialisation fails outright with "XDG_RUNTIME_DIR not set in the
# environment". The directory has to be private to the user to be accepted.
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

# The path inside the address, which is what has to appear on disk. The variable itself is set in
# devcontainer.json, where every shell in the container can see it.
BUS_SOCKET=${DBUS_SESSION_BUS_ADDRESS#unix:path=}

# Waits up to ten seconds for a path to appear. Each daemon below is started in the background and
# is not yet listening when the shell returns from launching it.
wait_for() {
    for _ in $(seq 1 100); do
        [ -e "$1" ] && return 0
        sleep 0.1
    done
    return 1
}

# The session bus. The portal is not a library the app links against but a service reached over
# this bus and activated from it on the first call, so with no bus there is no file dialog at all.
#
# Each guard below is in two parts, and both are needed. `pgrep` keeps a second run from starting
# a duplicate; the `rm` clears the socket a previous run left in /tmp, which survives a plain
# container restart while the daemon that owned it does not. No such daemon is running at this
# point, so nothing can be using the socket that is being removed.
if ! pgrep -x dbus-daemon >/dev/null; then
    rm -f "$BUS_SOCKET"
    setsid dbus-daemon --session --nofork --nopidfile \
        --address="$DBUS_SESSION_BUS_ADDRESS" >>"$LOG" 2>&1 &
fi
if ! wait_for "$BUS_SOCKET"; then
    echo "start-wayland.sh: the session bus did not come up; see $LOG" >&2
    exit 1
fi

# The compositor. `setsid` puts it in its own process group so it is not signalled when
# postStartCommand finishes.
#
# The two WLR_ variables belong to sway alone, which is why they are set on this line rather than
# in devcontainer.json with the variables the app reads: headless gives it a virtual output in
# place of a DRM device, and declaring no libinput devices stops it waiting for a seat that is
# never going to be granted in a container.
if ! pgrep -x sway >/dev/null; then
    rm -f "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY.lock" "$SWAYSOCK"
    WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 setsid sway >>"$LOG" 2>&1 &
fi
if ! wait_for "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY"; then
    echo "start-wayland.sh: $WAYLAND_DISPLAY did not come up; see $LOG" >&2
    exit 1
fi

# A D-Bus-activated service inherits the environment of the bus daemon, not of whoever called it.
# The portal would otherwise start with no display at all and answer a file dialog it has nowhere
# to put. These four are what it cannot work out for itself.
dbus-update-activation-environment \
    WAYLAND_DISPLAY XDG_RUNTIME_DIR XDG_CURRENT_DESKTOP XDG_SESSION_TYPE

# The Wayland socket exists from the moment the compositor binds it, which is before it has an
# output to place a window on. Asking it for the output is the first question whose answer means a
# client would have got somewhere -- and it is worth asking, because a session that is listening
# but has no screen accepts an app and shows nothing, which reads as an application bug.
if ! swaymsg -t get_outputs >/dev/null 2>&1; then
    echo "start-wayland.sh: sway is not answering on $SWAYSOCK; see $LOG" >&2
    exit 1
fi
