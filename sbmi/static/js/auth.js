
// Create a shared namespace if it doesn't exist yet
var SBMI = SBMI || {};

// Mock Auth Calls
SBMI.auth = {

    handleLogin: async (event) => {
        event.preventDefault();
        
        let info = SBMI.helper.getFormDataAsJSON('loginForm');
        await SBMI.auth.login(info)
        .then(async data => {
            try {
                // Update the basePath to the provided url
                SBMI.basePath = info.url;

                // save the token
                token = data.jwt;
                SBMI.session.setToken(token);

                // extract uuid
                uuid = SBMI.helper.parseJwt(data.jwt).sub;
                SBMI.session.setUuid(uuid);

                // get user info
                info = await SBMI.usersAPI.info(uuid);
                SBMI.session.setUser(info);

                // clear password input
                document.getElementById("loginPassword").value = "";

                // load dashboard to show everything besides the login page
                loadDashboard();
            } catch {
                // Make sure that state is reset to not logged in if any error occurs
                SBMI.session.logout();
            }
        })
        .catch(error => formFeedbackErr("loginFormFeedback", error));
    },

    handleLogout: async () => {
        try{
            await SBMI.auth.logout();
        } finally {
            // the call might fail for any reason but we still need to clear all local variables
            // to get to a cleared state
            SBMI.session.logout();
            loadDashboard();
        }
    },

    render: () => {
        let me = SBMI.session.getUser();

        document.getElementById("navUserInfo").innerHTML =
        /*template*/`
        <a class="nav-link dropdown-toggle" href="#" role="button" data-bs-toggle="dropdown" aria-expanded="false">
            ${me.name}
        </a>
        <ul class="dropdown-menu">
            <li><h6 class="dropdown-header">Email</h6></li>
            <li>
                <a class="dropdown-item" href="#">
                    ${me.email}
                </a>
            </li>

            <li><h6 class="dropdown-header">UUID</h6></li>
            <li>
                <a class="dropdown-item" href="#">
                    ${me.id}
                </a>
            </li>

            <li><h6 class="dropdown-header">Roles</h6></li>
            <li>
                <a class="dropdown-item" href="#">
                    ${me.roles.map(role => SBMI.rolesAPI.renderRole(role.id)).join("")}
                </a>
            </li>
            <li><hr class="dropdown-divider"></li>
            <li class="p-2">
                <button class="w-100 btn btn-danger" onclick="SBMI.auth.handleLogout()">
                <i class="bi bi-box-arrow-right"></i>&nbsp;Logout
                </button>
            </li>
        </ul>
        `;
    },

    /**
     * 
     * Make a request against the configured SensBee backend using the logged in user for authorization.
     *
     * @async
     * 
     * @param {string} path - The API path to call
     * @param {string} [method] - The HTTP method to use. GET by default.
     * @param {Object} [body] - The body of the request as a Object. Will be converted to string if it exists. Optional.
     * 
     * @returns {Promise<any>} The response depends on the API Endpoint.
     *
     */
    Request: async (path, method, body) => {
        if(method == undefined){
            method = "GET";
        }
        return fetch(`${SBMI.basePath}${path}`, {
            method: method,
            body: body != undefined ? JSON.stringify(body):undefined,
            headers: {
                "Content-Type": "application/json",
                "Authorization": SBMI.session.getToken(),
            },
        })
        .then(response => {
            if (!response.ok) {
                throw new Error(`HTTP error! Status: ${response.status}`);
            }
            return response.json();
        })
        .catch(error => {
            console.error('Error:', error.message);
            throw error;
        });
    },

    // ###########################
    // SensBee auth API calls
    // ###########################
    
    /**
     * POST /auth/login
     * 
     * Try to login using the provided credentials.
     *
     * @async
     * 
     * @param {Object} info - The login info.
     * 
     * @returns {Promise<JWT>} A JWT key to be used for authentication.
     *
     */
    login: async (info) => SBMI.auth.Request(`/auth/login`, "POST", info),

    /**
     * GET /auth/logout
     * 
     * Logout the current user.
     *
     * @async
     * 
     * @returns {Promise<void>}
     *
     */
    logout: () => SBMI.auth.Request(`/auth/logout`),
};

