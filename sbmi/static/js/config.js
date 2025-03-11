// config.js
//
// This file contains the configuration settings for SBMI.
//

const config = {
    // The name to be used in the header
    app: {
        name: '<i class="bi bi-database-fill-gear"></i>&nbsp;SensBee Management Interface',

        // Controls wether to show the register button in the login form
        allowRegister: true,
    },

    // Options for the actual URL to be used for queries
    api: {
        baseUrl: 'http://localhost:8080',
    },
};