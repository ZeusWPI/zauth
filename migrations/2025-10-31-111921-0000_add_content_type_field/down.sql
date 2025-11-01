-- This file should undo anything in `up.sql`
ALTER TABLE mails DROP COLUMN content_type;
DROP TYPE content_type;
