# SBMI - SensBee Management Interface

A graphical management interface.
Entierly written in javascript.
Uses the SensBee API.

## NOTE

State management is done entirely in the browser and saved in localStorage.

If the UI is in a state where nothing works:

```
localStorage.clear()
```

That returns the interface to the login screen on page reload.

Alternativly, you can use the red reload button in the bottom left corner to achieve the same result.

## Structure

sbmi.js is the entrypoint or the main for all logic.

All other .js files have to loaded before if they are to be used.

Each .js file creates a namespace where its functionality is made available.
