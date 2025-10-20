// Creates a new WebSocket connection to the specified URL.
const socket = new WebSocket('ws://localhost:9002');

const SCRIPT_ID = Math.floor(Math.random() * 10001);

// Executes when the connection is successfully established.
socket.addEventListener('open', event => {
    console.log('WebSocket connection established!');
    socket.send(JSON.stringify({ "script_id": SCRIPT_ID, "type": 2, "data": `{"col1": "42","col2": "56.789","col3": "Hello"}` }));

    // wait for script load response

    // send script

    // wait for transform reponse
});

// Listen for messages and executes when a message is received from the server.
socket.addEventListener('message', event => {
    console.log('Message from server: ');

    var resp = JSON.parse(event.data);

    console.log(resp);

    switch (resp.type) {
        case 3:
            socket.send(JSON.stringify({
                "script_id": SCRIPT_ID, "type": 4, "data": `
                //console("hello from the otter-side");
                let inputData = data;
                let col1 = parseInt(inputData.col1);
                let col2 = parseFloat(inputData.col2);
                JSON.stringify([{'col1': col1, 'col2':col2,'col3':inputData.col3}]);
            `}));
            break;

        default:

            break;
    }
});

// Executes when the connection is closed, providing the close code and reason.
socket.addEventListener('close', event => {
    console.log('WebSocket connection closed:', event.code, event.reason);
});

// Executes if an error occurs during the WebSocket communication.
socket.addEventListener('error', error => {
    console.error('WebSocket error:', error);
});


