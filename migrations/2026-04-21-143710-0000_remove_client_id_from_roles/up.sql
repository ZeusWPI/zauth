UPDATE roles SET visibility = 'global'
    WHERE client_id IS NULL;
UPDATE roles SET visibility = 'limited'
    WHERE client_id IS NOT NULL;

TRUNCATE TABLE roles_limited_to_clients;
INSERT INTO roles_limited_to_clients (role_id, client_id) (
    SELECT id, client_id FROM roles
        WHERE client_id IS NOT NULL
);

ALTER TABLE roles DROP COLUMN client_id;
