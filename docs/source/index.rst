Welcome to SensBee - Sensor Data Backend!
=========================================

.. meta::
   :description: SensBee is a database backend for Smart City and IoT applications.
   :keywords: SensBee, IoT, Smart City, Sensor Data, REST API, PostgreSQL, Rust, Docker, Sphinx

.. image:: https://dbgit.prakinf.tu-ilmenau.de/code/sensbee/badges/main/pipeline.svg
   :alt: pipeline status
   :target: https://dbgit.prakinf.tu-ilmenau.de/code/sensbee/-/commits/main

.. image:: https://dbgit.prakinf.tu-ilmenau.de/code/sensbee/badges/main/coverage.svg
   :alt: coverage report
   :target: https://dbgit.prakinf.tu-ilmenau.de/code/sensbee/-/commits/main

SensBee is a database backend for **Smart City** and **IoT applications**. To this end, SensBee provides the ability to register sensors and upload measurement data for these sensors, or download the current values, a range of data or all data. 
These functions are accessible through a REST interface.
Access rights (read data, write data) to sensors are managed by roles, that can be created and assigned to users. 
To access non-public data, designated API keys are required, which can be created individually for each accessible sensor.

Metadata and measurement data are stored in a **PostgreSQL database**. Each sensor has its own table. 

Sensors can send their data via HTTP or MQTT. For more information about the schema please head to :ref:`ref-sensor`.

A quick start guide is available. For more in depth explanations consult the tutorial.


.. toctree::
   :maxdepth: 1
   :caption: Links

   GitLab and Issue Tracker <https://dbgit.prakinf.tu-ilmenau.de/code/sensbee>
   GitHub Mirror <https://github.com/dbis-ilm/sensbee>
   API Docs <https://todo.todo/>

.. toctree::
   :maxdepth: 1
   :caption: User guide
   
   user-guide/quick-start
   user-guide/sbmi
   user-guide/tutorial
   user-guide/deployment

.. toctree::
   :maxdepth: 1
   :caption: Development

   developer-guide/docker
   developer-guide/openapi
   developer-guide/otel
   developer-guide/debugging
   developer-guide/testing

.. toctree::
   :maxdepth: 1
   :caption: References

   references/auth
   references/config
   references/user
   references/roles
   references/sensor
   references/data-transformer
   references/event-system
   references/arch
   
