
// Create a shared namespace if it doesn't exist yet
var SBMI = SBMI || {};

SBMI.usersAPI = {

    // A dict with id:user for easy access to all users retrieved by a list call
    users: {},

    openUserRegister: () => {
        openModal(
            "Register user",
            /*template*/`
            <form id="registerNewUserForm" onsubmit="SBMI.usersAPI.registerNewUser(event)">
                <div class="mb-3">
                    <label for="emailInput" class="form-label">Email address</label>
                    <input name="email" type="email" class="form-control" id="emailInput" required>
                </div>
                <div class="mb-3">
                    <label for="nameInput" class="form-label">Name</label>
                    <input name="name" type="text" class="form-control" id="nameInput" required>
                </div>
                <div class="mb-3">
                    <label for="passwordInput" class="form-label">Password</label>
                    <input name="password" type="password" class="form-control" id="passwordInput" required>
                </div>
                <button type="submit" class="btn btn-primary w-100">
                    Submit
                </button>
                <div id="registerFormFeedback" class="mt-2 d-none">
                </div>
            </form>
            `,
        );
    },
    registerNewUser: async (event) => {
        event.preventDefault();

        SBMI.usersAPI.register(SBMI.helper.getFormDataAsJSON("registerNewUserForm"))
        .then(uuid => {
            formFeedback("registerFormFeedback", `Created user with id: ${uuid}`);
    
            // reload user list
            SBMI.usersAPI.render();
        })
        .catch(error => formFeedbackErr("registerFormFeedback", error));
    },

    openEditUser: (id) => {
        let user = SBMI.usersAPI.users[id];
        if(!user){
            console.error(id + " does not exist in users dict");
            return;
        }

        openOffcanvas(
            "Edit user",
            /*template*/`
            <h4>Information</h4>
            <form id="editUserInfoForm" onsubmit="SBMI.usersAPI.editUserInfo(event, '${id}')">
                <div class="mb-3">
                    <label class="form-label">Email</label>
                    <input name="email" type="email" class="form-control" placeholder="${user.email}" value="${user.email}" required>
                </div>
                <div class="mb-3">
                    <label class="form-label">Name</label>
                    <input name="name" type="text" class="form-control" placeholder="${user.name}" value="${user.name}" required>
                </div>
                <button type="submit" class="btn btn-primary">Submit</button>
                <div id="formFeedbackUserInfo" class="mt-2 d-none"></div>
            </form>
            <hr>
            <h4>Security</h4>
            <form id="updatePwForm" onsubmit="SBMI.usersAPI.editUserPassword(event, '${id}')">
                ${SBMI.session.isAdmin() ? '<p class="alert alert-info" role="alert">As an admin you dont provide the old password</p>' : ""}
                <div class="mb-3">
                    <label for="oldPw" class="form-label">Old password</label>
                    <input name="old" type="password" class="form-control" id="oldPw">
                </div>
                <div class="mb-3">
                    <label for="newPw" class="form-label">New Password</label>
                    <input name="new" type="password" class="form-control" id="newPw" required>
                </div>
                <div class="mb-3">
                    <label for="newPw2" class="form-label">Confirm new Password</label>
                    <input type="password" class="form-control" id="newPw2" required>
                </div>
                <button type="submit" class="btn btn-primary" disabled>Submit</button>
                <div id="formFeedbackUserPW" class="mt-2 d-none"></div>
            </form>`,
        );

        const form = document.getElementById("updatePwForm");
        const newPassword = document.getElementById("newPw");
        const confirmPassword = document.getElementById("newPw2");
        const submitButton = form.querySelector("button[type='submit']");
        const feedback = document.getElementById("formFeedbackUserPW");

        // Function to validate matching passwords
        function validatePasswords() {
            const minLength = 8; // Minimum password length enforced by the backend
        if (newPassword.value.length < minLength) {
            // New password is too short
            submitButton.disabled = true;
            feedback.classList.remove("d-none");
            feedback.textContent = `Password must be at least ${minLength} characters long.`;
            feedback.classList.add("alert", "alert-danger");
        } else if (newPassword.value === confirmPassword.value) {
            // Passwords match
            submitButton.disabled = false;
            feedback.classList.add("d-none");
            feedback.textContent = "";
        } else {
            // Passwords do not match
            submitButton.disabled = true;
            feedback.classList.remove("d-none");
            feedback.textContent = "Passwords do not match.";
            feedback.classList.add("alert", "alert-danger");
        }
    }

    // Add event listeners to the password fields
    newPassword.addEventListener("input", validatePasswords);
    confirmPassword.addEventListener("input", validatePasswords);
    },
    editUserInfo: (event, id) => {
        event.preventDefault();

        SBMI.usersAPI.editInfo(id, SBMI.helper.getFormDataAsJSON('editUserInfoForm'))
        .then(() => {
            // TODO close offcanvas?
            SBMI.usersAPI.render();
        })
        .catch(error => formFeedbackErr("formFeedbackUserInfo", error));
    },
    editUserPassword: (event, id) => {
        event.preventDefault();

        SBMI.usersAPI.editPassword(id, SBMI.helper.getFormDataAsJSON('updatePwForm'))
        .then(() => {
            formFeedback("formFeedbackUserPW", 'Password updated');
            // TODO close offcanvas?
        })
        .catch(error => formFeedbackErr("formFeedbackUserPW", error));
    },

    openUserDelete: (id) => {
        let user = SBMI.usersAPI.users[id];
        if(!user){
            console.error(id + " does not exist in users dict");
            return;
        }

        openModal(
            `Delete ${user.name}`,
            /*template*/`
            <form onsubmit="SBMI.usersAPI.deleteUser('${user.id}');return false;">
                Are you sure you want to delete this user?
                <div class="p-3 m-2">
                    ${user.name}
                    <br>
                    ${user.email}
                    <br>
                    ${user.id}
                </div>
                <button type="submit" class="btn btn-danger w-100">
                    Delete
                </button>
                <div id="formFeedback" class="mt-2 d-none"></div>
            </form>
            `,
        );
    },
    deleteUser: async (id) => {
        SBMI.usersAPI.delete(id)
        .then(() => {
            // TODO close modal?
            SBMI.usersAPI.render();
        })
        .catch(error => formFeedbackErr("formFeedback", error));
    },

    verifyUser: async (id) => {
        SBMI.usersAPI.editVerify(id)
        .then(() => {
            SBMI.usersAPI.render();
        });
    },

    openRoleAddDropdown: async (id) => {
        // Check which roles can be assigned
        let allRoles = SBMI.rolesAPI.roles;

        let userAssignedRoles = SBMI.usersAPI.users[id].roles;

        let assignableRoles = [];
        for (const [key, role] of Object.entries(allRoles)) {
            if(role.system){
                continue;
            }

            if(userAssignedRoles.find(role => role.id == key) == undefined){
                assignableRoles.push(role);
            }
        }
        
        // Set options
        document.getElementById("roleAddList-"+id).innerHTML = '<li><h6 class="dropdown-header">Assignable roles</h6></li><li><hr class="dropdown-divider"></li>' +
        assignableRoles.map(role => 
        /*template*/`
            <li>
                <button class="dropdown-item" type="button" onclick="SBMI.usersAPI.assignUserRole('${id}','${role.id}');">
                    ${SBMI.rolesAPI.renderRole(role.id)}
                </button>
            </li>
        `).join("");
    },
    assignUserRole:(id, role_id) => {
        SBMI.usersAPI.assignRole(id,role_id)
        .then(() => {
            SBMI.usersAPI.render();
        });
    },
    revokeUserRole: (id, role_id) => {
        SBMI.usersAPI.revokeRole(id,role_id)
        .then(() => {
            SBMI.usersAPI.render();
        });
    },

    // Function to render all users that can be listed
    // also saves the currently found useres for easy access via their id
    render: async () => {

        // Only Admins may use this function
        if(!SBMI.session.isAdmin()){
            return;
        }

        let me = SBMI.session.getUser();
        let userlist = await SBMI.usersAPI.list();

        document.getElementById("accordionUsers").classList.remove("d-none");
        document.getElementById("users-table-body").innerHTML = userlist.map(user => 
            /*template*/`
            <tr id="user-${user.id}">
                <td>
                    ${user.id == me.id ? '(You)':'' }
                    ${user.verified  ? "":
                    /*template*/`
                    <button class="btn btn-warning" onclick="SBMI.usersAPI.verifyUser('${user.id}')">
                        <i class="bi bi-person-fill-exclamation"></i>
                    </button>
                    `} 
                </td>
                <td>
                    ${user.name}  
                    <br>
                    <span class="fw-light">${user.id}</span>
                </td>
                <td>${user.email}</td>
                <td>
                    ${user.roles.filter(role => role.system == false).map(role => 
                    /*template*/`
                    <div class="btn-group me-1" role="group">
                        <button class="btn btn-sm btn-outline-dark">
                            ${SBMI.rolesAPI.renderRole(role.id)}
                        </button>
                        <button onclick="SBMI.usersAPI.revokeUserRole('${user.id}','${role.id}')" type="button" class="btn btn-sm btn-danger">
                            <i class="bi bi-x-circle"></i>
                        </button>
                    </div>
                    `).join("")}
                    <div class="d-inline dropdown me-1">
                        <button onclick="SBMI.usersAPI.openRoleAddDropdown('${user.id}')" class="btn btn-sm btn-secondary dropdown-toggle" type="button" data-bs-toggle="dropdown" aria-expanded="false">
                            <i class="bi bi-plus-circle"></i>
                        </button>
                        <ul id="roleAddList-${user.id}" class="dropdown-menu p-2">
                            <div class="spinner-border" role="status">
                                <span class="visually-hidden">Loading...</span>
                            </div>
                        </ul>
                    </div>
                </td>
                <td>
                    ${user.roles.filter(role => role.system == true).map(role => role.name)}
                </td>
                <td>
                    <button class="btn btn-sm btn-primary" onclick="SBMI.usersAPI.openEditUser('${user.id}')" data-bs-toggle="offcanvas" data-bs-target="#offcanvasEnd" aria-controls="offcanvasEnd">
                        <i class="bi bi-pencil-square"></i>
                    </button>
                    <button class="btn btn-sm btn-danger ${user.id == me.id ? 'd-none':''}" onclick="SBMI.usersAPI.openUserDelete('${user.id}')" data-bs-toggle="modal" data-bs-target="#globalModal">
                        <i class="bi bi-person-dash"></i>
                    </button>
                </td>
            </tr>
        `).join("");

        // update user dict
        const userdict = {}
        userlist.forEach(user => userdict[user.id] = user);
        userdict[me.id] = me;
        SBMI.usersAPI.users = userdict;
    },

    renderUser:(id) => {
        let user = SBMI.usersAPI.users[id];
        if(!user){
            console.error(id + " does not exist in users dict");
            return;
        }

        return /*template*/`
        <div class="container">
            <i class="bi bi-person-circle"></i>&nbsp;${user.name}
        </div>
        `;
    },

    // ###########################
    // SensBee Users API calls
    // ###########################

    /**
     * GET /api/users/list
     * 
     * Retreive the list of users.
     *
     * @async
     * 
     * @returns {Promise<Array<User>>}
     *
     */
    list: async () => SBMI.auth.Request(`/api/users/list`),

    /**
     * POST /api/users/register
     * 
     * Register a new user in the system using the provided information.
     *
     * @async
     * 
     * @param {info} - The information to use.
     * 
     * @returns {Promise<uuid>} - The uuid of the newly created user
     *
     */
    register: async (info) => SBMI.auth.Request(`/api/users/register`, "POST", info),

    /**
     * DELETE /api/users/{id}/delete
     * 
     * Removes the specified user from the system. This will also remove all associated resources of that user.
     *
     * @async
     * 
     * @param {id} - The id of the user.
     * 
     * @returns {Promise<void>}
     *
     */
    delete: async (id) => SBMI.auth.Request(`/api/users/${id}/delete`, "DELETE"),

    /**
     * POST /api/users/{id}/edit/info
     * 
     * Updates information of the specified user.
     *
     * @async
     * 
     * @param {id} - The id of the user.
     * @param {info} - The updated information to use.
     * 
     * @returns {Promise<void>}
     *
     */
    editInfo: async (id, info) => SBMI.auth.Request(`/api/users/${id}/edit/info`, "POST", info),

    /**
     * POST /api/users/{id}/edit/security/password
     * 
     * Updates the password of the specified user if able.
     *
     * @async
     * 
     * @param {id} - The id of the user.
     * @param {info} - The updated information to use.
     * 
     * @returns {Promise<void>}
     *
     */
    editPassword: async (id, info) => SBMI.auth.Request(`/api/users/${id}/edit/security/password`, "POST", info),

    /**
     * GET /api/users/{id}/info
     * 
     * Get more detailed information for the specified user.
     *
     * @async
     * 
     * @param {id} - The id of the user.
     * 
     * @returns {Promise<Info>} - The detailed information of the user.
     *
     */
    info: async (id) => SBMI.auth.Request(`/api/users/${id}/info`),

    /**
     * POST /api/users/{id}/edit/verify
     * 
     * Verify the specified user.
     *
     * @async
     * 
     * @param {id} - The id of the user.
     * 
     * @returns {Promise<void>}
     *
     */
    editVerify: async (id) => SBMI.auth.Request(`/api/users/${id}/edit/verify`, "POST"),

    /**
     * POST /api/users/{id}/role/{role_id}/assign
     * 
     * Assign the specified role to the specified user.
     *
     * @async
     * 
     * @param {id} - The id of the user.
     * @param {role_id} - The id of the role.
     * 
     * @returns {Promise<void>}
     *
     */
    assignRole: async (id, role_id) => SBMI.auth.Request(`/api/users/${id}/role/${role_id}/assign`, "POST"),

    /**
     * DELETE /api/users/{id}/role/{role_id}/revoke
     * 
     * Revoke the specified role from the specified user.
     *
     * @async
     * 
     * @param {id} - The id of the user.
     * @param {role_id} - The id of the role.
     * 
     * @returns {Promise<void>}
     *
     */
    revokeRole: async (id, role_id) => SBMI.auth.Request(`/api/users/${id}/role/${role_id}/revoke`, "DELETE"),
}