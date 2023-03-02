-- Your SQL goes here
ALTER TABLE mails ADD COLUMN author VARCHAR(255) REFERENCES users(username) NOT NULL;