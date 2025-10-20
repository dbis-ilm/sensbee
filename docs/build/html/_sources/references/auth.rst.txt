.. _ref-auth:

Authentication & Authorization
=============================

To access endpoints that require authorized access the request must contain an Authorization header.
Receiving such a header is done via one the following methods:

Authentication
--------------
To authenticate a user we use `OpenID Connect <https://openid.net/developers/how-connect-works>`_.

You can also use any other OIDC server by simply providing its credentials.
Once OIDC credentials have been created add them to the :ref:`config`.

Mock OIDC
^^^^^^^^^
By default the repository comes with a preconfigured mock OIDC. It will always respond with a successfully login for the same user.

.. caution::
    This should only be used for local development or CI.


DBIS Authentik
^^^^^^^^^^^^^^
For projects at TU Ilmenau we provide a ready to use Authentik based OIDC upon request.


Google's OIDC
^^^^^^^^^^^^^
Any project can also use Google's OpenIC Connect. A guide on how to set it up can be found here:
https://developers.google.com/identity/openid-connect/openid-connect



Guest Access
------------

The :ref:`sbmi` allows accessing public endpoints by using the "Guest Access" button on the login screen.
To switch to an authorized session use the red reload button to clear all session related data from local storage.
