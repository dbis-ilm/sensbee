<a id="ref-sensor"></a>

# Sensors

Sensors are the fundamental entities of the system and define a digital or physical sensor that should ingest data into the system.

## Sensor Creation

During the creation of the sensor, various parameters can be configured.
Please refer to the [OpenAPI Documentation](../developer-guide/openapi.md#openapi) for more information about the parameters, their data types and notation.

**Name**
Identifies the sensor in a user-friendly way.

**Position**
The latitude, longitude position of the sensor.

**Description**
An arbitrary description of the sensor for users to review.

**Permissions**

A list of role-specific access permissions to the sensor.
For each desired role, the following permissions may be assigned:

- READ to retrieve sensor data.
- WRITE to ingest sensor data.
- INFO to access the sensor information, such as name and description.

**Columns**

A list of data columns for the sensor to be created in the database.
By default, a time column created_at is created for each sensor to store the timestamp of data ingestion.
Each custom column is defined by the following attributes:

- name to identify the data column.
- data type (UNKNOWN, INT, FLOAT, STRING), which is required for subsequent operations, such as data aggregation.
- unit [optional] to specific the unit of measurement for the sensor data. Any custom string may be defined here.
- data ingestion type (LITERAL, INCREMENTAL), which defines, how to provided data values are ingested into the columns.
  For a literal ingestion type, the raw data will be stored directly into the column, while incremental will increase the raw value by the most recent value in the database before storing into the column.
  Incremental columns are especially convenient for sensors that send delta values, such as the entry of a new user (+1) or leave of an existing user (-1).

  #### NOTE
  From an architectural point of view, incremental columns will set up a database trigger, that retrieves the most recent values from the table before storing the new data values.

**Storage Type**

Defines, how the overall data of the sensor is stored:

- Default to store all data ingested into the sensor.
- Ringbuffer Count allows to configure a maximal amount of rows in the sensor table. When exceeding the limit, the least recent values will be dropped from the table.
- Ringbuffer Time allows to configure a maximal time range of rows to be stored in the sensor table. Upon ingestion of new data values, the system will check previous values in the database and drop values outside the defined time interval.

The Ringbuffer storage mode is especially convenient for sensors that produce data that is only valid for a specific period of time and allows to reduce the required storage size.

#### NOTE
From an architectural point of view, sensors with a defined Ringbuffer setup a database trigger, that executes after insertion of new data values and removes any outdated values from the column.

## API Keys

API keys are used to manage the user-specific access to data of a specific sensor.
Users that belong to a group that has READ or WRITE access for a specific sensor may create custom API keys for that sensor.
The generated keys may be used within the sensor or sensor applications to retrieve or ingest data into the sensor.

All requests to the ingest, load or delete API endpoint must provide a valid API key or are considered unauthenticated GUEST accesses.

A user can create any number of API keys with custom names to use across an arbitrary amount of applications.

#### NOTE
API keys are automatically removed when a user looses access to a sensor. This might either happen when he was removed from a specific role that granted access to the sensor
or the sensor permissions were modified to exclude specific user roles.

## Data Ingestion

Ingesting data into the sensor is usually the first step after creating a sensor (and corresponding API keys).
Various ingestion methods are supported, such as HTTP and MQTT, to send data to the system.
For non-public sensors, a valid WRITE API key must be provided during the ingestion process.

- **HTTP**: `https://{SENSBEE_DOMAIN}:8443/api/sensors/{SENSOR_ID}/data/ingest?key={WRITE_API_KEY}`
- **MQTT**: `https://{SENSBEE_DOMAIN}:1883` with topic `/api/sensors/{SENSOR_ID}/{WRITE_API_KEY}`

All ingestion protocols require a JSON body with the respective sensor data to ingest.
During this step, either a single data tuple or a batch of tuples may be ingested at once into the sensor.
The JSON body for each tuple must contain key-value entries for each column of the sensor.
Omitted or invalid columns will receive NULL values.

Each tuple may provide a custom timestamp created_at to be used for declaring the timestamp of the data tuple.
If omitted, the current system time is utilized.

#### NOTE
For batch ingestion of multiple tuples, omitting custom timestamps may result in the same timestamp for all ingested rows.

## Data Retrieval

To retrieve data from the sensor, the load data API endpoint `https://{SENSBEE_DOMAIN}:8443/api/sensors/{SENSOR_ID}/data/load?key={READ_API_KEY}` can be used.
Similar to the data ingestion, a valid READ API key must be provided to access the sensor data.

Various query parameters allow to specific how and what data should be retrieved from the sensor.
Please refer to the [OpenAPI Documentation](../developer-guide/openapi.md#openapi) for more information about the parameters, their data types and notation.

**Limit** [optional]
The maximal amount of result tuples to retrieve. Default: 100.

**Ordering** [optional]
Either ASC or DESC to retrieve the result tuples in a specific order. Default: DESC based on the timestamp.

**Order Column** [optional]
Allows to specific which column to use for ordering the result tuples. Default: the created_at time column.

**From** [optional]
The lower limit for timestamps of data tuples to include in the result set. Default: no limit

**To** [optional]
The upper limit for timestamps of data tuples to include in the result set. Default: no limit

**From Inclusive** [optional]
If the lower from range should be considered as inclusive (>=) or exclusive (>). Default: true (inclusive)

**To Inclusive** [optional]
If the upper to range should be considered as inclusive (<=) or exclusive (<). Default: true (inclusive)

**Time Grouping** [optional]
Allows to request an aggregation of data tuples based on a specified time interval, such as per hour or per day. Default=No aggregation

**Columns** [optional]
The data columns (names) to include in the result set.
If a data aggregation is requested, each column must be annotated with an additional aggregation mode [MIN, MAX, SUM, AVG, COUNT].
default=All columns

## Data Deletion

To delete data from a sensor, the delete data API endpoint `https://{SENSBEE_DOMAIN}:8443/api/sensors/{SENSOR_ID}/data/delete?key={WRITE_API_KEY}` can be used.
Similar to the data ingestion, a valid WRITE API key must be provided to remove the sensor data.

Various query parameters allow to specific what data tuples should be removed from the sensor.
Please refer to the [OpenAPI Documentation](../developer-guide/openapi.md#openapi) for more information about the parameters, their data types and notation.

**From** [optional]
The lower limit for timestamps of data tuples to remove from the sensor. Default: no limit

**To** [optional]
The upper limit for timestamps of data tuples to to remove from the sensor. Default: no limit

**From Inclusive** [optional]
If the lower from range should be considered as inclusive (>=) or exclusive (>). Default: true (inclusive)

**To Inclusive** [optional]
If the upper to range should be considered as inclusive (<=) or exclusive (<). Default: true (inclusive)

**Purge** [optional]
If no lower and upper limits are specified the system will interpret this as a full clear of all stored data for the sensor.
To confirm this purge a specific flag must be provided. Default=false
