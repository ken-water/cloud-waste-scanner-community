(function () {
  const networks = [
    {
      id: "x",
      label: "X",
      title: "Share on X",
      color: "#0f172a",
      url: (pageUrl, pageTitle) =>
        `https://twitter.com/intent/tweet?url=${pageUrl}&text=${pageTitle}`,
    },
    {
      id: "linkedin",
      label: "in",
      title: "Share on LinkedIn",
      color: "#0a66c2",
      url: (pageUrl) =>
        `https://www.linkedin.com/sharing/share-offsite/?url=${pageUrl}`,
    },
    {
      id: "reddit",
      label: "R",
      title: "Share on Reddit",
      color: "#ff4500",
      url: (pageUrl, pageTitle) =>
        `https://www.reddit.com/submit?url=${pageUrl}&title=${pageTitle}`,
    },
    {
      id: "facebook",
      label: "f",
      title: "Share on Facebook",
      color: "#1877f2",
      url: (pageUrl) =>
        `https://www.facebook.com/sharer/sharer.php?u=${pageUrl}`,
    },
    {
      id: "hackernews",
      label: "HN",
      title: "Share on Hacker News",
      color: "#ff6600",
      url: (pageUrl, pageTitle) =>
        `https://news.ycombinator.com/submitlink?u=${pageUrl}&t=${pageTitle}`,
    },
  ];

  function ensureStyles() {
    if (document.getElementById("cws-blog-share-style")) return;
    const style = document.createElement("style");
    style.id = "cws-blog-share-style";
    style.textContent = `
      .cws-blog-share-bottom {
        margin: 30px 0 12px;
        display: flex;
        flex-wrap: wrap;
        align-items: center;
        justify-content: center;
        gap: 12px;
        padding: 16px 18px;
        border: 1px solid rgba(216, 225, 238, .9);
        border-radius: 18px;
        background:
          linear-gradient(150deg, rgba(255, 255, 255, .94), rgba(255, 255, 255, .84)),
          radial-gradient(circle at 10% 0%, rgba(73, 198, 183, .12), transparent 12rem);
        box-shadow: 0 12px 32px rgba(8, 18, 32, .07);
      }
      .cws-blog-share-label {
        font-size: 12px;
        font-weight: 800;
        letter-spacing: .14em;
        color: #087568;
        text-transform: uppercase;
      }
      .cws-share-btn {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        gap: 8px;
        min-height: 40px;
        padding: 0 14px;
        border: 1px solid var(--line, #d8e1ee);
        border-radius: 999px;
        background: rgba(255, 255, 255, .86);
        color: var(--ink, #0c1724);
        font-size: 13px;
        font-weight: 800;
        line-height: 1;
        text-decoration: none;
        box-shadow: 0 6px 18px rgba(8, 18, 32, .05);
        transition: border-color .15s ease, box-shadow .15s ease, transform .15s ease, background .15s ease;
      }
      .cws-share-btn:hover {
        border-color: rgba(73, 198, 183, .58);
        background: #fff;
        color: var(--ink, #0c1724);
        box-shadow: 0 10px 24px rgba(8, 18, 32, .08);
        transform: translateY(-1px);
      }
      .cws-share-icon {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 20px;
        height: 20px;
        border-radius: 999px;
        background: var(--deep, #07111f) !important;
        color: #fff;
        font-size: 10px;
        font-weight: 800;
        flex: 0 0 auto;
      }
      .cws-share-text {
        white-space: nowrap;
      }
      .cws-blog-share-side {
        position: fixed;
        right: max(18px, calc((100vw - 1160px) / 2 - 58px));
        top: 42%;
        transform: translateY(-50%);
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 8px;
        padding: 10px 8px;
        border: 1px solid rgba(216, 225, 238, .9);
        border-radius: 16px;
        background: rgba(255, 255, 255, .94);
        box-shadow: 0 12px 28px rgba(8, 18, 32, .09);
        z-index: 35;
      }
      .cws-blog-share-side .cws-blog-share-label {
        color: #087568;
        font-size: 11px;
        writing-mode: vertical-rl;
        transform: rotate(180deg);
      }
      .cws-blog-share-side .cws-share-btn {
        width: 40px;
        height: 40px;
        padding: 0;
        border-radius: 999px;
      }
      .cws-blog-share-side .cws-share-text {
        display: none;
      }
      @media (max-width: 1279px) {
        .cws-blog-share-side {
          display: none;
        }
      }
    `;
    document.head.appendChild(style);
  }

  function canonicalUrl() {
    const canonical = document.querySelector('link[rel="canonical"]');
    return canonical ? canonical.href : window.location.href.split("#")[0];
  }

  function pageTitle() {
    const ogTitle = document.querySelector('meta[property="og:title"]');
    return ogTitle?.content || document.title || "Cloud Waste Scanner";
  }

  function createButton(network, compact) {
    const url = encodeURIComponent(canonicalUrl());
    const title = encodeURIComponent(pageTitle());
    const button = document.createElement("a");
    button.className = "cws-share-btn";
    button.href = network.url(url, title);
    button.target = "_blank";
    button.rel = "noopener noreferrer";
    button.setAttribute("aria-label", network.title);
    button.title = network.title;
    button.innerHTML = `
      <span class="cws-share-icon" style="background:${network.color}">${network.label}</span>
      <span class="cws-share-text">${compact ? network.title.replace("Share on ", "") : network.title}</span>
    `;
    return button;
  }

  function buildShareBox(className, compact) {
    const box = document.createElement("div");
    box.className = className;
    box.setAttribute("aria-label", "Share");
    const label = document.createElement("span");
    label.className = "cws-blog-share-label";
    label.textContent = "Share";
    box.appendChild(label);
    networks.forEach((network) => box.appendChild(createButton(network, compact)));
    return box;
  }

  function inject() {
    if (!window.location.pathname.toLowerCase().includes("/blog/")) return;
    const article = document.querySelector("main article");
    if (!article) return;
    ensureStyles();

    if (!document.querySelector(".cws-blog-share-side")) {
      document.body.appendChild(buildShareBox("cws-blog-share-side", true));
    }

    if (!document.querySelector(".cws-blog-share-bottom")) {
      const cta = article.querySelector(".article-cta");
      const bottom = buildShareBox("cws-blog-share-bottom", true);
      if (cta) cta.insertAdjacentElement("beforebegin", bottom);
      else article.appendChild(bottom);
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", inject);
  } else {
    inject();
  }
})();
