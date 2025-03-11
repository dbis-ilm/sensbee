
// Create a shared namespace if it doesn't exist yet
var SBMI = SBMI || {};

SBMI.rolesAPI = {

    // dict with id:role for all present roles
    roles: {},

    openCreateRole: () => {
        openModal(
            "Create new role",
            /*template*/`
            <form id="createRoleForm" onsubmit="SBMI.rolesAPI.createNewRole(event)">
                <div class="mb-3">
                    <label for="nameInput" class="form-label">Name</label>
                    <input name="name" type="text" class="form-control" id="nameInput"  required>
                </div>

                <button type="submit" class="btn btn-primary w-100">Submit</button>
                <div id="createRoleFormFeedback" class="mt-2 d-none">
                </div>
            </form>
            `,
        );
    },
    createNewRole: async (event) => {
        event.preventDefault();

        SBMI.rolesAPI.create(SBMI.helper.getFormDataAsJSON('createRoleForm'))
        .then(newRole => {
            formFeedback("createRoleFormFeedback",`Created role with id: ${newRole.id}`);

            // reload user list
            SBMI.rolesAPI.render();
        })
        .catch(error => formFeedbackErr("createRoleFormFeedback",`Failed with: ${error}`));
    },

    openRoleDelete: (id) => {
        role = SBMI.rolesAPI.roles[id];

        if(!role){
            console.error(id + " does not exist in roles dict");
            return;
        }

        openModal(
            `Delete ${role.name}`,
            /*template*/`
            <form onsubmit="SBMI.rolesAPI.deleteRole(event, '${role.id}')">
                <span>
                    Are you sure you want to delete this role?
                </span>
                <div class="p-3 m-2">
                    ${role.id}
                    <br>
                    ${role.name}
                </div>
                <button type="submit" class="btn btn-danger w-100">
                    Delete
                </button>
                <div id="deleteFormFeedback" class="mt-2 d-none">
                </div>
            </form>
            `,
        );
    },
    deleteRole: async (event, id) => {
        event.preventDefault();
        
        SBMI.rolesAPI.delete(id)
        .then(() => {
            // TODO close modal?
            SBMI.usersAPI.render();
            SBMI.rolesAPI.render();
            SBMI.sensorsAPI.render();
        })
        .catch(error => formFeedbackErr("deleteFormFeedback",`Failed with: ${error}`));
    },

    render: async () => {

        const roleslist = await SBMI.rolesAPI.list();
        const rolesdict = {};
        roleslist.forEach(role => rolesdict[role.id] = role);
        SBMI.rolesAPI.roles = rolesdict;

        if (SBMI.session.isAdmin()) {
            document.getElementById("accordionRoles").classList.remove("d-none");
        } else {
            document.getElementById("accordionRoles").classList.add("d-none");
            return;
        }

        document.getElementById("accordionRoles").classList.remove("d-none");
        document.getElementById("roles-table-body").innerHTML = roleslist.map(role => 
            role.system ? "": 
            /*template*/`
            <tr>
                <td>
                    ${SBMI.rolesAPI.renderRole(role.id)}  
                </td>
                <td>
                    ${role.id}
                </td>
                <td>
                    <button class="btn btn-sm btn-danger" onclick="SBMI.rolesAPI.openRoleDelete('${role.id}')" data-bs-toggle="modal" data-bs-target="#globalModal">
                        <i class="bi bi-x-circle"></i>
                    </button>
                </td>
            </tr>
        `).join("");
    },

    // [id] id of the role that should be rendered
    renderRole: (id) => {
        let r  = SBMI.rolesAPI.roles[id];
        if(!r) {
            console.debug("roles: renderRole id not found", id)
            return "";
        }
        return /*template*/`
            <span class="badge text-bg-primary" data-role-id="${r.id}">
                ${r.name}
            </span>
        `;
    },

    // ###########################
    // SensBee Roles API calls
    // ###########################

    /**
     * POST /api/roles/create
     * 
     * Create a new role with the given information.
     *
     * @async
     * 
     * @param {Object} info - The information to use.
     * 
     * @returns {Promise<void>}
     *
     */
    create: async (info) => SBMI.auth.Request(`/api/roles/create`, "POST", info),

    /**
     * GET /api/roles/list
     * 
     * List all roles the current user can access.
     *
     * @async
     * 
     * @returns {Promise<Array<Role>>}
     *
     */
    list: async () => SBMI.auth.Request(`/api/roles/list`),

    /**
     * DELETE /api/roles/{id}/delete
     * 
     * Deletes the role with the given id.
     *
     * @async
     * 
     * @param {string} id - The id of the role.
     * 
     * @returns {Promise<void>}
     *
     */
    delete: async (id) => SBMI.auth.Request(`/api/roles/${id}/delete`, "DELETE"),
}
