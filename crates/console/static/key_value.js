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
    const newKeyValuePair = `
      <div class="form-group-inline ${groupName}">
        <input class="kv-input" type="text" name="${keyName}" placeholder="${keyPlaceholder}" />
        <input class="kv-input" type="text" name="${valueName}" placeholder="${valuePlaceholder}" />
        <button type="button" class="delete-btn">
          <i class="fas fa-trash"></i>
        </button>
      </div>`;
    keyValuePairs.insertAdjacentHTML("beforeend", newKeyValuePair);
  });

  keyValuePairs.addEventListener("click", (e) => {
    if (e.target.closest(".delete-btn")) {
      e.target.closest(".key-value-group").remove();
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
