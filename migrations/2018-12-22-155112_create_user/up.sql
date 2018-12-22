-- Your SQL goes here
CREATE TABLE user (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username VARCHAR NOT NULL,
    password VARCHAR NOT NULL,
    admin    BOOLEAN NOT NULL DEFAULT 0
);

INSERT INTO user (username, password) VALUES ("rien", "rien");
INSERT INTO user (username, password, admin) VALUES ("admin",  "admin", 1);
