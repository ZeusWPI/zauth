-- Your SQL goes here

ALTER TABLE sessions
    DROP CONSTRAINT sessions_client_id_fkey,
    ADD CONSTRAINT sessions_client_id_fkey
        FOREIGN KEY (client_id)
        REFERENCES clients(id)
        ON DELETE CASCADE;
