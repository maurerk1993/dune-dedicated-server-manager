# Owning and Releasing This Fork

This guide explains how this fork becomes your build of Dune Dedicated Server
Manager, how installers are made, and how app updates reach installed copies.

## What This App Is

This is not a Docker web app. It is:

- A Windows/macOS/Linux desktop app built with Tauri.
- A React frontend in `app/`.
- Rust code in `app/src-tauri/` and `crates/`.
- A Linux `dune-server-service` binary that gets bundled into the desktop app.
- A GitHub Releases based updater.

The app itself does not run from your GitHub repo. Users install a desktop
installer from GitHub Releases. The installed app later checks a `latest.json`
file attached to the latest GitHub Release to decide whether an update exists.

## Current Owner Settings

This fork is configured to use:

```text
https://github.com/maurerk1993/dune-dedicated-server-manager
```

The main places that point to the owner repo are:

- `app/src-tauri/tauri.conf.json`
- `app/src/components/dialogs/AboutDialog.tsx`
- `app/src/components/dialogs/UpdateDialog.tsx`
- `README.md`

If you ever move or rename the repo, update all of those together and search for
old links with:

```powershell
rg "github.com/.*/dune-dedicated-server-manager|adainrivers"
```

## One-Time GitHub Setup

1. Make sure your GitHub repo exists and Actions are enabled.
2. Keep releases public if you want the app updater to work without login.
3. Generate or preserve your Tauri updater signing key.
4. Put the private key in GitHub Secrets.
5. Put the public key in `app/src-tauri/tauri.conf.json`.

The signing key is what lets an installed app prove an update came from you. Do
not commit the private key.

## Generate The Updater Signing Key

This checkout uses an ignored workspace-local key path:

```text
.tmp/tauri-signing/dune-dedicated-server-manager.key
```

If that file is missing, regenerate a key on your development machine:

```powershell
Push-Location app
npm ci
New-Item -ItemType Directory -Force "..\.tmp\tauri-signing"
npm run tauri signer generate -- --ci -w "..\.tmp\tauri-signing\dune-dedicated-server-manager.key" --force
Pop-Location
```

The command writes both a private key and a public key:

```text
.tmp/tauri-signing/dune-dedicated-server-manager.key
.tmp/tauri-signing/dune-dedicated-server-manager.key.pub
```

The public key belongs in:

```text
app/src-tauri/tauri.conf.json -> plugins.updater.pubkey
```

Open your GitHub repo, then go to:

```text
Settings -> Secrets and variables -> Actions -> New repository secret
```

Create these secrets:

```text
TAURI_SIGNING_PRIVATE_KEY
TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

For `TAURI_SIGNING_PRIVATE_KEY`, use the contents of the private key file, not a
file path. If you did not set a password, leave
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` unset.

Keep this private key backed up somewhere safe. If it is lost, already-installed
apps cannot trust future automatic updates from a replacement key. You can still
recover by making users manually install a new build, but automatic handoff will
not work.

## How To Build Locally

Use local builds for quick testing on your own machine.

Install frontend dependencies:

```powershell
Push-Location app
npm ci
Pop-Location
```

Run native checks:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo doc -p dune-manager-core --no-deps
Push-Location app
npm run build
Pop-Location
```

Build a local production executable without installer packaging:

```powershell
Push-Location app
npm run tauri -- build --no-bundle
Pop-Location
```

Build a local Windows installer:

```powershell
Push-Location app
npm run tauri -- build --bundles nsis
Pop-Location
```

Local builds are useful, but the normal release path is GitHub Actions because
Actions builds all supported platforms.

## How To Make A Release

Use a version like `0.3.18`.

1. Update the version in these files:

```text
app/package.json
app/package-lock.json
app/src-tauri/tauri.conf.json
crates/dune-server-service/Cargo.toml
Cargo.lock
```

2. Add release notes:

```text
release-notes/0.3.18.md
```

3. Run the checks from the previous section.
4. Commit the changes and merge them into `main`.
5. Create and push a version tag:

```powershell
git checkout main
git pull origin main
git tag v0.3.18
git push origin v0.3.18
```

6. GitHub Actions runs `.github/workflows/release.yml`.
7. When it finishes, open the GitHub Release for `v0.3.18`.
8. Download and test the installer for your operating system.

The workflow builds:

- Windows NSIS installer.
- Linux AppImage and Debian package.
- macOS DMGs for Apple Silicon and Intel.
- The Linux `dune-server-service` binary.
- Signed updater artifacts and `latest.json`.

## How Updates Work

The installed app checks:

```text
https://github.com/maurerk1993/dune-dedicated-server-manager/releases/latest/download/latest.json
```

That file is produced by the release workflow. It tells the app:

- The latest version.
- Which installer/update artifact to download for the user's operating system.
- The signature that proves the artifact came from your signing key.

Important first-release rule:

The first owned build must be installed manually. An app that was installed from
the upstream project still has the upstream update URL and public key inside it.
After you install your fork's build once, future updates can come from your
GitHub Releases automatically.

## Keeping Up With Upstream

Your local repo already has two remotes:

```text
origin   -> your fork
upstream -> original project
```

To bring in upstream work:

```powershell
git checkout main
git pull origin main
git fetch upstream
git merge upstream/main
```

After merging upstream, check that your owned links did not get replaced:

```powershell
rg "adainrivers|github.com/adainrivers"
```

If that search finds anything in active app/config files, change it back to your
repo before releasing.

## Management Service Updates

The desktop installer includes the Linux `dune-server-service` binary. Updating
the desktop app does not automatically push that service onto each Dune host.

If a release changes `crates/dune-server-service/`, then after installing the
new desktop app:

1. Open each server in the app.
2. Go to the Management Service card.
3. Click **Install / Update**.

That uploads the bundled service binary to the remote host over SSH.

## SQL And Data

There is no app-owned SQL migration runner in this project today.

The app uses:

- Browser `localStorage` for attached server profiles and UI preferences.
- A SQLite database inside the remote `dune-server-service`.
- The existing Dune server PostgreSQL database for live admin/server data.

If a future change adds SQL, write down exactly what file to run, where to run
it, and in what order before releasing.
