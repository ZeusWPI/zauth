-- This file should undo anything in `up.sql`
ALTER TABLE users ALTER COLUMN email SET NOT NULL;
ALTER TABLE users ALTER COLUMN state SET DEFAULT 'pending_approval';
