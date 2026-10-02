ALTER TABLE roles ADD COLUMN client_id INTEGER REFERENCES clients(id) DEFAULT NULL;

UPDATE roles SET client_id = (
    SELECT client_id FROM roles_limited_to_clients
        WHERE roles_limited_to_clients.role_id = roles.id
);
TRUNCATE TABLE roles_limited_to_clients;

UPDATE roles SET visibility = 'global';
