-- This file should undo anything in `up.sql`
ALTER TABLE users DROP COLUMN pending_email;
ALTER TABLE users DROP COLUMN pending_email_token;
ALTER TABLE users DROP COLUMN pending_email_expiry;
DROP INDEX ix_users_email_token;
