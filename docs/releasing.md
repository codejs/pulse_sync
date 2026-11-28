# Releasing

Releases are built by [dist](https://opensource.axo.dev/cargo-dist/) (cargo-dist).
Pushing a version tag runs `.github/workflows/release.yml`, which:

1. builds the web UI and then `pulsed` (with `embed-web`) for macOS (arm64,
   x86_64), Linux (x86_64, arm64) and Windows (x86_64);
2. creates a GitHub Release with the archives, a Windows `.msi`, checksums,
   and install scripts for macOS/Linux (`sh`) and Windows (PowerShell);
3. pushes a Homebrew formula to the `codejs/homebrew-tap` repo.

## One-time setup

1. Create a **public** GitHub repo named `codejs/homebrew-tap` (it can be empty).
2. Create a fine-grained personal access token with **Contents: read and
   write** on that repo only.
3. Add it to this repo as an Actions secret named `HOMEBREW_TAP_TOKEN`
   (Settings → Secrets and variables → Actions).

Without the secret, releases still work; only the Homebrew publish step fails.

## Cutting a release

1. Bump `version` in the root `Cargo.toml` (`[workspace.package]`), run
   `cargo check` to update `Cargo.lock`, and commit.
2. Check the plan: `dist plan` (or `pnpm release-plan`).
3. Tag and push:

   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```

The tag must match the Cargo version. Tags with a pre-release suffix
(`v0.2.0-beta.1`) are published as GitHub pre-releases.

## Changing the release setup

Settings live in `dist-workspace.toml`, plus `[package.metadata.dist]` in
`crates/pulse-sync/Cargo.toml`. After editing them, regenerate the workflow
(don't hand-edit `release.yml`):

```sh
cargo install cargo-dist --locked   # once; or: cargo binstall cargo-dist
dist generate
```

The web build steps injected into each build job are in
`.github/build-setup.yml`. Every step there needs a `name:` (dist 0.34's
template fails without one).

## Test a release build locally

```sh
pnpm --dir web build
dist build --artifacts=local      # writes target/distrib/
```

## Not yet done

- **Code signing.** macOS binaries aren't notarized and Windows binaries
  aren't signed. Files downloaded with a browser trigger Gatekeeper and
  SmartScreen warnings. The shell installer and Homebrew avoid the macOS
  quarantine prompt.
- **Auto-update.** `install-updater = false`; users update via
  `brew upgrade` or by re-running the installer.
