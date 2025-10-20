// Create a shared namespace if it doesn't exist yet
var SBMI = SBMI || {};

// Session Management
SBMI.session = {

    // creates a session from a valid jwt
    sessnionFromToken: async (token) => {
        // extract uuid
        uuid = SBMI.helper.parseJwt(token).sub;
        SBMI.session.setUuid(uuid);

        SBMI.session.setToken(token);

        // get user info
        info = await SBMI.usersAPI.info(uuid);
        SBMI.session.setUser(info);
    },

    isLoggedIn: async () => {

        // Check if we have an active session already
        const activeSession = !!localStorage.getItem('sessionToken');
        if (activeSession) {
            return true;
        }

        // We might be returning from an OIDC auth flow
        try {
            // Current OpenID flow returns us home with the token as a search param
            const url = new URL(window.location.href);
            const token = url.searchParams.get("jwt");
            if (token) {
                await SBMI.session.sessnionFromToken(token);

                // remove the search params here to make current auth logic work
                url.searchParams.delete("jwt");
                history.replaceState({}, document.title, url.toString());

                return true;
            }
        } catch (error) {
            console.error(error);
        }

        return false;
    },
    isAdmin: () => {
        if (!SBMI.session.isLoggedIn() || SBMI.session.isGuest()) {
            return false;
        }
        user = SBMI.session.getUser();
        return undefined !== user.roles.find(role => role.name == "Admin" && role.system == true);
    },
    isRoot: () => {
        if (!SBMI.session.isLoggedIn() || SBMI.session.isGuest()) {
            return false;
        }
        user = SBMI.session.getUser();
        return undefined !== user.roles.find(role => role.name == "Root" && role.system == true);
    },
    isSelf: (id) => {
        if (!id) {
            return false;
        }
        if (!SBMI.session.isLoggedIn()) {
            return false;
        }
        return id == SBMI.session.getUuid();
    },

    //
    // Guest Access
    //

    // Bypass JWT based session and show board with publicly available data
    asGuest: () => {
        SBMI.session.clear();
        SBMI.session.setUser({ name: "Guest", roles: [] });
        SBMI.session.setToken("guest");
        window.location.reload();
    },
    isGuest: () => {
        return SBMI.session.getToken() == "guest";
    },

    //
    // Developer Access
    //

    // SensBee grants a default token when configured that way
    tryDevelopmentAccess: () => {
        SBMI.session.clear();
        // We need to switch to guest mode
        SBMI.session.setToken("guest");
        // Now try to aquire token from dev login endpoint
        SBMI.auth.login().then(async (resp) => {
            await SBMI.session.sessnionFromToken(resp.jwt);

            window.location.reload();
        });
    },

    // Session state data

    // [token] The jwt token returned from a sucessfull login
    setToken: (token) => localStorage.setItem('sessionToken', token),
    getToken: () => localStorage.getItem('sessionToken'),

    // [uuid] The extracted uuid from the given token
    // TODO Remove this?
    setUuid: (uuid) => localStorage.setItem('sessionUuid', uuid),
    getUuid: () => localStorage.getItem('sessionUuid'),

    // [user] The user info returned for the uuid
    setUser: (user) => localStorage.setItem('user', JSON.stringify(user)),
    getUser: () => JSON.parse(localStorage.getItem('user')),

    //
    // Session clear functions
    //

    clear: () => {
        localStorage.removeItem('sessionToken');
        localStorage.removeItem('sessionUuid');
        localStorage.removeItem('user');
    },

    // Reset the UI state to not logged in
    logout: () => {
        SBMI.auth.logout()
            .finally(() => {
                SBMI.session.clear();
                window.location.reload();
            });
    },
};