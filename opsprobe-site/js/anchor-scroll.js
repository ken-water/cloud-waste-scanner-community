(() => {
  const nav = document.querySelector("[data-anchor-nav]");
  if (!nav) return;

  const offset = Number(nav.dataset.anchorOffset || 96);

  const scrollToHash = (hash, behavior = "smooth") => {
    if (!hash || hash === "#") return;
    const target = document.getElementById(hash.slice(1));
    if (!target) return;
    const top = target.getBoundingClientRect().top + window.scrollY - offset;
    window.scrollTo({ top, behavior });
  };

  nav.querySelectorAll('a[href^="#"]').forEach((link) => {
    link.addEventListener("click", (event) => {
      event.preventDefault();
      const hash = link.getAttribute("href");
      window.history.pushState(null, "", hash);
      scrollToHash(hash);
    });
  });

  if (window.location.hash) {
    window.requestAnimationFrame(() => scrollToHash(window.location.hash, "auto"));
  }

  document.querySelectorAll("[data-code-tabs]").forEach((tabs) => {
    const buttons = Array.from(tabs.querySelectorAll("[data-tab-target]"));
    const panels = Array.from(tabs.querySelectorAll("[data-tab-panel]"));

    buttons.forEach((button) => {
      button.addEventListener("click", () => {
        const target = button.dataset.tabTarget;
        buttons.forEach((item) => {
          const active = item === button;
          item.classList.toggle("is-active", active);
          item.setAttribute("aria-selected", String(active));
        });
        panels.forEach((panel) => {
          panel.hidden = panel.dataset.tabPanel !== target;
        });
      });
    });
  });
})();
