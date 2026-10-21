# Dependency security maintenance

The 0.9.2 dependency refresh was reviewed on October 21, 2026. It updates the
Tauri, SSH, PostgreSQL and frontend build chains without changing Funcom's
files, game database schema, key discovery, admin commands or restart workflow.

## Updater compatibility

The supported release artifacts remain signed Windows NSIS `.exe`, raw Linux
`.AppImage` and `.deb`. Updater signature verification, TLS and system-proxy
support remain enabled. The optional `zip` feature is disabled to remove the
vulnerable legacy ZIP extraction dependency. ZIP-wrapped Windows installers
and gzip-wrapped AppImages are no longer accepted: continue publishing the
existing direct artifact formats in `.github/workflows/release.yml`.

## Known upstream findings

Do not suppress the remaining findings just to obtain a green audit:

- `RUSTSEC-2023-0071`: the RSA dependency has no patched release. Keep support
  for existing client keys; changing key types or SSH server configuration is
  outside this dependency update. Review the affected operations and upstream
  fixes before any further change.
- `RUSTSEC-2024-0429`: Linux GTK's `glib 0.18` dependency has an unsound-iterator
  warning. The compatible Tauri/GTK3 dependency chain still requires it;
  replacing it with an incompatible major version is not a safe lockfile fix.
- `RUSTSEC-2024-0370`: GTK's `proc-macro-error` dependency is unmaintained and
  remains an upstream build-time warning.

Recheck these findings against the current RustSec database on future updates.
Use `npm audit` for the entire frontend tree and RustSec `cargo audit` for
`Cargo.lock`; a vulnerability count is not a proof of exploitability or safety.

## Validation boundaries

SSH regression tests generate temporary Ed25519, RSA and ECDSA keys, run only
loopback fixtures, and exercise authentication, cached commands, error status,
chunked uploads and tunnels. They never read vendor keys or contact a saved
server profile. Production/gameplay validation requires separate authorization.

Use Node.js 22.12 or newer for Vite 8. The explicit browser build target retains
the earlier Vite 5 baseline. Linux installer validation remains part of the
normal Windows/Linux release workflow.
