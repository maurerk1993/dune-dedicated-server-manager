-- Metadata inspection only. Do not execute any of the described game functions.
BEGIN TRANSACTION READ ONLY;
SET LOCAL statement_timeout = '5s';
SET LOCAL lock_timeout = '1s';
SET LOCAL idle_in_transaction_session_timeout = '5s';

-- Redact string literals in view definitions; never fetch the decryption function body.
SELECT 'view_definition', c.relname,
       regexp_replace(pg_catalog.pg_get_viewdef(c.oid, true),
                      $pattern$'([^']|'')*'$pattern$, '''[literal]''', 'g')
FROM pg_catalog.pg_class c
JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
WHERE n.nspname='dune' AND c.relkind='v' AND c.relname IN ('player_state','accounts');

-- These small game-contract definitions are read as text, never invoked.
SELECT 'function_definition', p.proname, pg_catalog.pg_get_functiondef(p.oid)
FROM pg_catalog.pg_proc p
JOIN pg_catalog.pg_namespace n ON n.oid=p.pronamespace
WHERE n.nspname='dune' AND p.proname IN
 ('get_actor_state','check_actor_state','load_markers','set_specialization_xp_and_level')
ORDER BY p.proname,p.oid;

SELECT 'marker_composite_field', t.typname, a.attname,
       pg_catalog.format_type(a.atttypid,a.atttypmod)
FROM pg_catalog.pg_type t
JOIN pg_catalog.pg_namespace n ON n.oid=t.typnamespace
JOIN pg_catalog.pg_attribute a ON a.attrelid=t.typrelid
WHERE n.nspname='dune' AND t.typname IN
 ('mapmarker','mapmarkersharedsave','mapmarkerplayersave','mapmarkerplayerdelete')
 AND a.attnum>0 AND NOT a.attisdropped ORDER BY t.typname,a.attnum;
ROLLBACK;
