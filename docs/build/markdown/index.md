# Welcome to SensBee - Sensor Data Backend!

[![pipeline status](https://dbgit.prakinf.tu-ilmenau.de/code/sensbee/badges/main/pipeline.svg)](https://dbgit.prakinf.tu-ilmenau.de/code/sensbee/-/commits/main)[![coverage report](https://dbgit.prakinf.tu-ilmenau.de/code/sensbee/badges/main/coverage.svg)](https://dbgit.prakinf.tu-ilmenau.de/code/sensbee/-/commits/main)

SensBee is a database backend for **Smart City** and **IoT applications**. To this end, SensBee provides the ability to register sensors and upload measurement data for these sensors, or download the current values, a range of data or all data.
These functions are accessible through a REST interface.
Access rights (read data, write data) to sensors are managed by roles, that can be created and assigned to users.
To access non-public data, designated API keys are required, which can be created individually for each accessible sensor.

Metadata and measurement data are stored in a **PostgreSQL database**. Each sensor has its own table.

Sensors can send their data via HTTP or MQTT. For more information about the schema please head to [Sensors](references/sensor.md#ref-sensor).

A quick start guide is available. For more in depth explanations consult the tutorial.

# Links

* [GitLab and Issue Tracker](https://dbgit.prakinf.tu-ilmenau.de/code/sensbee)
* [GitHub Mirror](https://github.com/dbis-ilm/sensbee)
* [API Docs](https://todo.todo/)

# User guide

* [Quick Start Guide](user-guide/quick-start.md)
* [SensBee Management Interface](user-guide/sbmi.md)
* [Tutorial](user-guide/tutorial.md)
* [Production deployment](user-guide/deployment.md)

# Development

* [Docker Compose Setup](developer-guide/docker.md)
* [OpenAPI Documentation](developer-guide/openapi.md)
* [OpenTelementry](developer-guide/otel.md)
* [Debug with a running Docker Compose Setup](developer-guide/debugging.md)
* [Testing](developer-guide/testing.md)

# References

* [Authentication & Authorization](references/auth.md)
* [Server Configuration](references/config.md)
* [User](references/user.md)
* [Roles](references/roles.md)
* [Sensors](references/sensor.md)
* [Data Transformer](references/data-transformer.md)
* [Event System](references/event-system.md)
* [Architecture](references/arch.md)
