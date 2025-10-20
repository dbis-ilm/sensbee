.. _tutorial:

Tutorial
========

The following example demonstrates the setup of an initial SensBee instance, sensor registration, and data upload/download.

1. Prerequisites
-------------------------------------

Make sure that the config has a value for the `root_user_email` option. 
More information about the SensBee server configuration can be found here :ref:`config`.

Now start the stack as explained in :ref:`docker`.


2. Register a Sensor via the REST API
-------------------------------------

Before registering a new sensor, log in as the root user to retrieve a JWT token:

.. code-block:: bash
   :caption: Bash

   curl --location http://127.0.0.1:8080/auth/dev/login -X POST
      

**Expected Response (JWT Token):**

.. code-block:: js

   {"jwt":"eyJ0eXAiOi..."}

You can use https://www.jwt.io to inspect the contents of this granted token.

Use the granted JWT token (from now on passed in the ``Authorization`` header) to register a new sensor named ``MySensor`` with ``count`` (INT) and ``temperature`` (FLOAT) columns:
More information about specific attributes can be found in :ref:`ref-sensor`.


.. code-block:: bash
   :caption: Bash

   curl --location http://127.0.0.1:8080/api/sensors/create -X POST \
            --header 'Content-Type: application/json' \
            --header 'Authorization: eyJ0eXAiOi...' \
            --data '{"columns":[{"name":"count","val_type":"INT","val_unit":"number","val_ingest":"INCREMENTAL"},{"name":"temperature","val_type":"FLOAT","val_unit":"celsius","val_ingest":"LITERAL"}],"description":"This is my first sensor.","name":"MySensor","permissions":[{"operations":["INFO","READ","WRITE"],"role_id":"72122092-1154-4189-8dde-d72b663b55eb"}],"position":[50.68322,10.91858],"storage":{"params":{},"variant":"DEFAULT"}}'

**Expected Response (Sensor UUID):**

.. code-block:: js

   {"uuid":"89ecbd44-9e45-4a96-bcb3-bf3515479bfe"}

3. Upload Sensor Data
---------------------

With the login token (JWT Token) and sensor identifier (Sensor UUID), you can now push sensor data. 
For private sensors you need to create an API key for **WRITE** operations.
This can be done via the following command:

.. code-block:: bash
   :caption: Bash

   curl --location http://127.0.0.1:8080/api/sensors/89ecbd44-9e45-4a96-bcb3-bf3515479bfe/api_key/create -X POST \
            --header 'Content-Type: application/json' \
            --header 'Authorization: eyJ0eXAiOi...' \
            --data '{ "name": "MyFirstKey", "operation": "WRITE" }'

**Expected Response (API Key UUID):**

.. code-block:: js

   {"id":"1bfe0954-b6da-4dc4-abb9-18514291987f","name":"MyFirstKey","operation":"WRITE","sensor_id":"89ecbd44-9e45-4a96-bcb3-bf3515479bfe","user_id":"c25ccc26-600e-..."}

After such a key has been created, it is used instead of our (JWT Token) to authorize data ingestion.
Instead of a header it is placed directly in the URL.
Now, push data using the newly created API key via the following command:

**Via HTTP:**

.. code-block:: bash
   :caption: Bash

   curl --location http://127.0.0.1:8080/api/sensors/89ecbd44-9e45-4a96-bcb3-bf3515479bfe/data/ingest?key=1bfe0954-b6da-4dc4-abb9-18514291987f -X POST \
            --header 'Content-Type: application/json' \
            --data '[{ "count": 7, "temperature": 22.2 }]'

**Expected Response:**

.. code-block:: js

   {}

It is also possible to ingest data via MQTT.

**Via MQTT:**

The following command can be executed inside the docker mosquitto container to achieve the same result as the previous HTTP request.
One key difference is that we do not get feedback immediatly. In case of data not beeing recieved consult the log for more information.

.. code-block:: bash 
   :caption: Bash

   mosquitto_pub -t '/api/sensors/89ecbd44-9e45-4a96-bcb3-bf3515479bfe/1bfe0954-b6da-4dc4-abb9-18514291987f' -m '[{ "count": 7, "temperature": 22.2 }]'

No specific configuration is needed for MQTT ingestion, but the topic must adhere to ``/api/sensors/<sensor_id>[/<api_key>]``.

4. Retrieve Sensor Data
-----------------------

To fetch uploaded data of private sensors, create a separate API key for **READ** operations:

.. code-block:: bash
   :caption: Bash

   curl --location http://127.0.0.1:8080/api/sensors/89ecbd44-9e45-4a96-bcb3-bf3515479bfe/api_key/create -X POST \
            --header 'Content-Type: application/json' \
            --header 'Authorization: eyJ0eXAiOi...' \
            --data '{ "name": "MySecondKey", "operation": "READ" }'

**Expected Response (API Key UUID):**

.. code-block:: js

   {"id":"387e164c-d0f9-4478-b8dc-0c9689b76e59","name":"MySecondKey","operation":"READ","sensor_id":"89ecbd44-9e45-4a96-bcb3-bf3515479bfe","user_id":"c25ccc26-600e-..."}

With this key, uploaded data can be retrieved based on optional conditions. The following request returns the 10 most recent tuples from the last 7 days:

.. code-block:: js

   {
      "from": "2024-12-07T12:00:00.000Z",
      "to": null,
      "limit": 10,
      "ordering": "DESC"
   }

.. code-block:: bash
   :caption: Bash

   curl --location http://127.0.0.1:8080/api/sensors/89ecbd44-9e45-4a96-bcb3-bf3515479bfe/data/load?key=387e164c-d0f9-4478-b8dc-0c9689b76e59 -X GET \
            --header 'Content-Type: application/json' \
            --data '{"from":"2024-12-07T12:00:00","to":null,"limit":10,"ordering":"DESC"}'

**Expected Response:**

.. code-block:: js

   [{"count":7,"created_at":"2025-10-08T21:10:23.654","temperature":22.2}]
