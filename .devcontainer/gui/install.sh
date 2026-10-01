#!/usr/bin/env bash
# .devcontainer/gui/install.sh -- the `./gui` devcontainer feature. Runs as root during the image
# build.
set -e

####################################################################################################
# WARNING: ANY EDIT TO ANY FILE IN THIS DIRECTORY REBUILDS THE GUI LAYER -- EVEN AN EDIT TO A
# COMMENT.
#
# Docker keys this layer on the content of every file in .devcontainer/gui/, comments included. One
# changed character in this script or in devcontainer-feature.json downloads a few hundred MB of
# packages again at the next container build.
#
# Keep documentation that is likely to change outside this directory: see
# .devcontainer/gui-feature.md.
####################################################################################################

# The Wayland session the eframe/egui app renders onto, and the tooling to drive and observe it
# headlessly.
#
# This is a feature rather than a step of setup.sh so it becomes a cached image layer. Docker keys
# its build cache on the instructions and base image, not on the workspace path, whereas the
# devcontainer's own identity is a hash of that path -- so moving the project rebuilds the
# container but reuses this layer, instead of re-downloading a few hundred MB over the network.
#
# `installsAfter` in devcontainer-feature.json makes it the last layer of the image. Every layer
# below it is free of GUI packages, so a devcontainer with no GUI that names the same base image
# and the same toolchain features reuses those layers from the build cache.
#
# `--no-install-recommends` is deliberately NOT used: the portal packages below pull working
# defaults through recommends, and trimming them is exactly the kind of change that turns into a
# silently broken file dialog.
#
# The set is chosen so that the app takes the same path here as it takes on a Plasma desktop:
#
# * `sway` is the display server, a Wayland compositor and not an X server, because Wayland is
#   what the app meets on the desktop. `grim` takes the screenshots and `wtype` types the keys;
#   both speak wlroots protocols, which is what settles sway against `kwin_wayland`. KWin
#   implements neither, and would want PipeWire screen-cast for a screenshot and `/dev/uinput`
#   -- a privileged container -- for a keystroke.
# * `xdg-desktop-portal` with the `kde` backend answers the file dialog, out of process, with the
#   Qt dialog a Plasma desktop puts up. `dbus-daemon` is not optional beside it: the portal is
#   reached over the session bus and started from it on the first call.
# * `fonts-noto-core` is Plasma's interface font. The dialog is the one surface here that draws in
#   the system font; the app embeds its own faces (see `src/bin/ev_cost_recovery/theme.rs`).
# * The Mesa packages are the software renderer. There is no GPU, so llvmpipe and lavapipe are
#   what the compositor and the app's wgpu surface both draw with.
# * `libwayland-client0` and `libxkbcommon0` are dlopened by winit at runtime. They belong to the
#   app rather than to the compositor, so they are named here instead of being left to arrive as
#   somebody else's dependency.
# * `imagemagick` crops and compares the PNGs `grim` writes.
# * `python3-pywayland` runs virtual-pointer.py, the seat's pointer. `python3-cffi-backend` is a
#   dependency it does not declare: without it the import fails.
#
# No X11 client libraries, and Xwayland is switched off in the sway configuration. winit binds to
# whichever display server it can reach, so an X server within reach would let it take a path the
# desktop has no equivalent of, and that shows up as a screenshot nobody can account for rather
# than as an error.
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq \
    sway grim wtype imagemagick python3-pywayland python3-cffi-backend \
    libwayland-client0 libxkbcommon0 \
    libgl1 libegl1 libgles2 libgl1-mesa-dri libvulkan1 mesa-vulkan-drivers \
    fonts-dejavu-core fonts-noto-core \
    dbus-daemon dbus-bin \
    xdg-desktop-portal xdg-desktop-portal-kde
apt-get clean
rm -rf /var/lib/apt/lists/*
