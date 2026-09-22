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

# Waits up to ten seconds for a command to succeed. Each daemon below is started in the background,
# and is neither listening nor answering yet when the shell returns from launching it.
wait_for() {
    for _ in $(seq 1 100); do
        "$@" >/dev/null 2>&1 && return 0
        sleep 0.1
    done
    return 1
}

# Reports a failure where it can still be read afterwards, then stops. What postStartCommand prints
# is shown once, on the console that started the container, and cannot be read from inside it.
fail() {
    echo "start-wayland.sh: $1" | tee -a "$LOG" >&2
    exit 1
}

# `set -e` stops the script at the first failing command and says nothing about it; this names the
# line instead. Everything after a stop is skipped, the keyboard below included, while the daemons
# already started keep running -- so a session can look complete and not be.
trap 'fail "stopped at line $LINENO, status $?; see $LOG"' ERR

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
wait_for test -e "$BUS_SOCKET" || fail "the session bus did not come up; see $LOG"

# The compositor. `setsid` puts it in its own process group so it is not signalled when
# postStartCommand finishes.
#
# The WLR_ variables belong to sway alone, which is why they are set on this line rather than in
# devcontainer.json with the variables the app reads. Headless gives it a virtual output in place
# of a DRM device; declaring no libinput devices stops it waiting for a seat that is never going
# to be granted in a container; and pixman is the software renderer. Without that last one,
# wlroots looks for a GPU, logs `drmGetDevices2 failed` twice, and then falls back to pixman
# anyway -- two lines that read as the reason for any failure that follows, and never are.
#
# The configuration is read from the repository, beside this script, rather than from a copy in the
# image: an edit to it then takes effect at the next container start, with no rebuild to forget.
if ! pgrep -x sway >/dev/null; then
    rm -f "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY.lock" "$SWAYSOCK"
    WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman \
        setsid sway -c "$(dirname "$(readlink -f "$0")")/sway-config" >>"$LOG" 2>&1 &
fi
wait_for test -e "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ||
    fail "$WAYLAND_DISPLAY did not come up; see $LOG"

# A D-Bus-activated service inherits the environment of the bus daemon, not of whoever called it.
# The portal would otherwise start with no display at all and answer a file dialog it has nowhere
# to put. These four are what it cannot work out for itself.
dbus-update-activation-environment \
    WAYLAND_DISPLAY XDG_RUNTIME_DIR XDG_CURRENT_DESKTOP XDG_SESSION_TYPE

# The Wayland socket exists from the moment the compositor binds it, which is before it has an
# output to place a window on, and before its IPC socket answers. Asking it for the output is the
# first question whose answer means a client would have got somewhere -- and it is worth waiting
# for, because a session that is listening but has no screen accepts an app and shows nothing,
# which reads as an application bug.
wait_for swaymsg -t get_outputs || fail "sway is not answering on $SWAYSOCK; see $LOG"

# The keyboard sway starts from its configuration (see sway-config). An app launched on a seat with
# no keyboard never binds one and ignores every key, so a seat that ends up without it is a failure
# here, where it can be named, rather than later, where it looks like the app.
wait_for sh -c 'swaymsg -t get_inputs | grep -q "\"type\": \"keyboard\""' ||
    fail "sway started no keyboard on its seat; see $LOG"
