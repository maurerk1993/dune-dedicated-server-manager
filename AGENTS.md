# Agent Instructions

This repo is not one of the usual Docker-hosted web apps. Treat it as a Rust
workspace plus a Tauri desktop application with a bundled Linux management
service.

This app has nothing to do with docker, so no need to use a docker validation path for this app.

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

## External Reference Repositories

Future agents should know these two external repos are intentional references
for this fork:

- Original upstream app repo: `https://github.com/adainrivers/dune-dedicated-server-manager`
  - Local remote is usually named `upstream`.
  - Use this to compare original app behavior, pull upstream fixes, and
    understand baseline architecture.
  - Do not restore upstream ownership, updater endpoints, release links, or
    signing configuration over this fork unless explicitly asked.
- DST_Custom feature/data reference: `https://github.com/maurerk1993/DST_Custom`
  - Use this for Dune admin feature research, especially specialization
    progression behavior, item quality/grade handling, item catalog metadata,
    gradeable flags, and stack size data.
  - Preserve Apache 2.0 attribution in `THIRD_PARTY_NOTICES.md` and license
    files whenever adapting implementation guidance or metadata from it.
  - Treat DST-derived code/data as reference material to adapt safely into this
    app's Rust/Tauri architecture, not as code to paste blindly.

This fork's release/updater surface is `origin`:
`https://github.com/maurerk1993/dune-dedicated-server-manager`. Release assets,
Tauri updater metadata, and installed-app update checks should continue to point
to this fork unless the user explicitly changes ownership again.

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
workflow because it builds on Windows and Linux and injects the bundled Linux
service binary into the desktop app.

Docker is not currently part of this repo. Check for Docker/Compose files if the
task suggests it, but do not force Docker validation when the project has no
Docker surface.

## Release And Ownership Notes

### Privacy before every GitHub publication

Before committing or pushing, inspect the exact staged files and remove
server-specific or personal information. Apply the same rule to PR descriptions,
comments, release notes, screenshots, PDFs (including metadata), logs, exports,
fixtures, attachments, and generated evidence.

- Never publish credentials, tokens, private keys, database connection secrets,
  player/account identifiers, character names, inventories, or raw player data.
- Remove actual host/IP addresses, hostnames, server/battlegroup/namespace/pod
  identifiers, personal filesystem paths, assigned ports, restart schedules,
  deployment authentication/network settings, and host-specific measurements.
- Use generic placeholders or clearly synthetic examples. Preserve reusable
  schema contracts, vendor defaults, public keys, and dated vendor-build
  references only when they do not reveal a private deployment.
- Review images visually and inspect PDF text, metadata, attachments and fields.
  Redact underlying content, not merely a visible overlay; label reduced audit
  evidence as a sanitized public summary rather than an unchanged raw output.
- Keep unsanitized evidence and local profiles outside version control. Check
  staged filenames and content for secrets before committing; recheck the final
  GitHub diff before publication. Ignoring a file does not remove prior commits.
- If a secret has already been published, do not repeat it in chat or GitHub.
  Report its type and location privately; identify credential rotation and any
  separately authorized history cleanup required. Never rewrite public history
  or rotate production credentials without the user's authorization.

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

## Whole-App Game Patch Compatibility Audit

The agreed plan is in `docs/game-patch-compatibility-audit-plan.md`. Read it
before any game-patch compatibility audit. The user may start it by saying
"let's get the audit done"; until then, do not access the server for this audit.

The audit targets live production with players online and covers the whole app,
not only items or community-reported changes. Its stricter safeguards take
precedence over the general item-survey defaults below: one production probe
at a time, read-only transactions, 5-second statement and 1-second lock timeouts,
no expensive scans/transfers, and defer unsafe checks. Audit approval authorizes
findings and a proposed repair plan only, not changes or gameplay tests.

## Manual Dune Database Item Survey

The user wants database item discovery to remain a manual Codex workflow, not
an automatic application feature. A read-only production survey was completed
on 2026-07-28. Use this playbook after a Dune patch when asked to check for new
or changed item evidence.

### Safe Connection Pattern

- Connect to the Dune VM over SSH using the server profile/key already managed
  by the desktop app. Discover the current VM address, namespace, and database
  pod rather than assuming old values are still valid.
- The Alpine VM may not put Kubernetes tools on the non-interactive SSH `PATH`.
  Use `sudo -n /usr/local/bin/kubectl`.
- Find the active `funcom-seabass-*` namespace and its PostgreSQL database pod.
  Run `/usr/local/bin/psql` inside the pod's `database` container.
- Use the container's existing `POSTGRES_USER` and `POSTGRES_PASSWORD`
  environment variables without printing their values. The game database is
  `dune`; the observed internal PostgreSQL port was `15432`, but rediscover it
  if the connection fails after a vendor update.
- Wrap every survey in:

```sql
BEGIN TRANSACTION READ ONLY;
SET LOCAL statement_timeout = '20s';
-- SELECT/COPY TO STDOUT queries only
ROLLBACK;
```

Never print database credentials, player/account identifiers, inventory IDs,
character names, or raw player-owned JSON. Return template-level aggregates
only. Do not create temporary objects, indexes, functions, or migrations in the
vendor database for this survey.

### Item Reference Sources

Inventory rows are item instances, not a master definition catalog. Collect
distinct template references from these explicit sources when they exist:

- `dune.items.template_id`
- `dune.landsraad_house_rewards.template_id`
- `dune.landsraad_task_rewards.template_id`
- `dune.vehicle_modules.template_id`
- `dune.vendor_stock_state.template_id`
- `dune.dune_exchange_orders.template_id`
- `dune.item_operations_staging_table.template_id`
- `dune.temp_items_dune_161802.template_id`
- `dune.removed_items.name` (historical removal tombstones)

First inspect `information_schema.columns`; vendor patches can rename, add, or
remove tables and columns. Also search all `dune` columns whose names contain
`template`, `item`, `recipe`, `reward`, or `stock` so a new vendor source is not
silently missed.

Compare the distinct references case-insensitively against:

- Active service status:
  `GET http://127.0.0.1:29187/api/admin/item-catalog/status`
- Bundled catalog:
  `crates/dune-server-service/data/items.json`

Keep catalog row count and unique template-ID count separate because the
catalog intentionally has known duplicate IDs. Report duplicate conflicts
instead of silently choosing one row.

Useful validation aggregates from `dune.items` are:

- Row count and distinct `template_id`.
- Maximum observed `stack_size` per template.
- Minimum/maximum observed `quality_level` per template.
- Distinct JSON key shapes from `stats`, without dumping raw instance data.

Interpret those aggregates conservatively:

- A stack larger than the catalog's `stackMax` proves the catalog is missing or
  too low, but the largest observed stack is only a lower bound and does not
  reveal the true maximum.
- A nonzero quality proves that template can exist at that quality. Seeing only
  quality zero does not prove an item is non-gradeable.
- Absence from one server does not prove an item was removed from the game.
- Vehicle locomotion, wheel/wing, and segmented hull templates may be internal
  vehicle subcomponents rather than grantable inventory items. Keep these in a
  separate review class unless inventory/reward or packaged-content evidence
  confirms them.

Actor `properties` JSON can contain `m_KnownItemRecipes`, `BaseRecipeId`,
`ItemKey`, and recipe quality state. These are player/station knowledge records,
not canonical item definitions. Recipe suffix stripping can provide discovery
candidates, but must never automatically create catalog entries. Likewise,
`map_areas.items_surveyed_target` contains resource and point-of-interest marker
types, not a general item catalog.

The database does not provide authoritative display names, user-facing
categories, source labels, tiers, gradeable flags, or exact stack limits for
all possible items. Obtain those fields from the installed game server's
packaged content/DataTables or another reviewed canonical source. Use the
database survey as patch evidence and validation, then manually review any
catalog change through the existing diff/export workflow.

For patch correlation, record the active `seabass-server` container image tag,
a schema fingerprint, the active catalog hash, and `dune.applied_patches`
summary. Treat the server image tag as the primary build fingerprint:
`applied_patches` tracks database migrations and may not change for a
content-only patch.

### 2026-07-28 Reference Baseline

Deployment-specific inventory counts are omitted from this public instruction
file. Instantiated templates matched the catalog; unmatched vehicle-module
references were internal locomotion/segmented-hull-style names.
The bundled catalog reference had 2,558 rows, 2,551 unique IDs, and hash
`3c60472aa9b989a2`. Four instantiated templates proved missing stack metadata:
`SolarisCoin` (observed 50,000), `Ammo` (1,000), `WeldingMaterial` (500), and
`HeavyAmmo` (445). Recompute every count after a patch; this is a dated
comparison baseline, not permanent game truth.

## Final Response Expectations

For completed work, include these sections when applicable:

- Version Bump
- Patch Notes (User-Facing)
- SQL Required?
- Anything Required After PR Merge?
- Step-by-Step Deployment/Runbook

If a section is not applicable, say so briefly rather than inventing release or
deployment work.
