document.addEventListener("DOMContentLoaded", () => {
  if (!window.location.pathname.toLowerCase().includes("/blog/")) return;

  const article = document.querySelector("main article.article");
  if (!article || article.querySelector(".article-brief")) return;

  const title = article.querySelector("h1");
  const lead = title?.nextElementSibling?.tagName === "P" ? title.nextElementSibling : null;
  const meta = article.querySelector(".meta-line");
  const articleBody = article.querySelector(".article-body");
  const headings = Array.from(articleBody?.querySelectorAll("h2") || [])
    .map((heading) => heading.textContent.trim())
    .filter(Boolean)
    .slice(0, 3);

  if (!title || !lead || !meta || headings.length === 0) return;

  const brief = document.createElement("section");
  brief.className = "article-brief";
  brief.setAttribute("aria-label", "Article summary");
  brief.innerHTML = `
    <div>
      <span class="article-brief-label">Quick read</span>
      <p>${lead.textContent.trim()}</p>
    </div>
    <div>
      <span class="article-brief-label">What to look for</span>
      <ul>${headings.map((heading) => `<li>${heading}</li>`).join("")}</ul>
    </div>
  `;

  meta.insertAdjacentElement("afterend", brief);

  const cta = article.querySelector(".article-cta");
  if (cta && !article.querySelector(".article-next-step")) {
    const nextStep = document.createElement("section");
    nextStep.className = "article-next-step";
    nextStep.innerHTML = `
      <div>
        <span class="article-brief-label">Next step</span>
        <h2>Turn the article into a local review.</h2>
        <p>Use the desktop app to scan one account, export the evidence, and compare the findings with the operating pattern described here.</p>
      </div>
      <a class="btn" href="../solutions.html">Browse related playbooks</a>
    `;
    cta.insertAdjacentElement("beforebegin", nextStep);
  }
});
