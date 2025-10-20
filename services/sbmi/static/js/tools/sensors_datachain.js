class DataChain {
    constructor() { }

    async saveChain() {
        if (!SBMI.sensorsAPI.pipeline) {
            return;
        }
        if (!SBMI.sensorsAPI.pipeline.state) {
            throw "pipeline is missing state var";
        }
        const cur_data = SBMI.sensorsAPI.pipeline.state;

        // Convert state to request format
        let chain = {
            "chain": {
                "inbound": cur_data.inbound ? cur_data.inbound.id : undefined,
                "outbound": cur_data.outbound ? cur_data.outbound.map(e => ({ event_handler_id: e.handler.id, data_transformer_id: e.transformer ? e.transformer.id : undefined })) : {},
            }
        };

        await SBMI.sensorsAPI.data_chain.setChain(cur_data.sensor.id, chain).then(userFeedbackSucc("Saved"));
    }

    /**
     * GET /api/sensors/${id}/data_chain/load
     *
     * Loads the data chain of the given sensor ID if it exsists.
     *
     * @async
     *
     * @param {string} id - The UUID of the sensor to which the data chain belongs.
     *
     * @returns {Promise<DataChain>} DataChain
     *
     */
    async loadChain(id) {
        return SBMI.auth.Request(`/api/sensors/${id}/data_chain/load`, "GET");
    }

    /**
     * POST /api/sensors/${id}/data_chain/set
     *
     * Sets the data chain of the given sensor ID.
     *
     * @async
     *
     * @param {string} id - The UUID of the sensor to which the data chain belongs.
     * @param {Object} req - The data chain Object. Refer to the OpenAPI doc for details.
     *
     * @returns {Promise<void>}
     *
     */
    async setChain(id, req) {
        return SBMI.auth.Request(`/api/sensors/${id}/data_chain/set`, "POST", req);
    }

    /**
     * POST /api/sensors/${id}/data_chain/set
     *
     * Sets the data chain of the given sensor ID.
     *
     * @async
     *
     * @param {string} id - The UUID of the sensor to which the data chain belongs.
     *
     * @returns {Promise<void>}
     *
     */
    async deleteChain(id) {
        return SBMI.auth.Request(`/api/sensors/${id}/data_chain/delete`, "DELETE");
    }
}

class PipelineEditor {
    constructor(sensor_id) {
        let sensor = SBMI.sensorsAPI.sensors[sensor_id];
        if (!sensor) {
            throw id + " does not exist in sensors dict";
        }

        // Internal State (to be loaded/saved externally)
        // This 'state' property will hold the *data* representing the current graph.
        // The rendering functions will build the DOM based on this state.
        this.state = {
            // The sensor for this chain
            sensor: sensor,
            // Inbound data transformer
            inbound: null,
            // Outbound event handler and their datat transformer
            outbound: [],
        };

        // Initial render and setup
        this.init(sensor_id);
    }

    /**
     * Initializes the pipeline manager by setting up event listeners and performing initial rendering.
     * This function is the entry point for the rendering and interaction part.
     */
    async init(id) {
        // We need to lib all data transformer and event handler
        await SBMI.eventsAPI.render();
        await SBMI.dataTransformAPI.render();

        const chain = await SBMI.sensorsAPI.data_chain.loadChain(id);
        if (chain) {
            if (chain.inbound) {
                this.state.inbound = SBMI.dataTransformAPI.transformer[chain.inbound];
            }
            if (chain.outbound) {
                chain.outbound.forEach(e => (this.state.outbound.push({
                    handler: SBMI.eventsAPI.handler[e.event_handler_id],
                    transformer: e.data_transformer_id ? SBMI.dataTransformAPI.transformer[e.data_transformer_id] : undefined,
                })));
            }
        }

        // Set static elements
        document.getElementById("pE-sensorName").innerHTML = SBMI.sensorsAPI.renderSensorRef(this.state.sensor.id);

        // Initial render of existing state
        this._renderPipeline();

        document.getElementById("pipelineEditor").hidden = false;
    }

    // --- Rendering Functions ---

    /**
   * Renders the inbound node depending on the 'state.inbound' value
   */
    _renderInboundNode() {

        const targetDropBtn = document.getElementById("pE-inboundDropBtn");
        const target = document.getElementById("pE-inboundSet");
        const targetWrapper = document.getElementById("pE-inboundSetWrapper")

        // Show / Hide the add btn
        if (!this.state.inbound) {
            targetDropBtn.closest(".dropdown").classList.remove("d-none");

            targetWrapper.classList.add("d-none");
            target.innerHTML = "";

            return;
        }
        targetDropBtn.closest(".dropdown").classList.add("d-none");
        targetWrapper.classList.remove("d-none");
        target.innerHTML = "";

        // Create selected node
        const newInputNode = document.createElement("div");
        newInputNode.innerHTML = `${this.state.inbound.name}`;
        target.appendChild(newInputNode);
        // Create remove button
        const removeBtn = this._createRemoveButton(() => {
            this.state.inbound = null;
        });
        target.appendChild(removeBtn);
    }

    /**
    * Renders an event handler entry.
    * @param {string} eventHandler - The ID of the main output transform.
    * @param {string|null} dataTransformer - The ID of the optional pre-step transform, or null.
    */
    _renderEventHandler(handler) {

        const eventHandler = handler.handler;
        const dataTransformer = handler.transformer;

        const outboundHandlerWrapper = document.createElement("div");
        outboundHandlerWrapper.classList.add(
            "d-flex",
            "justify-content-end",
            "pb-2",
            "mb-2",
            "border-bottom",
        );

        const currentPairIndex = this.state.outbound.length - 1; // Get index of the pair being created

        // The transformer
        const dataTransformerForEventHanlder = document.createElement("div");
        dataTransformerForEventHanlder.classList.add("me-1");
        outboundHandlerWrapper.appendChild(dataTransformerForEventHanlder);
        if (dataTransformer) {
            const transformerNode = document.createElement("div");
            transformerNode.innerHTML = `${dataTransformer.name}`;
            transformerNode.classList.add("border", "rounded-3", "d-flex", "justify-content-between", "p-2");
            const removeTransformerBtn = this._createRemoveButton(() => {
                this.state.outbound[currentPairIndex].transformer = null;
                this._renderPipeline();
            });
            transformerNode.appendChild(removeTransformerBtn);
            dataTransformerForEventHanlder.appendChild(transformerNode);
        } else {
            // Create the '+ Add Pre-step' button
            this._createEventHandlerAddDataTransformerButton(dataTransformerForEventHanlder, currentPairIndex);
        }

        // The selected event handler
        const eventHandlerNode = document.createElement("div");
        eventHandlerNode.classList.add(
            "border",
            "p-3",
            "w-auto",
            "h-auto",
        );
        eventHandlerNode.innerHTML = `${eventHandler.name}`;
        outboundHandlerWrapper.appendChild(eventHandlerNode);
        const removePairBtn = this._createRemoveButton(() => {
            this.state.outbound.splice(currentPairIndex, 1);
        });
        outboundHandlerWrapper.appendChild(removePairBtn);

        document.getElementById("pE-outboundEventHandlerContainer").appendChild(outboundHandlerWrapper);
    }

    /**
     * Renders the entire pipeline based on the current `state`.
     */
    _renderPipeline() {
        // Render inbound node
        this._renderInboundNode();

        // Render outbound handler
        document.getElementById("pE-outboundEventHandlerContainer").innerHTML = "";
        this.state.outbound.forEach((elem) => {
            this._renderEventHandler(elem);
        });
    }

    // --- Generic ---

    /**
     * Creates a reusable "remove" button element for nodes/pairs.
     * @param {Function} removeCallback - The function to call when the button is clicked.
     * @returns {HTMLElement} The created button element.
     */
    _createRemoveButton(removeCallback) {
        const removeBtn = document.createElement("button");
        removeBtn.innerHTML = '<i class="bi bi-x-circle"></i>';
        removeBtn.classList.add("btn", "btn-danger", "ms-1");
        removeBtn.setAttribute("aria-label", "Remove");
        removeBtn.addEventListener("click", (event) => {
            event.stopPropagation();
            removeCallback();
            this._renderPipeline();
        });
        return removeBtn;
    }

    /**
     * Populates a given Bootstrap dropdown menu (<ul>) with items from an array of data.
     * @param {HTMLElement} dropdownElement - The <ul> element for the dropdown menu.
     * @param {Array<Object>} dataArray - An array of objects with { id, name, classColor }.
     * @param {Function} onClickCallback - The function to call when an item is clicked.
     */
    _populateDropdown(dropdownElement, dataArray, onClickCallback) {
        dropdownElement.innerHTML = "";
        dataArray.forEach((item) => {
            const listItem = document.createElement("li");
            const dropdownItem = document.createElement("a");
            dropdownItem.classList.add("dropdown-item");
            dropdownItem.href = "#";
            dropdownItem.textContent = item.name;
            dropdownItem.dataset.id = item.id;

            dropdownItem.addEventListener("click", (event) => {
                event.preventDefault();
                onClickCallback(item);
            });

            listItem.appendChild(dropdownItem);
            dropdownElement.appendChild(listItem);
        });
    }

    // --- Specific ---

    /**
     * Populates a Data transformer dropdown menu.
     */
    async _populateDataTransformerDropdown(targetDrop, cb) {
        this._populateDropdown(
            targetDrop,
            await SBMI.dataTransformAPI.getTransforms(),
            (e) => {
                cb(e);
                this._renderPipeline();
            },
        );
    }

    /**
     * 
     */
    async populateInboundDataTransformerDropdown() {
        this._populateDropdown(
            document.getElementById("pE-inboundDropContent"),
            await SBMI.dataTransformAPI.getTransforms(),
            (e) => {
                this.state.inbound = e;
                this._renderPipeline();
            },
        );
    }

    /**
     * 
     */
    async populateOutboundEventHandlerDropdown() {
        this._populateDropdown(
            document.getElementById("pE-outboundHandlerDropContent"),
            await SBMI.eventsAPI.getHandler(),
            (e) => {
                this.state.outbound.push({
                    handler: e,
                    transformer: null,
                });
                this._renderPipeline();
            },
        );
    }

    /**
     * Creates the '+ Add Pre-step' button within a pre-step container.
     * @param {HTMLElement} preStepContainer - The container div for the pre-step button/node.
     * @param {number} outputHandlerIndex - The index of the output handler pair in the state.
     */
    _createEventHandlerAddDataTransformerButton(preStepContainer, outputHandlerIndex) {
        preStepContainer.innerHTML = ""; // Clear previous content

        const dropdownDiv = document.createElement("div");
        dropdownDiv.classList.add("dropdown");

        const button = document.createElement("button");
        button.classList.add("btn", "btn-outline-primary", "rounded-3", "p-2");
        button.type = "button";
        button.setAttribute("data-bs-toggle", "dropdown");
        button.setAttribute("aria-expanded", "false");
        button.innerHTML = `
          + <i class="bi bi-envelope-paper"></i>&nbsp;&nbsp;Data Transformer
          <br>
          <span class="fw-light">Outbound</span>
          `;

        const dropdownMenu = document.createElement("ul");
        dropdownMenu.classList.add(
            "dropdown-menu",
            "border",
            "border-secondary",
            "rounded-3",
        );

        this._populateDataTransformerDropdown(dropdownMenu, (e) => (this.state.outbound[outputHandlerIndex].transformer = e));

        dropdownDiv.appendChild(button);
        dropdownDiv.appendChild(dropdownMenu);
        preStepContainer.appendChild(dropdownDiv);
    }
}