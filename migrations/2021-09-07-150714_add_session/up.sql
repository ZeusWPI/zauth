-- Your SQL goes here

CREATE TABLE sessions
(
    id          SERIAL PRIMARY KEY                      ,
    key         VARCHAR(255)                    UNIQUE  ,
    user_id     integer REFERENCES users(id)    NOT NULL,
    client_id   integer REFERENCES clients(id)          ,
    created_at  TIMESTAMP                       NOT NULL,
    expires_at  TIMESTAMP                       NOT NULL,
    valid       BOOLEAN                         NOT NULL DEFAULT true,
    scope       TEXT
);

CREATE INDEX ix_sessions_key ON sessions (key);
