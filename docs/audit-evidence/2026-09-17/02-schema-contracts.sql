-- Metadata only, same production read-only session limits as 01.
BEGIN TRANSACTION READ ONLY;
SET LOCAL statement_timeout = '5s';
SET LOCAL lock_timeout = '1s';
SET LOCAL idle_in_transaction_session_timeout = '5s';

SELECT 'type', t.typname, t.typtype,
       COALESCE(string_agg(e.enumlabel, ', ' ORDER BY e.enumsortorder), '')
FROM pg_catalog.pg_type t
JOIN pg_catalog.pg_namespace n ON n.oid = t.typnamespace
LEFT JOIN pg_catalog.pg_enum e ON e.enumtypid = t.oid
WHERE n.nspname = 'dune'
  AND t.typname IN ('actorstate','playerconnectionstatus','characterstate',
                   'playerlifestate','specializationtracktype',
                   'markerpayloadtype','transform','vector')
GROUP BY t.typname, t.typtype ORDER BY t.typname;

SELECT 'composite_field', t.typname, a.attname,
       pg_catalog.format_type(a.atttypid, a.atttypmod)
FROM pg_catalog.pg_type t
JOIN pg_catalog.pg_namespace n ON n.oid = t.typnamespace
JOIN pg_catalog.pg_attribute a ON a.attrelid = t.typrelid
WHERE n.nspname = 'dune' AND t.typname IN ('transform','vector','quaternion')
  AND a.attnum > 0 AND NOT a.attisdropped ORDER BY t.typname, a.attnum;

SELECT 'column', c.relname, a.attname,
       pg_catalog.format_type(a.atttypid, a.atttypmod), a.attnotnull,
       COALESCE(pg_catalog.pg_get_expr(d.adbin, d.adrelid), '')
FROM pg_catalog.pg_attribute a
JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
LEFT JOIN pg_catalog.pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
WHERE n.nspname = 'dune' AND c.relname IN
 ('encrypted_player_state','actor_inventories','purchased_specialization_keystones',
  'items','inventories','applied_patches')
 AND a.attnum > 0 AND NOT a.attisdropped ORDER BY c.relname,a.attnum;

SELECT 'constraint', c.relname, con.conname, con.contype,
       pg_catalog.pg_get_constraintdef(con.oid)
FROM pg_catalog.pg_constraint con
JOIN pg_catalog.pg_class c ON c.oid = con.conrelid
JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
WHERE n.nspname = 'dune' AND c.relname IN
 ('encrypted_player_state','actors','actor_inventories','items','inventories',
  'specialization_tracks','purchased_specialization_keystones',
  'journey_story_node','journey_story_node_cooldown','markers','player_markers','applied_patches')
ORDER BY c.relname,con.conname;

SELECT 'index', c.relname, pg_catalog.pg_get_indexdef(i.indexrelid)
FROM pg_catalog.pg_index i
JOIN pg_catalog.pg_class c ON c.oid=i.indrelid
JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
WHERE n.nspname='dune' AND c.relname IN
 ('encrypted_player_state','actors','actor_inventories','items','inventories',
  'specialization_tracks','purchased_specialization_keystones','applied_patches')
ORDER BY c.relname,i.indexrelid;

SELECT 'trigger', c.relname, tr.tgname, p.proname,
       pg_catalog.pg_get_triggerdef(tr.oid)
FROM pg_catalog.pg_trigger tr
JOIN pg_catalog.pg_class c ON c.oid=tr.tgrelid
JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
JOIN pg_catalog.pg_proc p ON p.oid=tr.tgfoid
WHERE n.nspname='dune' AND NOT tr.tgisinternal
  AND c.relname IN ('actors','items','inventories','encrypted_player_state')
ORDER BY c.relname,tr.tgname;

SELECT 'function_signature', p.proname,
       pg_catalog.pg_get_function_identity_arguments(p.oid),
       pg_catalog.pg_get_function_result(p.oid), p.provolatile, l.lanname
FROM pg_catalog.pg_proc p
JOIN pg_catalog.pg_namespace n ON n.oid=p.pronamespace
JOIN pg_catalog.pg_language l ON l.oid=p.prolang
WHERE n.nspname='dune' AND (p.proname='set_specialization_xp_and_level'
 OR p.proname LIKE '%actor%state%' OR p.proname LIKE '%marker%')
ORDER BY p.proname,p.oid;
ROLLBACK;
