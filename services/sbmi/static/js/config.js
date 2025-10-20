// config.js
//
// This file contains the configuration settings for SBMI.
//

const config = {
    // The name to be used in the header
    app: {
        name: '<strong>S</strong>ens<strong>B</strong>ee </i><br><strong>M</strong>anagement <strong>I</strong>nterface',

        // Controls wether to show the register button in the login form
        allowRegister: true,

        // Starting Position for the Map if no sensor is shown
        mapStartPos: [50.681775, 10.940129],
        mapStartZoom: 15,

        // Zoom level when a sensor is shown on the map
        mapSensorZoom: 16,
    },

    // Options for the actual URL to be used for queries
    api: {
        defaultURL: new URL('http://localhost:8080'),
        url: undefined, // URL object that is valid after successfull login
        getURL: () => {
            // 1. Check if a successfull login has create a valid URL object
            if (config.api.url !== undefined) {
                return config.api.url;
            }

            // 2. Check if the user has provided an override for the URL
            let input = document.getElementById("sbmi-api-baseUrl").value;
            if (input != "") {
                try {
                    // Make sure its a valid URL
                    let userOverride = new URL(input);
                    // Only use it if it differs from the default URL
                    if (userOverride.toString() != config.api.defaultURL.toString()) {
                        return userOverride;
                    }
                } catch (error) {
                    userFeedbackErr(error.toString());
                }
            }

            // 3. Use the default value
            return config.api.defaultURL;
        },
    },
};


// Apply the config values
document.addEventListener('DOMContentLoaded', function () {

    config.app.version = '1.0.0';

    // Set app name
    document.querySelectorAll('.sbmi-app-name').forEach((e) => {
        e.innerHTML = config.app.name;
    });

    // Set version
    document.getElementById('sbmi-app-version').textContent = config.app.version;

    // Set base url
    document.getElementById('sbmi-api-baseUrl').value = config.api.getURL();
    // TODO if the baseUrl is not available then show the options
});