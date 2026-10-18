# Whole-App Dune Patch Compatibility Audit Plan

Agreed scope and safeguards: September 17, 2026.

**Status: authorized low-impact audit completed September 17, 2026 against
[VM address omitted] after the user corrected the reinstalled VM address. Local
review/tests and serialized read-only production checks are complete. Gameplay
tests and unavailable version-matched content remain explicitly unverified.
See `game-patch-compatibility-audit-report.md` and `audit-evidence/2026-09-17/`.
No app/server changes or deployment; stopped for the user's repair-plan review.**

## Authorization and outcome

When the user explicitly says to begin (for example, "let's get the audit done"),
perform the investigation under this plan. The target is a **live production
server with people actively playing**, not an empty test server. This supersedes
earlier suggestions to assume a maintenance window.

Audit every existing manager feature against the installed game build. Produce
evidence, a feature-by-feature compatibility matrix, and a prioritized repair
plan. Then stop for the user's review. Audit authorization does not authorize
fixes, deployments, game commands, write tests, or production configuration
changes. If any requirement cannot be satisfied safely, defer that check rather
than weakening these safeguards.

## Mandatory live-production safeguards

Server availability, player progress, and performance take priority over audit
completion. Read-only work can still create load; this is a low-impact audit,
not a promise of zero risk.

- No database writes, migrations, schema changes, administrative functions,
  game-command publishing, spawn tests, task triggers, retries of deliveries,
  service changes, restarts, installations, or changes to Funcom's files.
- Run only one production check at a time, including across any subagents. Local
  code review can be parallelized; production probes must remain serialized.
- Use dedicated read-only database transactions with a **5-second statement
  timeout**, **1-second lock timeout**, and a short idle-transaction timeout.
  End every transaction with rollback and close the session on completion or
  failure. These limits replace the older 20-second general-survey default for
  this audit; do not lengthen them to force a check through.
- Inspect metadata first. Allow only reviewed, bounded reads with a known
  inexpensive access path. No full-table scans, unrestricted joins/aggregates,
  bulk exports, locking reads, or `EXPLAIN ANALYZE`. A result limit alone does
  not establish that a query is inexpensive. Defer checks whose cost is unclear.
- Do not invoke a database function merely because it appears inside a SELECT;
  inspect its definition and dependencies first. Never invoke game mutation
  functions as an audit probe.
- No executable scanning, package/archive extraction, large file transfers,
  broad filesystem crawls, or large-file hashing on the production VM. Analyze
  existing local copies. Small, identified metadata/script reads are permissible
  within the same load limits; defer missing large artifacts to separately
  approved acquisition or an isolated environment.
- Do not launch the manager or test harnesses against live profiles where they
  could start automatic refresh loops, trigger background activity, or mutate
  state. Review a tool's behavior before trusting its "dry-run" mode or a GET
  endpoint. Do not launch an additional management service against production.
- Use existing trusted SSH identity/key handling; never bypass host-key checks.
  Discover the current target, namespace, and relevant resources after
  authorization rather than assuming historical names or addresses are current.
- Keep all credentials, tokens, player/account identifiers, character names,
  inventory IDs, and raw player-owned data out of tool output and reports.
  Return sanitized structural shapes, template metadata, or safe aggregates.
- Use minimal existing health signals before and between small batches. On
  contention, timeouts, unexpected resource pressure, player-impact reports, or
  degraded health, stop further probes, close the audit session, and report.
  No tight polling, repeated retries, automatic repair, or schedule changes.
- Mark unsafe or inconclusive checks **requires separately approved offline or
  isolated testing**. Do not claim a feature passed simply because it was not
  safe to test.

## Full feature coverage

Derive the inventory from all UI tabs/actions, registered desktop commands,
management-service routes, scheduled/background tasks, and reachable core/CLI
operations. Do not rely only on the examples in this table. Record inactive,
CLI-only, and removed surfaces separately; never re-enable removed server-update
or backup features as part of the audit.

| Feature area | Required coverage |
| --- | --- |
| Dashboard, server list, Maps, Pods | Player totals, map/partition/dimension joins, startup states, readiness, host/pod metrics, service identification, completed jobs, logs, stale-data behavior, refresh failures and recovery |
| Users and targeting | Account/character relationships, player search, online/offline status, ownership, inventories, positions and dimensions |
| Every admin command | Ordinary and graded item grants, vehicles, teleportation, inventory operations, water, XP, skills, specialization, broadcasts, kicks, progression resets and exposed raw commands |
| Catalogs and pickers | Items, display names, grades versus Mk tiers, stack limits, vehicles, skill modules, XP tags, retained Journey data, bundled catalog and active overrides |
| Welcome packages/messages | Eligibility, item delivery, player/chat targeting, duplicate prevention, failure tracking, retained settings and retry assumptions |
| Lifecycle and automation | Manual and scheduled start/stop/restart paths, warnings, timezone assumptions, vendor-script arguments/prompts, battlegroup selection and readiness waits |
| Supporting functionality | Server discovery/attachment/profiles, SSH/sudo, tunnels/service ports, management-service controls/version compatibility, settings, history, logging/export and app updater |
| Core/CLI-only and inactive surfaces | Identify game-facing operations such as setup/bootstrap, instance/region controls or legacy commands; distinguish them from active desktop functionality and inspect contracts without executing them |

For each feature trace UI -> desktop bridge -> service/core -> its actual game,
database, Kubernetes, messaging, vendor-script, filesystem, or local-only
dependencies. Include indirect failures, such as a broken player picker blocking
an otherwise unchanged game command. A feature with no game dependency still
receives an explicit applicability assessment.

## Investigation sequence

1. **Local preflight and coverage inventory.** Re-read this plan, inventory the
   current code, classify every proposed check as local/read-only/unsafe, and
   review its cost and side effects. Do not execute write-capable app workflows.
2. **Identify deployed versions.** After explicit authorization, collect only
   small read-only evidence: installed game image tag/digest, operator versions,
   desktop/service version relationship, schema metadata, migration summary,
   and active catalog status. Compute schema fingerprints locally. Do not
   confuse a public release date with proof of the installed build.
3. **Survey schema structure broadly.** Inspect game tables/views, columns,
   types, relationships, defaults, constraints, indexes, triggers, and function
   signatures. Compare with the app's expectations and any available pre-patch
   artifacts. If no historical snapshot exists, explicitly limit claims about
   what changed. Do not scan every table's data.
4. **Follow moved or restructured data.** Treat the reported `dune.actor_state`
   -> `dune.actors.state`, `m.marker`/coordinate-to-JSON, and Journey changes as
   unverified leads. Establish what each name represents and locate the actual
   source via metadata/migrations and bounded structural samples. Do not assume
   the replacement type, JSON path, or meaning from its name.
5. **Validate semantics and app contracts.** Review actual SELECT statements and
   safely validate only those meeting the production limits. Check identity,
   ownership, cardinality, coordinate axes/units/origin, map/dimension matching,
   inventory rules, status values, missing/null fields and nested JSON types.
   Inspect insert/update/function requirements without executing them. Flag
   silent wrong results and unsafe defaults, not only SQL errors or crashes.
6. **Inspect available game files locally.** Review version-matched migration
   definitions, executable evidence, packaged definitions, and vendor scripts.
   Compare item/vehicle metadata, grading, stack limits, inventory rules, and
   command formats with both bundled and active catalogs. Binary strings are
   clues, not sufficient proof. Missing local artifacts remain a documented
   limitation until safe acquisition is separately approved.
7. **Check non-database dependencies.** Inspect the Kubernetes resource shapes,
   labels, selectors, service endpoints/ports, RabbitMQ payload contracts,
   vendor command arguments/prompts and output formats the manager relies on.
   Check lifecycle targets and readiness parsing without invoking lifecycle
   actions. Review existing bounded, sanitized automation errors without
   clearing ledgers, triggering retries, or changing schedules.
8. **Local validation and report.** Review regression scenarios and run safe
   existing native checks only after confirming they cannot reach production.
   Do not change app code to conduct this first phase. Record unavailable tools,
   skipped tests, and coverage limits rather than relaxing safety constraints.

## Known local-code observations, not confirmed production findings

The planning review found `dune.actors` joins for player selection, position, and
backpack resolution. It found no direct `dune.actor_state` reference. The code
expects top-level actor fields including transform, class, partition and
dimension information; moving those into another representation could matter.

Ordinary grants and vehicle spawns publish game commands. Graded grants and
welcome packages also depend on direct database operations. Journey commands
were already disabled, and the desktop Maps page reads Kubernetes resources,
not a general mapmarker table. Recheck these observations against the current
revision before drawing conclusions; they do not narrow the whole-app scope.

Inventory instances are not a master catalog: absence does not prove removal,
quality zero does not prove non-gradeability, and observed maximum stacks do
not establish true limits. Preserve catalog duplicate/conflict visibility.

## Validation cases and evidence standard

- Valid current data, missing/renamed fields, nested JSON, changed types,
  missing/null values, unknown statuses and incompatible actor structures.
- Correct player/inventory ownership and correct partition/dimension mapping;
  no interpretation of absent coordinates as valid zero coordinates.
- Unavailable dependencies, partial/stale status, timeouts, missing metrics and
  empty results without false healthy/success indications or refresh hangs.
- New or changed item/vehicle/grade/stack metadata and catalog overrides.
- Indirect effects on automated tasks, player selection and readiness logic.

Read-only query success is not proof of gameplay correctness. RabbitMQ accepting
a payload is not proof that the game applied it. Real grants, teleports,
progression changes, lifecycle actions, and other writes remain unverified
until separately authorized isolated or offline testing.

## Deliverables and stop point

Produce a sanitized report (suggested destination:
`docs/game-patch-compatibility-audit-report.md`) with:

- The build/revision and evidence baseline, plus checks actually performed.
- A coverage matrix: feature, entry points, dependencies, evidence, result,
  player/data risk, severity, proposed fix, and remaining verification.
- A disposition for every feature: read behavior verified; structural review
  only; confirmed incompatible; no game dependency; or unverified, with reason.
- A prioritized minimal app-side repair plan, including safe compatibility
  errors and handling of unknown statuses/coordinate formats where warranted.
- A separate list of tests requiring new authorization and a reusable checklist
  for future patches. No blanket "fully compatible" verdict while relevant
  gameplay behavior remains unverified.

Then stop for the user's review. Do not fix, deploy, replay failed deliveries,
disable automation, update catalogs, or alter the server as an audit follow-up.

## Version, SQL, and deployment notices

This documentation and audit phase require no app version bump, release notes,
deployment, database migration, or data-changing SQL. The later audit uses only
reviewed read-only inspection SQL; document any such script's exact path and
execution scope when it is prepared. Any implementation, production change, or
in-game verification belongs to a separately approved phase with the usual
versioning, SQL disclosure, testing, and deployment requirements.
