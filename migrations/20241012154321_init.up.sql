-- Add up migration script here

CREATE TABLE IF NOT EXISTS sensor (
	id UUID DEFAULT gen_random_uuid() PRIMARY KEY,
	name VARCHAR(50) UNIQUE NOT NULL,
	tbl_name VARCHAR(50) NOT NULL,
	longitude FLOAT8,
	latitude FLOAT8,
	publisher VARCHAR(255) NOT NULL DEFAULT '*',
	consumer VARCHAR(255) NOT NULL DEFAULT '*'
);

CREATE TABLE IF NOT EXISTS sensor_schema (
	sensor_id UUID REFERENCES sensor(id),
	col_name VARCHAR(50) NOT NULL,
	col_type INT NOT NULL,
	col_unit VARCHAR(10),
	PRIMARY KEY(sensor_id, col_name)
);

CREATE TABLE IF NOT EXISTS users (
    id UUID NOT NULL PRIMARY KEY DEFAULT (gen_random_uuid()),
    name VARCHAR(100) NOT NULL,
    email VARCHAR(255) NOT NULL UNIQUE,
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    password VARCHAR(100) NOT NULL,
    role VARCHAR(50) NOT NULL DEFAULT 'user'
);

CREATE INDEX IF NOT EXISTS users_email_idx ON users (email);