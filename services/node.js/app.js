// Init untrusted js execution environment
import ivm from 'isolated-vm';

// NOTE logging
// Not sure if we can enable it somehow based on env vars

// Init WebSocketServer for transform requests
import { WebSocketServer } from 'ws';

const WS_REQ_TYPE_ERR_RESP = 1;
const WS_REQ_TYPE_TRANSFORM_REQUEST = 2; // id, type, script_id, data<target_ingest_data>
const WS_REQ_TYPE_SCRIPT_REQ = 3;       // id, type, script_id
const WS_REQ_TYPE_SCRIPT_RESP = 4;      // id, type, script_id, data<script_data>

const wss = new WebSocketServer({ port: 9002 });

const script_cache = new Map();
const req_data_cache = new Map();

function sendGetScript(ws, req) {
    // send request to get the transform script
    ws.send(JSON.stringify({ "script_id": req.script_id, "type": WS_REQ_TYPE_SCRIPT_REQ, "data": "" }));
}

function sendError(ws, req, err) {

    console.error(req, err);

    ws.send(JSON.stringify({ "script_id": req.script_id, "type": WS_REQ_TYPE_ERR_RESP, "data": err.toString() }));
}

function requireField(ws, req, field) {
    if (!(field in req)) {
        let msg = "field '" + field + "' missing";
        console.error(msg + " (" + JSON.stringify(req) + ")");
        sendError(ws, req, msg);
        return true;
    }
}

async function storeScript(req) {
    try {
        const scriptRunner = new ReusableScriptRunner(req.data);
        await scriptRunner.init();

        script_cache.set(req.script_id, scriptRunner);

        //console.debug(`[${req.script_id}] storeScript success`);
    } catch (error) {
        throw error;
    }
}

async function transform(ws, req) {
    try {
        const data = req_data_cache.get(req.script_id);
        if (data === undefined) {
            throw new Error("data missing for id ", req.script_id);
        }
        const scriptRunner = script_cache.get(req.script_id);
        if (data === undefined) {
            throw new Error("scriptRunner missing for script_id ", req.script_id);
        }

        console.debug(`[${req.script_id}] Transform with `, data);

        let res = await scriptRunner.run(data);

        console.debug(`[${req.script_id}] Transform res:`, res);

        // This should not be the case but as data is not optional we ensure that it is set
        if (res === undefined) {
            res = "undefined"
        }

        // Send result back
        ws.send(JSON.stringify({ "script_id": req.script_id, "type": WS_REQ_TYPE_TRANSFORM_REQUEST, "data": res }));
    } catch (err) {
        sendError(ws, req, err);
    }

    // clear input data in all cases!
    req_data_cache.delete(req.script_id);
}

// --------
// Handle incoming connections

wss.on('connection', function connection(ws, req) {
    console.log("Client connected");

    ws.on('close', function message(event) {
        console.log("Client disconnected: %s", event);
    });

    // TODO maybe add some client info?
    ws.on('error', console.error);

    ws.on('message', async function message(data) {

        //console.debug("recieved a message");

        try {
            // Parse to JSON
            var req = JSON.parse(data);

            // script_id: <uuid>
            // the id of the script that needs to be used for transformation
            var r = requireField(ws, req, 'script_id');
            if (r) {
                return;
            }

            // type: <some_enum_value>
            // Request Type
            r = requireField(ws, req, 'type');
            if (r) {
                return;
            }

            // data: String
            // Dependant on the request type
            r = requireField(ws, req, 'data');
            if (r) {
                return;
            }

            //console.debug(req);

            // Messages can either be:
            switch (req.type) {
                // A new data transformation request
                case WS_REQ_TYPE_TRANSFORM_REQUEST:
                    //console.log(`[${req.script_id}] Recieved WS_REQ_TYPE_TRANSFORM_REQUEST`);

                    //store transform data as JSON
                    req_data_cache.set(req.script_id, JSON.parse(req.data));

                    // Check if we have the script cached
                    if (!script_cache.has(req.script_id)) {
                        console.log(`[${req.script_id}] Script not locally present. Loading...`);
                        // Send script load request if its not available
                        return sendGetScript(ws, req);
                    }

                    break;
                // A script response if the transformation script has not been cached already
                case WS_REQ_TYPE_SCRIPT_RESP:
                    console.log(`[${req.script_id}] Recieved WS_REQ_TYPE_SCRIPT_RESP`);

                    await storeScript(req);

                    break;
                // An unhandeld request type
                default:
                    throw "unsupported reqeust type: " + req.type;
            }

            // Do the actual transformation
            await transform(ws, req);

        } catch (error) {
            sendError(ws, req, error);
        }
    });
});


// NOTE this is straight copied from gemini so there might be issues in there
/**
 * Manages a single, reusable script that can be run multiple times
 * with different inputs within a dedicated V8 isolate.
 */
class ReusableScriptRunner {
    /**
     * @param {string} scriptString The JavaScript code to be compiled and reused.
     * @param {object} [isolateOptions] Options for the isolate (e.g., memoryLimit).
     * @param {number} [isolateOptions.memoryLimit=128] Memory limit for the isolate in MB.
     */
    constructor(scriptString, isolateOptions = { memoryLimit: 128 }) {
        this.isolate = new ivm.Isolate(isolateOptions);
        this.scriptString = `const res = (() => {${scriptString}})();JSON.stringify(res);`;
        this.compiledScript = null; // Will hold the ivm.Script object
        this._isInitialized = false;
    }

    /**
     * Initializes the runner by compiling the script.
     * Must be called before the first run.
     * @throws {Error} If script compilation fails.
     */
    async init() {
        if (this._isInitialized) {
            console.warn("ReusableScriptRunner already initialized.");
            return;
        }
        if (this.isolate.isDisposed) {
            throw new Error("Cannot initialize: Isolate has been disposed.");
        }
        try {
            this.compiledScript = await this.isolate.compileScript(this.scriptString);
            this._isInitialized = true;
            console.log("Reusable script compiled successfully.", this.scriptString);
        } catch (compileError) {
            console.error("Failed to compile script during initialization:", compileError);
            // If compilation fails, the isolate might be in a bad state or unusable.
            // It's safer to dispose of it.
            if (!this.isolate.isDisposed) {
                this.isolate.dispose();
            }
            throw compileError;
        }
    }

    /**
     * Runs the pre-compiled script with the given inputData.
     * A new context is created for each run to ensure isolation.
     *
     * @param {any} inputData The data to be made available to the script (via global.inputData) as JSON.
     * @param {object} [executionOptions] Options for this specific run.
     * @param {number} [executionOptions.timeout=1000] Execution timeout in milliseconds.
     * @returns {Promise<any>} A promise that resolves with the script's result.
     * @throws {Error} If the runner is not initialized, isolate is disposed, or script execution fails.
     */
    async run(inputData, executionOptions = { timeout: 1000 }) {
        if (!this._isInitialized || !this.compiledScript) {
            throw new Error("ReusableScriptRunner is not initialized or script compilation failed. Call init() first.");
        }
        if (this.isolate.isDisposed) {
            throw new Error("Cannot run script: Isolate has been disposed.");
        }

        let context;
        try {
            // Create a new, clean context for this execution.
            context = await this.isolate.createContext();
            const jail = context.global;

            // Inject inputData for the script to use.
            await jail.set('data', new ivm.ExternalCopy(inputData).copyInto({ release: true }));

            // Execute the *pre-compiled* script.
            // The `compiledScript` object is reused here.
            const resultFromVM = await this.compiledScript.run(context, {
                timeout: executionOptions.timeout,
                // The 'release' option here pertains to the *returned Reference* from script.run,
                // not the compiledScript itself. We'll manage the result reference manually.
            });

            // Process the result (handle primitives, references, and promises).
            if (resultFromVM instanceof ivm.Reference) {
                let isIsolatePromise = false;
                if (resultFromVM.typeof === 'object' && resultFromVM !== null) {
                    try {
                        const thenFunction = await resultFromVM.get('then');
                        if (typeof thenFunction === 'function') isIsolatePromise = true;
                    } catch (e) { /* Not a promise or 'then' not accessible */ }
                }

                if (isIsolatePromise) {
                    // If the script returns a promise, bridge it.
                    // { release: true } for toPromise ensures the isolate's promise reference is released.
                    return resultFromVM.toPromise({ release: true });
                } else {
                    // For other references (objects/arrays), copy them.
                    const copiedResult = await resultFromVM.copy();
                    resultFromVM.release(); // Release the ivm.Reference now that we have a copy.
                    return copiedResult;
                }
            } else {
                // Primitive result, return directly.
                return resultFromVM;
            }
        } catch (runError) {
            // console.error("Error running reusable script:", runError); // For debugging
            throw runError;
        } finally {
            // IMPORTANT: Release the context after each run to free its resources.
            // The isolate itself remains for the next run.
            if (context) {
                context.release();
            }
        }
    }

    /**
     * Disposes of the isolate and its resources, including the compiled script.
     * Call this when the ReusableScriptRunner is no longer needed.
     */
    async dispose() {
        if (!this.isolate.isDisposed) {
            console.log("Disposing ReusableScriptRunner's isolate.");
            this.isolate.dispose();
        }
        this.compiledScript = null;
        this._isInitialized = false;
    }
}