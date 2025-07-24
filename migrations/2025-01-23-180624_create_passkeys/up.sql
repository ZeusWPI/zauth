-- Your SQL goes here
CREATE TABLE passkeys (
  id          SERIAL PRIMARY KEY,
  user_id     INTEGER      NOT NULL REFERENCES users(id),
  name        VARCHAR(255) NOT NULL,
  cred        VARCHAR      NOT NULL,
  cred_id     VARCHAR      NOT NULL,
  last_used   TIMESTAMP    NOT NULL,
  created_at  TIMESTAMP    NOT NULL DEFAULT NOW()
);

CREATE INDEX ix_passkeys_cred_id ON passkeys (cred_id);
