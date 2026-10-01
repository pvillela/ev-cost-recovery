# The GUI as a local devcontainer feature

The GUI packages (the headless Wayland session and the tools that drive it) are installed by a
local devcontainer feature, `.devcontainer/gui/`, and not by a Dockerfile. This document gives the
reason and the costs.

This file is outside `.devcontainer/gui/` on purpose. An edit here does not rebuild the GUI layer.

## Layer order

1. Base OS: `mcr.microsoft.com/devcontainers/base:resolute`
2. Node (nvm): `ghcr.io/devcontainers/features/node:2`
3. Rust: `ghcr.io/devcontainers/features/rust:1`
4. GUI: `./gui`
5. `setup.sh`, which runs after the image is built and is not a layer

## What sets the install order of features

The order of the keys under `"features"` in `devcontainer.json` does **not** set the install
order. The devcontainer tooling applies two rules:

1. **Dependencies.** A feature that declares `dependsOn` or `installsAfter` in its
   `devcontainer-feature.json` installs after the features it names. `./gui` names Rust and Node,
   so it is last.
2. **Alphabetical order.** Features with no dependency between them are sorted by identifier.
   Rust and Node have no dependency on each other, and `.../node` sorts before `.../rust`, so Node
   installs first.

To force a different order, list the feature identifiers in `overrideFeatureInstallOrder` in
`devcontainer.json`. That changes the layers, so each devcontainer that reuses them needs the same
setting.

## Why a feature

The layers below the GUI hold no GUI packages. A devcontainer without a GUI can reuse them from
Docker's build cache.

Devcontainer features always install after the Dockerfile. A GUI step in a Dockerfile is therefore
below the Rust and Node features, and cannot be the last layer. A feature can: `installsAfter` in
`gui/devcontainer-feature.json` puts it after Rust and Node.

## Reuse in a devcontainer without a GUI

```jsonc
"image": "mcr.microsoft.com/devcontainers/base:resolute",
"features": {
  "ghcr.io/devcontainers/features/rust:1": { "version": "latest", "profile": "default" },
  "ghcr.io/devcontainers/features/node:2": {}
}
```

Keep the image and the feature options the same as in `devcontainer.json` here. If they are
different, Docker does not reuse the layers.

Reuse across projects is expected but has not been tested. To confirm it, look for `CACHED` on the
Rust and Node steps in the build log of the second project.

## Disadvantages

- **Edits to comments rebuild the layer.** Docker keys the layer on the content of all files in
  `.devcontainer/gui/`. A change to one comment in `install.sh` downloads the GUI packages again.
  Comments in a Dockerfile do not have this effect.
- **`docker build` alone cannot make the image.** Features are applied by the devcontainer tooling
  (VS Code or the `devcontainer` CLI), which generates the Dockerfile. That file cannot be read or
  run directly.
- **The order is less visible and less strict.** `installsAfter` only puts the GUI after Rust and
  Node. A feature added later can install after the GUI unless it is also given an order.
- **Cache reuse depends on the tooling.** The generated build steps can change between tooling
  versions.
- **Only a script is available.** Dockerfile instructions such as `COPY`, `ENV` and `USER` cannot
  be used. The package install does not need them.

## The alternative

Install Rust and nvm by hand in a Dockerfile, before the GUI step. That gives exact, visible
layers and works with `docker build`. The cost is everything the two features add (for example
rust-analyzer, lldb and the VS Code settings), and the maintenance of those installs.
