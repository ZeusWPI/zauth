CREATE TYPE role_visibility AS ENUM ('global', 'limited');
ALTER TABLE roles ADD COLUMN visibility role_visibility DEFAULT 'global' NOT NULL;
