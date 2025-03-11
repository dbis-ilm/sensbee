
DROP TABLE sensor_permissions;
DROP TABLE user_roles;
DROP TABLE roles;

CREATE TABLE roles (
    id UUID PRIMARY KEY,
    name VARCHAR(50) UNIQUE NOT NULL,
    system BOOLEAN NOT NULL DEFAULT false
);

-- Sets up mandatory system roles
INSERT INTO roles(id, name, system) VALUES('0e804d35-c8e3-49ee-86d4-3e556a82a1af','Admin', true) ON CONFLICT DO NOTHING;
INSERT INTO roles(id, name, system) VALUES('72122092-1154-4189-8dde-d72b663b55eb','User', true) ON CONFLICT DO NOTHING;
INSERT INTO roles(id, name, system) VALUES('51fd9bb7-3214-4089-adb9-474eb82b447a','Guest', true) ON CONFLICT DO NOTHING;

CREATE TABLE sensor_permissions (
    sensor_id UUID REFERENCES sensor(id),
    role_id UUID REFERENCES roles(id),
    allow_info BOOLEAN NOT NULL DEFAULT false,
    allow_read BOOLEAN NOT NULL DEFAULT false,
    allow_write BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY(sensor_id, role_id)
);

CREATE TABLE user_roles (
    user_id UUID REFERENCES users(id),
    role_id UUID REFERENCES roles(id),
    PRIMARY KEY(user_id, role_id)
);
