-- Your SQL goes here
ALTER TABLE users ADD COLUMN pending_email VARCHAR(255);
ALTER TABLE users ADD COLUMN pending_email_token VARCHAR(255) UNIQUE;
ALTER TABLE users ADD COLUMN pending_email_expiry TIMESTAMP;

CREATE INDEX ix_users_email_token ON users (pending_email_token);
