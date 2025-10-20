.. _docker:

Docker Compose Setup
=====================

An all-inclusive SensBee stack can be started using **Docker Compose**. 
We provide files for building a Docker image of the SensBee server and a compose file for setting up containers with additional services like PostgreSQL DB and the web-based SensBee Management Interface (SBMI).
For a more detailed look into all available services consult the :ref:`ref-arch` description.

Use the following command to start the complete compose stack:

.. code-block:: bash
   :caption: Inside the project root:

   docker compose up -d

The compose setup exposes the PostgreSQL connection port. The ``.env`` file provides a local connection string, allowing any cargo command to use the DB connection. 
When SensBee first connects to the database, it automatically sets up all necessary tables and runs all migrations.

.. note::
   If you dont want to use a local instance of sensbee add ``--profile full`` to the compose commands. This will compile a sensbee instance from your local code and add it to the stack.

To stop and remove the containers, use:

.. code-block:: bash
   :caption: Inside the project root:

   docker compose down

If you want to start with a fresh database use the following flag to **DELETE all persistent data**.
Use the ``-v`` option with ``docker compose down`` to remove named volumes. 