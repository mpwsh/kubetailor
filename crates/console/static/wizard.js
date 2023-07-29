window.onload = function () {
  // Define your form ids
  var formIds = [
    "tapp-input",
    "container-input",
    "git-input",
    "environment-input",
    "domain-input",
  ];

  // Create navigation bar
  var navbar = document.createElement("div");
  navbar.className = "navbar";

  formIds.forEach((id, index) => {
    // Create a tab for each form
    var tab = document.createElement("button");
    tab.className = "tab";
    tab.innerHTML =
      id.split("-")[0].charAt(0).toUpperCase() + id.split("-")[0].slice(1); // Convert to title case

    tab.onclick = function () {
      // Hide all forms and remove 'active' class from all tabs
      formIds.forEach((formId) => {
        document.getElementById(formId).style.display = "none";
        document
          .querySelector(`[data-form="${formId}"]`)
          .classList.remove("active");
      });

      // Show the clicked form and add 'active' class to clicked tab
      document.getElementById(id).style.display = "block";
      tab.classList.add("active");
    };

    // Add data attribute to link tab with form
    tab.setAttribute("data-form", id);

    // Add the tab to the navigation bar
    navbar.appendChild(tab);
  });

  // Add the navigation bar to your specific div
  var tabsContainer = document.getElementById("tabsContainer");
  tabsContainer.appendChild(navbar);

  // Initialize the first form as visible and the rest as hidden
  formIds.forEach((id, index) => {
    var form = document.getElementById(id);
    form.style.display = index === 0 ? "block" : "none";
  });

  // Add 'active' class to first tab
  document.querySelector(".tab").classList.add("active");
};
