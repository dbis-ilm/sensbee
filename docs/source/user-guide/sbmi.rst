.. _sbmi:

SensBee Management Interface
===================================

**SBMI** is a standalone web application providing a visual interface for sensor and user management functionalities. 

With a running :ref:`docker` it can be accessed via:

.. code-block::

   http://localhost:8082

Internally, the frontend communicates with the SensBee backend's REST API.

.. note::
   All credentials for SBMI are the same as for SensBee.


Sensor creation
---------------

Any user may create their own sensors.

Click on `Sensors` and then on the `+` sign. Now fill out the form to create a sensor.


Sensor related resources
------------------------

A user can create other resources associated with sensors they can access.


Data transfomer 


Event handler


Other resources
---------------
User and roles may be created by users with the admin role. Consult :ref:`ref-roles` for an overview on how to assign roles to users.