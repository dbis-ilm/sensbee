-- Add up migration script here
-----------------------------------------------------------------------------------
-- All initial tables, data and functions

-----
-- Table for storing users accessing the system
-----
CREATE TABLE users (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY NOT NULL, -- user id
    name character varying(100) NOT NULL,                   -- Uuer name
    email character varying(255) UNIQUE NOT NULL,           -- email address
    verified boolean DEFAULT false NOT NULL                 -- flag to enable a user account for login, after they self registered
);

-----
-- Index on email
-----
CREATE INDEX users_email_idx ON users(email);


-----
-- Table for storing metadata of a sensor
-----
CREATE TABLE sensor (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY, -- sensor id
    name varchar(50) UNIQUE NOT NULL,              -- sensor name
    tbl_name varchar(50) NOT NULL,                 -- name of the corresponding table
    longitude double precision,                    -- position of the sensor
    latitude double precision,
    description text,                              -- a textual description
    owner uuid REFERENCES users(id),                -- the user owning the sensor
    storage_type varchar(50)                       -- ??
       DEFAULT 'Default'::character varying NOT NULL,
    storage_params text                            -- ??
);

-----
-- Table storing the schema of a sensor
-----
CREATE TABLE sensor_schema (
    sensor_id uuid REFERENCES sensor(id),    -- the sensor id
    col_name varchar(50) NOT NULL,           -- name of the column
    col_type integer NOT NULL,               -- type of the column (1 = int, 2 = float, 3 = string)
    col_unit varchar(10),                    -- the unit of the column values
    col_ingest integer DEFAULT 0 NOT NULL,   -- LITERAL or INCREMENTAL 
    PRIMARY KEY(sensor_id, col_name)
);


-----
-- Table for storing all registered roles
-----
CREATE TABLE roles (
    id uuid PRIMARY KEY,                  -- identifier of the role
    name varchar(50) UNIQUE NOT NULL,     -- the name of the role
    system boolean DEFAULT false NOT NULL -- flag to signify that this role cant be assigned or revoked by the admin, must be done by system or root
);

-- Sets up mandatory system roles
INSERT INTO roles(id, name, system) 
    VALUES('0e804d35-c8e3-49ee-86d4-3e556a82a1af','Admin', true) ON CONFLICT DO NOTHING;
INSERT INTO roles(id, name, system) 
    VALUES('72122092-1154-4189-8dde-d72b663b55eb','User', true) ON CONFLICT DO NOTHING;
INSERT INTO roles(id, name, system) 
    VALUES('51fd9bb7-3214-4089-adb9-474eb82b447a','Guest', true) ON CONFLICT DO NOTHING;
INSERT INTO roles(id, name, system) 
    VALUES('54344b08-d833-4ac3-8928-b6c646b2c9c1','Root', true) ON CONFLICT DO NOTHING;


-----
-- Table for storing assigments of roles to users
-----
CREATE TABLE user_roles (
    user_id uuid NOT NULL REFERENCES users(id), -- the user id
    role_id uuid NOT NULL REFERENCES roles(id), -- role assigned to the user
    PRIMARY KEY(user_id, role_id)
);


-----
-- Table for storing permissions to access sensors via roles
-----
CREATE TABLE sensor_permissions (
    sensor_id uuid NOT NULL REFERENCES sensor(id), -- reference to the sensor for which these permissions are granted
    role_id uuid NOT NULL REFERENCES roles(id),    -- reference to the role that grants these permissions
    allow_info boolean DEFAULT false NOT NULL,     -- Allows the role to access resources that require at most INFO permissions
    allow_read boolean DEFAULT false NOT NULL,     -- Allows the role to access resources that require at most READ permissions
    allow_write boolean DEFAULT false NOT NULL,    -- Allows the role to access resources that require at most WRITE permissions
    PRIMARY KEY(sensor_id, role_id)
);


-----
-- Table to store user Save OIDC sub and iss claims for OpenID authentication
-----
CREATE TABLE users_oidc (
    id uuid REFERENCES users(id),
    iss varchar NOT NULL,
    sub varchar NOT NULL,
    UNIQUE(id, iss),
    UNIQUE(iss, sub)
);


-----
-- Table for storing generated API Keys of a sensor
-----
CREATE TABLE api_keys (
    id uuid PRIMARY KEY,                           -- identifier of the API Key
    user_id uuid NOT NULL REFERENCES users(id),    -- identifier of the user that created this key, only they may manage it
    sensor_id uuid NOT NULL REFERENCES sensor(id), -- identifier of the sensor to which this key belongs
    name varchar(255) NOT NULL,                    -- a human readable name for this API Key
    operation varchar(50) NOT NULL                 -- Signifies that either READ or WRITE privileges are granted to the sensor when using this key
);


-----
-- Table to store available data transformer
-----
CREATE TABLE data_transformer (
    id uuid PRIMARY KEY,                                                                       -- identifier for this data transformer
    name text DEFAULT 'generic_transformer'::text NOT NULL,                                    -- human readable name
    script text NOT NULL,                                                                      -- the actual script that gets executed on incoming data
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,                 -- when this data transformer was created
    updated_at timestamp without time zone,                                                    -- the last update of this data transformer
    version integer DEFAULT 1 NOT NULL,                                                        -- how many times this transformer has been updated
    CONSTRAINT data_transformer_created_at_check CHECK ((created_at <= CURRENT_TIMESTAMP))
);


-----
-- Table to store event handler for system events
-----
CREATE TABLE event_handler (
    id uuid PRIMARY KEY,   -- identifier for this event handler
    name text NOT NULL,    -- human readable name
    filter text,           -- ??
    url text NOT NULL,     -- The url that will be called with the data
    method text            -- GET or POST
);


-----
-- Table to store relationships between sensors, event handler and data transformer for event handler
-----
CREATE TABLE sensor_data_chain_outbound (
    sensor_id uuid NOT NULL           -- reference to the sensor to which this outbound data chain belongs
        REFERENCES sensor(id) ON UPDATE CASCADE ON DELETE CASCADE,        
    data_transformer_id uuid          -- reference to the data transformer to be used before executing a event handler
        REFERENCES data_transformer(id) ON UPDATE CASCADE ON DELETE CASCADE,
    event_handler_id uuid NOT NULL    -- reference to the event handler where will end up
        REFERENCES event_handler(id) ON UPDATE CASCADE ON DELETE CASCADE,
    UNIQUE (sensor_id, event_handler_id)
);


-----
-- Table to store active data transformer for a sensor
-----
CREATE TABLE sensor_data_chain (
    sensor_id uuid UNIQUE NOT NULL    -- reference to the sensor to which this inbound data chain belongs
        REFERENCES sensor(id) ON UPDATE CASCADE ON DELETE CASCADE,
    inbound_dt_id uuid NOT NULL       -- the data transformer to be used when data arrives
        REFERENCES data_transformer(id) ON UPDATE CASCADE ON DELETE CASCADE,
    UNIQUE(sensor_id)
);


-----
-- Table to store system events
-----
CREATE TABLE log_events (
    t timestamp without time zone NOT NULL, -- creation time of the log event
    sensor_id uuid,                         -- OPTIONAL reference to a sensor
    data jsonb NOT NULL                     -- various meta data for this event
);


-----------------------------------------------------------------------------------
-----
-- Overloaded functions to generically add two values of same type
-----
CREATE FUNCTION add_values(value1 double precision, value2 double precision) RETURNS double precision
    LANGUAGE plpgsql
    AS $$
BEGIN
    RETURN COALESCE(value1, 0) + COALESCE(value2, 0);
END;
$$;

CREATE FUNCTION add_values(value1 integer, value2 integer) RETURNS integer
    LANGUAGE plpgsql
    AS $$
BEGIN
    RETURN COALESCE(value1, 0) + COALESCE(value2, 0);
END;
$$;

CREATE FUNCTION add_values(value1 text, value2 text) RETURNS text
    LANGUAGE plpgsql
    AS $$
BEGIN
    RETURN COALESCE(value1, '') || COALESCE(value2, '');
END;
$$;

-----
-- Function for creating a trigger implementing a count-based ring buffer
-----
CREATE FUNCTION create_ring_buffer_count(tbl character varying, num integer) RETURNS void
    LANGUAGE plpgsql
    AS $_$
begin
execute format('create or replace function check_data_storage_' || tbl || '()
returns trigger language plpgsql as
$$
begin
delete from ' || tbl || '
where ctid IN (
  select ctid
  from (
    select ctid,
           row_number() over (order by created_at desc) as rn
  from ' || tbl || ') sub
  where rn > ' || num || '
);
return null;
end;
$$;');
EXECUTE format('create or replace trigger '|| tbl || '_trigger after insert on ' || tbl || '
            for each statement
            execute function check_data_storage_' || tbl || '()');
end
$_$;

-----
-- Function for creating a trigger implementing a count-based ring buffer
-----
CREATE FUNCTION create_ring_buffer_interval(tbl character varying, interval_min double precision) RETURNS void
    LANGUAGE plpgsql
    AS $_$
begin
execute format('create or replace function check_data_storage_' || tbl || '()
returns trigger language plpgsql as
$$
begin
delete from ' || tbl || '
where created_at < NOW() - INTERVAL '' ' || interval_min || ' minutes'';
return null;
end;
$$;');
EXECUTE format('create or replace trigger '|| tbl || '_trigger after insert on ' || tbl || '
            for each statement
            execute function check_data_storage_' || tbl || '()');
end
$_$;

-----
-- -- Incremental sensor data ingest trigger
-----
CREATE FUNCTION create_sensor_column_ingest_incremental(tbl character varying, col character varying) RETURNS void
    LANGUAGE plpgsql
    AS $_$
BEGIN
    EXECUTE format('create or replace function incr_sdata_ingest_' || hashed_col_name(tbl, col, 8) || '()
    returns trigger language plpgsql as
    $$
    begin
        new. ' || col || ' := add_values((select ' || col || ' from ' || tbl || ' order by created_at desc limit 1), new. ' || col || ');
        return new;
    end;
    $$;');
    EXECUTE format('create or replace trigger _' || hashed_col_name(tbl, col, 8) || '_ingest before insert on ' || tbl || '
                for each row
                execute function incr_sdata_ingest_' || hashed_col_name(tbl, col, 8) || '()');
END;
$_$;

CREATE OR REPLACE FUNCTION remove_sensor_column_ingest_incremental(tbl character varying, col character varying) RETURNS void
    LANGUAGE plpgsql
    AS $$
BEGIN
    EXECUTE format('DROP FUNCTION incr_sdata_ingest_' || hashed_col_name(tbl, col, 8) || ' CASCADE');
END;
$$;


-----
-- Custom hashed name since function names are limited in length - [63 byte]
-----
CREATE FUNCTION hashed_col_name(tbl text, col text, lmt integer) RETURNS text
    LANGUAGE plpgsql
    AS $$
BEGIN
    RETURN tbl || SUBSTRING(md5(col) FROM 1 FOR lmt);
END;
$$;

