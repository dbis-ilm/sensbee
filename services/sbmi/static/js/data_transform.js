//
var SBMI = SBMI || {};

SBMI.dataTransformAPI = {

  // dict transformer_id:transformer_info, populated via render
  transformer: {},

  getTransforms: async () => {
    const transformerList = await SBMI.dataTransformAPI.list();

    return transformerList;

    // TODO for some reason this doesnt update when a new one is created lol
  },

  //
  dataTransformScriptPrefix: "var output = [];\n\n",
  dataTransformScriptSuffix: (return_variant) =>
    `\n\n${return_variant ? " return " : ""}JSON.stringify(output);`,

  openDataTransformEdit: async (id) => {
    // TODO detect if this is an update!

    if ("script_editor" in SBMI.dataTransformAPI) {
      SBMI.dataTransformAPI.script_editor.destroy();
    }
    if ("script_editor_in" in SBMI.dataTransformAPI) {
      SBMI.dataTransformAPI.script_editor_in.destroy();
    }

    if (id) {
      var transform = await SBMI.dataTransformAPI.load(id);
    }

    // Generate some sample data based on the sensor config
    exampleData = [{ todo: "select sensor to generate sample data" }];

    let script = transform ? transform.script : `
// Do something with 'data'
// NOTE If used as for ingest 'output' must be: [ {}, ... ]

return data;`;

    // Cut Pre and Suffix from the script to avoid duplications
    script = script.replace(
      SBMI.dataTransformAPI.dataTransformScriptPrefix,
      "",
    );
    script = script.replace(
      SBMI.dataTransformAPI.dataTransformScriptSuffix(),
      "",
    );

    const editorTarget = document.getElementById("dataTransformerEditor");
    editorTarget.innerHTML = `
      <div class="card">
        <div class="card-header">
          <i class="bi bi-file-earmark-code"></i>&nbsp;&nbsp;Data Transformer Editor
        </div>
        <div class="card-body">

          <div class="mb-1">
            <label for="dataTransformerScriptName" class="form-label">Name</label>
            <input type="text" id="dataTransformerScriptName" class="form-control" aria-describedby="dataTransformName" value="${transform ? transform.name : ""}">
            <div id="dataTransformName" class="form-text">
              Name must be at least 3 chars. Any unicode.
            </div>
          </div>
          Version: ${transform ? transform.version : ""}

          <div class="mb-1">
            <h6>Example input data</h6>

            <div class="alert alert-secondary mb-0" role="alert">
              <div class="position-relative">
                <div class="position-absolute top-0 end-0">
                  <div class="dropdown">
                    <button class="btn btn-secondary dropdown-toggle" type="button" data-bs-toggle="dropdown" aria-expanded="false">
                      Generate
                    </button>
                    <ul class="dropdown-menu">
                      <li><a class="dropdown-item" href="#">List of Sensors</a></li>
                      <hr>
                      <li><a class="dropdown-item" href="#">TODO</a></li>
                    </ul>
                  </div>
                </div>
              </div>
              Define the 'data' array for the transform script in JSON
            </div>

            <div id="transform-in" style="min-height:20vh;">${JSON.stringify(exampleData, null, 2)}</div>
          </div>

          <div class="mb-1">
            <h6>Data transform script</h6>

            <div class="alert alert-secondary mb-0" role="alert">
              <div class="position-relative">
                <div class="position-absolute top-0 end-0">
                  <button class="btn" type="button" data-bs-toggle="collapse" data-bs-target="#collapseScriptInfo" aria-expanded="false" aria-controls="collapseScriptInfo">
                    <i class="bi bi-info-circle"></i>
                  </button>
                </div>
              </div>

              transform(input: data[array of Objects]) -> output: any string
            </div>

            <div id="transform-script" style="min-height:30vh;">${script}</div>

            <div class="collapse" id="collapseScriptInfo">
              <div class="card card-body">
                Die Eingabe wird als Script und nicht als Function ausgewertet.<br>
                <br>
                [script]<br>
                ${SBMI.dataTransformAPI.dataTransformScriptPrefix}<br>
                <br>
                [your-script]<br>
                <br>
                ${SBMI.dataTransformAPI.dataTransformScriptSuffix()}<br>
                [/script]<br>
                <br>
                <a href="https://github.com/laverdet/isolated-vm">Runtime</a>
              </div>
            </div>
          </div>

          <h6>Output</h6>

          <div class="alert alert-secondary p-1" role="alert">
            <div id="transform-out" style="min-height:80px;"></div>
          </div>

          <hr>

          <button class="btn btn-primary" onclick="SBMI.dataTransformAPI.runTestTransform()">
            Test
          </button>

          <hr>
                
          <button id="dataTransformerConfirmButton" class="btn btn-primary" data-id="${id}" onclick="SBMI.dataTransformAPI.saveDataTransform()">
            ${id ? "Update" : "Create"}
          </button>
        </div>
      </div>
    `;

    SBMI.dataTransformAPI.script_editor_in = ace.edit("transform-in");
    SBMI.dataTransformAPI.script_editor_in.setTheme("ace/theme/monokai");
    SBMI.dataTransformAPI.script_editor_in.session.setMode("ace/mode/javascript");

    SBMI.dataTransformAPI.script_editor = ace.edit("transform-script");
    SBMI.dataTransformAPI.script_editor.setTheme("ace/theme/monokai");
    SBMI.dataTransformAPI.script_editor.session.setMode("ace/mode/javascript");
  },
  runTestTransform: () => {
    var out = document.getElementById("transform-out");
    try {
      var data = SBMI.dataTransformAPI.script_editor_in.getValue();
      var d = JSON.parse(data);
      console.debug(d);

      var script = SBMI.dataTransformAPI.script_editor.getValue();

      // We dont use the Suffix here because we want to check if the output is actually a correct object
      // Internally the transform service calls JSON.parse on the output but the current setup can only return a
      // string from the executed script which is why the JSON.stringify is required.
      script = `const res = ((data) => {
        ${script}
      })(data);
      return JSON.stringify(res);`;
      console.debug(script);

      // make it a executable function
      var fn = new Function("data", script);
      // run it
      var result_string = fn(d);

      var result = JSON.parse(result_string);
      console.debug(result);

      // make sure that result is [{},...,{}]
      // TODO maybe display type of result?
      /*
      if (!Array.isArray(result)) {
        throw Error("result must be a list of objects '[{}]'");
      }
      for (const item of result) {
        if (typeof item !== "object" || item === null || Array.isArray(item)) {
          throw Error("result must be a list of objects '[{}]'");
        }
      }*/

      // Seems like all checks passed
      out.innerHTML = JSON.stringify(result, null, 2);
    } catch (error) {
      out.innerHTML = error;
    }
  },
  saveDataTransform: async () => {

    let id = document.getElementById("dataTransformerConfirmButton").dataset.id;

    const script_code = SBMI.dataTransformAPI.script_editor.getValue();
    const script_name = document.getElementById(
      "dataTransformerScriptName",
    ).value;

    if (id != "undefined") {
      SBMI.dataTransformAPI
        .update(id, { name: script_name, script: script_code })
        .then((e) => {
          userFeedbackSucc(`Data transfomer: '${script_name}' updated`);
          SBMI.dataTransformAPI.render();
          document.getElementById("dataTransformerConfirmButton").dataset.id = e.uuid;
        });
    } else {
      SBMI.dataTransformAPI
        .create({ name: script_name, script: script_code })
        .then((e) => {
          userFeedbackSucc(`Data transfomer: '${script_name}' created`);
          SBMI.dataTransformAPI.render();
          SBMI.dataTransformAPI.openDataTransformEdit(e.uuid);
          document.getElementById("dataTransformerConfirmButton").dataset.id = e.uuid;
        });
    }
  },

  openDelete: (id) => {
    let t = SBMI.dataTransformAPI.transformer[id];
    if (!t) {
      console.error(id + " does not exist in transformer dict");
      return;
    }

    openModal(
      `Delete Data Transformer`,
      /*template*/ `
      <form onsubmit="SBMI.dataTransformAPI.deleteDataTransform('${id}');return false;">
        Are you sure you want to delete this Transformer?
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
  deleteDataTransform: async (id) => {
    SBMI.dataTransformAPI.delete(id).then((e) => {
      userFeedbackSucc("Removed");
      SBMI.dataTransformAPI.render(id);
    });
  },

  // Queries the backend for the list of transformer, saving them and rendering them afterwards
  render: async () => {
    const transformerList = await SBMI.dataTransformAPI.list();

    const transformerDict = {};
    transformerList.forEach((elem) => (transformerDict[elem.id] = elem));
    SBMI.dataTransformAPI.transformer = transformerDict;

    const eventHandlerUIElement = document.getElementById("accordionDataTransforms");
    eventHandlerUIElement.classList.remove("d-none");

    document.getElementById("dataTransforms-table-body").innerHTML =
      transformerList
        .map(
          (elem) => /*template*/ `
              <tr>
                  <td>
                    ${elem.name}
                  </td>
                  <td>
                    ${elem.id}
                  </td>
                  <td>
                    ${elem.updated_at ? elem.updated_at : elem.created_at}
                  </td>
                  <td>
                    ${elem.version}
                  </td>
                  <td>
                    <button class="btn btn-sm btn-primary" onclick="SBMI.dataTransformAPI.openDataTransformEdit('${elem.id}')">
                      <i class="bi bi-file-earmark-code"></i>
                    </button>
                    <button class="btn btn-sm btn-danger" onclick="SBMI.dataTransformAPI.openDelete('${elem.id}')" data-bs-toggle="modal" data-bs-target="#globalModal">
                      <i class="bi bi-x-circle"></i>
                    </button>
                  </td>
              </tr>
          `,
        )
        .join("");
  },

  // ###########################
  // SensBee API calls
  // ###########################

  /**
   * GET /api/data_transformer/list
   *
   * TODO
   *
   * @async
   *
   * @returns {Promise<void>} [{}]
   *
   */
  list: async () => SBMI.auth.Request(`/api/data_transformer/list`, "GET"),

  /**
   * GET /api/data_transformer/{id}/load
   *
   * TODO
   *
   * @async
   *
   * @param {string} id - The UUID of the sensor to which the data transformation script belongs.
   * @param {string} TODO
   *
   * @returns {Promise<void>} mhm todo
   *
   */
  load: async (id) => SBMI.auth.Request(`/api/data_transformer/${id}/load`, "GET"),

  /**
   * POST /api/data_transformer/{id}/update
   *
   * TODO
   *
   * @async
   *
   * @returns {Promise<void>} mhm todo
   *
   */
  create: async (req) => SBMI.auth.Request(`/api/data_transformer/create`, "POST", req),

  /**
   * POST /api/data_transformer/{id}/update
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
  update: async (id, script) => SBMI.auth.Request(`/api/data_transformer/${id}/update`, "POST", script),

  /**
   * DELETE /api/data_transformer/{id}/delete
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
  delete: async (id) => SBMI.auth.Request(`/api/data_transformer/${id}/delete`, "DELETE"),
};
