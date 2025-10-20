<a id="quick-start"></a>

# Quick Start Guide

SensBee requires docker to run this project. The project is based on multiple containers working toegether.
For a detailed component overview consult the [Architecture](../references/arch.md#ref-arch) documentation page.

## Preamble

According to the [Authentication & Authorization](../references/auth.md#ref-auth) section, SensBee uses OIDC to authenticate users. This requires to setup custom OIDC provider.

For quick-start purpose, we offer a ‘development’ mode which creates a default admin user during startup, according to the specified root_user_email field in the [Server Configuration](../references/config.md#config).
All login requests with this user will automatically result in a successful login, without requiring to provide further authentication parameters.
This allows to try out the SensBee system before setting up OIDC provider for the deployment during production.

## Getting the setup up and running

The fastest way to get a full stack up and running is using the following command inside the root folder.

```sh
docker compose --profile full up -d
```

Starting the compose stack automatically builds all required container and sets up the correct networking configuration.

#### NOTE
To start the stack using a local sensbee server refer to [Docker Compose Setup](../developer-guide/docker.md#docker).
For debugging refer to [Debug with a running Docker Compose Setup](../developer-guide/debugging.md#debug).

Once all container have been started open the [SensBee Management Interface](sbmi.md#sbmi) in your browser to setup sensors.

**Optional next steps**

Setup Observability tools via [OpenTelementry](../developer-guide/otel.md#opentelemetry). You can also use it to visualize incoming sensor data.
