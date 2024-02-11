// Get all the buttons with class name 'content-btn' and loop over them
const contentButtons = document.querySelectorAll(".content-btn");
contentButtons.forEach((button) => {
  const uniqueId = button.getAttribute("data-id");
  const hiddenInput = document.querySelector(
    `input[name='hidden-file-content-${uniqueId}']`
  );
  checkAndUpdateButtonContent(button, hiddenInput);
});

function checkAndUpdateButtonContent(button, hiddenInput) {
  if (hiddenInput.value.trim() !== "") {
    button.textContent = "Edit content";
  } else {
    button.textContent = "Add content";
  }
}
