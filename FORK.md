# Fork notes

This is a personal fork of [warpdotdev/warp](https://github.com/warpdotdev/warp) that adds a
tmux-style workflow (prefix keys, project sessions, copy mode) on top of the OSS build. It targets
Arch Linux with Hyprland. Work is tracked in the issues labelled `fork`, grouped by milestone.

## Build and run on Arch

Do not use `script/bootstrap` or `script/run`. They assume Ubuntu, reach private Warp repositories
over SSH and download a remote skill installer. Use cargo directly.

Packages and toolchain:

```sh
sudo pacman -S --needed base-devel cmake clang pkgconf protobuf openssl fontconfig freetype2 \
  expat libgit2 alsa-lib brotli jq git-lfs vulkan-radeon cargo-nextest cargo-deny fish \
  shellcheck github-cli
rustup toolchain install 1.92.0
git lfs install --skip-repo
git lfs pull
```

`--skip-repo` keeps the LFS hooks out of `.git/hooks`, because this machine routes hooks through a
global `core.hooksPath`. The LFS filter is still configured in `.git/config`, so checkouts and
pulls work.

Debug build and run:

```sh
cargo build --bin warp-oss --features gui
WARP_ENABLE_WAYLAND=1 ./target/debug/warp-oss
```

The first build takes around 12 minutes and `target/` grows to roughly 13 GB. The window runs
natively on Wayland with the Hyprland class `dev.warp.WarpOss`. Config lives in
`~/.config/warp-oss`, data in `~/.local/share/warp-oss`, both separate from a stable Warp install.
The first start shows the "Welcome to Warp" onboarding page. "Get started" leads straight to a
working shell; logging in is optional.

Release package:

```sh
./script/bundle --channel oss --packages arch --release-tag 0.1.0 \
  --features prefix_keys
```

The release tag becomes the `pkgver` of the Arch package and may not contain hyphens. The OSS
channel builds with the `release_bundle` feature set only, so every fork flag that should be in
the package has to be passed through `--features`. The build uses the `release-lto` profile. If
the linker runs out of memory, retry with `CARGO_BUILD_JOBS=6`.

Tests and lint for a single crate:

```sh
cargo nextest run -p <crate>
cargo clippy -p <crate> --all-targets --tests -- -D warnings
./script/format
```

## Branches

- `master` mirrors `upstream/master`. Never commit to it.
- `fork/main` carries all fork features and is the base for releases.
- One branch per ticket, named `fork/<short-topic>`, branched from `fork/main` and merged back
  through a pull request against `fork/main`.

GitHub Actions stay disabled on the fork. Upstream's release and triage workflows would otherwise
run on every push.

## Feature flags

Every fork feature gets its own `FeatureFlag`, following the upstream plumbing:

1. Add a Cargo feature in `app/Cargo.toml` under `[features]`, not in `default`.
2. Add the variant to the `FeatureFlag` enum in `crates/warp_features/src/lib.rs`.
3. Add the `#[cfg(feature = "...")]` entry to `enabled_features()` in `app/src/features.rs`.
4. Gate code with `FeatureFlag::YourFlag.is_enabled()`.

Build with the flags you want enabled, for example once the `prefix_keys` flag of M1 exists:

```sh
cargo build --bin warp-oss --features gui,prefix_keys
```

New logic goes into separate files, for example an `impl Workspace` block in
`app/src/workspace/view/<feature>.rs` included by one `mod` line in `app/src/workspace/view.rs`,
with only such minimal hooks in the large upstream files. This keeps rebases onto upstream small.
Keybindings that belong to a gated feature carry an enabled predicate on the binding so they stay
out of the keyboard settings while the flag is off.

## Syncing with upstream

```sh
git fetch upstream
git checkout master
git merge --ff-only upstream/master
git push origin master
git checkout fork/main
git rebase master
cargo build --bin warp-oss --features gui
git push --force-with-lease origin fork/main
```

Resolve conflicts on `fork/main` during the rebase. Open ticket branches rebase onto the new
`fork/main` afterwards.
