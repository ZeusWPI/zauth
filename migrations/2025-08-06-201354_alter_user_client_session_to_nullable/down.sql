-- This file should undo anything in `up.sql`

ALTER TABLE sessions
  ALTER COLUMN user_id ADD NOT NULL;
