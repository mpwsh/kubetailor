window.onload = function () {
  // Define your form ids
  var formIds = [
    "tapp-input",
    "container-input",
    "domain-input",
    "environment-input",
    "secrets-input",
    "files-input",
    "volumes-input",
  ];

  // Initialize the first form as visible and the rest as hidden
  formIds.forEach((id, index) => {
    var form = document.getElementById(id);
    form.style.display = index === 0 ? "block" : "none";
  });

  // Initialize progress bar
  var progressBar = document.getElementById("progressBar");
  progressBar.max = formIds.length;
  progressBar.value = 1;

  // Create Next and Previous buttons
  formIds.forEach((id, index) => {
    var form = document.getElementById(id);
    var nextButton = document.createElement("button");
    var prevButton = document.createElement("button");

    // Next button setup
    nextButton.innerHTML = index < formIds.length - 1 ? "Next" : "Submit";
    nextButton.onclick = function (e) {
      e.preventDefault();
      if (index < formIds.length - 1) {
        // Hide current form and show next form
        form.style.display = "none";
        var nextForm = document.getElementById(formIds[index + 1]);
        nextForm.style.display = "block";

        // Update progress bar
        progressBar.value += 1;
      } else {
        // Submit the final form
        document.getElementById("editForm").submit();
      }
    };

    // Previous button setup
    prevButton.innerHTML = "Previous";
    prevButton.onclick = function (e) {
      e.preventDefault();
      if (index > 0) {
        // Hide current form and show previous form
        form.style.display = "none";
        var prevForm = document.getElementById(formIds[index - 1]);
        prevForm.style.display = "block";

        // Update progress bar
        progressBar.value -= 1;
      }
    };

    // Only show the 'Previous' button if it's not the first form
    if (index > 0) {
      form.appendChild(prevButton);
    }

    form.appendChild(nextButton);
  });
};
