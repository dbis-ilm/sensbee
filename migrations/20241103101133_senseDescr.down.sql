-- Add down migration script here

ALTER TABLE sensor DROP COLUMN IF EXISTS description;