
// Create a shared namespace if it doesn't exist yet
var SBMI = SBMI || {};

SBMI.sensorsAPI = {

    sensors: {},

    // NOTE The presence of the id parameter governs if the form is used for sensor update or creation 
    openSensorCreate: (id) => {
        openOffcanvas(
            "Create new sensor",
            /*template*/`
            <form id="createNewSensorForm" onsubmit="SBMI.sensorsAPI.createNewSensor(event, '${id ? id : ''}')">
                <div class="mb-3">
                    <label for="name" class="form-label">Name</label>
                    <input name="name" type="text" class="form-control" required>
                </div>

                <div class="mb-3">
                    <label for="name" class="form-label">Description</label>
                    <textarea name="description" class="form-control" placeholder="More detailed description" id="InputSensorDescription"></textarea>
                </div>

                <hr>
                <!-- Input Field for Coordinates -->
                <div class="input-group mb-1">
                    <label for="locationInput" class="form-label">Location</label>
                    <div class="input-group">
                        <input name="position-lat" id="locationInput-lat" type="text" class="form-control" placeholder="Latitude" required>
                        <input name="position-lng" id="locationInput-lng" type="text" class="form-control" placeholder="Longitude" required>
                        <!-- Recenter to starting position -->
                        <button class="btn btn-primary" type="button" onclick="SBMI.sensorsAPI.creationMap.setView(SBMI.sensorsAPI.creationMapMarker.getLatLng(), 13)">
                            <i class="bi bi-geo-alt-fill"></i>
                        </button>
                        <button class="btn btn-primary" type="button" onclick="SBMI.sensorsAPI.creationMap.setView([50.68322,10.91858], 13)">
                            <i class="bi bi-arrow-repeat"></i>
                        </button>
                    </div>
                </div>
                <!-- NOTE This is broken inside a modal -->
                <div id="sensorLocationPicker" class="mb-1 p-1 border border-primary rounded" style="min-height: 300px;"></div>
                
                <div id="sensorCreateColumns">
                    <hr>
                    <div class="row">
                        <div class="col">
                            <label class="form-label">Columns</label>
                        </div>
                        <div class="col-3">
                            <button type="button" class="btn btn-sm btn-secondary" onclick="SBMI.sensorsAPI.addColToSensorCreation();">
                                Add column
                            </button>
                        </div>
                    </div>
                    <div id="sensorCreationCols">
                    </div>
                </div>

                <hr>
                <label class="form-label">Storage</label>
                <select id="storageType" name="storage" class="form-select" required>
                    <option value="DEFAULT" selected>Default</option>
                    <optgroup label="Ringbuffer">
                        <option value="RINGBUFFERCOUNT">Count</option>
                        <option value="RINGBUFFERINTERVAL">Interval</option>
                    </optgroup>
                </select>

                <div id="DEFAULT-form" class="storage-form">
                    <!-- No default options -->
                </div>

                <div id="RINGBUFFERCOUNT-form" class="storage-form d-none">
                    <div class="input-group mb-3">
                        <span class="input-group-text">Entries</span>
                        <input id="RINGBUFFERCOUNT-input" name="rbcounter" type="number" class="form-control" placeholder="10">
                    </div>
                </div>
                <div id="RINGBUFFERINTERVAL-form" class="storage-form d-none">
                    <div class="input-group mb-3">
                        <span class="input-group-text">Interval in minutes</span>
                        <input id="RINGBUFFERINTERVAL-input" name="rbinterval" type="float" class="form-control" placeholder="10.5">
                    </div>
                </div>

                <div id="createSensorPerms">
                    <hr>
                    <label class="form-label">Permissions</label>
                    ${Object.entries(SBMI.rolesAPI.roles).map(([key,role]) => 
                    /*template*/`
                        <div class="input-group w-100 mb-1">
                            <span class="input-group-text w-50">
                                ${SBMI.rolesAPI.renderRole(role.id)}
                            </span>

                            <input name="role_id" class="form-control d-none" type="text" value="${role.id}" autocomplete="off" readonly>
                        
                            <input name="role-perm" type="checkbox" class="btn-check" id="role-${role.id}-info" value="INFO" autocomplete="off">
                            <label class="btn btn-outline-primary" for="role-${role.id}-info">INFO</label>

                            <input name="role-perm" type="checkbox" class="btn-check" id="role-${role.id}-read" value="READ" autocomplete="off">
                            <label class="btn btn-outline-primary" for="role-${role.id}-read">READ</label>

                            <input name="role-perm" type="checkbox" class="btn-check" id="role-${role.id}-write" value="WRITE" autocomplete="off">
                            <label class="btn btn-outline-primary" for="role-${role.id}-write">WRITE</label>
                        </div>
                    `).join("")}
                </div>

                <hr>

                <button id="sensorCreateFormSubmitBtn" type="submit" class="btn btn-primary w-100">
                    Create Sensor
                </button>
                <div id="createSensorFormFeedback" class="mt-2 d-none">
                </div>
            </form>`,
        );

        // Initialize the map to Ilmenau
        // TODO ENV make this configurable
        SBMI.sensorsAPI.creationMap = L.map('sensorLocationPicker').setView([50.68322,10.91858], 13);
        L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
            attribution: '© OpenStreetMap contributors'
        }).addTo(SBMI.sensorsAPI.creationMap);
        // Add a marker that moves on click
        let marker;
        SBMI.sensorsAPI.creationMap.on('click', function(e) {
            const { lat, lng } = e.latlng;

            // Update marker position or create a new one
            if (marker) {
                marker.setLatLng([lat, lng]);
                
            } else {
                marker = L.marker([lat, lng], { draggable: true }).addTo(SBMI.sensorsAPI.creationMap);
                // Update the inputs on drag end
                marker.on('dragend', function (event) {
                    const position = event.target.getLatLng();

                    // Display coordinates
                    document.getElementById('locationInput-lat').value = position.lat.toFixed(6);
                    document.getElementById('locationInput-lng').value = position.lng.toFixed(6);
                });
                SBMI.sensorsAPI.creationMapMarker = marker;
            }

            // Display coordinates
            document.getElementById('locationInput-lat').value = lat.toFixed(6);
            document.getElementById('locationInput-lng').value = lng.toFixed(6);
        });
        // Functions to update the marker position on user input
        //
        function updateMarker(lat, lng) {
            if (marker) {
                marker.setLatLng([lat, lng]); // Update marker position
                map.setView([lat, lng], 13); // Optionally recenter the map
            } else {
                marker = L.marker([lat, lng], { draggable: true }).addTo(SBMI.sensorsAPI.creationMap);
            }
        }
        function isValidLatLng(lat, lng) {
            return (
                !isNaN(lat) && lat >= -90 && lat <= 90 &&
                !isNaN(lng) && lng >= -180 && lng <= 180
            );
        }
        const latInput = document.getElementById('locationInput-lat');
        const lngInput = document.getElementById('locationInput-lng');
        latInput.addEventListener('input', () => {
            const lat = parseFloat(latInput.value);
            const lng = parseFloat(lngInput.value);

            if (isValidLatLng(lat, lng)) {
                updateMarker(lat, lng);
            }
        });
        lngInput.addEventListener('input', () => {
            const lat = parseFloat(latInput.value);
            const lng = parseFloat(lngInput.value);

            if (isValidLatLng(lat, lng)) {
                updateMarker(lat, lng);
            }
        });

        // Storage options
        const storageType = document.getElementById("storageType");
        const forms = document.querySelectorAll(".storage-form");

        // Show the relevant form based on the selected storage type
        storageType.addEventListener("change", () => {
            forms.forEach(form => {
                form.classList.add("d-none"); 
                let t = document.getElementById(form.id.split("-")[0]+"-input")
                if(t){t.removeAttribute("required")};
            });
            const selectedForm = document.getElementById(storageType.value+"-form");
            if (selectedForm) {
                selectedForm.classList.remove("d-none");
                let t = document.getElementById(storageType.value+"-input")
                if(t){t.setAttribute("required", "true")};
            }
        });
    },
    addColToSensorCreation: () => {
        // TODO make the remove btn work
        let newCOl = document.createElement("div");
        newCOl.classList.add("input-group");
        newCOl.classList.add("my-3");
        newCOl.innerHTML=
        /*template*/`
            <input name="colName" type="text" class="form-control" placeholder="Column name" aria-label="column name">
            <select name="colType" class="form-select">
                <option value="UNKNOWN" selected>UNKNOWN</option>
                <option value="INT">INT</option>
                <option value="FLOAT">FLOAT</option>
                <option value="STRING">STRING</option>
            </select>
            <input name="colUnit" type="text" class="form-control" placeholder="Unit" aria-label="unit">
            <button class="btn btn-outline-danger" type="button"><i class="bi bi-x-circle"></i></button>
        `;
        document.getElementById("sensorCreationCols").append(newCOl);
    },
    // NOTE due to the complexity of the parsing of sensor information this function can also be used to update
    // an existing sensor. Simply set the second parameter to a truthy value then the function will update instead
    // of creating a new sensor.
    createNewSensor: async (event, id) => {
        event.preventDefault();

        let f = SBMI.helper.getFormData('createNewSensorForm');

        info = {columns: [], permissions: []};
        curRole = "";
        storageVar = "";
        f.forEach((value, name)=>{
            if(name == "colUnit"){
                // this marks the end of a column value
                // extract it and add it to the columns array
                info.columns.push({
                    "name": info.colName,
                    "val_type":info.colType,
                    "val_unit":value,
                });
                delete info.colName;
                delete info.colType;
                return;
            }
            if(name == "position-lng"){
                info.position = [parseFloat(info["position-lat"]),parseFloat(value)];
                delete info["position-lat"];
                return;
            }
            if(name == "storage"){
                info.storage = {
                    params: {},
                    variant: value,
                };
                storageVar = value;
                return;
            }
            if(name == "role_id"){
                if(curRole !=""){
                    delete info["role-perm"];
                }
                curRole = value;
                return;
            }
            if(name=="role-perm"){
                let e = info.permissions.find(e => e.role_name == curRole);
                if(e){
                    e.operations.push(value);
                }else{
                    info.permissions.push({
                        "role_id":curRole,
                        "operations":[value],
                    });
                }
                return;
            }

            if(name=="rbcounter"){
                if(storageVar=="RINGBUFFERCOUNT"){
                    let counter = parseInt(value);
                    info.storage.params = {"count": counter};
                    storageVar = "";
                }
                return;
            }
            if(name=="rbinterval"){
                if(storageVar=="RINGBUFFERINTERVAL"){
                    let interval = parseFloat(value);
                    info.storage.params = {"interval": interval};
                    storageVar = "";
                }
                return;
            }

            info[name]=value;
        });

        // Check if this is an update or create request
        if(id){
            delete info.columns;

            SBMI.sensorsAPI.edit(id, info)
            .then(() => {
                formFeedback("createSensorFormFeedback", `Updated sensor`);
    
                // reload user list
                SBMI.sensorsAPI.render();
            })
            .catch(error => formFeedbackErr("createSensorFormFeedback",`Update failed with: ${error}`));
        }else{
            SBMI.sensorsAPI.create(info)
            .then(resp => {
                formFeedback("createSensorFormFeedback", `Created sensor with id: ${resp.uuid}`);
    
                // reload user list
                SBMI.sensorsAPI.render();
            })
            .catch(error => formFeedbackErr("createSensorFormFeedback",`Sensor creation failed with: ${error}`));
        }
    },

    parsePermissionBits: (bitSet) => {
        if (typeof bitSet === 'string') {
            const splitValues = bitSet.split(',');
            return splitValues.map(item => item.trim());
        }

        if (Number.isInteger(bitSet)) {
            const bitmap = [
                "Info",
                "Read",
                "Write",
                "Edit",
                "Delete",
                "ApiKeyRead",
                "ApiKeyWrite"
            ];
        
            const activeBits = [];
        
            for (let i = 0; i < bitmap.length; i++) {
                if (bitSet & (1 << i)) {
                    activeBits.push(bitmap[i]);
                }
            }
        
            return activeBits;
        }

        console.error("Unsupported type for bitSet:", bitSet);
    },

    loadSensorInfo: async (id) => {
        let sensor = SBMI.sensorsAPI.sensors[id];
        if(!sensor){
            console.error(id + " does not exist in sensors dict");
            return;
        }

        let info = await SBMI.sensorsAPI.info(id);
        let owner = SBMI.usersAPI.users[info.sensor_info.owner];
        
        info.bit_set = SBMI.sensorsAPI.parsePermissionBits(info.bit_set);

        document.getElementById('sensorInfo-'+id).innerHTML =  
        /*template*/`
            <div class="row p-1 m-0">
                <div class="col-2 p-2 border rounded">
                    <p class="fw-light">Owner</p>
                    ${owner ? owner.email :  info.sensor_info.owner}

                    <hr>

                    <p class="fw-light">Description</p>
                    ${info.sensor_info.description}
                </div>
                <div class="col-2 p-2 border rounded">

                    <p class="fw-light">Columns</p>
                    ${info.sensor_info.columns.map(col => 
                    /*template*/`
                        <span>
                        ${col.val_type}
                        </span>
                        <span>
                        ${col.name}
                        </span>
                        <span>
                        (${col.val_unit})
                        </span>
                    `).join("")}
                    
                    <hr>
               
                    <p class="fw-light">Positon</p>
                    ${info.sensor_info.position[0]},${info.sensor_info.position[1]}
                    
                    <hr>

                    <p class="fw-light">Storage</p>
                    ${info.sensor_info.storage_type}
                    <br>
                    ${JSON.stringify(info.sensor_info.storage_params)}


                </div>
                <div class="col-4 p-2 border rounded">
                    <p class="fw-light">Permissions</p>
                    ${SBMI.sensorsAPI.renderPermissions(info.sensor_info.permissions)}
                    
                    <hr>

                    <p class="fw-light">Permission bits</p>
                    ${info.bit_set}
                </div>
                <div class="col p-2 border rounded">
                    <div class="">
                        <span class="fw-light">API Keys</span>
                        <span class="float-end">
                            <button type="button" class="btn btn-secondary" onclick="SBMI.sensorsAPI.openAddAPIKey('${id}');" data-bs-toggle="modal" data-bs-target="#globalModal">
                                <i class="bi bi-key"></i><i class="bi bi-node-plus"></i>
                            </button>
                        </span>
                    </div>

                    <table class="table table-striped">
                        <thead>
                            <tr>
                                <th scope="col">Name</th>
                                <th scope="col">ID</th>
                                <th scope="col">User</th>
                                <th scope="col">Operation</th>
                                <th scope="col"></th>
                            </tr>
                        </thead>
                        <tbody>
                        ${info.api_keys.map(key => 
                        /*template*/`
                            <tr>
                                <td>${key.name}</td>
                                <td class="fw-light">${key.id}</td>
                                <td>${SBMI.usersAPI.renderUser(key.user_id)}</td>
                                <td>${key.operation}</td>
                                <td>
                                    <button type="button" class="btn btn-danger" onclick="SBMI.sensorsAPI.openDeleteAPIKey('${id}','${key.id}');" data-bs-toggle="modal" data-bs-target="#globalModal">
                                        <i class="bi bi-x-circle-fill"></i>
                                    </button>
                                </td>
                            </tr>
                        `).join("")}
                        </tbody>
                    </table>
                </div>
            </div>
        `;

        // Save info to sensor dict entry
        SBMI.sensorsAPI.sensors[id].info = info;
    },

    openAddAPIKey: (id) => {
        openModal(
            `Create API Key`,
            /*template*/`
                <form id="createAPIKeyForm" onsubmit="SBMI.sensorsAPI.createAPIKey(event, '${id}')">
                    <div class="mb-3">
                        <label for="nameInput" class="form-label">Name</label>
                        <input name="name" type="text" class="form-control" id="nameInput"  required>
                    </div>

                    <div class="mb-3">
                        <label for="opSelect" class="form-label">Name</label>
                        <select name="operation" id="opSelect" class="form-select" aria-label="Default select example">
                            <option value="READ">Read</option>
                            <option value="WRITE">Write</option>
                        </select>
                    </div>

                    <button type="submit" class="btn btn-primary w-100">Submit</button>
                    <div id="createAPIKeyFormFeedback" class="mt-2 d-none">
                    </div>
                </form>
            `,
        );
    },
    createAPIKey: async (event, id) => {
        event.preventDefault();

        SBMI.sensorsAPI.APIKeyCreate(id, SBMI.helper.getFormDataAsJSON('createAPIKeyForm'))
        .then(newRole => {
            formFeedback("createAPIKeyFormFeedback",`Created API Key with id: ${newRole.id}`);
    
            // reload user list
            SBMI.sensorsAPI.loadSensorInfo(id);
        })
        .catch(error => formFeedbackErr("createAPIKeyFormFeedback",`Failed with: ${error}`));
    },

    openSensorEdit: async (id) => {

        let sensor = SBMI.sensorsAPI.sensors[id];
        if(!sensor){
            console.error(sid + " does not exist in sensors dict");
            return;
        }
        let info = await SBMI.sensorsAPI.info(id);

        // Use the sensor creation form
        SBMI.sensorsAPI.openSensorCreate(id);

        let form = document.getElementById("createNewSensorForm");

        // hide columns options as we cant change them anymore
        document.getElementById("sensorCreateColumns").classList.add("d-none");

        // name and description
        form.elements["name"].value = info.sensor_info.name;
        form.elements["description"].value = info.sensor_info.description;

        // position
        form.elements["position-lat"].value = info.sensor_info.position[0];
        form.elements["position-lng"].value = info.sensor_info.position[1];
        form.elements["position-lng"].dispatchEvent(new Event('input', { bubbles: true }));

        // storage
        form.elements["storage"].value = info.sensor_info.storage_type;
        form.elements["storage"].dispatchEvent(new Event('change', { bubbles: true }));
        if(info.sensor_info.storage_params.count){
            document.getElementById("RINGBUFFERCOUNT-input").value = info.sensor_info.storage_params.count;
        }
        if(info.sensor_info.storage_params.interval){
            document.getElementById("RINGBUFFERINTERVAL-input").value = info.sensor_info.storage_params.interval;
        }

        // permissions
        for (i in info.sensor_info.permissions){
            let p = info.sensor_info.permissions[i];
            document.getElementById("role-"+p.role_id+"-info").checked = p.allow_info;
            document.getElementById("role-"+p.role_id+"-read").checked = p.allow_read;
            document.getElementById("role-"+p.role_id+"-write").checked = p.allow_write;
        }

        // Update the button text
        document.getElementById("offcanvasHeader").innerHTML = `Edit ${info.sensor_info.name}`;
        document.getElementById("sensorCreateFormSubmitBtn").innerHTML = "Update sensor";
    },

    /**
     * Open a Dialog to confirm the deletion of the given API Key
     * 
     * @param {Array<perms>} perms - the permissions to render. A single perm is {"role_id":uuid,"sensor_id":uuid, }
     * 
     * @returns void
     */
    renderPermissions: (perms) => {

        // role

        // sensor - always self so no point in rendering that

        return perms.map(perm =>
            /*template*/`
            <div class="input-group w-100 mb-1">
                <span class="input-group-text w-50">
                    ${SBMI.rolesAPI.renderRole(perm.role_id)}
                </span>

                <input name="role-perm" type="checkbox" class="btn-check"  ${perm.allow_info ? "checked" : ""} disabled>
                <label class="btn btn-outline-primary" >INFO</label>

                <input name="role-perm" type="checkbox" class="btn-check"  ${perm.allow_read ? "checked" : ""} disabled>
                <label class="btn btn-outline-primary" >READ</label>

                <input name="role-perm" type="checkbox" class="btn-check" ${perm.allow_write ? "checked" : ""} disabled>
                <label class="btn btn-outline-primary" >WRITE</label>
            </div>
            `
        ).join("")
    },

    /**
     * Open a Dialog to confirm the deletion of the given API Key
     * 
     * @param {uuid} sid - the sensor ID to which the API Key belongs
     * @param {uuid} kid - the API Key ID to delete
     * 
     * @returns void
     */
    openDeleteAPIKey: (sid, kid) => {
        let sensor = SBMI.sensorsAPI.sensors[sid];
        if(!sensor){
            console.error(sid + " does not exist in sensors dict");
            return;
        }
        let apiKey = sensor.info.api_keys.find(key => key.id == kid);
        if(!apiKey){
            console.error(kid + " does not exist in sensor info");
            return;
        }

        openModal(
            `Delete API Key`,
            /*template*/`
            <form onsubmit="SBMI.sensorsAPI.deleteAPIKey(event, '${sid}','${kid}')">
                Are you sure you want to delete this API Key?
                <div class="p-3 m-2">
                    ${apiKey.name}
                    <br>
                    ${apiKey.id}
                    <br>
                    ${apiKey.operation}
                    <br>
                    Sensor: ${SBMI.sensorsAPI.renderSensor(apiKey.sensor_id)}
                    <br>
                    User: ${SBMI.usersAPI.renderUser(apiKey.user_id)}
                </div>
                <button type="submit" class="btn btn-danger w-100">
                    Delete
                </button>
                <div id="genericFormFeedback" class="mt-2 d-none">
                </div>
            </form>`,
        );
    },
    deleteAPIKey: (event, id, key_id)=>{
        event.preventDefault();

        SBMI.sensorsAPI.APIKeyDelete(id, key_id)
        .then(() => {
            // TODO close modal
            SBMI.sensorsAPI.loadSensorInfo(id);
        })
        .catch(error => formFeedbackErr("genericFormFeedback",`Failed with: ${error}`));
    },

    openSensorDelete: (id) => {
        let sensor = SBMI.sensorsAPI.sensors[id];
        if(!sensor){
            console.error(sid + " does not exist in sensors dict");
            return;
        }

        openModal(
            `Delete Sensor`,
            /*template*/`
            <form onsubmit="SBMI.sensorsAPI.deleteSensor(event, '${id}')">
                Are you sure you want to delete this sensor and all associated data?
                <div class="p-3 m-2">
                    ${sensor.name}
                    <br>
                    ${sensor.id}
                </div>
                <button type="submit" class="btn btn-danger w-100">
                    Delete
                </button>
                <div id="genericFormFeedback" class="mt-2 d-none">
                </div>
            </form>`,
        );
    },
    deleteSensor: (event, id)=>{
        event.preventDefault();

        SBMI.sensorsAPI.delete(id)
        .then(() => {
            // TODO close modal
            SBMI.sensorsAPI.render();
        })
        .catch(error => formFeedbackErr("genericFormFeedback",`Failed with: ${error}`));
    },

    showOnMap: async (id) => {

        // check if sensor info is loaded
        let info = SBMI.sensorsAPI.sensors[id].info;
        if(!info){
            await SBMI.sensorsAPI.loadSensorInfo(id);
            info = SBMI.sensorsAPI.sensors[id].info;
        }

        // If this is the first call to this function we need to do some init as well
        if(!SBMI.sensorsAPI.isInitialized){
            let globalMapDiv = document.getElementById("generalSensorMap");
            globalMapDiv.classList.remove("d-none");

            // Apply saved height and width
            let mapHeight = localStorage.getItem('sensorMapHeight');
            if(mapHeight){
                globalMapDiv.style.height = mapHeight + "px";
            }
            let mapWidth = localStorage.getItem('sensorMapWidth');
            if(mapWidth){
                globalMapDiv.style.width = mapWidth + "px";
            }
            
        
            SBMI.sensorsAPI.map = L.map("generalSensorMap");
            //.setView(info.sensor_info.position, 13);
            L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
                attribution: '© OpenStreetMap contributors'
            }).addTo(SBMI.sensorsAPI.map);

            // Add Resizer Control
            L.control.resizer().addTo(SBMI.sensorsAPI.map);

            // Once a resize has happened we want to store the size locally
            let resizeObserver = new ResizeObserver(() => {
                const { width, height } = globalMapDiv.getBoundingClientRect();
                localStorage.setItem('sensorMapHeight', Math.round(height));
                localStorage.setItem('sensorMapWidth', Math.round(width));
            });
            resizeObserver.observe(globalMapDiv);



            SBMI.sensorsAPI.isInitialized = true;
        }

        if(info.mapData){

            SBMI.sensorsAPI.map.removeLayer(SBMI.sensorsAPI.sensors[id].info.mapData.mapMarker);

            document.getElementById("showSensorOnMapBtn-"+id).classList.remove("active");

            delete SBMI.sensorsAPI.sensors[id].info.mapData;
        }else{
            // Has no marker yet so create one
            
            // , {"title": sensor.name,}
            let marker = L.marker([info.sensor_info.position[0], info.sensor_info.position[1]]).addTo(SBMI.sensorsAPI.map);

            // set view to currently added map with marker
            SBMI.sensorsAPI.map.setView(info.sensor_info.position, 13);

            document.getElementById("showSensorOnMapBtn-"+id).classList.add("active");

            SBMI.sensorsAPI.sensors[id].info.mapData = {"mapMarker": marker};
        }
    },

    render: async () => {
        if(!SBMI.session.isLoggedIn()){
            return;
        }

        const sensorlist = await SBMI.sensorsAPI.list();

        document.getElementById("sensor-table-body").innerHTML = sensorlist.map(sensor => 
        /*template*/`
        <tr>
            <td>
                ${sensor.name}
            </td>
            <td>
                ${sensor.id} 
            </td>
            <td>
                <button type="button" class="btn btn-sm btn-primary" onclick="SBMI.sensorsAPI.loadSensorInfo('${sensor.id}');" data-bs-toggle="collapse" data-bs-target="#sensorInfo-${sensor.id}" aria-expanded="false" aria-controls="sensorInfo-${sensor.id}">
                    <i class="bi bi-info-circle-fill"></i>
                </button>
                <button id="showSensorOnMapBtn-${sensor.id}" type="button" class="btn btn-sm btn-secondary" onclick="SBMI.sensorsAPI.showOnMap('${sensor.id}')">
                    <i class="bi bi-map"></i>
                </button>
                <button type="button" class="btn btn-sm btn-primary" onclick="SBMI.sensorsAPI.openSensorEdit('${sensor.id}');" data-bs-toggle="offcanvas" data-bs-target="#offcanvasEnd" aria-controls="offcanvasEnd">
                    <i class="bi bi-pencil-square"></i>
                </button>
                <button type="button" class="btn btn-sm btn-danger" onclick="SBMI.sensorsAPI.openSensorDelete('${sensor.id}')" data-bs-toggle="modal" data-bs-target="#globalModal">
                    <i class="bi bi-x-circle"></i>
                </button>
                
            </td>
        </tr>
        <tr>
            <td colspan="4" class="p-0">
                <div id="sensorInfo-${sensor.id}" class="collapse">
                </div>
            </td>
        </tr>
        `).join("");

        // update sensor dict
        const sensorsdict = {};
        sensorlist.forEach(sensor => sensorsdict[sensor.id] = sensor);
        SBMI.sensorsAPI.sensors = sensorsdict;
    },

    renderSensor: (id) => {
        let sensor = SBMI.sensorsAPI.sensors[id];
        if(!sensor){
            console.error(id + " does not exist in sensors dict");
            return;
        }

        return /*template*/`
        <div class="container">
            ${sensor.name}
        </div>
        `;
    },

    // ###########################
    // SensBee Sensor API calls
    // ###########################

    /**
     * POST /api/sensors/create
     * 
     * Creates an API key for a specific sensor.
     *
     * @async
     * 
     * @param {Object} info - TODO
     * 
     * @returns {Promise<GenericUuidResponse>} Object containing the UUID of the created sensor.
     *
     */
    create: async (info) => SBMI.auth.Request(`/api/sensors/create`, "POST", info),

    /**
     * GET /api/sensors/list
     * 
     * Lists all sensors that the current user is allowed to see.
     *
     * @async
     * 
     * 
     * @returns {Promise<Array<Sensor>>} The list of sensors.
     *
     */
    list: async () => SBMI.auth.Request(`/api/sensors/list`),

    /**
     * GET /api/sensors/{id}
     * 
     * Retrieve detailed information of the given sensor.
     *
     * @async
     * 
     * @param {string} id - The UUID of the sensor for which to get more detailed information the API key.
     * 
     * @returns {Promise<SensorDetailed>} The details of the sensor.
     *
     */
    info: async (id) => SBMI.auth.Request(`/api/sensors/${id}/info`),

    /**
     * POST /api/sensors/{id}/edit
     * 
     * Update the information of the specified sensor. 
     * NOTE Columns can not be changed.
     *
     * @async
     * 
     * @param {string} id - The UUID of the sensor for which to get more detailed information the API key.
     * @param {Object} info - The updated sensor information.
     * 
     * @returns {Promise<void>}
     *
     */
    edit: async (id, info) => SBMI.auth.Request(`/api/sensors/${id}/edit`, "POST", info),

    /**
     * DELETE /api/sensors/{id}/delete
     * 
     * Detele the specified sensor and all associated data.
     *
     * @async
     * 
     * @param {string} id - The uuid of the sensor to delete.
     * 
     * @returns {Promise<void>}
     *
     */
    delete: async (id) => SBMI.auth.Request(`/api/sensors/${id}/delete`, "DELETE"),

    /**
     * POST /api/sensors/{id}/api_key/create
     * 
     * Creates an API key for a specific sensor.
     *
     * @async
     * 
     * @param {string} id - The UUID of the sensor for which to create the API key.
     * @param {Object} info - TODO
     * 
     * @returns {Promise<string>} The UUID of the created API key.
     *
     */
    APIKeyCreate: async (id, info) => SBMI.auth.Request(`/api/sensors/${id}/api_key/create`, "POST", info),

    /**
     * DELETE /api/sensors/{id}/api_key/{key_id}/delete
     * 
     * Deletes the given API Key of the given sensor ID.
     *
     * @async
     * 
     * @param {string} id - The UUID of the sensor to which the API key belongs.
     * @param {string} key_id - The UUID of the API key to delete.
     * 
     * @returns {Promise<void>} mhm todo
     *
     */
    APIKeyDelete:  async (id, key_id) => SBMI.auth.Request(`/api/sensors/${id}/api_key/${key_id}/delete`, "DELETE"),
}

