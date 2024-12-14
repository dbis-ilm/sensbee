-- Sets up mandatory system roles
INSERT INTO roles(name, system) VALUES('Guest', true) ON CONFLICT DO NOTHING;