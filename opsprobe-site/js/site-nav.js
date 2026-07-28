document.addEventListener("DOMContentLoaded", () => {
  const navInner = document.querySelector(".nav-inner");
  const navLinks = document.querySelector(".nav-links");

  if (!navInner || !navLinks || navInner.querySelector(".nav-menu-toggle")) return;

  const currentPath = window.location.pathname.replace(/\/index\.html$/, "/");
  const primaryLinks = [
    { href: "/", label: "Home", matches: (path) => path === "/" },
    { href: "/help", label: "Guide", matches: (path) => path === "/help" || path === "/help.html" },
    { href: "/solutions", label: "Solutions", matches: (path) => path === "/solutions" || path === "/solutions.html" || path.startsWith("/solutions/") },
    { href: "/blog", label: "Blog", matches: (path) => path === "/blog" || path === "/blog.html" || path.startsWith("/blog/") || path === "/blog-series" || path === "/blog-series.html" },
    { href: "/pricing", label: "Pricing", matches: (path) => path === "/pricing" || path === "/pricing.html" },
  ];

  navLinks.innerHTML = primaryLinks.map(({ href, label, matches }) => {
    const current = matches(currentPath) ? ' aria-current="page"' : "";
    return `<a href="${href}"${current}>${label}</a>`;
  }).join("");

  const footer = document.querySelector(".footer .footer-grid");
  if (footer) {
    const brand = footer.querySelector(".brand")?.outerHTML
      || '<a class="brand" href="/"><img src="/assets/product-icon.svg" alt="Cloud Waste Scanner"><span>Cloud Waste Scanner</span></a>';

    footer.innerHTML = `
      <div class="footer-brand">${brand}<p>A local desktop app for finding cloud waste before it becomes another billing cycle.</p></div>
      <div><h3>Start</h3><div class="footer-links"><a href="/download/">Download</a><a href="/pricing">Pricing</a><a href="/help">First-scan guide</a></div></div>
      <div><h3>Explore</h3><div class="footer-links"><a href="/solutions">Solutions</a><a href="/blog">Blog</a><a href="/roadmap">Roadmap</a></div></div>
      <div><h3>Support</h3><div class="footer-links"><a href="/help">Help center</a><a href="/feedback">Send feedback</a><a href="/security">Security</a></div></div>
      <div><h3>Legal</h3><div class="footer-links"><a href="/license">License</a><a href="/privacy">Privacy</a><a href="/terms">Terms</a></div></div>
    `;
  }

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
