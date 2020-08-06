CREATE TABLE users
(
    id               SERIAL PRIMARY KEY,
    username         VARCHAR(255) NOT NULL UNIQUE,
    hashed_password  VARCHAR(255) NOT NULL,
    admin            BOOLEAN      NOT NULL DEFAULT false,
    firstname        VARCHAR(255) NOT NULL,
    lastname         VARCHAR(255) NOT NULL,
    email            VARCHAR(255) NOT NULL, -- TO BE OR NOT TE UNIQUE?
    ssh_key          TEXT,
    last_accessed_at DATETIME     NOT NULL,
    created_at       DATETIME     NOT NULL DEFAULT NOW()
);

CREATE INDEX ix_user_username ON user (username);
CREATE INDEX ix_user_email ON user (email);


CREATE TABLE clients
(
    id                SERIAL PRIMARY KEY,
    name              VARCHAR(255) NOT NULL UNIQUE,
    secret            VARCHAR(255) NOT NULL,
    needs_grant       BOOLEAN      NOT NULL DEFAULT false,
    redirect_uri_list TEXT         NOT NULL DEFAULT ''
);
