function processFormData(event, url) {
  event.preventDefault(); // Prevent the form from submitting the default way

  const form = document.getElementById("editForm");
  const formData = new FormData(form);

  // Collect environment variables as a nested JSON object
  const env = {};
  const keys = formData.getAll("env_key");
  const values = formData.getAll("env_value");

  for (let i = 0; i < keys.length; i++) {
    if (keys[i] && values[i]) {
      env[keys[i]] = values[i];
    }
  }

  const custom_active = document.getElementById("toggleCustomDomain");
  // Create the final JSON object
  const data = {
    name: formData.get("name"),
    domains: {
      shared: formData.get("shared"),
      custom: formData.get("custom"),
    },
    container: {
      image: formData.get("image"),
      port: Number(formData.get("port")),
    },
    config: env,
  };
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
