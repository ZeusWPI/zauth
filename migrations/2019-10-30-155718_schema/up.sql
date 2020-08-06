CREATE TABLE users (
    id       SERIAL PRIMARY KEY,
    username VARCHAR(255) NOT NULL UNIQUE,
    password VARCHAR(255) NOT NULL,
    admin    BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE clients (
    id                  SERIAL PRIMARY KEY,
    name                VARCHAR(255) NOT NULL UNIQUE,
    secret              VARCHAR(255) NOT NULL,
    needs_grant         BOOLEAN NOT NULL DEFAULT false,
    redirect_uri_list   TEXT NOT NULL DEFAULT ''
);
