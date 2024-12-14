-- Add down migration script here

DELETE FROM roles WHERE name='Guest';
