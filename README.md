# SensBee - Sensor Data Backend Database

SensBee is a database backend for Smart City and IoT applications. To this end, SensBee provides the ability to register sensors and upload measurement data for these sensors, or download the current values, a range of data or all data. These functions are accessible through a REST interface.
Access rights (read data, write data) to sensors are managed by roles, that can be created and assigned to users. To access non-public data, designated API keys are required, which can be created individually for each accessible sensor.

Metadata and measurement data are stored in a PostgreSQL database. Each sensor has its own table. Two binaries are available based on the database:
 * sb_srv: is the REST server, accessible via HTTP.
 * sb_cli: is a command line tool that accesses the database directly and supports simple administration tasks.

## Installation and Database Preparation

SensBee requires a PostgreSQL database for operation. The easiest way is to install it via Docker:

```sh
sh> docker pull postgres
sh> docker run --name my-postgres -e POSTGRES_PASSWORD=my_secret -p 5432:5432 -d postgres
```

A separate database with a special user should be created for SenseBee:

```sh
postgres> create database testdb;
postgres> create user dev with password 'my_secret2';
postgres> alter database testdb owner to dev;
```

The connection information must be entered in a `.env` file in the root directory of the project:

```
# Database connection
PSQL_DATABASE=testdb
PSQL_USER=dev
PSQL_PASSWORD=my_secret2
DATABASE_URL=postgres://${PSQL_USER}:${PSQL_PASSWORD}@127.0.0.1:5432/${PSQL_DATABASE}
```

Note, if you want to run unit tests later, the user needs to have superuser privileges.

For authentication, we use JWT that must be provided for each API call. The JWT handling requires a public and private RSA key (Base64 encoded) which should be **exchanged** on deployment!

## Building SensBee

The SensBee project is written in Rust. For installation simply clone the repository, go to the home directory and run

```sh
sh> cargo build
```

SensBee requires a few database tables for metadata. To create these tables simple run

```sh
sh> sqlx migrate run
```
This assumes that you have configured your settings in the `.env` file as described above.

There are also several unit tests which you can run with
```sh
sh> cargo test
```

## User Management and Authorization

Users must be explicitly managed in SensBee. Based on this, the following applies:
* Most of the API paths require a valid access token, that can be acquired by a successful login
* Therefore, a user account needs to be created and verified by an admin
* Some paths allow guest access (read, write sensor data) if it is permitted by the sensor

An admin user should therefore be created for commissioning:

```sh
sh> sb_cli add-user <name> <email> <password> true
```

Additional users can then also be registered via the REST API.

## sb_srv

`sb_srv` is a REST server and the main entry point for the system. It connects to the PostgreSQL database and provides various services.

The API documentation is available as OpenAPI (former Swagger) service. Just point your browser at: http://localhost:8080/swagger-ui/ after starting the server.

## scdb_cli

`scdb_cli` is an administrative command line tool for managing users and roles. The database connection info is taken from the `.env` file (if available) or the `--db-url` option. The tool is controlled via the following commands

| Command      | Description                                                 | Parameter                        |
|--------------|-------------------------------------------------------------|----------------------------------|
| add-user     | Create a new verified user                                  | name, email, password, (--admin) |
| list-users   | Show the list of registered users                           | -                                |
| create-role  | Creates the given role                                      | role name                        |
| list-roles   | List all roles in the system                                | -                                |
| help         | Print a help message or the help of the given subcommand(s) | -                                |

## Tutorial

The following example shows the setup of an initial SensBee instance, the registration of sensors as well as upload and download of data. 
For this example we assume a database with applied migration scripts but otherwise empty. Furthermore, the database connection is configured in an `.env` file.

1. Create an admin user

```sh
sh> sb_cli add-user 'John Doe' john@gmail.com MySecret --admin
```

2. Start the server
```sh
sh> sb_srv &
```

3. Register a sensor via the REST API

In the following we use `curl` for sending REST calls.
But, before we can register a new sensor we have to login as the admin user and retrieve a token.

```sh
sh> curl --location http://127.0.0.1:8080/auth/login -X POST --header 'Content-Type: application/json' \
     --data '{ "email": "john@gmail.com", "password": "MySecret" }'

{"eyJ0eXAiOi..."}
```

The result of this REST call contains the JWT token that we need for registering a sensor called `MySensor`. This sensor produces two values which we store in the columns `count` and `temperature`. Note that the token is passed to the server via the authorization header. Without the token the request will fail.

```sh
sh> curl --location http://127.0.0.1:8080/api/sensors/create -X POST 
         --header 'Content-Type: application/json' 
         --header 'Authorization: eyJ0eXAiOi...'
         --data '{"columns":[{"name":"count","val_type":"INT","val_unit":"number"},{"name":"temperature","val_type":"FLOAT","val_unit":"celsius"}],"description":"This is my first sensor.","name":"MySensor","permissions":[{"operations":["INFO","READ","WRITE"],"role_name":"User"}],"position":[50.68322,10.91858],"storage":{"params":{},"variant":"DEFAULT"}}'
```
The result of this request contains the sensor identifier:

```JSON
"89ecbd44-9e45-4a96-bcb3-bf3515479bfe"
```

4. Upload sensor data

With the login and the sensor identifier we can periodically push sensor data. Since our created sensor only allows access to registered Users, we need to first create an API key to WRITE sensor data:

```sh
sh> curl --location http://127.0.0.1:8080/api/sensors/89ecbd44-9e45-4a96-bcb3-bf3515479bfe/api_key -X POST
         --header 'Content-Type: application/json'
         --header 'Authorization: eyJ0eXAiOi...'
         --data '{ "name": "MyFirstKey, "operation": "WRITE" }'
```

```JSON
"1bfe0954-b6da-4dc4-abb9-18514291987f"
```

With the created key we can now push data to the sensor by providing the key:

```sh
sh> curl --location http://127.0.0.1:8080/api/sensors/89ecbd44-9e45-4a96-bcb3-bf3515479bfe/data?key=1bfe0954-b6da-4dc4-abb9-18514291987f -X POST
         --header 'Content-Type: application/json'
         --header 'Authorization: eyJ0eXAiOi...'
         --data '{ "count": 7, "temperature": 22.2 }'
```

These values are stored in the sensor data table together with the current timestamp.

5. Retrieve sensor data

To fetch uploaded data we first need to create a separate API key for READ operations following the same approach as before:

```sh
sh> curl --location http://127.0.0.1:8080/api/sensors/89ecbd44-9e45-4a96-bcb3-bf3515479bfe/api_key -X POST
         --header 'Content-Type: application/json'
         --header 'Authorization: eyJ0eXAiOi...'
         --data '{ "name": "MySecondKey, "operation": "READ" }'
```

```JSON
"387e164c-d0f9-4478-b8dc-0c9689b76e59"
```

With the key, uploaded data can be retrieved based on different optional conditions. The following request will return the 10 most recent tuples that where stored in the last 7 days:

```JSON
{
    "from": "2024-12-07T12:00:00.000Z",
    "to": null,
    "limit": 10,
    "ordering": "DESC"
}
```

```sh
sh> curl --location http://127.0.0.1:8080/api/sensors/89ecbd44-9e45-4a96-bcb3-bf3515479bfe/data?key=387e164c-d0f9-4478-b8dc-0c9689b76e59 -X GET
         --header 'Content-Type: application/json'
         --header 'Authorization: eyJ0eXAiOi...'
         --data '{"from":"2024-12-07T12:00:00","to":null,"limit":10,"ordering":"DESC"}'
```

## Docker

SensBee can run within the Docker environment. We provide files for building a Docker image containing the SensBee server
as well as a Docker Compose file for setting up containers with PostgreSQL, pgAdmin, and SensBee.
Due to the fact that we use sqlx to run compile time checks against the database, the setup is split into two consecutive steps.
First, we start the database itself.

```sh
sh> docker compose --profile precompile up -d
```

This should start all services that are required to run during the build step of `SensBee`. If you want to run local tests the database started in the 
previous step will be used as well by default. The compose file uses host networking which allows a seamless development experience.

Using the following command starts our server.

```sh
sh> docker compose --profile runtime up -d
```

The `SensBee` server can be accessed via its REST API or the command line interface.
NOTE this two steps are only required for the first time you build the `SensBee` docker image. During development, you can simply use the runtime profile 
to bring up all services at once as the compilation step only happens if the image is not present.
If you want to force a rebuild of the `SensBee` image run this command.

```sh
sh> docker compose --profile runtime up -d --build
```

Finally, the containers can be stopped and removed using the following command.

```sh
sh> docker compose --profile runtime down
```

**Please note**: When running docker compose under **Windows**, it might be required to enable the "Enable host networking" option in the Docker / Docker Desktop settings.

### Cli

The CLI tool inside the docker container can be used by calling:

```sh
sh> docker exec -it {SENSBEE_CONTAINER_ID} /bin/sb_cli
```