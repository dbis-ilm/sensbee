<a id="deployment"></a>

# Production deployment

Some considerations for deployments.

## Generate JWT keypair

The production compose file expects a key pair to be present in the configuration directory.

An example generation command using the openssl command:

Now place the two .pem\` files into config/jwt/. During startup the log should indicate that custom keys are used.

## Set URL in SBMI

Under services/sbmi/static/js/config.js you must set the correct URL for where to access sensbee.

Keep in mind that compose mounts the files from the filesystem. So the access rights are the same as on the host.

## Reverse proxy

Deployments should always combine the compose stack with a reverse proxy.
Exposing the stack as is should not be done!

The following ports should be forwareded by your reverse proxy:
SensBee HTTP
SBMI HTTP (and WebSocket upgrade)
Mosquitto TCP Ports

Consult the compose file for more details.
