// The Events API lets us monitor what happens inside of sensebee
// 
var SBMI = SBMI || {};

SBMI.eventsAPI = {
    // 
    init: () => {

        if (SBMI.session.isGuest()) {
            console.debug("not using eventsAPI due to Guest mode");
            return;
        }

        // check if there is already an open connection
        if ('socket' in SBMI.eventsAPI.state) {
            console.warn("closing older WS");
            SBMI.eventsAPI.state.socket.close();
        }

        let host_url = config.api.getURL();
        host_url.protocol = "ws";
        if (location.protocol === 'https:') {
            host_url.protocol = "wss";
        }
        let url = new URL("api/les/v1/stream/ws", host_url)
        url.searchParams.append("jwt", SBMI.session.getToken())
        let socket = new WebSocket(url);

        // Register handler
        const conState = document.getElementById("events-connection-state");
        socket.onopen = (event) => {
            console.debug("[LES] ws open", event);
            conState.classList.add("d-none");
        };
        socket.onclose = (event) => {
            console.debug("[LES] closed", event);
            conState.classList.remove("d-none");
        };
        socket.onmessage = (event) => {
            console.debug("[LES] ", JSON.parse(event.data));

            try {
                let log_event = JSON.parse(event.data);

                let details = SBMI.eventsAPI.pathToSymbols(log_event);
                let event_id = `${details.group}/${log_event.t}`;
                SBMI.eventsAPI.state.timeline.itemsData.add({
                    "id": event_id,
                    "start": vis.moment.utc(log_event.t),
                    "content": details.content,
                    "group": details.group,
                });

                SBMI.eventsAPI.state.events[event_id] = event.data;
            } catch (error) {
                console.error(error);
            }
        };

        SBMI.eventsAPI.state.socket = socket;
        SBMI.eventsAPI.state.events = {};

        // Show the log
        document.getElementById("accordionEvents").classList.remove("d-none");
        // DOM element where the Timeline will be attached
        const logvis = document.getElementById('logEvents-visualization');

        // Create a Timeline
        SBMI.eventsAPI.state.timeline = new vis.Timeline(logvis, new vis.DataSet([]), {
            rollingMode: { follow: true },
        });
        // add event listener
        SBMI.eventsAPI.state.timeline.on('select', function (properties) {
            if ('items' in properties) {
                if (properties.items.length > 0) {
                    let elem = SBMI.eventsAPI.state.events[properties.items[0]];
                    if (elem) {
                        alert(JSON.stringify(JSON.parse(elem), null, 2));
                    }
                }
            }
        });
    },
    showLiveSensorData: (id) => {
        SBMI.eventsAPI.socketShowChannel(id);
        delete SBMI.eventsAPI.state.is_general_channel;
    },
    showLiveData: () => {
        SBMI.eventsAPI.socketShowChannel("log_events");
        SBMI.eventsAPI.state.is_general_channel = true;
    },
    socketShowChannel: (channel) => {
        if (!'socket' in SBMI.eventsAPI.state) {
            return;
        }

        // Tell the backend to send us events on the requested channel
        SBMI.eventsAPI.state.socket.send(JSON.stringify({
            "sensor": channel,
        }));

        // Non general events shall be grouped by their sensor_id
        if (channel != "log_events") {
            let groups = SBMI.eventsAPI.state.timelinegroups;
            if (groups == undefined) {
                groups = [];
            }
            // If the group already exists, remove it
            if (groups.some(g => g.id === channel)) {
                // we unsubscribed on this channel, reomve its data as well
                let sensor = SBMI.sensorsAPI.sensors[channel];
                if (!sensor) {
                    console.error("there should have been a channel to unsubscribed from");
                    return;
                }
                let updatedGroups = SBMI.eventsAPI.state.timelinegroups.filter(e => e.id !== channel);
                SBMI.eventsAPI.state.timelinegroups = updatedGroups;
                SBMI.eventsAPI.state.timeline.setGroups(updatedGroups);
                return;
            }

            let sensor = SBMI.sensorsAPI.sensors[channel];
            groups.push({
                "id": channel,
                "content": sensor.name,
            });
            SBMI.eventsAPI.state.timelinegroups = groups;
            SBMI.eventsAPI.state.timeline.setGroups(groups);
        }
    },

    state: {},

    // this function renders a log_event into an HTML element
    // Goal is to make it visually easy to identify what the event actually contains
    pathToSymbols: (event) => {

        let path = event.path;
        let res = {};

        // A sensor event path is '/api/sensor/{id}/{op}'
        if (path.startsWith("/api/sensors/")) {
            let sensor_id = path.substring(13, 49);
            let op = path.substring(50);

            res["group"] = sensor_id;
            res["content"] = `${event.proto} ${op} ${event.status}`;
        } else {
            res["content"] = `${event.proto} ${event.path} ${event.status}`;
        }

        return res;
    },

    // --------
    // Event Handler

    // dict with handler_id:handler_info, populated via render
    handler: {},

    getHandler: async () => {

        if (!SBMI.eventsAPI.handler) {
            await SBMI.eventsAPI.render();
        }

        return Object.keys(SBMI.eventsAPI.handler).map(function (key) {
            return SBMI.eventsAPI.handler[key];
        });

        // TODO for some reason this doesnt update when a new one is created lol
    },

    openEventHandlerEdit: async (id) => {

        // Check user wants to edit existing or create new event handler
        var handler_info = ('handler' in SBMI.eventsAPI) ? SBMI.eventsAPI.handler[id] : undefined;
        if (handler_info) {
            handler_info = await SBMI.eventsAPI.load(id);
        }

        openOffcanvas(
            (handler_info !== undefined) ? handler_info.name : "New Event Handler",
            /*template*/`
            <form id="eventHandlerEditForm" onsubmit="SBMI.eventsAPI.saveEventHandlerScript(event, '${id}');return false;" class="mb-1">
                <div class="mb-3">
                    <label for="formGroupExampleInput" class="form-label">Name</label>
                    <input type="text" class="form-control" name="name" placeholder="Example input placeholder" value="${handler_info ? handler_info.name : ""}">
                </div>
                
                <hr>
                
                <div class="mb-3">
                    <label for="formGroupExampleInput2" class="form-label">Filter</label>
                    <input type="text" class="form-control" name="filter" placeholder="Another input placeholder" value="${handler_info ? handler_info.filter : ""}">
                </div>

                <hr>
                
                <div class="mb-3">
                    <label for="formGroupExampleInput2" class="form-label">Url</label>
                    <input type="text" class="form-control" name="url" placeholder="Another input placeholder" value="${handler_info ? handler_info.url : ""}">
                </div>
                <div class="mb-3">
                    <label for="formGroupExampleInput2" class="form-label">Method</label>
                    <input type="text" class="form-control" name="method" placeholder="Another input placeholder" value="${handler_info ? handler_info.method : ""}">
                </div>
            </div>

            <hr>
           
            <button type="submit" class="btn btn-primary">
                Save
            </button>
        `, true);

    },
    saveEventHandlerScript: async (event, id) => {
        event.preventDefault();

        const data = SBMI.helper.getFormDataAsJSON('eventHandlerEditForm');

        // check for update

        SBMI.eventsAPI.create(data)
            .then(e => {
                userFeedbackSucc("Event handler created");

                SBMI.eventsAPI.render();
            });
    },

    openEventHandlerDelete: (id) => {
        let t = SBMI.eventsAPI.handler[id];
        if (!t) {
            console.error(id + " does not exist in handler dict");
            return;
        }

        openModal(
            `Delete Event Handler`,
          /*template*/ `
          <form onsubmit="SBMI.eventsAPI.removeEventHandler('${id}');return false;">
            Are you sure you want to delete this Event handler?
            <div class="p-3 m-2">
              ${t.name}
              <br>
              ${t.id}
            </div>
            <button type="submit" class="btn btn-danger w-100">
              Delete
            </button>
            <div id="genericFormFeedback" class="mt-2 d-none"></div>
          </form>`,
        );
    },
    removeEventHandler: async (id) => {
        SBMI.eventsAPI.delete(id).then((e) => {
            userFeedbackSucc("Removed");
            SBMI.eventsAPI.render(id);
        });
    },

    // Render all event handler
    render: async () => {

        const handlerList = await SBMI.eventsAPI.list();

        const handlerDict = {};
        handlerList.forEach(elem => handlerDict[elem.id] = elem);
        SBMI.eventsAPI.handler = handlerDict;

        const eventHandlerUIElement = document.getElementById("accordionEventHandler");
        eventHandlerUIElement.classList.remove("d-none");

        document.getElementById("eventHandler-table-body").innerHTML = handlerList.map(e => /*template*/`
            <tr>
                <td>
                    ${e.name}
                </td>
                <td>
                    ${e.id}
                </td>
                <td>
                    <button type="button" class="btn btn-sm btn-primary" onclick="SBMI.eventsAPI.openEventHandlerEdit('${e.id}');" data-bs-toggle="offcanvas" data-bs-target="#offcanvasEnd" aria-controls="offcanvasEnd" >
                        <i class="bi bi-file-earmark-code"></i>
                    </button>
                    <button type="button" class="btn btn-sm btn-danger" onclick="SBMI.eventsAPI.openEventHandlerDelete('${e.id}')" data-bs-toggle="modal" data-bs-target="#globalModal">
                        <i class="bi bi-x-circle"></i>
                    </button>
                </td>
            </tr>
        `).join("");
    },

    // ###########################
    // SensBee API calls
    // ###########################

    /**
     * GET /api/event_handler/list
     * 
     * TODO
     *
     * @async
     * 
     * @returns {Promise<void>} [{}]
     *
     */
    list: async () => SBMI.auth.Request(`/api/event_handler/list`, "GET"),

    /**
    * GET /api/event_handler/{id}/load
    *
    * @async
    * 
    * @param {string} id - The UUID of the event_handler.
    * 
    * @returns {Promise<void>} EventHandler struct
    *
    */
    load: async (id) => SBMI.auth.Request(`/api/event_handler/${id}/load`, "GET"),

    /**
    * POST /api/event_handler/{id}/create
    * 
    * TODO
    *
    * @async
    * 
    * @returns {Promise<void>} mhm todo
    *
    */
    create: async (req) => SBMI.auth.Request(`/api/event_handler/create`, "POST", req),

    /**
    * POST /api/event_handler/{id}/update
    * 
    * TODO
    *
    * @async
    * 
    * @param {string} id - The UUID of the sensor to which the API key belongs.
    * 
    * @returns {Promise<void>} mhm todo
    *
    */
    update: async (id, req) => SBMI.auth.Request(`/api/event_handler/${id}/update`, "POST", req),

    /**
    * DELETE /api/event_handler/{id}/delete
    * 
    * TODO
    *
    * @async
    * 
    * @param {string} id - The UUID of the sensor to which the API key belongs.
    * 
    * @returns {Promise<void>} mhm todo
    *
    */
    delete: async (id) => SBMI.auth.Request(`/api/event_handler/${id}/delete`, "DELETE"),
};


