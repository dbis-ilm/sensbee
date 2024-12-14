-- Add up migration script here

ALTER TABLE sensor ADD COLUMN IF NOT EXISTS storage_type varchar(50) NOT NULL DEFAULT 'Default';
ALTER TABLE sensor ADD COLUMN IF NOT EXISTS storage_params TEXT;

-- Add sensor data storage functions

create or replace function create_ring_buffer_count(tbl varchar, num integer)
returns void
as
$func$
begin
execute format('create or replace function check_data_storage_' || tbl || '()
returns trigger language plpgsql as
$$
begin
delete from ' || tbl || '
where created_at in (
  select created_at
  from (
    select created_at, rank() over (order by created_at desc) as rank
    from ' || tbl || ') as t
  where t.rank > ' || num || ');
return new;
end;
$$;');
EXECUTE format('create or replace trigger '|| tbl || '_trigger after insert on ' || tbl || '
            for each row
            execute function check_data_storage_' || tbl || '()');
end
$func$
LANGUAGE plpgsql;

--

create or replace function create_ring_buffer_interval(tbl varchar, interval_min float)
returns void
as
$func$
begin
execute format('create or replace function check_data_storage_' || tbl || '()
returns trigger language plpgsql as
$$
begin
delete from ' || tbl || '
where created_at < NOW() - INTERVAL '' ' || interval_min || ' minutes'';
return new;
end;
$$;');
EXECUTE format('create or replace trigger '|| tbl || '_trigger after insert on ' || tbl || '
            for each row
            execute function check_data_storage_' || tbl || '()');
end
$func$
LANGUAGE plpgsql;