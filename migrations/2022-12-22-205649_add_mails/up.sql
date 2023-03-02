-- Your SQL goes here
CREATE TABLE mails
(
	id      SERIAL PRIMARY KEY,
	sent_on TIMESTAMP NOT NULL DEFAULT NOW(),
	subject TEXT NOT NULL,
	body    TEXT NOT NULL
);
