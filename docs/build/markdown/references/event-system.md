<a id="ref-event-handler"></a>

# Event System

The Event System is an internal event generation and callback system that is usefull to track and debug sensor data activity.

## Event Handler

It is possible to call generic web hooks.

## Live Event in the [SensBee Management Interface](../user-guide/sbmi.md#sbmi)

Inside the SBMI, clicking on the time icon of a sensor, a request on the websocket tells the backend to subscribe to all events on that sensor.
It also loads previously recored events.
The button acts as a toggle, so clicking again unsubscribes the session from events on this sensor.

Clicking on an event inside the Live Event view shows more details about the event, like the recieved payload.

#### NOTE
The current implementation only generates events for Sensor / Data API calls (only ingest & delete!).
