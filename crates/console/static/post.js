function processFormData(event, url) {
  event.preventDefault();

  const form = document.getElementById("editForm");
  const formData = new FormData(form);

  const env = collectData("environment_key", "environment_value", formData);
  const secrets = collectData("secret_key", "secret_value", formData);
  const volumes = collectData("volume_key", "volume_value", formData);
  const files = {};
  const keyInputs = document.querySelectorAll("input[name='file_key[]']");
  keyInputs.forEach((keyInput) => {
    const key = keyInput.value;
    const uniqueId = keyInput.nextElementSibling.getAttribute("data-id");
    const hiddenInput = document.querySelector(
      `input[name='hidden-file-content-${uniqueId}']`
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
    },
    env: env,
    secrets: secrets,
  };
  if (custom_active.innerText == "Enable") {
    data.domains.custom = null;
  }

  console.info(JSON.stringify(data));
  fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(data),
  })
    .then((response) => {
      if (response.ok) {
        window.location.href = `/dashboard/loading?name=${data.name}`;
      } else {
        console.error("Form submission failed:", response.statusText);
      }
    })
    .catch((error) => {
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
