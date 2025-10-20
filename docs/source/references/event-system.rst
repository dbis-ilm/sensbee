.. _ref-event-handler:

Event System
=====================

The Event System is an internal event generation and callback system that is usefull to track and debug sensor data activity.

Event Handler
-------------

It is possible to call generic web hooks. 

.. caution::

    This feature is work in progress!


Live Event in the :ref:`sbmi`
-----------------------------

Inside the SBMI, clicking on the time icon of a sensor, a request on the websocket tells the backend to subscribe to all events on that sensor.
It also loads previously recored events.
The button acts as a toggle, so clicking again unsubscribes the session from events on this sensor.

Clicking on an event inside the Live Event view shows more details about the event, like the recieved payload.

.. note::

    The current implementation only generates events for Sensor / Data API calls (only ingest & delete!).


.. caution::

    At the moment ingest requests via HTTP dont display the recieved payload!