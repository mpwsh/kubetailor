const customDomainInput = document.getElementById("customDomain");
const toggleCustomDomainBtn = document.getElementById("toggleCustomDomain");
const enableCustomDomainHidden = document.getElementById("enableCustomDomain");

toggleCustomDomainBtn.addEventListener("click", () => {
  const isActive = toggleCustomDomainBtn.classList.toggle("active");
  customDomainInput.disabled = !isActive;
  enableCustomDomainHidden.value = isActive ? "1" : "0";
  toggleCustomDomainBtn.textContent = isActive ? "Disable" : "Enable";
});
