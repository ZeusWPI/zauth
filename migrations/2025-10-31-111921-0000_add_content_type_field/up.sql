-- Your SQL goes here
CREATE TYPE content_type AS ENUM ('text/plain', 'text/markdown');

ALTER TABLE mails ADD COLUMN content_type content_type DEFAULT 'text/plain' NOT NULL;
