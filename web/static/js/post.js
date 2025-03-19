document.body.addEventListener("htmx:beforeRequest", function (event) {
  if (event.detail.elt.id === "editForm") {
    const formData = new FormData(event.detail.elt);

    console.log(formData);
    const env = collectData("environment_key", "environment_value", formData);
    const secrets = collectData("secret_key", "secret_value", formData);
    const volumes = collectData("volume_key", "volume_value", formData);
    const files = {};
    const keyInputs = document.querySelectorAll("input[name='file_key[]']");
    keyInputs.forEach((keyInput) => {
      const key = keyInput.value;
      const uniqueId = keyInput.nextElementSibling.getAttribute("data-id");
      const hiddenInput = document.querySelector(
        `input[name='hidden-file-content-${uniqueId}']`,
      );
      if (hiddenInput) {
        console.log("Reading File: " + key);
        console.log("Content: " + hiddenInput.value);
        files[key] = hiddenInput.value;
      }
    });
    const custom_active = document.getElementById("toggleCustomDomain");

    const data = {
      name: formData.get("name"),
      group: formData.get("group"),
      domains: {
        shared: formData.get("shared"),
        custom: formData.get("custom"),
      },
      container: {
        image: formData.get("image"),
        port: Number(formData.get("port")),
        replicas: Number(formData.get("replicas")),
        volumes: volumes,
        files: files,
        buildCommand: formData.get("buildcmd"),
        runCommand: formData.get("runcmd"),
      },
      git: {
        repository: formData.get("repository"),
        branch: formData.get("branch"),
      },
      env: env,
      secrets: secrets,
    };
    if (custom_active.innerText == "Enable") {
      data.domains.custom = null;
    }

    event.detail.xhr.send(JSON.stringify(data));
  }
});

// Use htmx:configRequest to set headers
document.body.addEventListener("htmx:configRequest", function (event) {
  if (event.detail.elt.id === "editForm") {
    // Add or modify headers here
    event.detail.headers["Content-Type"] = "application/json";
  }
});

function collectData(keyName, valueName, formData) {
  const data = {};
  const keys = formData.getAll(keyName);
  const values = formData.getAll(valueName);

  for (let i = 0; i < keys.length; i++) {
    if (keys[i] && values[i]) {
      data[keys[i]] = values[i];
    }
  }

  console.info(data);
  return data;
}
