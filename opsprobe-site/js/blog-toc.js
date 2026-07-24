(() => {
  const body = document.querySelector("[data-article-body], [data-page-toc-body]");
  const toc = document.querySelector("[data-blog-toc], [data-page-toc]");
  const card = document.querySelector("[data-blog-toc-card], [data-page-toc-card]");
  const offset = Number(body?.dataset.tocOffset || 96);

  if (!body || !toc || !card) return;

  const headings = Array.from(body.querySelectorAll("h2, h3")).filter((heading) =>
    heading.textContent.trim()
  );

  if (!headings.length) {
    card.hidden = true;
    return;
  }

  const slugCounts = new Map();
  const slugify = (value) => {
    const base = value
      .toLowerCase()
      .replace(/&[a-z0-9#]+;/g, "")
      .replace(/[^a-z0-9\u4e00-\u9fa5]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 80) || "section";
    const count = slugCounts.get(base) || 0;
    slugCounts.set(base, count + 1);
    return count ? `${base}-${count + 1}` : base;
  };

  headings.forEach((heading) => {
    if (!heading.id) heading.id = slugify(heading.textContent.trim());
    heading.style.scrollMarginTop = `${offset}px`;
    const link = document.createElement("a");
    link.href = `#${heading.id}`;
    link.className = heading.tagName === "H3" ? "toc-link toc-link-sub" : "toc-link";
    link.textContent = heading.textContent.trim();
    link.addEventListener("click", (event) => {
      event.preventDefault();
      const target = document.getElementById(heading.id);
      if (!target) return;
      const top = target.getBoundingClientRect().top + window.scrollY - offset;
      window.history.pushState(null, "", `#${heading.id}`);
      window.scrollTo({ top, behavior: "smooth" });
    });
    toc.appendChild(link);
  });

  if (window.location.hash) {
    const target = document.getElementById(window.location.hash.slice(1));
    if (target) {
      window.requestAnimationFrame(() => {
        const top = target.getBoundingClientRect().top + window.scrollY - offset;
        window.scrollTo({ top, behavior: "auto" });
      });
    }
  }
})();
