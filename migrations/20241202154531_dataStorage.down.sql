-- Add down migration script here

ALTER TABLE sensor DROP COLUMN IF EXISTS storage_type;
ALTER TABLE sensor DROP COLUMN IF EXISTS storage_params;

DROP FUNCTION IF EXISTS create_ring_buffer_count CASCADE;
DROP FUNCTION IF EXISTS create_ring_buffer_interval CASCADE;