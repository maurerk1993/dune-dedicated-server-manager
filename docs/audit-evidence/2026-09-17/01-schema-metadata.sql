-- Production audit: metadata only. Run with psql -X -v ON_ERROR_STOP=1.
-- PGOPTIONS must additionally enforce default_transaction_read_only=on,
-- statement_timeout=5000, lock_timeout=1000, idle_in_transaction_session_timeout=5000.
BEGIN TRANSACTION READ ONLY;
SET LOCAL statement_timeout = '5s';
SET LOCAL lock_timeout = '1s';
SET LOCAL idle_in_transaction_session_timeout = '5s';

SELECT 'database_version', current_setting('server_version');
SELECT 'game_relation', c.relname, c.relkind
FROM pg_catalog.pg_class c
JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
WHERE n.nspname = 'dune' AND c.relkind IN ('r', 'p', 'v', 'm')
ORDER BY c.relname;

SELECT 'column_shape', c.relname,
       string_agg(a.attname || ':' || pg_catalog.format_type(a.atttypid, a.atttypmod)
                  || CASE WHEN a.attnotnull THEN ':notnull' ELSE ':nullable' END,
                  ', ' ORDER BY a.attnum)
FROM pg_catalog.pg_attribute a
JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
WHERE n.nspname = 'dune' AND c.relkind IN ('r', 'p', 'v', 'm')
  AND a.attnum > 0 AND NOT a.attisdropped
  AND (c.relname IN ('actors', 'actor_state', 'player_state', 'accounts',
                    'encrypted_accounts', 'inventories', 'items',
                    'specialization_tracks', 'specialization_purchased_keystones',
                    'world_partition', 'world_partitions', 'applied_patches')
       OR c.relname LIKE '%marker%' OR c.relname LIKE '%journey%')
GROUP BY c.relname ORDER BY c.relname;

SELECT 'column_metadata_fingerprint_md5',
       md5(string_agg(c.relname || ':' || a.attnum::text || ':' || a.attname || ':'
                     || pg_catalog.format_type(a.atttypid, a.atttypmod) || ':'
                     || a.attnotnull::text, E'\n' ORDER BY c.relname, a.attnum))
FROM pg_catalog.pg_attribute a
JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
WHERE n.nspname = 'dune' AND c.relkind IN ('r', 'p', 'v', 'm')
  AND a.attnum > 0 AND NOT a.attisdropped;
ROLLBACK;
