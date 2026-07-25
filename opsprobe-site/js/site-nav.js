document.addEventListener("DOMContentLoaded", () => {
  const navInner = document.querySelector(".nav-inner");
  const navLinks = document.querySelector(".nav-links");

  if (!navInner || !navLinks || navInner.querySelector(".nav-menu-toggle")) return;

  const toggle = document.createElement("button");
  const navId = navLinks.id || "primary-navigation";

  navLinks.id = navId;
  toggle.className = "nav-menu-toggle";
  toggle.type = "button";
  toggle.setAttribute("aria-controls", navId);
  toggle.setAttribute("aria-expanded", "false");
  toggle.innerHTML = '<span>Menu</span><i aria-hidden="true"></i>';

  navInner.insertBefore(toggle, navLinks);

  const setOpen = (open) => {
    document.body.classList.toggle("nav-open", open);
    toggle.setAttribute("aria-expanded", String(open));
  };

  toggle.addEventListener("click", () => {
    setOpen(!document.body.classList.contains("nav-open"));
  });

  navLinks.addEventListener("click", (event) => {
    if (event.target.closest("a")) setOpen(false);
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") setOpen(false);
  });

  document.addEventListener("click", (event) => {
    if (!document.body.classList.contains("nav-open")) return;
    if (event.target.closest(".nav-inner")) return;
    setOpen(false);
  });
});
