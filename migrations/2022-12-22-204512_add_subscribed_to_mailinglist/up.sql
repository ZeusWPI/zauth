-- Your SQL goes here
ALTER TABLE users ADD COLUMN subscribed_to_mailing_list BOOLEAN NOT NULL DEFAULT TRUE;

CREATE INDEX ix_users_subscribed_to_mailing_list ON users (subscribed_to_mailing_list);
