document.addEventListener("DOMContentLoaded", () => {
  const list = document.querySelector(".article-list");
  const controls = document.querySelector(".blog-controls");
  if (!list || !controls) return;

  const cards = Array.from(list.querySelectorAll(".card"));
  const buttons = Array.from(controls.querySelectorAll("[data-blog-filter]"));
  const search = controls.querySelector("[data-blog-search]");
  const empty = document.querySelector(".blog-empty");
  const state = { category: "all", query: "" };
  const backgroundCategories = new Set(["Whitepapers", "Industry Intelligence"]);

  cards.forEach((card) => {
    const category = card.querySelector(".eyebrow")?.textContent.trim() || "";
    card.dataset.category = category;
  });

  const apply = () => {
    const query = state.query.trim().toLowerCase();
    let visible = 0;

    cards.forEach((card) => {
      const categoryMatch = state.category === "all"
        ? !backgroundCategories.has(card.dataset.category)
        : card.dataset.category === state.category;
      const queryMatch = !query || card.textContent.toLowerCase().includes(query);
      const show = categoryMatch && queryMatch;
      card.hidden = !show;
      if (show) visible += 1;
    });

    buttons.forEach((button) => {
      const active = button.dataset.blogFilter === state.category;
      button.classList.toggle("is-active", active);
      button.setAttribute("aria-pressed", String(active));
    });

    if (empty) empty.hidden = visible !== 0;
  };

  buttons.forEach((button) => {
    button.addEventListener("click", () => {
      state.category = button.dataset.blogFilter || "all";
      apply();
    });
  });

  search?.addEventListener("input", () => {
    state.query = search.value;
    apply();
  });
});
