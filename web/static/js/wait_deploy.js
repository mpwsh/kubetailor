document.body.addEventListener("htmx:afterSwap", function (event) {
  let container = {
    spinner: document.getElementById("tapp-spinner-container"),
    success: document.getElementById("tapp-success-container"),
    value: document.getElementById("tapp-name").value,
    available: false,
  };
  let custom_domain = {
    spinner: document.getElementById("custom-spinner-container"),
    success: document.getElementById("custom-success-container"),
    value: document.getElementById("tapp-custom-domain").value,
    available: false,
  };
  let shared_domain = {
    spinner: document.getElementById("shared-spinner-container"),
    success: document.getElementById("shared-success-container"),
    value: document.getElementById("tapp-shared-domain").value,
    available: false,
  };
  let custom_domain_ssl = {
    spinner: document.getElementById("custom-ssl-spinner-container"),
    success: document.getElementById("custom-ssl-success-container"),
    available: false,
  };
  let shared_domain_ssl = {
    spinner: document.getElementById("shared-ssl-spinner-container"),
    success: document.getElementById("shared-ssl-success-container"),
    available: false,
  };
  if (`${custom_domain.value}` === "") {
    custom_domain.available = true;
    custom_domain_ssl.available = true;
  }

  while (
    !container.available ||
    !custom_domain.available ||
    !custom_domain_ssl.available ||
    !shared_domain.available ||
    !shared_domain_ssl.available
  ) {
    if (container.available === false) {
      try {
        const response = fetch(`/dashboard/tapp/view?name=${container.value}`);
        if (response.status === 200) {
          console.log("Container ready");
          container.spinner.style.display = "none";
          container.success.style.display = "inline-flex";
          container.success.classList.add("drawn");
          container.available = true;
        }
      } catch (error) {
        //console.error("Error:", error);
      }
    }
    if (custom_domain.available === false && custom_domain.value != "") {
      try {
        const response = fetch(`http://${custom_domain.value}`);
        if (response.status === 308 || response.status === 200) {
          console.log("Custom domain ready");
          custom_domain.spinner.style.display = "none";
          custom_domain.success.style.display = "inline-flex";
          custom_domain.success.classList.add("drawn");
          custom_domain.available = true;
        } else if (response.status === 404) {
        }
      } catch (error) {
        //console.error("Error:", error);
      }
    }
    if (custom_domain_ssl.available === false && custom_domain.value != "") {
      try {
        const response = fetch(`https://${custom_domain.value}`);
        if (response.status === 200) {
          console.log("Custom domain SSL ready");
          custom_domain_ssl.spinner.style.display = "none";
          custom_domain_ssl.success.style.display = "inline-flex";
          custom_domain_ssl.success.classList.add("drawn");
          custom_domain_ssl.available = true;
        }
      } catch (error) {
        //console.error("Error:", error);
      }
    }
    if (shared_domain.available === false) {
      try {
        const response = fetch(`http://${shared_domain.value}`);
        if (
          response.status === 308 ||
          response.status === 200 ||
          response.status === 303
        ) {
          console.log("Shared domain ready");
          shared_domain.spinner.style.display = "none";
          shared_domain.success.style.display = "inline-flex";
          shared_domain.success.classList.add("drawn");
          shared_domain.available = true;
        }
      } catch (error) {
        //console.error("Error:", error);
      }
    }
    if (shared_domain_ssl.available === false) {
      try {
        const response = fetch(`https://${shared_domain.value}`);
        if (response.status === 200) {
          console.log("Shared domain SSL ready");
          shared_domain_ssl.spinner.style.display = "none";
          shared_domain_ssl.success.style.display = "inline-flex";
          shared_domain_ssl.success.classList.add("drawn");
          shared_domain_ssl.available = true;
        }
      } catch (error) {
        //console.error("Error:", error);
      }
    }

    new Promise((resolve) => setTimeout(resolve, 3000));
  }
  window.location.href = `https://${shared_domain.value}`;
});
