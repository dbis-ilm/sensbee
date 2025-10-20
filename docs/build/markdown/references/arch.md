<a id="ref-arch"></a>

# Architecture

The following container are parts of the docker compose stack:

1. SBMI (HTTP)
2. SensBee (HTTP)
3. Postgres
4. Mosquitto (MQTT)
5. DataTransformer
6. EventHandler
7. CI-testing
8. [OpenTelementry](../developer-guide/otel.md#opentelemetry)-Collector

#### NOTE
Container with marked Endpoints (HTTP or MQTT) are exposed to the host by default. Consult the compose file for more information.

The heart of everything is SensBee.

Persistent data is stored in the Postgres Database.

The [Data Transformer](data-transformer.md#data-transformer) component allows SensBee to execute isolated javascript functions. Management of these functions is done via SBMI.
Actual invocations are submitted via a WebSocket connection between SensBee and the DataTransformer Service.

Mosquitto ([https://mosquitto.org](https://mosquitto.org)) is used as a MQTT broker. SensBee connects to it via a WebSocket connection and subscribes on a prefixed wildcard topic.

The CI-testing container is used to invoke the test-suite in our GitLab CI.

The OpenTelemetry container forwards configured observability data to an optional Grafana stack. More information can be found in services/grafana.
