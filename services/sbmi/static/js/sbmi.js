/*

  JavaScript 

*/

//
// Page Entry Point
//
const loadDashboard = async () => {
  // Check if a session is present
  const has_session = await SBMI.session.isLoggedIn();
  if (!has_session) {
    // No session detected -> show login box
    document.getElementById("login-section").classList.remove("d-none");
    document.getElementById("dashboard-section").classList.add("d-none");
    document.getElementById("navUserInfo").classList.add("d-none");

    // Render OIDC options
    SBMI.auth.renderIDPs();

    // Check if developer access is available
    SBMI.auth.checkDevAccess();

    return;
  }

  document.getElementById("login-section").classList.add("d-none");
  document.getElementById("dashboard-section").classList.remove("d-none");
  document.getElementById("navUserInfo").classList.remove("d-none");

  // Load roles and render self which requires roles to have been loaded
  await SBMI.rolesAPI.render().then(() => SBMI.auth.render());

  // Try render Sensors
  SBMI.sensorsAPI.render();

  // Try render users
  SBMI.usersAPI.render();

  // Load events
  SBMI.eventsAPI.init();

  // If in guest Mode hide all non public buttons from the sidebar
  if (SBMI.session.isGuest()) {
    document.getElementById("accordionDataTransforms").classList.add("d-none");
    document.getElementById("accordionEventHandler").classList.add("d-none");
  }
};

window.onload = loadDashboard;

// #----------------------------------#
// UI Elements
//

const toggleSidebar = () => {
  const current_state = document.getElementById("sbmi-app-name").hidden;
  document.querySelectorAll(".sidebar-full").forEach((e) => e.hidden = current_state ? false : true);
  document.getElementById("sidebarToggleBtn").innerHTML = current_state ? '<i class="bi bi-layout-sidebar"></i>' : '<i class="bi bi-layout-sidebar-inset"></i>';
  // TODO save state?
};


// #----------------------------------#
// Reuseable UI Elements 
//

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
const openOffcanvasTop = (header, body, show) => {
  document.getElementById("offcanvasBottomHeader").innerHTML = header;
  document.getElementById("offcanvasBottomBody").innerHTML = body;
  if (show) {
    new bootstrap.Offcanvas('#offcanvasBottom').show();
  }
};

const formFeedback = (id, msg) => {
  feedback = document.getElementById(id);
  feedback.classList.remove("d-none", "alert", "alert-danger");
  feedback.classList.add("alert", "alert-success");
  feedback.innerHTML = msg;

  userFeedbackSucc(msg);
};
const formFeedbackErr = (id, error) => {
  feedback = document.getElementById(id);
  feedback.classList.remove("d-none", "alert", "alert-success");
  feedback.classList.add("alert", "alert-danger");
  feedback.innerHTML = `Failed with: ${error}`;

  userFeedback({
    "bg": "danger",
    "head": "Error",
    "body": error,
  });
};


// #----------------------------------#
//  TOAST
// 

const toastContainer = document.getElementById("bs5-toast-wrapper");
const toastTemplate = document.getElementById("bs5-toast");
const userFeedback = (opts) => {

  if (! typeof opts === 'object') {
    console.error("userFeedback called with invalid opts", opts);
    return;
  }

  const toastFrag = toastTemplate.content.cloneNode(true);
  const actualToast = toastFrag.querySelector('.toast');

  var header = actualToast.querySelector('.toast-header');
  switch (opts.bg) {
    case "success":
      header.classList.add("text-bg-primary", "bg-primary");
      actualToast.dataset.bsAutohide = true;
      actualToast.dataset.bsDelay = 5000;
      break;
    case "warning":
      header.classList.add("text-bg-warning", "bg-warning");
      break;
    case "danger":
      header.classList.add("text-bg-danger", "bg-danger");
      break;
    default:
      break;
  }

  header = actualToast.querySelector(".bs5-toast-header");
  if ('head' in opts) {
    header.innerHTML = opts.head;
  }

  var body = actualToast.querySelector(".toast-body");
  if ('body' in opts) {
    body.innerHTML = opts.body;
  }

  toastContainer.appendChild(toastFrag);
  new bootstrap.Toast(actualToast).show();
};
const userFeedbackSucc = (msg) => userFeedback({ "bg": "success", "head": "Success Response", "body": msg });
const userFeedbackErr = (msg) => userFeedback({ "bg": "danger", "head": "Error", "body": msg });


// #----------------------------------#
//  PWA
// 
if ('serviceWorker' in navigator) {
  navigator.serviceWorker.register('js/pwa/service-worker.js')
    .catch((error) => console.error('Service Worker registration errorr ' + error));
}


// #----------------------------------#
//  THEME
// 
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


// #----------------------------------#
//  STATEFULL UI
// 
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