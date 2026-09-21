# .devcontainer/Dockerfile

# Pinned to resolute (Ubuntu 26.04) deliberately, not merely inherited: it matches the host and the
# `ubuntu-26.04` runners in .github/workflows, so all three agree on one glibc and a binary built
# here behaves like the one CI ships. glibc is backward compatible only, so the release runs on
# 26.04 and every later release but nothing earlier. That floor is a property of the build image,
# which is why the tag is fixed rather than left to follow the newest.
FROM mcr.microsoft.com/devcontainers/base:resolute

# The Wayland session the eframe/egui app renders onto, and the tooling to drive and observe it
# headlessly.
#
# This lives here rather than in setup.sh so it becomes a cached image layer. Docker keys its
# build cache on the instructions and base image, not on the workspace path, whereas the
# devcontainer's own identity is a hash of that path -- so moving the project rebuilds the
# container but reuses this layer, instead of re-downloading a few hundred MB over the network.
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
#
# No X11 client libraries, and Xwayland is switched off in the sway configuration. winit binds to
# whichever display server it can reach, so an X server within reach would let it take a path the
# desktop has no equivalent of, and that shows up as a screenshot nobody can account for rather
# than as an error.
RUN export DEBIAN_FRONTEND=noninteractive \
    && apt-get update -qq \
    && apt-get install -y -qq \
        sway grim wtype imagemagick \
        libwayland-client0 libxkbcommon0 \
        libgl1 libegl1 libgles2 libgl1-mesa-dri libvulkan1 mesa-vulkan-drivers \
        fonts-dejavu-core fonts-noto-core \
        dbus-daemon dbus-bin \
        xdg-desktop-portal xdg-desktop-portal-kde \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# The compositor's configuration and the colour scheme the portal reports. Both are read by
# processes running as `vscode`, and both are image layers, so they survive a plain container
# restart as well as a rebuild.
#
# `~/.config` already exists and already belongs to the user, so the one subdirectory below it is
# all that has to be made. It stays root-owned: sway only reads its configuration, and 0755 is
# enough to traverse and read, so handing the directory over would widen what the container user
# may do to it for nothing in return. Nothing else under `~/.config` is touched either -- a
# recursive `chown` there would have a scope set by whatever the base image ships, present and
# future, which is not this file's to decide.
RUN mkdir -p /home/vscode/.config/sway
COPY --chown=vscode:vscode sway-config /home/vscode/.config/sway/config
COPY --chown=vscode:vscode kdeglobals /home/vscode/.config/kdeglobals
