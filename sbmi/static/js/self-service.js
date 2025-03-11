// Register

// Create a shared namespace if it doesn't exist yet
var SBMI = SBMI || {};

SBMI.selfService = {

    // ###########################
    // API calls
    // ###########################

    // /api/users/register
    register: () => {

    },


    /* ------------------------- */
    // 

    registerRender: () => {
        let target = document.getElementById("");
        target.innerHTML(`<hr><button class="btn btn-primary">Register</button>`);
    },

    enable: () => {
        SBMI.selfService.registerRender();
    }
}