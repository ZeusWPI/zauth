-- Your SQL goes here
CREATE TABLE clients_roles (
  client_id INTEGER REFERENCES clients(id) ON DELETE CASCADE,
  role_id INTEGER REFERENCES roles(id) ON DELETE CASCADE,
  PRIMARY KEY (client_id, role_id)
)
