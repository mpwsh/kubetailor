function setUpKeyValuePair(
  groupName,
  keyName,
  valueName,
  addButtonId,
  keyValuePairsId,
  keyPlaceholder,
  valuePlaceholder
) {
  const addButton = document.getElementById(addButtonId);
  const keyValuePairs = document.getElementById(keyValuePairsId);

  addButton.addEventListener("click", () => {
    const uniqueId = Date.now(); // create a unique id based on the current time
    let newKeyValuePair = `<div class="form-group-inline ${groupName}">`;
    if (groupName === "file-group") {
      newKeyValuePair += `
        <input class="kv-input" type="text" name="${keyName}[]" placeholder="${keyPlaceholder}" />
        <button type="button" class="content-btn" data-id="${uniqueId}">Add content</button>
        <input class="hidden-input" type="hidden" name="hidden-file-content-${uniqueId}" />`;
    } else {
      newKeyValuePair += `
      <input class="kv-input" type="text" name="${keyName}" placeholder="${keyPlaceholder}" />
      <input class="kv-input" type="text" name="${valueName}" placeholder="${valuePlaceholder}" />`;
    }
    newKeyValuePair += `<button type="button" class="delete-btn">
        <i class="fas fa-trash"></i>
      </button>
    </div>`;
    keyValuePairs.insertAdjacentHTML("beforeend", newKeyValuePair);
  });

  keyValuePairs.addEventListener("click", (e) => {
    const targetButton = e.target.closest("button.content-btn");
    if (targetButton) {
      const uniqueId = targetButton.getAttribute("data-id"); // Get uniqueId from button's data-id attribute
      const hiddenInput = document.querySelector(
        `input[name='hidden-file-content-${uniqueId}']`
      );
      const editorModal = document.getElementById("editor-modal");
      const editorTextarea = document.getElementById("editor-textarea");
      const editorOkBtn = document.getElementById("editor-ok-btn");
      const editorCancelBtn = document.getElementById("editor-cancel-btn");

      editorTextarea.value = hiddenInput.value; // Load the text from the hidden input field

      // Change the button text based on whether there's data in the hidden input
      if (editorTextarea.value.trim() !== "") {
        targetButton.textContent = "Edit content";
      } else {
        targetButton.textContent = "Add content";
      }

      editorModal.style.display = "block";

      editorOkBtn.onclick = () => {
        // When the OK button is clicked, close the modal and add a new file mount.
        hiddenInput.value = editorTextarea.value; // Save the textarea content back into the hidden input field
        editorModal.style.display = "none";
        editorTextarea.value = ""; // Clear the textarea for next time.
      };

      editorCancelBtn.onclick = () => {
        // When the Cancel button is clicked, just close the modal.
        editorModal.style.display = "none";
        editorTextarea.value = ""; // Clear the textarea for next time.
      };
    }

    if (e.target.closest(".delete-btn")) {
      e.target.closest(`.${groupName}`).remove();
    }
  });
}

const configs = [
  { group: "environment", key: "Key", value: "Value" },
  { group: "secret", key: "Secret Key", value: "Secret Value" },
  { group: "file", key: "File Path", value: "Content" },
  { group: "volume", key: "Mount Path", value: "Size" },
];

configs.forEach((config, index) => {
  const input = config.group;
  const groupName = config.group + "-group";
  const keyName = input + "_key";
  const valueName = input + "_value";
  const addButtonId = "add-" + input + "-btn";
  const keyValuePairsId = input + "-pairs";
  setUpKeyValuePair(
    groupName,
    keyName,
    valueName,
    addButtonId,
    keyValuePairsId,
    config.key,
    config.value
  );
});
