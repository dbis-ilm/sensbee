/*

    JavaScript 

*/

// Create a shared namespace if it doesn't exist yet
var SBMI = SBMI || {};

SBMI.basePath = "http://localhost:8080";

// Session Management
SBMI.session = {
  isLoggedIn: () => !!localStorage.getItem('sessionToken'),
  isAdmin: () => {
    if (!SBMI.session.isLoggedIn()) {
      return false;
    }
    user = SBMI.session.getUser();
    return undefined !== user.roles.find(role => role.name == "Admin" && role.system == true);
  },

  // Session state data

  // [token] The jwt token returned from a sucessfull login
  setToken: (token) => localStorage.setItem('sessionToken', token),
  getToken: () => localStorage.getItem('sessionToken'),

  // [uuid] The extracted uuid from the given token
  setUuid: (uuid) => localStorage.setItem('sessionUuid', uuid),
  getUuid: () => localStorage.getItem('sessionUuid'),

  // [user] The user info returned for the uuid
  setUser: (user) => localStorage.setItem('user', JSON.stringify(user)),
  getUser: () => JSON.parse(localStorage.getItem('user')),

  // Reset the UI state to not logged in
  logout: () => {
    localStorage.removeItem('sessionToken');
    localStorage.removeItem('sessionUuid');
    localStorage.removeItem('user');
  },
};

// UI Functions
const loadDashboard = async () => {
  if (!SBMI.session.isLoggedIn()) {
    document.getElementById("login-section").classList.remove("d-none");
    document.getElementById("dashboard-section").classList.add("d-none");
    document.getElementById("navUserInfo").classList.add("d-none");
    return;
  }

  document.getElementById("login-section").classList.add("d-none");
  document.getElementById("dashboard-section").classList.remove("d-none");
  document.getElementById("navUserInfo").classList.remove("d-none");

  // Load roles
  await SBMI.rolesAPI.render();

  // Load user
  // This requires that roles have already been load
  SBMI.auth.render();

  // Load Sensors
  SBMI.sensorsAPI.render();

  if (SBMI.session.isAdmin()) {
    SBMI.usersAPI.render();
    document.getElementById("accordionUsers").classList.remove("d-none");
  } else {
    document.getElementById("accordionUsers").classList.add("d-none");
  }
};

// UI Elements 

const openModal = (title, body, acceptFn, acceptTxt) => {
  document.getElementById("globalModalTitle").innerHTML = title;
  document.getElementById("globalModalBody").innerHTML = body;

  btn = document.getElementById("globalModalAcceptBtn");
  footer = document.getElementById("globalModalFooter");
  if (acceptFn === undefined && acceptTxt === undefined) {
    footer.classList.add("d-none");
  } else {
    footer.classList.remove("d-none");
    btn.innerHTML = acceptTxt ?? "Submit";
    btn.onclick = function () {
      acceptFn();
    };
  }
};

const openOffcanvas = (header, body) => {
  document.getElementById("offcanvasHeader").innerHTML = header;
  document.getElementById("offcanvasBody").innerHTML = body;
};

const formFeedback = (id, msg) => {
  feedback = document.getElementById(id);
  feedback.classList.remove("d-none", "alert", "alert-danger");
  feedback.classList.add("alert", "alert-success");
  feedback.innerHTML = msg;
};
const formFeedbackErr = (id, error) => {
  feedback = document.getElementById(id);
  feedback.classList.remove("d-none", "alert", "alert-success");
  feedback.classList.add("alert", "alert-danger");
  feedback.innerHTML = `Failed with: ${error}`;
};

window.onload = loadDashboard;


// Progressive Web App

if ('serviceWorker' in navigator) {
  navigator.serviceWorker.register('js/pwa/service-worker.js')
    .catch((error) => console.error('Service Worker registration errorr ' + error));
}

/*!
 * Color mode toggler for Bootstrap's docs (https://getbootstrap.com/)
 * Copyright 2011-2024 The Bootstrap Authors
 * Licensed under the Creative Commons Attribution 3.0 Unported License.
 */

(() => {
  'use strict'

  const getStoredTheme = () => localStorage.getItem('theme')
  const setStoredTheme = theme => localStorage.setItem('theme', theme)

  const getPreferredTheme = () => {
    const storedTheme = getStoredTheme()
    if (storedTheme) {
      return storedTheme
    }

    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  }

  const setTheme = theme => {
    if (theme === 'auto') {
      document.documentElement.setAttribute('data-bs-theme', (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'))
    } else {
      document.documentElement.setAttribute('data-bs-theme', theme)
    }
  }

  setTheme(getPreferredTheme())

  const showActiveTheme = (theme, focus = false) => {
    const themeSwitcher = document.querySelector('#bd-theme')

    if (!themeSwitcher) {
      return
    }

    //const themeSwitcherText = document.querySelector('#bd-theme-text')
    const activeThemeIcon = document.querySelector('.theme-icon-active use')
    const btnToActive = document.querySelector(`[data-bs-theme-value="${theme}"]`)

    document.querySelectorAll('[data-bs-theme-value]').forEach(element => {
      element.classList.remove('active')
      element.setAttribute('aria-pressed', 'false')
    })

    btnToActive.classList.add('active')
    btnToActive.setAttribute('aria-pressed', 'true')
    //const themeSwitcherLabel = `${themeSwitcherText.textContent} (${btnToActive.dataset.bsThemeValue})`
    //themeSwitcher.setAttribute('aria-label', themeSwitcherLabel)

    if (focus) {
      themeSwitcher.focus()
    }
  }

  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    const storedTheme = getStoredTheme()
    if (storedTheme !== 'light' && storedTheme !== 'dark') {
      setTheme(getPreferredTheme())
    }
  })

  window.addEventListener('DOMContentLoaded', () => {
    showActiveTheme(getPreferredTheme())

    document.querySelectorAll('[data-bs-theme-value]')
      .forEach(toggle => {
        toggle.addEventListener('click', () => {
          const theme = toggle.getAttribute('data-bs-theme-value')
          setStoredTheme(theme)
          setTheme(theme)
          showActiveTheme(theme, true)
        })
      })
  })
})()

// Save and restore the state of the main collapse elements

document.addEventListener('DOMContentLoaded', function () {
  const accordionItems = document.querySelectorAll('.accordion-collapse');

  // Restore state from localStorage
  accordionItems.forEach(item => {
    const id = item.getAttribute('id');
    const isExpanded = localStorage.getItem(id) === 'true';

    if (isExpanded) {
      item.classList.add('show'); // Open the collapse
      const button = document.querySelector(`[data-bs-target="#${id}"]`);
      if (button) {
        button.classList.remove('collapsed');
        button.setAttribute('aria-expanded', 'true');
      }
    }
  });

  // Save state on toggle
  accordionItems.forEach(item => {
    item.addEventListener('shown.bs.collapse', function () {
      const id = item.getAttribute('id');
      localStorage.setItem(id, 'true'); // Save as open
    });

    item.addEventListener('hidden.bs.collapse', function () {
      const id = item.getAttribute('id');
      localStorage.setItem(id, 'false'); // Save as closed
    });
  });
});

// Apply the config values
document.addEventListener('DOMContentLoaded', function () {

  config.app.version = '0.2';

  // Set app name
  document.getElementById('sbmi-app-name').innerHTML = config.app.name;

  // Set version
  document.getElementById('sbmi-app-version').textContent = config.app.version;

  // Set base url
  document.getElementById('sbmi-api-baseUrl').value = config.api.baseUrl;
  // TODO if the baseUrl is not available then show the options
  
  if(config.app.allowRegister === true){
    document.getElementById('sbmi-app-registerEnabled').classList.remove("d-none");
  }
});