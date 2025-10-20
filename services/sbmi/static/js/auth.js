
// Create a shared namespace if it doesn't exist yet
var SBMI = SBMI || {};

// Mock Auth Calls
SBMI.auth = {

    // Check if the SensBee instance exposes the developer access endpoint
    checkDevAccess: async () => {
        const t = document.getElementById('dev_access');
        t.hidden = true;
        try {
            const resp = await fetch(new URL("auth/dev/login", config.api.getURL()), { method: "POST" });
            const parsed_resp = await resp.json();
            if ('jwt' in parsed_resp) {
                t.hidden = false;
            }
        } catch { }
    },

    // Render a button for each OIDC
    renderIDPs: () => {
        const t = document.getElementById("idps_btns");
        fetch(config.api.getURL() + "auth/openid/list_idps", { headers: { "Content-Type": "application/json" } })
            .then((response) => {
                t.innerHTML = "";
                response.json().then((idps) => {
                    if (idps.length == 0) {
                        t.innerHTML = "No external Identity Provider available.";
                    }
                    idps.forEach(idp => {
                        let idpBtn = document.createElement("button");
                        idpBtn.classList.add("btn", "btn-primary", "w-100", "mb-2");
                        idpBtn.innerHTML = idp.name
                        idpBtn.addEventListener('click', () => {
                            window.location.href = idp.final_url;
                        });
                        t.appendChild(idpBtn);
                    });
                })
            })
            .catch((err) => {
                console.log(err);
                t.innerHTML = `Failed to fetch IDP list due to <br><span class="text-danger">${err}<span>`
            });
    },

    // Render the currently logged in user
    render: () => {
        let me = SBMI.session.getUser();

        document.getElementById("navUserInfo").innerHTML =
        /*template*/`
        <a class="nav-link dropdown-toggle" href="#" role="button" data-bs-toggle="dropdown" aria-expanded="false">
            ${SBMI.session.isAdmin() ? '<i class="bi bi-person-badge"></i>' : '<i class="bi bi-person-vcard"></i>'}<span class="sidebar-full">&nbsp;&nbsp;${me.name}</span>
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
                <button class="w-100 btn btn-danger" onclick="SBMI.session.logout()">
                    <i class="bi bi-box-arrow-right"></i>&nbsp;Logout
                </button>
            </li>
        </ul>`;
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
        if (method == undefined) {
            method = "GET";
        }
        let headers = {
            "Content-Type": "application/json",
        };
        if (!SBMI.session.isGuest()) {
            headers["Authorization"] = SBMI.session.getToken();
        }
        return fetch(new URL(path, config.api.getURL()), {
            method: method,
            body: body != undefined ? JSON.stringify(body) : undefined,
            headers: headers,
        })
            .then(async response => {
                if (response.status == 200) {
                    return response.json();
                }
                if (!response.ok) {
                    console.debug(response);
                    throw new Error(`${response.status} ${await response.json().then((d) => JSON.stringify(d)).catch(() => "(Non json response)")}`);
                }
            })
            .catch(error => {
                console.error(error);

                userFeedbackErr(error.toString());

                throw error;
            });
    },

    // ###########################
    // SensBee auth API calls
    // ###########################

    /**
     * POST /auth/login
     * 
     * Try to use the development login endpoint.
     *
     * @async
     * 
     * @returns {Promise<JWT>} A JWT key to be used for authentication.
     *
     */
    login: async () => SBMI.auth.Request(`/auth/dev/login`, "POST"),

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

