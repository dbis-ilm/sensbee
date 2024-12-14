-- Add down migration script here

DROP TABLE IF EXISTS sensor_schema;
DROP TABLE IF EXISTS sensor;

DROP INDEX IF EXISTS users_email_idx;
DROP TABLE IF EXISTS users;