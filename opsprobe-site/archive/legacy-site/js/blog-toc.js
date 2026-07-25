document.addEventListener("DOMContentLoaded", () => {
  const article = document.querySelector("[data-blog-body]");
  const toc = document.querySelector("[data-blog-toc]");
  if (!article || !toc) return;

  const headings = Array.from(article.querySelectorAll("h2, h3")).filter((heading) => {
    const text = (heading.textContent || "").trim();
    return text.length > 0;
  });

  if (!headings.length) {
    const tocCard = toc.closest("[data-blog-toc-card]");
    if (tocCard) tocCard.classList.add("hidden");
    return;
  }

  const slugify = (value) =>
    value
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9\s-]/g, "")
      .replace(/\s+/g, "-")
      .replace(/-+/g, "-");

  headings.forEach((heading, index) => {
    if (!heading.id) {
      heading.id = slugify(heading.textContent || `section-${index + 1}`) || `section-${index + 1}`;
    }
    const link = document.createElement("a");
    link.href = `#${heading.id}`;
    link.className = "blog-sidebar-link";
    link.dataset.blogTocLink = heading.id;
    link.dataset.level = heading.tagName.toLowerCase();
    link.textContent = heading.textContent || "";
    toc.appendChild(link);
  });

  const links = Array.from(toc.querySelectorAll("[data-blog-toc-link]"));
  const observer = new IntersectionObserver(
    (entries) => {
      const visible = entries
        .filter((entry) => entry.isIntersecting)
        .sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top)[0];
      if (!visible) return;
      const activeId = visible.target.id;
      links.forEach((link) => {
        link.classList.toggle("is-active", link.dataset.blogTocLink === activeId);
      });
    },
    {
      rootMargin: "-20% 0px -65% 0px",
      threshold: [0, 1],
    }
  );

  headings.forEach((heading) => observer.observe(heading));
});
