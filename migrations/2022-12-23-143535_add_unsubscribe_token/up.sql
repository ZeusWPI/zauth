-- Your SQL goes here
ALTER TABLE users ADD COLUMN unsubscribe_token VARCHAR(32) NOT NULL UNIQUE DEFAULT substring(md5(random()::text), 0, 32);

CREATE INDEX ix_users_unsubscribe_token ON users (unsubscribe_token);
