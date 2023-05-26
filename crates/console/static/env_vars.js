const addButton = document.getElementById("add-env-btn");
const keyValuePairs = document.getElementById("key-value-pairs");

addButton.addEventListener("click", () => {
  const newKeyValuePair = `
    <div class="form-group-inline key-value-group">
      <input class="kv-input" type="text" name="env_key" placeholder="Key" />
      <input class="kv-input" type="text" name="env_value" placeholder="Value" />
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
