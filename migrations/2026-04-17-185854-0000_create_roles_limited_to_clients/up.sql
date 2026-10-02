CREATE TABLE roles_limited_to_clients (
  role_id   INTEGER REFERENCES roles(id)   ON DELETE CASCADE,
  client_id INTEGER REFERENCES clients(id) ON DELETE CASCADE,
  PRIMARY KEY (role_id, client_id)
);
