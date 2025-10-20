.. _ref-roles:

Roles
=====================

Roles can be assigned to users and manage their access rights for system resources, such as sensors and sensor data.
For each created sensor, the sensor owner (or admins) can configure, which user-roles can access the sensor information and sensor data,
or ingest new sensor data into the system. Moreover, admin users have additional access privileges and can manage users and all sensors in the system.

In general, we differentiate between static system roles, that are initially generated when starting the system
and application-specific roles, that can be created (and deleted) by admin users during the runtime.


System roles
------------

The following roles and theirs IDs exist across all instances. 

**Root**
``54344b08-d833-4ac3-8928-b6c646b2c9c1``

**Admin**
``0e804d35-c8e3-49ee-86d4-3e556a82a1af``

**User** 
``72122092-1154-4189-8dde-d72b663b55eb``

**Guest** 
``51fd9bb7-3214-4089-adb9-474eb82b447a``

.. note::
    System roles cant be removed from the system. Except the admin role, system roles can not be assigned to or revoked from users.


Admin
^^^^^

Allows to manage users, roles and sensors across the system. Admins are not permitted to assign or revoke the admin role from other users.

Root
^^^^

Root is considered the super-admin and may access all functionalities of the system and also assign new admin users.
A root user can only be created/assigned when starting the system, according to the :ref:`config`.

.. note::
    If the root role is assigned it cant be revoked without modifying the database directly.


User
^^^^

Every registered user is part of the user role group and is allowed to create own sensors or access existing sensors that permit an access for all users.

.. note::
    By default, users must be verified by an admin user when first registered before they can login.


Guest
^^^^^

The guest role identifies users without login and may be used to configure sensors to be access publicly.
Every access to the API or the SBMI without login is considered a guest access.


Application-specific roles
--------------------------

Admin users may create more use-case-specific roles to manage the access of their users in a more fine-granular way.

After creating custom roles, sensors allow to configure their access permissions for the newly created roles, such as
accessing sensor data or ingesting data into the system.

