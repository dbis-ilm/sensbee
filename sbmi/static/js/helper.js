
// Create a shared namespace if it doesn't exist yet
var SBMI = SBMI || {};

SBMI.helper = {

    // from https://stackoverflow.com/questions/38552003/how-to-decode-jwt-token-in-javascript-without-using-a-library
    //
    parseJwt: (token) => {
        var base64Url = token.split('.')[1];
        var base64 = base64Url.replace(/-/g, '+').replace(/_/g, '/');
        var jsonPayload = decodeURIComponent(window.atob(base64).split('').map(function(c) {
            return '%' + ('00' + c.charCodeAt(0).toString(16)).slice(-2);
        }).join(''));

        return JSON.parse(jsonPayload);
    },

    // Form pasring and data retrieval
    getFormData: (id) => {
        return new FormData(document.forms[id]);
    },
    getFormDataAsJSON: (id) => {
        let data = SBMI.helper.getFormData(id);
        let jsonData = {};
        data.forEach((k,v)=>{
            jsonData[v]=k;
        });
        return jsonData;
    },
}