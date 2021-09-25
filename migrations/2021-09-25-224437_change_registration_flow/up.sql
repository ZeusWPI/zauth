-- Your SQL goes here
ALTER TABLE users ALTER COLUMN email DROP NOT NULL;
ALTER TABLE users ALTER COLUMN state SET DEFAULT 'pending_mail_confirmation';
