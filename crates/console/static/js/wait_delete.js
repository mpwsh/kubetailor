document.addEventListener("DOMContentLoaded", async function () {
  const urlParams = new URLSearchParams(window.location.search);
  const loading = urlParams.get("loading");

  if (loading === "true") {
    const spinner = document.getElementById("loading-spinner");
    const subtitle = document.getElementById("subtitle");
    const tapp_name = document.getElementById("tapp-name");
    const responseContainer = document.getElementById("response-container");
    spinner.style.display = "block";
    subtitle.style.display = "none";
    responseContainer.style.display = "none";
    while (true) {
      try {
        const response = await fetch(`/dashboard/view?name=${tapp_name.value}`);
        if (response.status === 404) {
          window.location.href = "/dashboard";
        }
        await new Promise((resolve) => setTimeout(resolve, 1500));
      } catch (error) {
        console.error("Error:", error);
      }
    }
  }
});
