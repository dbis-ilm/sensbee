.. _ref-arch:

Architecture
=====================

The following container are parts of the docker compose stack:

#. SBMI (HTTP)

#. SensBee (HTTP)

#. Postgres

#. Mosquitto (MQTT)

#. DataTransformer

#. EventHandler

#. CI-testing

#. :ref:`opentelemetry`-Collector

.. note::

    Container with marked Endpoints (HTTP or MQTT) are exposed to the host by default. Consult the compose file for more information.


The heart of everything is SensBee. 

Persistent data is stored in the Postgres Database.

The :ref:`data-transformer` component allows SensBee to execute isolated javascript functions. Management of these functions is done via SBMI. 
Actual invocations are submitted via a WebSocket connection between SensBee and the DataTransformer Service.

Mosquitto (https://mosquitto.org) is used as a MQTT broker. SensBee connects to it via a WebSocket connection and subscribes on a prefixed wildcard topic.

The CI-testing container is used to invoke the test-suite in our GitLab CI.

The OpenTelemetry container forwards configured observability data to an optional Grafana stack. More information can be found in services/grafana.