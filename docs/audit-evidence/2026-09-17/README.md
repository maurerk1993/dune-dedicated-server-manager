# Sanitized compatibility-audit evidence

Collected September 17, 2026 against the user-confirmed reinstalled VM at
[VM address omitted]. See `../../game-patch-compatibility-audit-report.md` for conclusions
and the exact execution scope. This directory contains evidence, not a deploy
package, database migration or request for the user to run anything.

- SQL 01-04 inspect PostgreSQL schema metadata only. SQL 05 reads five migration
  names in lexical order through a verified name index; it is not a latest-patch
  list. They ran in numerical order with enforced read-only transactions,
  5-second statement/idle and 1-second lock limits, ending in rollback.
- Corresponding outputs are 01-04 and 08. No player-row values, inventories,
  names/identifiers, credentials or raw owned JSON were exported.
- 05 records filtered management-service health/config/catalog GET results.
- 06-07 record selected Kubernetes field shapes and safe health/build/port data.
  They are concatenated JSON documents, not a single JSON configuration file.
- 09 contains **source excerpts read from the installed vendor wrapper**. None
  of the commands shown in those excerpts was executed by this audit. The file
  was 31,689 bytes, resolved from the existing battlegroup symlink; no executable
  or package scan/extraction was performed.
- 10 records an empty `GET /api/runs?limit=5` result. The PropertyNames field is
  a PowerShell empty-array artifact, not fields in a returned task record.
- 11 is the final minimal health snapshot. Completed jobs have false readiness
  because their containers exited; they are not failing running services.

All VM probes were serialized by the primary agent using trusted native SSH.
Subagents reviewed local code only. No app/game mutation, game command, restart,
backup, server update, deployment or configuration save was run. Normal service
request/access logging is outside the intended read-only data operations.

The current evidence does not replace isolated gameplay tests. Future audits
must rediscover resources/types and review query cost before reusing any SQL;
do not copy these resource names or privileges as permanent assumptions.

## Public-copy redactions

Host addresses, resource identifiers, automation settings, assigned network
ports and exact host-health measurements have been omitted. Files 05, 07 and
11 are reduced public summaries, not raw outputs. File 06 retains field/type
shapes only. Vendor schema definitions and dated compatibility findings remain.
Do not interpret placeholders as executable configuration or live status.
