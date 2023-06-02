function processFormData(event, url) {
  event.preventDefault(); // Prevent the form from submitting the default way

  const form = document.getElementById("editForm");
  const formData = new FormData(form);

  const env_vars = collectData(
    "environment_key",
    "environment_value",
    formData
  );
  const secrets = collectData("secret_key", "secret_value", formData);
  const volumes = collectData("volume_key", "volume_value", formData);
  const files = collectData("file_key", "file_value", formData);

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
    },
    env_vars: env_vars,
    secrets: secrets,
  };
  console.log("sending: " + JSON.stringify(data));
  if (custom_active.innerText == "Enable") {
    data.domains.custom = null;
  }
  // Submit the form using the Fetch API
  fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(data),
  })
    .then((response) => {
      if (response.ok) {
        // Redirect or perform other actions on successful submission
        window.location.href = `/dashboard/loading?name=${data.name}`;
      } else {
        // Handle errors
        console.error("Form submission failed:", response.statusText);
      }
    })
    .catch((error) => {
      // Handle network errors
      console.error("Network error:", error);
    });
}

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
