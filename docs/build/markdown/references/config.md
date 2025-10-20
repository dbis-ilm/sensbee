<a id="config"></a>

# Server Configuration

To configure the server we use a YAML file.

## Config file

By default the file in `config/config.yml` is used by either the local instance or the compose stack instance via a volume mount.
After a change you need to restart the server because the config is only parsed once during startup.

## Config file options

This gives a quick overview of important configuration options.
For a detailed list consult the config file itself.
Some options have a DEFAULT value as stated in the config file.

### Server

#### Host & Port

The host IP and Port to use for sensbee server.

#### External Host

The external Hostname under which this instance is reachable. Required when a OIDC is given for the callback. Defaults to localhost.

### Authenticaion

#### OIDC clients

A list of client configurations to be used for user authentication.

#### Root user email

If a OIDC authenticates a user and returns this email address then the resulting sensbee user will be assigned the root role.
Emails are used to identify a user regardless of what OIDC they used.

#### Default verified

When this option is activated, every new user will automatically be verified.

## Environment

SB_CONTAINER
: If set the instance will use docker container names to connect to other internal services.
  Otherwise “localhost” will be used.
