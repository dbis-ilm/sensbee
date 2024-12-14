-- Add up migration script here

CREATE TABLE IF NOT EXISTS api_keys (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(id) NOT NULL,
    sensor_id UUID REFERENCES sensor(id) NOT NULL,
    name VARCHAR(255) NOT NULL,
    operation VARCHAR(50) NOT NULL
);