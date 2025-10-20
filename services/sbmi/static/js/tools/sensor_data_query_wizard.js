/*

    The Query Wizard allows accessing data of a sensor
    -> It lets you select the sensor and values based on already present data
    -> Also lets you edit the final query manually


    Also add a dropdown or smth to a sensor to run queries for it directly?
    -> they are in the localStorage so the setup adds them when they are found?

    TODOs
    - add automatic overflow to query filed
    - add common params

*/

class SensorDataQueryWizard {
    constructor() {

        this.state = {
            targetID: "queryWizardWrapper",
            sensors: SBMI.sensorsAPI.get_sensors(),
            url: config.api.getURL(),
        };
    }

    _sensorSelectChanged() {
        this._updateAvailableAPIKeys();
        this._updateQueryString();
    }

    async _updateAvailableAPIKeys() {
        const sensor_id = document.getElementById("sdqwSensorSelect").value;
        if (sensor_id) {
            const s = await SBMI.sensorsAPI.loadSensorInfo(sensor_id);
            if ('info' in s) {
                // Save info to decode returned data
                this.state.sensor_info = s.info;

                // Set available API Keys
                const k = s.info.api_keys;
                const t = document.getElementById("sdqwAPIKeySelect");
                t.innerHTML = k.map((key) => key.operation == "READ" ? `<option value="${key.id}">${key.name}</option>` : "").join("") + '<option value="" selected>None</option>';
            } else {
                delete this.state.sensor_info;
            }
        }
    }

    _updateQueryString() {
        // Get Sensor ID
        const cur_sensor_id = document.getElementById("sdqwSensorSelect").value;

        this.state.url = new URL("/api/sensors/" + cur_sensor_id + "/data/load", config.api.getURL());

        // Add API Key
        const k = document.getElementById("sdqwAPIKeySelect").value;
        if (k) {
            this.state.url.searchParams.append("key", k);
        }

        // Get Options
        // "from":"2024-12-07T12:00:00","to":null,"limit":10,"ordering":"DESC"

        document.getElementById("sdqwQuery").value = this.state.url.toString();
    }

    executeQuery() {
        fetch(document.getElementById("sdqwQuery").value).then((r) => {
            r.json().then((j) => {
                const t = document.getElementById("resultWrapper");
                try {
                    t.innerHTML = `<hr>
                    <table class="table table-striped">
                        <thead>
                            <tr>
                                <th scope="col">created_at</th>
                                ${'sensor_info' in this.state ? `
                                    ${this.state.sensor_info.sensor_info.columns.map((c) => `
                                        <th scope="col">${c.name}</th>
                                    `).join("")}
                                `: '<th scope="col">Values</th>'}
                            </tr>
                        </thead>
                        <tbody>
                        ${j.map((e) => `
                        <tr>
                            <th scope="row">${e.created_at}</th>
                            ${'sensor_info' in this.state ? `
                                ${this.state.sensor_info.sensor_info.columns.map((c) => `
                                    <td>${e[c.name]}</td>
                                `).join("")}
                            `: '<td>' + JSON.stringify(e) + '</td>'}
                        </tr>
                        `).join("")}
                        </tbody>
                    </table>`;
                } catch (error) {
                    console.error(error);
                    // Fallback print
                    t.innerHTML = '<hr>' + JSON.stringify(j, null, 2);
                }
            });
        });
    }

    // --- Rendering Functions ---

    openModal() {
        // Opens the modal where the form will render itself to
        const m = document.getElementById("globalModal");
        if (!m.classList.contains("modal-xl")) {
            m.classList.add("modal-xl");
        }

        const t = m.querySelector(".modal-title");
        t.innerHTML = "Sensor Data Query Wizard";

        const b = m.querySelector(".modal-body");
        const d = document.createElement("div");
        d.id = this.state.targetID;
        b.innerHTML = "";
        b.appendChild(d);

        this.render();
    }

    render() {
        const t = document.getElementById(this.state.targetID);
        t.innerHTML = `
            <div hidden>
                <div class="btn-group">
                    <div class="input-group mb-3">
                        <span class="input-group-text" id="basic-addon1"><i class="bi bi-floppy"></i></span>
                        <input type="text" class="form-control" placeholder="query name" aria-label="Username" aria-describedby="basic-addon1">
                    </div>
                    <button type="button" class="btn btn-secondary dropdown-toggle dropdown-toggle-split" data-bs-toggle="dropdown" aria-expanded="false" data-bs-reference="parent">
                        <span class="visually-hidden">Toggle Dropdown</span>
                    </button>
                    <ul class="dropdown-menu">
                        <li><a class="dropdown-item" href="#">Query 1</a></li>
                        <li><a class="dropdown-item" href="#">Another action</a></li>
                    </ul>
                </div>
                <br>
                Export-> generates encoded config for the url?
                <br>
                Import-> opens input field to paste code?
            </div>
            <br>
            <div class="mb-3">
                <label for="basic-url" class="form-label">Generated Query</label>
                <div class="input-group">
                    <span class="input-group-text" id="basic-addon3"><i class="bi bi-code-slash"></i></span>
                    <input id="sdqwQuery" type="text" class="form-control" id="basic-url" aria-describedby="basic-addon3 basic-addon4" value="${config.api.getURL().toString()}">
                </div>
                <div class="form-text" id="basic-addon4">This query is generated based on the following options. You can also edit it yourself.</div>
            </div>
            <br>
            <select id="sdqwSensorSelect" class="form-select" aria-label="Sensor selection">
                <option value="" selected>Select sensor</option>
            ${this.state.sensors.map((sensor) => `
                <option value="${sensor.id}">${sensor.name}</option>
            `).join("")}
            </select>
            <br>
            <div class="mb-3">
                <label for="basic-url" class="form-label">API Key</label>
                <div class="input-group">
                    <span class="input-group-text" id="basic-addon3"><i class="bi bi-code-slash"></i></span>
                    <select id="sdqwAPIKeySelect" class="form-select" aria-label="Sensor selection">
                        <option value="" selected>None</option>
                    </select>
                </div>
                <div class="form-text" id="basic-addon4">Only READ keys shown</div>
            </div>
            <div hidden>
                Query Options
                <br>
                TODO
            </div>
            <button id="sdqwExecute" type="button" class="btn btn-primary">Run query</button>
            <br>
            <div id="resultWrapper"></div>
        `;
        document.getElementById("sdqwSensorSelect").onchange = this._sensorSelectChanged.bind(this);
        document.getElementById("sdqwAPIKeySelect").onchange = this._updateQueryString.bind(this);
        document.getElementById("sdqwExecute").onclick = this.executeQuery.bind(this);
    }
}