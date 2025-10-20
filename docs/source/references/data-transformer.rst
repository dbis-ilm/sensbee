.. _data-transformer:

Data Transformer
=====================

.. caution::

   Although every effort has been made to securely sandbox scripts, running untrusted code from users is always risky.

SensBee supports creating small JavaScript snippets to transform an incoming sensor data payload into the correct ingest schema for the sensor, giving you a very powerful ability to transform any input playload.

Executuon of scripts is done using https://github.com/laverdet/isolated-vm. 


Example script
--------------


A snippet must explicitly return an Array of Objects. These objects will then be placed into the database according to their sensor schema.

The input will always be availabe in a variable called ``data``

In the following example ``data`` = ``[{"some":"data"}]``:

.. code-block:: JavaScript

    // This is a comment
    if('some' in data){
        return [{"messages":"1"}];
    }
    return [];

This would return ``[{"messages":"1"}]`` to SensBee which will then try to insert the data into the sensor table.


Notes
--------------

A script is able to drop incoming data completely by returning ``[]``. 

If that is the case the generated event will have the status **204**.
If data has been ingested into the DB then the status will be **200**.

All error cases have the generic status of **500**.

.. caution::

    The current setup limits ressources of a single script execution to 128MB of RAM and 1 second of execution time!