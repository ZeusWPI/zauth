-- Your SQL goes here
CREATE EXTENSION IF NOT EXISTS pgcrypto;
ALTER TABLE users ADD COLUMN unsubscribe_token VARCHAR(32) NOT NULL UNIQUE DEFAULT translate(encode(gen_random_bytes(24), 'base64'), '+/=', '-_');

CREATE INDEX ix_users_unsubscribe_token ON users (unsubscribe_token);
