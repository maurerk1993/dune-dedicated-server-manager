# Agent Instructions

This repo is not one of the usual Docker-hosted web apps. Treat it as a Rust
workspace plus a Tauri desktop application with a bundled Linux management
service.

## Project Shape

- Desktop app: `app/`, React + Vite + Tauri v2.
- Tauri shell/config: `app/src-tauri/`.
- Core Rust library/CLI: `crates/dune-manager-core/`.
- Remote host daemon: `crates/dune-server-service/`.
- Release automation: `.github/workflows/release.yml`.
- Release notes shown through GitHub Releases/updater: `release-notes/<version>.md`.

The app manages existing Dune dedicated servers over SSH and Kubernetes. Do not
treat it as a web app deployment, a Supabase app, a Prisma app, or a normal
server-side Docker Compose project unless Docker files are actually introduced.

## Validation Defaults

Use the repo's native checks first:

```powershell
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo doc -p dune-manager-core --no-deps
Push-Location app
npm ci
npm run build
npm run tauri -- build --no-bundle
Pop-Location
```

For full installer/release validation, prefer the GitHub Actions release
workflow because it builds on Windows, Linux, and macOS and injects the bundled
Linux service binary into the desktop app.

Docker is not currently part of this repo. Check for Docker/Compose files if the
task suggests it, but do not force Docker validation when the project has no
Docker surface.

## Release And Ownership Notes

This app has a Tauri updater. Fork/ownership work must check every public repo
and updater surface, especially:

- `app/src-tauri/tauri.conf.json`
- `app/src/components/dialogs/AboutDialog.tsx`
- `app/src/components/dialogs/UpdateDialog.tsx`
- `README.md`
- `.github/workflows/release.yml`

If changing the app to use a new owner's releases, update the Tauri updater
endpoint to that owner's GitHub Releases `latest.json`, and replace user-facing
repository/issues/release links.

Updater signing is required. Never commit updater private keys. The public key
belongs in `tauri.conf.json`; the private key and optional password belong in
GitHub repository secrets:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

The first private-owner build usually needs to be installed manually once,
because an already-installed upstream build will keep checking the upstream
updater endpoint until replaced by a build whose config points to the new repo.

## Versioning

For app/software changes, keep versions aligned across:

- `app/package.json`
- `app/package-lock.json`
- `app/src-tauri/tauri.conf.json`
- `crates/dune-server-service/Cargo.toml`
- `release-notes/<version>.md`

Use semantic versioning:

- Patch for small fixes.
- Minor for user-visible improvements or new workflows.
- Major for breaking changes.

Do not bump the app version for repo-only maintenance such as this instruction
file unless the user explicitly wants a release for it.

## Installer Creation

Local quick production executable:

```powershell
Push-Location app
npm run tauri -- build --no-bundle
Pop-Location
```

Local Windows installer:

```powershell
Push-Location app
npm run tauri -- build --bundles nsis
Pop-Location
```

Normal public/private distribution should use `.github/workflows/release.yml`.
It builds:

- Windows NSIS installer.
- Linux AppImage and `.deb`.
- macOS DMGs for Apple Silicon and Intel.
- `dune-server-service` musl binary and service unit files.
- Tauri updater metadata/artifacts when signing secrets are present.

## Data And SQL

This project does not have app-owned SQL migration files today. It has:

- Local browser `localStorage` for attached server profiles/preferences.
- A remote `dune-server-service` SQLite store managed internally by Rust code.
- Reads/writes against the existing Dune server Postgres database through
  application/admin flows.

If a change introduces SQL, database migrations, or schema changes, explicitly
tell the user:

- Exact file(s) involved.
- Exact run order.
- Where to run them.
- Whether the change affects the remote service SQLite store, the Dune Postgres
  database, or both.

Assume the user is non-technical and write the database steps plainly.

## Final Response Expectations

For completed work, include these sections when applicable:

- Version Bump
- Patch Notes (User-Facing)
- SQL Required?
- Anything Required After PR Merge?
- Step-by-Step Deployment/Runbook

If a section is not applicable, say so briefly rather than inventing release or
deployment work.
