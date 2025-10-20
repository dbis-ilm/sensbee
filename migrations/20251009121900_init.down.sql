-- Add down migration script here
DROP FUNCTION IF EXISTS hashed_col_name;

DROP FUNCTION IF EXISTS remove_sensor_column_ingest_incremental;

DROP FUNCTION IF EXISTS create_sensor_column_ingest_incremental;

DROP FUNCTION IF EXISTS create_ring_buffer_interval;

DROP FUNCTION IF EXISTS create_ring_buffer_count;

DROP FUNCTION IF EXISTS add_values(value1 double precision, value2 double precision);

DROP FUNCTION IF EXISTS add_values(value1 integer, value2 integer);

DROP FUNCTION IF EXISTS add_values(value1 text, value2 text);

DROP TABLE IF EXISTS sensor_data_chain_outbound;

DROP TABLE IF EXISTS event_handler;

DROP TABLE IF EXISTS log_events;

DROP TABLE IF EXISTS sensor_data_chain;

DROP TABLE IF EXISTS data_transformer;

DROP TABLE IF EXISTS api_keys;

DROP TABLE IF EXISTS user_roles;

DROP TABLE IF EXISTS sensor_permissions;

DROP TABLE IF EXISTS roles;

DROP TABLE IF EXISTS sensor_schema;

DROP TABLE IF EXISTS sensor;

DROP INDEX IF EXISTS user_email_idx;

DROP TABLE IF EXISTS users;
