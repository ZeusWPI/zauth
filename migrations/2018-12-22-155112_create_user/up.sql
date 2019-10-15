-- Your SQL goes here
CREATE TABLE user (
    id INT PRIMARY KEY AUTO_INCREMENT,
    username VARCHAR(127) NOT NULL,
    password VARCHAR(127) NOT NULL,
    admin    BOOLEAN NOT NULL DEFAULT 0
);
