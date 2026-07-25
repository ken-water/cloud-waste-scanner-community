document.addEventListener("DOMContentLoaded", () => {
  const list = document.querySelector(".solution-list");
  if (!list) return;

  const cards = Array.from(list.querySelectorAll(".solution-card"));
  const controls = document.querySelector(".solution-controls");
  const empty = document.querySelector(".solution-empty");
  if (!cards.length || !controls) return;

  const providerButtons = Array.from(controls.querySelectorAll("[data-provider]"));
  const typeButtons = Array.from(controls.querySelectorAll("[data-type]"));
  const search = controls.querySelector("[data-solution-search]");

  const typeFor = (text) => {
    const value = text.toLowerCase();
    if (/(vm|ec2|instance|droplet|app service|compute)/.test(value)) return "compute";
    if (/(disk|volume|snapshot|bucket|s3|oss|blob|r2|storage|ami)/.test(value)) return "storage";
    if (/(ip|load balancer|nat|dns|network interface|eip)/.test(value)) return "network";
    if (/(rds|sql|database)/.test(value)) return "database";
    return "other";
  };

  cards.forEach((card) => {
    const provider = card.querySelector(".eyebrow")?.textContent.trim().toLowerCase() || "all";
    const text = card.textContent || "";
    card.dataset.provider = provider;
    card.dataset.type = typeFor(text);
  });

  const state = {
    provider: "all",
    type: "all",
    query: "",
  };

  const updateButtons = (buttons, key) => {
    buttons.forEach((button) => {
      const active = button.dataset[key] === state[key];
      button.classList.toggle("is-active", active);
      button.setAttribute("aria-pressed", String(active));
    });
  };

  const apply = () => {
    let visible = 0;
    const query = state.query.trim().toLowerCase();

    cards.forEach((card) => {
      const providerMatch = state.provider === "all" || card.dataset.provider === state.provider;
      const typeMatch = state.type === "all" || card.dataset.type === state.type;
      const queryMatch = !query || card.textContent.toLowerCase().includes(query);
      const show = providerMatch && typeMatch && queryMatch;
      card.hidden = !show;
      if (show) visible += 1;
    });

    if (empty) empty.hidden = visible !== 0;
    updateButtons(providerButtons, "provider");
    updateButtons(typeButtons, "type");
  };

  providerButtons.forEach((button) => {
    button.addEventListener("click", () => {
      state.provider = button.dataset.provider;
      apply();
    });
  });

  typeButtons.forEach((button) => {
    button.addEventListener("click", () => {
      state.type = button.dataset.type;
      apply();
    });
  });

  search?.addEventListener("input", () => {
    state.query = search.value;
    apply();
  });

  apply();
});
