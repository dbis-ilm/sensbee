# SensBee - Sensor Data Backend

SensBee is a robust database backend designed for Smart City and IoT applications. It provides a comprehensive solution for managing sensor data, offering functionalities to register sensors, upload real-time measurements, and retrieve data in various raw or preprocessed forms (current values, specific ranges, or full datasets).

### Sensors & Applications

Sensors may upload data via HTTP or MQTT in JSON format. SensBee allows JavaScript based transformations of any incoming json data.

Applications may use the RESTful API to query sensor data.

### Access management

SensBee incorporates a granular role-based access control (RBAC) system. This allows for the precise management of permissions (e.g., read-only, write access) to individual sensors. For secure access to non-public data, SensBee utilizes API keys, which can be generated and assigned on a per-sensor basis, providing fine-grained control over data ingestion and consumption.

To dive right in take a look at our [quick-start](docs/build/markdown/user-guide/quick-start.md) guide.

# What is the use case?

Any system where sensor data needs to be collected in a centralized platform via HTTP or MQTT.

# Who can use SensBee?

Everyone can use the SensBee system as per our MIT [license](LICENSE).

# Contributing to SensBee

SensBee is an open platform, and we welcome contributions from the community to enhance its functionality, improve documentation, and expand its capabilities.
