# Dune Manager Patch Compatibility Audit

Public, sanitized historical copy. Deployment identities, schedules and host
measurements have been removed; vendor schema/build references are retained
only as dated compatibility evidence, not current deployment information.

Date: September 17, 2026. Repository revision:
`a31ed62af30e7e55acd3b79f49b31486628c996e`. Local app/service release: 0.8.0.

## Executive result and production boundary

**The authorized low-impact audit is complete, with gameplay/write tests and
missing packaged-content analysis explicitly deferred. No blanket compatibility
sign-off is given.**

The reported actor/marker changes do **not** establish a broken spawning query
in this app. Its expected actor, inventory and item columns remain present.
The current healthy Dashboard/Maps read contract matches the inspected server.
However, two concrete app-side gaps deserve priority: offline-only writes allow
the game's `LoggingOut` state, and the desktop's 60-second lifecycle command
deadline is shorter than the vendor script's valid 90/120-second waits.

After confirming the reinstalled VM's identity, strict-host-key-checked SSH
succeeded. Host addresses and resource identifiers are omitted. All production probes
were serialized. Database sessions were read-only, with 5-second statement and
idle limits, a 1-second lock limit, rollback and session closure. No player rows,
inventories, credentials or raw player JSON were exported. Only metadata and a
five-name indexed migration sample were queried in PostgreSQL.

No game commands, database writes/migrations, restarts, installations, catalogue
updates, automation changes or edits to Funcom's files occurred. Existing
service GET endpoints and Kubernetes resources were read; incidental normal
access/audit logging can still occur. No application source changes were made.
Local builds/tests generated ordinary build artifacts and temporary test data.
The report, plan status and sanitized evidence are repository-only additions.

## Evidence baseline and limits

- Local registration inventory: **51 desktop commands**, **29 service HTTP
  paths / 30 method registrations**, **15 admin command specs**, and **4
  scheduled/background task implementations**.
- Game image: `registry.funcom.com/funcom/self-hosting/seabass-server:2111270-0-shipping`;
  running image ID `sha256:ece2278e28c7a69b49b1d0f96504d1d45e09057d79eca71a54210a378592e69d`.
  Four Funcom operators: `v1.7.0`. PostgreSQL: `17.4`, image
  `igw-postgres:17.4-alpine-fc-13`. Old completed dump jobs still reference an
  older image; those are not the running game build.
- App/service compatibility reference for this historical audit: **0.8.0**.
  Bundled catalog reference hash: `3c60472aa9b989a2`.
  Deployment-specific automation settings are omitted from this public copy.
- Local bundled catalog: 2,558 item rows / 2,551 unique IDs; 378 gradeable rows,
  106 rows with stack maximum above one; 9 vehicles, 145 skill modules, 708
  retained Journey nodes, and 44 XP tags. These are app data, not a fresh game
  content extraction.
- Scoped local artifact discovery found no version-matched server executable,
  packaged content, schema dump or migration/datamine copies in the workspace.
  The existing broad snapshot collector was not run because its scope does not
  meet this audit's minimal production-probe policy.
- Evidence is saved in `docs/audit-evidence/2026-09-17/`. There is no equivalent
  complete pre-patch schema snapshot, so this establishes the current contract,
  not when each field or table changed. The installed image is the build anchor;
  no public release-name-to-image mapping is assumed.
- Schema-column metadata MD5: `08caf7d22de144f2e982de0819abeab3` (small catalog
  aggregation in probe 01, not a game-table or file scan). Local SHA-256 of the
  saved `01-schema-metadata.txt` artifact:
  `ed4b1c44767f4cf327d76f20d50971ddd39f14353c1c7292f39a134144fb2368`.
  This is an evidence-file fingerprint, not a full dump hash. Probe 05 records
  five migration names in lexical order, **not the five latest applied patches**.

### Production health and scope actually observed

Initial and final snapshots showed no observed health degradation. Exact
resource counts, utilization, player counts and timestamps are omitted from
this public copy. Snapshots cannot prove zero impact. Live-production safeguards
remained in force; no gameplay test was performed based on observed activity.

## Current game contract findings

| Contract | Current evidence | Consequence for this app |
| --- | --- | --- |
| Actors and position | `dune.actors` exists; no `dune.actor_state` relation. `state` is `dune.actorstate` enum. Top-level `class`, `transform`, `partition_id`, `dimension_index`, `properties`, `gas_attributes` remain | No direct rename fix indicated. App does not query `dune.actor_state`; transform was not replaced by this state field |
| Coordinates | `transform.location` is `dune.vector` with x/y/z double precision; rotation is a quaternion | Current SQL field/type assumptions match. Live values, units, freshness and teleport results are not verified |
| Player identity | `player_state` view filters `character_state='Active'`. Underlying table has character `id` PK and a nonunique account index | Deleted characters are excluded by the Active-only view. Multiple Active rows per account are not proven, but not prevented by a reviewed uniqueness constraint; ambiguity checks remain prudent |
| Player connection state | Enum contains `Offline`, `LoggingOut`, `Online` | App's Online-only rejection incorrectly permits LoggingOut for offline-only writes; finding 1 |
| Items/inventories | Existing insert fields, stats JSONB and quality_level retained; omitted `volume_override` is nullable. Inventory has `max_item_count`/`max_item_volume` | No simple missing-column insertion failure found. Capacity, stats, stack/grade and online persistence semantics are not certified |
| Specialization | Tracks/keystones still reference `actors(id)`; `set_specialization_xp_and_level(bigint, dune.specializationtracktype, integer, real)` inserts the supplied actor ID | No evidence supports switching these IDs to new character IDs. Function definition read, never invoked; controller semantics/game effect remain untested |
| Markers | `markers` stores typed position plus payload JSONB; `player_markers` has separate player payload JSONB. `load_markers` builds shared/player composite data | No `marker` column to rely on. App has no direct marker-table query; its Maps page is Kubernetes-backed. This is not proof vehicle command handling is unchanged |
| Journey | Story-node/cooldown rows key `character_id` to `encrypted_player_state(id)` | Do not infer specialization identity from Journey changes. Dedicated Journey commands remain disabled; general ResetProgression still needs game-handler validation |
| Maps/health | BattleGroup rows join ServerStats rows, no duplicate join keys; expected phases, booleans, numeric-string FPS and count types present | Healthy read contract verified for the observed snapshot, not startup/degraded behavior |
| Lifecycle wrapper | Existing symlink targets a 31,689-byte vendor script supporting numeric BG selection and start/stop/restart; spec.stop patching retained | Wrapper was read, not executed. Its stop90/start120-second waits exceed desktop60-second command deadline; finding 4 |

Constraint/index/default checks and relevant trigger checks were metadata-only.
No custom triggers were found on the four inspected actor/item/inventory/player
tables, but this does not prove a raw insert reproduces all game logic. No user
data or decrypt function was evaluated. No data-changing SQL or game-defined
function was executed.

## Prioritized app findings

These are code-observed risks, not confirmed patch regressions or evidence that
production data has been damaged. Priorities indicate repair-review order.

### 1. P1 - Offline-only database writes allow the supported LoggingOut state

`crates/dune-server-service/src/postgres/queries.rs:427` recognizes only the
literal status `Online` (case-insensitive). Graded grants and specialization
edits reject only that result (`http/api_admin.rs:346` and `:232`). Empty,
unrecognized, or newly introduced transition states can therefore pass the
offline check; some target decode failures also become default values. The live
enum confirms `LoggingOut` is a supported value. This is a confirmed guard defect
by code/schema comparison, not a reproduced write or evidence of player damage.

Proposed fix: require a positively verified supported offline state and valid
target identifiers before a write. Unknown/transition/schema-incompatible
states should block the affected operation with a clear explanation. A
pre-write recheck does not by itself eliminate the login race; validate that
separately in an isolated environment.

### 2. P1 - Direct welcome grants rely on unverified inventory assumptions

`tasks/welcome_package.rs:346` resolves a backpack and inserts items without an
online-state gate. Each configured grant becomes one row at the requested
quantity, without active-catalog stack splitting. `postgres/queries.rs:171`
searches positions 0..10000 rather than deriving actual backpack capacity.

Deployment-specific automation settings are omitted. The free-slot assumption
also affects graded grants.

Proposed fix: first establish the new game's ownership, capacity, stack, stats,
and online-inventory persistence contracts. Gate incompatible direct writes;
do not simply replace table names. Preserve the welcome workflow unless a
separately approved change is necessary. No automation was disabled here.

### 3. P1 - App SSH connections do not verify host identity

`crates/dune-manager-core/src/orchestration/russh_runner/session.rs:37` defines
`AcceptAllHandler`; `check_server_key` always returns true. The shared code is
used for command execution and tunnels. This is an existing security issue,
not a reported game schema change.

Proposed fix: persisted trusted host-key verification with explicit first-use
and changed-key handling. The audit used native OpenSSH with strict verification,
not this app connection path.

### 4. P1 - Lifecycle deadlines and transport handling allow uncertain outcomes

`app/src-tauri/src/commands/shared.rs:13` applies an absolute **60-second**
command deadline to the desktop lifecycle wrapper path. The installed vendor
script can wait **90 seconds** for stop and **120 seconds** for start's webhook
retry. A legitimate slow operation can exceed the app's cap. Longer frontend
stop180/start480/restart600-second limits do not remove this inner deadline.
The resulting remote operation may have partly completed; its fate after SSH
disconnection is not assumed. No live timeout/restart reproduction was attempted.

The scheduled management-service restart is separate: its wrapper execution
budget is 1,200 seconds followed by up to 1,200 seconds for readiness. It does not
have the same desktop 60-second mismatch. Review the SSH client's separate
60-second inactivity timeout too (`russh_runner/session.rs:57`).

`russh_runner/runner.rs:129` reconnects and reruns a command after transport
errors without distinguishing reads from writes. If the first remote command
ran but its result was lost, a lifecycle or component operation may run again.

`session.rs:192` also treats missing SSH exit status as zero/success; an absent
completion signal should instead remain uncertain.

Proposed fix: preserve short read deadlines, give lifecycle execution an
appropriate bounded vendor-compatible budget, and permit automatic retry only
for explicitly safe reads/idempotent
operations. Report an uncertain outcome for writes and reconcile state without
automatically repeating the action. No failure/replay test was run remotely.

### 5. P2 - Database and task failure handling needs compatibility hardening

- `postgres/conn.rs:95` sets a connection timeout, not statement/lock deadlines.
  Queries can wait beyond the connection budget. Welcome discovery queries the
  account list every two seconds when enabled; player search uses wildcard
  matching and joins. These app endpoints are not automatically audit-safe.
- `postgres/queries.rs:358` uses infallible `row.get` for coordinates. A null or
  incompatible decoded value can panic the request. Optional dimension/partition
  decode failures can silently remove context.
- `tasks/restart.rs:50` treats failure to read the stopped-state flag as an empty
  value; a scheduled restart can continue despite an unknown stop state.
- `scheduler/runner.rs` releases its per-task running guard manually; early
  errors or panic/cancellation can leave a task marked running until restart.

Proposed fixes: bounded SQL waits; checked decoding and explicit unknown states;
fail closed on unknown lifecycle state; reliable running-guard cleanup. Inspect
new coordinate meaning before choosing any JSON extraction or conversion.

### 6. P2 - Welcome delivery has a cross-store uncertain-outcome window

The PostgreSQL grant commits at `postgres/queries.rs:634`; the SQLite delivery
ledger is updated afterward (`tasks/welcome_package.rs:210`). A process or
ledger/logging failure in between can leave an applied grant eligible for a
later attempt or manual Retry. The stores are not one atomic transaction.

Proposed fix: durable pre-delivery intent and an explicit uncertain state with
review/reconciliation; do not blindly replay uncertain deliveries. Do not add a
vendor database migration without separate review and approval.

### 7. P2 - Status and action targeting can hide or compound problems

- `app/src-tauri/src/commands/status_data.rs:588` removes `Not present`
  components before the dashboard can recognize missing required services.
  Preserve missing-required evidence while distinguishing an intentionally
  stopped battlegroup from a broken running one.
- The Pods restart button receives per-component busy state but no stale/error
  or whole-server lifecycle guard (`ServerPodRow.tsx`, `ServerDetailPage.tsx`,
  `useComponentActions.ts`). Revalidate target and fresh state before mutation.
- `status_data.rs:654` builds server identity from host/key, excluding
  namespace/battlegroup/user/port. Multiple battlegroups on the same host can
  collide. This is relevant to correct target selection, not proof that this
  installation currently has multiple battlegroups.

### 8. P2 - Catalog review and lookup resolve duplicate IDs differently

`admin/data.rs:139` selects the first matching ID; `admin/catalog.rs:472` builds
a map that keeps the last. Seven duplicate IDs exist locally. The two
`UniqueThumper_02` / `_03` entries have conflicting display names; other repeated
IDs include category/source aliases. A reviewed diff can disagree with lookup.

Proposed fix: expose conflicts and use one consistent source of execution
metadata while preserving intentional display aliases. Do not silently delete
catalog rows or infer removed items from absent inventory instances.

### 9. P2/P3 - Validation and delivery-success claims have gaps

Command building primarily checks presence/type, not every numeric domain,
select membership, or all-player restriction. Malformed typed welcome grants
can be silently dropped. MQ success proves publishing, not handler execution
or gameplay effect; some notice/message bookkeeping overstates that distinction.
Historical raw-command no-ops remain exposed. These require explicit validation
and accurate submitted/applied/unknown-result wording, not production test clicks.

### 10. P2 - Bind status, character and inventory to the same resolved target

Player search/UI identity uses account/FLS rather than character ID; location
uses an unordered account match, while admin targeting and backpack lookup each
choose a latest-login match independently (`postgres/queries.rs:23`, `:157`,
`:226`; `AdminTab.tsx:1613`). The Active-filtered view materially reduces the
concern: there is **no evidence of wrong-character selection or multiple Active
characters on this VM**. No reviewed uniqueness constraint guarantees one Active
character per account, however.

Proposed hardening: reject ambiguous active targets, bind status/pawn/controller/
inventory consistently, and keep account-level messaging identity distinct from
character-level DB identity. Do not replace all actor IDs with character IDs.
Welcome eligibility remains an explicit once-per-account product decision.

## Feature-by-feature coverage matrix

Dispositions distinguish observed read contracts from structural review and
unverified gameplay. No write operation receives a gameplay pass. Risks and
proposed fixes refer to the numbered findings above; local-only does not mean
a full interactive UI test. Entrypoints are exhaustively listed below the table.

| Feature | Dependency / evidence | Disposition and risk | Remaining validation / proposed work |
| --- | --- | --- | --- |
| Attach/discovery/profiles | Namespace/BG discovered at corrected IP; local profile identity review | Structural match for this single BG; P2 wrong-target risk, finding 7 | Multi-BG/custom-port cases; preserve explicit target identity |
| Dashboard/list/health cards | Current BG health + host proc/memory/disk; tests for parser/stale behavior | Observed healthy read shape matches; P2 hidden missing-service risk, finding 7 | Degraded/stopped/stale UI and live optional Metrics API behavior not exercised |
| Maps/player totals | Current BG rows match ServerStats rows; supported phase/ready/FPS/count types | Healthy read contract verified for snapshot; no missing counts, player counts omitted | Startup/post-landscape phases, missing telemetry, dimension ambiguities require fixtures/isolated tests |
| Pods/services/logs | Current ready/terminal pod states; local role/container/metrics code | Structural review; terminal jobs are not running-service failures | Full selector/log-container mapping and optional CPU/RAM availability not independently exercised; no log export |
| Component restart | Local selected-resource restart scripts | Mutation unverified; P1/P2 replay/stale-target risks, findings 4/7 | Fresh target gating and isolated restart/failure tests |
| Manual start/stop/restart | Current vendor wrapper selection/stop contract, inspected source + desktop deadlines | Confirmed deadline mismatch; P1 uncertain completion, finding 4 | Vendor-compatible bounded timeouts, no blind replay; isolated lifecycle tests |
| Daily restart/notices | Schedule omitted; vendor wrapper/CR contract retained; different service execution budget | Structural review only; P2 unknown stopped-state handling, finding 5 | Readiness, timezones/DST and actual notice/restart tests isolated; no same desktop60s conflict |
| Users/player picker | Active-filtered player view, account/actor fields/FKs and enum inspected | Structural match; cardinality conditional, P2 finding 10 | Names/decryption/real selection not queried; fail closed on ambiguous targets |
| Position/teleport helper | Current transform/vector composites and partition/dimension fields | Structural match, not live-coordinate verification; P2 checked-decoding gap, finding 5 | Units/current values and teleport application isolated; no zero-coordinate fallback |
| Ordinary item grants | AddItemToInventory MQ + player picker + active bundled catalog | Gameplay unverified; no renamed item/actor column failure identified | Current handler arguments/template acceptance require local game artifacts and isolated grant |
| Graded item grants | Same item/inventory fields and defaults; direct DB transaction + offline guard | Concrete LoggingOut gating defect, P1 finding 1; capacity/stats remain uncertain, finding 2 | Explicit Offline + bound target; verify grade/stats/stack/capacity/persistence on copy, never live write probe |
| Vehicle spawning | SpawnVehicleAt MQ, templates, actor position helper; no marker-table SELECT | Gameplay unverified; marker change does not directly break a manager SELECT | Template/class/coordinates/dimension/game-handler contract and spawn test isolated |
| XP/skills/progression/specialization | MQ specs + retained actor-based specialization FK/function | Structural specialization match; offline guard P1; hardcoded limits/XP tags unverified | Do not swap actor IDs for character IDs; check progression/reset semantics in isolated build |
| Other admin tools | Broadcast/kick/cleanup/water/raw command specs | Gameplay unverified; validation/submitted-vs-applied gaps, finding 9 | Per-command tests; historical raw no-ops not counted as successes |
| Catalog workflows/pickers | Live active catalog equals bundled hash/counts; local diff/lookup behavior | Status read verified; P2 duplicate lookup conflict, finding 8 | Content extraction absent: new/removed IDs, names, grades/tiers, stacks not certified; unify duplicate handling |
| Welcome packages/messages | Settings omitted; local task/DB/ledger/MQ review | P1/P2 latent capacity/online/duplicate risks, findings 2/5/6/9 | Preserve user settings; do not replay uncertain grants; isolated delivery tests |
| Service/API/config/history | OpenRC running; health0.8.0/config/catalog GETs; empty five-run sample | Read summaries verified; no historical task outcome evidence | No install/restart/config save; API health is not gameplay certification |
| SSH/tunnels/custom ports | Native trusted SSH succeeded; assigned port mappings omitted | Partial structural check; app trust P1 finding 3 | PgHero/File Browser tunnel endpoints, custom-port propagation and endpoint auth not certified; trust-store design separately |
| Local preferences/logging/updater | localStorage, local files, release config; frontend/native tests | No direct game schema dependency | Interactive save/export/update installation not exercised; no release change |
| CLI/core/dormant surfaces | Local setup/bootstrap/region/map-instance/vendor flows; world_partition columns retained | Structural only, separate from desktop; potential writes deliberately unexecuted | CLI unqualified SQL/search_path and vendor plaintext output need fixtures; legacy bg update not restored to UI |
| Removed backup/server update | No active desktop registration | Not applicable to active desktop audit | Remain removed; vendor backup/update functions observed as source only, never executed |
| Dedicated Journey features | Commands disabled, nodes retained; current character-ID FKs | Inactive; generic ResetProgression still unverified | Do not enable old Journey commands; review handler/schema before any future feature |

### All 15 admin specs

`AddItemToInventory`, `ServiceBroadcast`, `KickPlayer`, `CleanPlayerInventory`,
`ResetProgression`, `UpdateAllWaterFillables`, `AwardXP`,
`SpecializationLevelXp`, `SkillsSetModuleLevel`, `SkillsSetUnspentSkillPoints`,
`TeleportTo`, `TeleportToExact`, `SpawnVehicleAt`, `CheatScript`, `ServerExec`.

The graded-item HTTP operation is separate from these 15 specs. Specialization
uses a dedicated UI/DB path; generic publishing does not establish a valid
specialization implementation. AwardXP injects a historical Combat category.
These assumptions require game-side corroboration.

### All service HTTP paths

Registration source: `crates/dune-server-service/src/http/mod.rs:32`.

- GET `/`, `/api/health`, `/api/runs`, `/api/logs`, `/api/config`, `/api/timezones`.
- POST `/api/runs/trigger`, `/api/config`. Trigger runs with `dry_run=false`.
- GET `/api/admin/commands`, `/api/admin/items`, `/api/admin/vehicles`,
  `/api/admin/skill-modules`, `/api/admin/journey-nodes`, `/api/admin/xp-event-tags`.
- GET `/api/admin/item-catalog/status`, `/api/admin/item-catalog/export`.
- POST `/api/admin/item-catalog/diff`, `/api/admin/item-catalog/apply`,
  `/api/admin/item-catalog/revert`.
- GET `/api/admin/players`, `/api/admin/player-location`,
  `/api/admin/specialization`, `/api/admin/cluster`, `/api/admin/history`,
  `/api/admin/welcome-grants`.
- POST `/api/admin/items/grant-quality`, `/api/admin/specialization/level`,
  `/api/admin/publish`, `/api/admin/welcome-grants/retry`,
  `/api/admin/welcome-whisper`.

Service health only reports liveness; it does not certify database/game
compatibility. Only GET `/api/health`, `/api/config`,
`/api/admin/item-catalog/status`, and `/api/runs?limit=5` were called in this
audit. Handler side effects/query shapes were reviewed first; private fields
were omitted from output. Player search, raw logs/history, mutations and
catalog apply/export were not invoked.

### Desktop registration and background coverage

Registration source: `app/src-tauri/src/lib.rs:57`.

- Status/components: `remote_server_status`, `remote_server_components`.
- Tunnels: `start_server_tunnel`, `start_custom_tunnel`, `stop_server_tunnel`,
  `server_tunnel_status`, `stop_all_tunnels`.
- Components/lifecycle: `remote_component_log_tail`, `restart_remote_component`,
  `start_remote_battlegroup`, `stop_remote_battlegroup`,
  `restart_remote_battlegroup`.
- Discovery/logging: `detect_remote_ubuntu_servers`, `check_remote_sudo`,
  `record_operation_log`, `get_logs_folder`.
- Service controls: `install_management_service`, `uninstall_management_service`,
  `management_service_status`, `management_service_bundled_version`,
  `restart_management_service`.
- Management bridge: `ms_get_config`, `ms_set_config`, `ms_list_timezones`,
  `ms_player_location`, `ms_specialization`, `ms_set_specialization_level`,
  `ms_grant_quality_item`, `ms_item_catalog_status`, `ms_item_catalog_check`,
  `ms_item_catalog_apply`, `ms_item_catalog_revert`, `ms_item_catalog_export`,
  `write_item_catalog_export`, `ms_health`, `ms_list_runs`, `ms_list_logs`,
  `ms_trigger_run`, `ms_list_commands`, `ms_search_items`, `ms_search_vehicles`,
  `ms_search_players`, `ms_search_skill_modules`, `ms_search_journey_nodes`,
  `ms_search_xp_event_tags`, `ms_cluster`, `ms_history`, `ms_welcome_grants`,
  `ms_welcome_grant_retry`, `ms_welcome_whisper`, `ms_publish`.

The four registered tasks are `restart-notice`, `restart`, `welcome-package`,
and `welcome-message`. Config saves, catalogue apply/revert, delivery retry,
service controls, task triggers and all publishes are mutation-capable; their
presence in the matrix is not permission to execute them.

## Local checks performed

Tests were reviewed before running; they use mock runners, parser fixtures or
temporary local SQLite/files, not live profiles. Builds/tests used low
concurrency; no dependency installation/download was performed.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --workspace --offline -j 1` | Passed |
| `cargo test --workspace --offline -j 1 -- --test-threads=1` | Passed: 191 tests (23 desktop, 96 core, 72 service); zero failed/ignored |
| `cargo doc -p dune-manager-core --no-deps --offline -j 1` | Passed with four existing broken-link warnings |
| `npm run build` in `app` | Passed; existing >500 kB chunk-size warning |
| Tauri release/no-bundle build | Deferred to avoid unnecessary shared-host compile load for an audit with no software changes |

Rustdoc warnings reference `guest_bootstrap_ssh/mod.rs` links for provider,
world_creation, image_patching and scripts. These are documentation issues,
not evidence of gameplay failure. Existing tests cannot validate the new
production schema or actual game-command effects.

## Proposed repair order - separate approval required

1. **Small app-side safety patch:** require explicitly Offline for graded and
   specialization writes; reject invalid/ambiguous targets and checked-decode
   failures. Add LoggingOut/unknown/null fixtures. Do not change Funcom tables.
2. **Lifecycle reliability patch:** reconcile desktop command/inactivity/overall
   budgets with vendor waits; distinguish safe reads from mutation retries;
   handle missing exit status/uncertain completion without replay. Keep the
   existing restart workflow and short status-read budgets.
3. **SSH identity and accurate health:** design trust-on-first-use/pinned keys
   with explicit reinstall/key-change handling; fix missing-service/stale-action
   and explicit target/custom-port handling. These need deliberate UX review,
   not a silent breaking trust change.
4. **Direct-write/gameplay validation:** use an isolated server/database copy
   and version-matched files to establish inventory capacity, stats, grade/stack,
   character selection and login/logout races. Harden SQL timeouts and welcome
   uncertain-outcome accounting. No vendor schema migration is proposed here.
5. **Catalog and command validation:** resolve duplicate execution metadata,
   compare packaged templates/names/gradeability/stacks, then test every command
   family on the isolated target. Only reviewed evidence should alter catalog
   rows; don't treat an uninstantiated item as removed.

Recommended precaution until reviewed: avoid graded-item/specialization writes
while a player is logging out; this audit does not certify direct DB grants as
safe merely because their SQL columns still exist. Automation settings were not changed. No production setting was changed to
implement this advice.

### What remains unverified, and why

- No current executable/package extraction or broad item-instance survey was
  performed. Neither is appropriate for this live audit's load/privacy limits.
  New/removed items, gradeability, exact stack limits and command handlers need
  separately approved artifact acquisition followed by local analysis.
- No actual item grant, specialization edit, vehicle spawn, teleport, cleanup,
  reset, kick, broadcast, login race, restart/stop/start or delivery retry was
  executed. These require separately approved isolated/offline tests.
- No live player-search/position query was executed: schema matches do not
  establish decryption behavior, coordinate validity, character multiplicity,
  template/class strings or inventory ownership in real rows.
- Startup/degraded/stopped states, optional Metrics API and all selector/tunnel
  variants were not induced. Existing tests cover fixtures, not every vendor
  runtime possibility. An empty task ledger cannot substantiate reliability.
- No before/after schema diff can establish exactly which patch introduced the
  observed fields without a comparable historical snapshot.

### Reusable next-patch checklist

1. Re-read the saved safety plan; obtain current target/authorization; preserve
   trusted host checks and abort on pressure/timeouts.
2. Record current game image/digest, operators, service/catalog, initial health.
3. Compare reviewed metadata contracts under the same short read-only limits;
   rediscover views/types/keys before considering bounded instance checks.
4. Compare sanitized Kubernetes/resource/wrapper contracts; do not execute actions.
5. Analyze version-matched artifacts locally if safely available. Record missing
   evidence rather than expanding production load.
6. Run safe local tests; record final health, per-feature dispositions and a
   proposed minimal patch. Stop for approval before fixes/deployment/game tests.

## Version / SQL / deployment notice

Documentation-only output. No version bump or user-facing release notes required.
No data-changing SQL or migration is required, and the user has nothing to run.

Read-only SQL files **already executed**, in this order:

1. `docs/audit-evidence/2026-09-17/01-schema-metadata.sql`
2. `docs/audit-evidence/2026-09-17/02-schema-contracts.sql`
3. `docs/audit-evidence/2026-09-17/03-view-and-function-contracts.sql`
4. `docs/audit-evidence/2026-09-17/04-view-filter.sql`
5. `docs/audit-evidence/2026-09-17/05-migration-name-sample.sql`

Execution location: existing `/usr/local/bin/psql` inside the `database`
container of `[database pod omitted]`, namespace
`[namespace omitted]`, on `[VM address omitted]`; database `dune`,
loopback PostgreSQL port15432. Existing environment credentials were used without
displaying or storing them. Each script uses READ ONLY + rollback; PGOPTIONS
also enforces default read-only mode and the timeouts. These are inspection
scripts, **not migrations or instructions to modify the vendor database**.

The existing management service performed small read-only SQLite lookups for
its GET summaries. No separate Store/service was started and no SQLite schema
or task ledger was modified by the audit.

No deployment, PR merge, management-service update, post-merge action or runbook
execution is required. The authorized audit stops here for user review; none
of the proposed repairs has been implemented or shipped.
