-- Metadata only: confirm the view's state filter without fetching player rows.
BEGIN TRANSACTION READ ONLY;
SET LOCAL statement_timeout = '5s';
SET LOCAL lock_timeout = '1s';
SET LOCAL idle_in_transaction_session_timeout = '5s';
SELECT 'player_state_filters_active',
       position($literal$character_state = 'Active'::dune.characterstate$literal$
         in pg_catalog.pg_get_viewdef('dune.player_state'::regclass, true)) > 0;
ROLLBACK;
