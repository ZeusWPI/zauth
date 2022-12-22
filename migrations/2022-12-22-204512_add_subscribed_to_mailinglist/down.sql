-- This file should undo anything in `up.sql`
ALTER TABLE users DROP COLUMN subscribed_to_mailing_list;

DROP INDEX ix_users_subscribed_to_mailing_list;
