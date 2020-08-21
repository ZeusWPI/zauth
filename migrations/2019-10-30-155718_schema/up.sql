CREATE TABLE users
(
    id              SERIAL PRIMARY KEY,
    username        VARCHAR(255) NOT NULL UNIQUE,
    hashed_password VARCHAR(255) NOT NULL,
    admin           BOOLEAN      NOT NULL DEFAULT false,
    first_name      VARCHAR(255) NOT NULL,
    last_name       VARCHAR(255) NOT NULL,
    email           VARCHAR(255) NOT NULL UNIQUE,
    ssh_key         TEXT,
    last_login      TIMESTAMP    NOT NULL,
    created_at      TIMESTAMP    NOT NULL DEFAULT NOW()
);

CREATE INDEX ix_users_username ON users (username);
CREATE INDEX ix_users_email ON users (email);


CREATE TABLE clients
(
    id                SERIAL PRIMARY KEY,
    name              VARCHAR(255) NOT NULL UNIQUE,
    secret            VARCHAR(255) NOT NULL,
    needs_grant       BOOLEAN      NOT NULL DEFAULT false,
    redirect_uri_list TEXT         NOT NULL DEFAULT '',
    created_at        TIMESTAMP    NOT NULL DEFAULT NOW()
);
