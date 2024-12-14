-- Add up migration script here

ALTER TABLE sensor ADD COLUMN IF NOT EXISTS description TEXT;
