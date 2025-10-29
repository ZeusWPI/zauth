-- This file should undo anything in `up.sql`
ALTER TABLE mails
ADD CONSTRAINT mails_author_fkey;
FOREIGN KEY (author) REFERENCES users(username)
