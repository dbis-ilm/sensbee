.. _debug:

Debug with a running Docker Compose Setup
==========================================

.. .note::
    Stop any running sensbee container as this will lead to conlficts when opening ports.

The compose file exports all required ports to run the server on the host instead of in the container.

Start sb_srv locally and attach a debugger. 


VS Code
-------

.. code-block:: js
   :caption: .vscode/launch.json
   :name: launch.json

    {
        ...
        "configurations":[
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug executable 'sb_srv'",
            "cargo": {
                "args": [
                    "build",
                    "--bin=sb_srv",
                    "--package=sb_srv"
                ],
                "filter": {
                    "name": "sb_srv",
                    "kind": "bin"
                }
            },
            "args": [],
            "cwd": "${workspaceFolder}"
        },
        ],
        ...
    }
