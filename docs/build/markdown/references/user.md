<a id="ref-user"></a>

# User

We differentiate between OIDC and SensBee users. OIDC users are explained in [Authentication & Authorization](auth.md#ref-auth).

The email of an authenticated OIDC user is used to authorize the login request to aquire a JWT token for the SensBee user with the same email.

In case no SensBee user with the given email exists it will be created using the given email.
Admins must then verify the new account to allow it to interact with the system.

Validation must by done by an admin user via the user management in the [SensBee Management Interface](../user-guide/sbmi.md#sbmi).

## Root user

If the email returned by the OIDC is the same as the root user email set via the [Server Configuration](config.md#config) then that account will be verified and recieves the admin and root role everytime they authenticate.
