-- This file should undo anything in `up.sql`

ALTER TABLE sessions
  ALTER COLUMN user_id SET NOT NULL;
