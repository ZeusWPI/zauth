CREATE TYPE user_state AS ENUM ('pending_approval', 'pending_mail_confirmation', 'active', 'disabled');

CREATE TABLE users
(
    id                      SERIAL PRIMARY KEY,
    username                VARCHAR(255) NOT NULL UNIQUE,
    hashed_password         VARCHAR(255) NOT NULL,
    password_reset_token    VARCHAR(255) UNIQUE,
    password_reset_expiry   TIMESTAMP,
    admin                   BOOLEAN      NOT NULL DEFAULT false,
    full_name               VARCHAR(255) NOT NULL,
    email                   VARCHAR(255) NOT NULL UNIQUE,
    ssh_key                 TEXT,
    state                   user_state   NOT NULL DEFAULT 'pending_approval',
    last_login              TIMESTAMP    NOT NULL,
    created_at              TIMESTAMP    NOT NULL DEFAULT NOW()
);

CREATE INDEX ix_users_password_reset ON users (password_reset_token);
CREATE INDEX ix_users_username ON users (username);
CREATE INDEX ix_users_email ON users (email);


CREATE TABLE clients
(
    id                SERIAL PRIMARY KEY,
    name              VARCHAR(255)  NOT NULL UNIQUE,
    description       TEXT          NOT NULL DEFAULT '',
    secret            VARCHAR(255)  NOT NULL,
    needs_grant       BOOLEAN       NOT NULL DEFAULT false,
    redirect_uri_list TEXT          NOT NULL DEFAULT '',
    created_at        TIMESTAMP     NOT NULL DEFAULT NOW()
);

CREATE VIEW postfix_view AS
  SELECT username, email
  FROM users
  WHERE state = 'active';
