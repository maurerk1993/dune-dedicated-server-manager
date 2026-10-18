-- Five names via the verified unique btree index on applied_patches(name).
-- Lexical order is NOT migration execution order; no COUNT/full table scan.
BEGIN TRANSACTION READ ONLY;
SET LOCAL statement_timeout = '5s';
SET LOCAL lock_timeout = '1s';
SET LOCAL idle_in_transaction_session_timeout = '5s';
SELECT 'migration_name_lexical_sample', name
FROM dune.applied_patches ORDER BY name DESC LIMIT 5;
ROLLBACK;
