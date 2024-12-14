-- Add up migration script here

ALTER TABLE sensor DROP COLUMN IF EXISTS publisher;
ALTER TABLE sensor DROP COLUMN IF EXISTS consumer;

ALTER TABLE users DROP COLUMN IF EXISTS role;

-- May be null for "system" sensors that belong to no one...
ALTER TABLE sensor ADD COLUMN IF NOT EXISTS owner UUID REFERENCES users(id);

CREATE TABLE IF NOT EXISTS roles (
    id SERIAL PRIMARY KEY,
    name VARCHAR(50) UNIQUE NOT NULL,
    system BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE IF NOT EXISTS user_roles (
    user_id UUID REFERENCES users(id),
    role_id INTEGER REFERENCES roles(id),
    PRIMARY KEY(user_id, role_id)
);

CREATE TABLE IF NOT EXISTS sensor_permissions (
    sensor_id UUID REFERENCES sensor(id),
    role_id INTEGER REFERENCES roles(id),
    allow_info BOOLEAN NOT NULL DEFAULT false,
    allow_read BOOLEAN NOT NULL DEFAULT false,
    allow_write BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY(sensor_id, role_id)
);

-- Sets up mandatory system roles
INSERT INTO roles(name, system) VALUES('Admin', true) ON CONFLICT DO NOTHING;
INSERT INTO roles(name, system) VALUES('User', true) ON CONFLICT DO NOTHING;