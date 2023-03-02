-- This file should undo anything in `up.sql`
ALTER TABLE users DROP COLUMN unsubscribe_token;

DROP INDEX ix_users_unsubscribe_token;
