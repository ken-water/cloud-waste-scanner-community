(function () {
  var NETWORK_META = {
    x: {
      label: "X",
      longLabel: "Share on X",
      iconText: "X",
      color: "#111827"
    },
    linkedin: {
      label: "LinkedIn",
      longLabel: "Share on LinkedIn",
      iconText: "in",
      color: "#1e40af"
    },
    reddit: {
      label: "Reddit",
      longLabel: "Share on Reddit",
      iconText: "r",
      color: "#9a3412"
    },
    facebook: {
      label: "Facebook",
      longLabel: "Share on Facebook",
      iconText: "f",
      color: "#1d4ed8"
    },
    hackernews: {
      label: "Hacker News",
      longLabel: "Share on Hacker News",
      iconText: "HN",
      color: "#9a3412"
    }
  };

  var SERIES_META = [
    {
      id: "product-guide-series",
      title: "Product Guide Series",
      description: "Read the full usage track from first scan to governance-grade taxonomy and trends execution workflow.",
      hub: "/blog-series.html#product-guide-series",
      entries: [
        {
          path: "/blog/first-scan-in-15-minutes.html",
          label: "Part 1",
          title: "Finish the first scan in 15 minutes",
          summary: "Connect your first cloud account and get a reliable baseline quickly.",
          readTime: "5 min"
        },
        {
          path: "/blog/network-proxy-setup-restricted-environments.html",
          label: "Part 2",
          title: "Stabilize proxy and network routes",
          summary: "Keep scans, notifications, and updates stable in restricted environments.",
          readTime: "6 min"
        },
        {
          path: "/blog/scan-results-to-team-weekly-rhythm.html",
          label: "Part 3",
          title: "Turn scan results into weekly action",
          summary: "Use scan evidence in team meetings to drive ownership and execution.",
          readTime: "6 min"
        },
        {
          path: "/blog/notification-driven-weekly-cloud-governance.html",
          label: "Part 4",
          title: "Convert notification signals into completed actions",
          summary: "Use trigger strategy, API cadence, and ownership mapping to close execution loops.",
          readTime: "9 min"
        },
        {
          path: "/blog/cloud-governance-tools-taxonomy-trends-api.html",
          label: "Part 5",
          title: "Standardize taxonomy and trends for governance",
          summary: "Make weekly cloud cost optimization decisions comparable, auditable, and accountable.",
          readTime: "10 min"
        },
        {
          path: "/blog/cloud-governance-framework-weekly-operating-playbook.html",
          label: "Part 6",
          title: "Run a weekly operating playbook with current features",
          summary: "Use existing product capability to create stable weekly governance outcomes before complex expansion.",
          readTime: "10 min"
        },
        {
          path: "/blog/cloud-waste-scanner-user-guide-core-setup.html",
          label: "Part 7",
          title: "Finish the core setup in one sitting",
          summary: "Add the first account, test the route, and run the first usable scan without detours.",
          readTime: "9 min"
        },
        {
          path: "/blog/cloud-waste-scanner-user-guide-account-proxy-notification-scan-result.html",
          label: "Part 8",
          title: "Connect accounts, notifications, and scan-result handling",
          summary: "Route findings to the right owners and turn one scan into something the team can act on.",
          readTime: "11 min"
        },
        {
          path: "/blog/cloud-waste-scanner-user-guide-dashboard-monitor-governance-scan-result.html",
          label: "Part 9",
          title: "Read Dashboard, Scan Result, Governance, and Monitor in order",
          summary: "Use the right page at the right moment after each scan so new teammates can work without getting lost.",
          readTime: "9 min"
        }
      ]
    },
    {
      id: "interview-origins-series",
      title: "Interview Origins Series",
      description: "Follow the full architecture and operating-model interview track.",
      hub: "/blog-series.html#interview-origins-series",
      entries: [
        {
          path: "/blog/interview-origins-part1.html",
          label: "Part 1",
          title: "Why this product had to exist",
          summary: "Origin story and design intent from real operating pain.",
          readTime: "4 min"
        },
        {
          path: "/blog/interview-origins-part2-local-first.html",
          label: "Part 2",
          title: "Local-first as an architectural decision",
          summary: "Why credential custody and local execution were non-negotiable.",
          readTime: "5 min"
        },
        {
          path: "/blog/interview-origins-part3.html",
          label: "Part 3",
          title: "From scanner to decision support",
          summary: "How findings become actionable operating decisions.",
          readTime: "5 min"
        },
        {
          path: "/blog/interview-origins-part4-safe-automation.html",
          label: "Part 4",
          title: "Designing safe automation boundaries",
          summary: "Automation that stays auditable and predictable at scale.",
          readTime: "5 min"
        },
        {
          path: "/blog/interview-origins-part5-policy-simulation-edge-cases-reporting.html",
          label: "Part 5",
          title: "Policy simulation and edge cases",
          summary: "How reporting quality changes trust and adoption.",
          readTime: "6 min"
        },
        {
          path: "/blog/interview-origins-part6-operating-model-compounding-governance.html",
          label: "Part 6",
          title: "Operating model and compounding governance",
          summary: "How teams make savings repeatable across cycles.",
          readTime: "6 min"
        }
      ]
    },
    {
      id: "incident-series-series",
      title: "Incident Series",
      description: "Incident-driven stories about cloud bills, operator mistakes, and the trust boundaries that changed the outcome.",
      hub: "/blog-series.html#incident-series",
      entries: [
        {
          path: "/blog/boss-saw-the-cloud-bill-and-asked-if-i-was-secretly-mining.html",
          label: "Part 1",
          title: "The boss asked if Jack was secretly mining crypto",
          summary: "Jack had the bill, Rose wanted to help, and Jerry used CWS to turn panic into a clean local-first review flow.",
          readTime: "8 min"
        },
        {
          path: "/blog/march-rain-disappearing-coins.html",
          label: "Part 2",
          title: "March rain and the disappearing coins",
          summary: "Rose tracked a doubled bill, Jack found hidden leftovers, and a Telegram alert exposed idle waste across providers.",
          readTime: "8 min"
        }
      ]
    }
    ,
    {
      id: "industry-solutions-whitepaper-series",
      title: "Industry Solutions Whitepaper Series",
      description: "A practical governance whitepaper track from cloud debt baseline to procurement and rollout decision models.",
      hub: "/blog-series.html?series=industry-solutions-whitepaper-series#industry-solutions-whitepaper-series",
      entries: [
        {
          path: "/blog/industry-solutions-whitepaper-part1-invisible-cloud-debt-and-sovereign-governance.html",
          label: "Part 1",
          title: "Invisible Cloud Debt and Sovereign Governance",
          summary: "Establish the debt baseline and the custody-first operating principle.",
          readTime: "10 min"
        },
        {
          path: "/blog/industry-solutions-whitepaper-part2-regulated-environments-control-evidence-and-change-review.html",
          label: "Part 2",
          title: "Regulated Environments: Control Evidence and Change Review",
          summary: "Design review-ready evidence packets for security, finance, and engineering.",
          readTime: "10 min"
        },
        {
          path: "/blog/industry-solutions-whitepaper-part3-engineering-integration-ci-cd-and-runtime-guardrails.html",
          label: "Part 3",
          title: "Engineering Integration: CI/CD and Runtime Guardrails",
          summary: "Embed governance into delivery systems with practical gate patterns.",
          readTime: "7 min"
        },
        {
          path: "/blog/industry-solutions-whitepaper-part4-industry-playbooks-finance-saas-and-platform-teams.html",
          label: "Part 4",
          title: "Industry Playbooks: Finance, SaaS, and Platform Teams",
          summary: "Match governance cadence and controls to team operating model.",
          readTime: "9 min"
        },
        {
          path: "/blog/industry-solutions-whitepaper-part5-rollout-roadmap-governance-kpis-and-executive-reporting.html",
          label: "Part 5",
          title: "Rollout Roadmap, Governance KPIs, and Executive Reporting",
          summary: "Run a 30/60/90 implementation path with measurable governance outcomes.",
          readTime: "9 min"
        },
        {
          path: "/blog/industry-solutions-whitepaper-part6-security-and-compliance-mapping-for-local-first-governance.html",
          label: "Part 6",
          title: "Security and Compliance Mapping for Local-First Governance",
          summary: "Map controls to operational evidence and compliance-ready narratives.",
          readTime: "10 min"
        },
        {
          path: "/blog/industry-solutions-whitepaper-part7-procurement-and-selection-matrix-local-first-vs-saas.html",
          label: "Part 7",
          title: "Procurement and Selection Matrix: Local-First vs SaaS",
          summary: "Use a weighted decision model with custody and execution criteria.",
          readTime: "10 min"
        },
        {
          path: "/blog/industry-solutions-whitepaper-appendix-terms-metrics-templates-and-checklists.html",
          label: "Appendix",
          title: "Terms, Metrics, Templates, and Checklists",
          summary: "Apply shared definitions and templates to procurement and rollout handoff.",
          readTime: "8 min"
        }
      ]
    },
    {
      id: "whitepaper-series",
      title: "Whitepaper Series",
      description: "Security and technical whitepapers for architecture review, control validation, and rollout readiness.",
      hub: "/blog-series.html?series=whitepaper-series#whitepaper-series",
      entries: [
        { path: "/blog/security-whitepaper-local-first-part1-threat-model-and-trust-boundary.html", label: "Security Part 1", title: "Threat Model and Trust Boundary", summary: "Define threat actors and the local-first trust boundary.", readTime: "10 min" },
        { path: "/blog/security-whitepaper-local-first-part2-control-matrix-and-operational-safeguards.html", label: "Security Part 2", title: "Control Matrix and Operational Safeguards", summary: "Map architecture claims to practical controls.", readTime: "11 min" },
        { path: "/blog/security-whitepaper-local-first-part3-verification-checklist-and-incident-readiness.html", label: "Security Part 3", title: "Verification Checklist and Incident Readiness", summary: "Operational verification and incident response baseline.", readTime: "9 min" },
        { path: "/blog/security-whitepaper-local-first-part4-token-lifecycle-and-api-hardening.html", label: "Security Part 4", title: "Token Lifecycle and API Hardening", summary: "Token generation, rotation, and local API hardening controls.", readTime: "10 min" },
        { path: "/blog/security-whitepaper-local-first-part5-transport-integrity-and-operational-auditability.html", label: "Security Part 5", title: "Transport Integrity and Operational Auditability", summary: "Transport path controls, logs, and release-time security gates.", readTime: "10 min" },
        { path: "/blog/technical-whitepaper-local-first-part1-engine-architecture-and-component-boundaries.html", label: "Technical Part 1", title: "Engine Architecture and Component Boundaries", summary: "Runtime composition and execution boundaries.", readTime: "11 min" },
        { path: "/blog/technical-whitepaper-local-first-part2-findings-data-model-and-policy-evaluation.html", label: "Technical Part 2", title: "Findings Data Model and Policy Evaluation", summary: "Normalization model and deterministic policy flow.", readTime: "10 min" },
        { path: "/blog/technical-whitepaper-local-first-part3-performance-reliability-and-rollout-patterns.html", label: "Technical Part 3", title: "Performance, Reliability, and Rollout Patterns", summary: "Production rollout and reliability operations.", readTime: "9 min" },
        { path: "/blog/technical-whitepaper-local-first-part4-quality-gates-auditability-and-delivery-discipline.html", label: "Technical Part 4", title: "Quality Gates, Auditability, and Delivery Discipline", summary: "QA governance, auditability controls, and delivery gate evidence.", readTime: "12 min" },
        { path: "/blog/technical-whitepaper-local-first-part5-rust-tauri-runtime-tradeoffs-and-platform-roadmap.html", label: "Technical Part 5", title: "Rust + Tauri Runtime Tradeoffs and Platform Roadmap", summary: "Runtime stack decisions, platform tradeoffs, and roadmap constraints.", readTime: "10 min" }
      ]
    }

  ];

  var POPULAR_META = [
    {
      path: "/blog/deep-finops-anatomy.html",
      label: "Popular",
      title: "Inside a $50k Cloud Leak: What Shallow Scripts Miss",
      summary: "A full cloud-cost incident teardown with practical prevention steps.",
      readTime: "11 min"
    },
    {
      path: "/blog/cloud-waste-horror-stories.html",
      label: "Popular",
      title: "3 Expensive Cloud Incidents and What Prevented Them",
      summary: "Three costly production incidents and the controls that prevented recurrence.",
      readTime: "8 min"
    },
    {
      path: "/blog/v2-launch-global.html",
      label: "Popular",
      title: "v1.10 Adds 15-Provider Coverage Without 15 Different Tools",
      summary: "Why multi-provider coverage changed adoption speed in real teams.",
      readTime: "8 min"
    },
    {
      path: "/blog/local-first-finops.html",
      label: "Popular",
      title: "Why We Chose Local-First for Cloud Credentials",
      summary: "Credential custody, security boundaries, and why local-first mattered.",
      readTime: "6 min"
    },
    {
      path: "/blog/first-scan-in-15-minutes.html",
      label: "Popular",
      title: "First Effective Cloud Scan in 15 Minutes",
      summary: "A practical quick-start path to run the first scan and share results.",
      readTime: "7 min"
    }
  ];

  var DOWNLOAD_LATEST_URL = "https://dl.cloud-waste-scanner.com/api/download/latest";
  var PRICING_PAGE_URL = "/pricing.html";
  var HEADER_NAV_ORDER = ["home", "feedback", "enterprise", "pricing", "security", "blog", "faq", "docs"];
  var FOOTER_NAV_ORDER = ["privacy", "terms", "refunds", "pricing", "partners", "email", "playbooks"];

  function buildShareUrl(network, url, title) {
    var encodedUrl = encodeURIComponent(url);
    var encodedTitle = encodeURIComponent(title || "Cloud Waste Scanner");

    if (network === "x") {
      return "https://x.com/intent/tweet?url=" + encodedUrl + "&text=" + encodedTitle;
    }
    if (network === "linkedin") {
      return "https://www.linkedin.com/sharing/share-offsite/?url=" + encodedUrl;
    }
    if (network === "reddit") {
      return "https://www.reddit.com/submit?url=" + encodedUrl + "&title=" + encodedTitle;
    }
    if (network === "facebook") {
      return "https://www.facebook.com/sharer/sharer.php?u=" + encodedUrl;
    }
    if (network === "hackernews") {
      return "https://news.ycombinator.com/submitlink?u=" + encodedUrl + "&t=" + encodedTitle;
    }
    return "";
  }

  function currentShareUrl() {
    var href = window.location.href || "";
    return href.split("#")[0];
  }

  function ensureBlogShareStyles() {
    if (document.getElementById("cws-blog-share-style")) return;
    var style = document.createElement("style");
    style.id = "cws-blog-share-style";
    style.textContent = [
      "html{scroll-padding-top:var(--cws-anchor-offset,112px)}",
      "main section[id],main article section[id],main [id].scroll-target,h1[id],h2[id],h3[id],h4[id],h5[id],h6[id]{scroll-margin-top:var(--cws-anchor-offset,112px)}",
      ".cws-blog-share-bottom{margin:26px auto 10px;display:flex;flex-wrap:wrap;align-items:center;justify-content:center;gap:10px;padding:12px 14px;border:1px solid #cbd5e1;border-radius:14px;background:#f8fafc;box-shadow:0 8px 20px rgba(15,23,42,.06)}",
      ".cws-blog-share-label{font-size:12px;font-weight:800;letter-spacing:.08em;color:#475569;text-transform:uppercase;margin-right:2px}",
      ".cws-share-row{display:flex;flex-wrap:wrap;align-items:center;justify-content:center;gap:10px}",
      ".cws-share-row .cws-blog-share-label{margin-right:2px}",
      ".cws-share-btn{display:inline-flex;align-items:center;justify-content:center;gap:8px;padding:8px 12px;border-radius:10px;border:1px solid #cbd5e1;background:#fff;color:#334155;font-size:13px;font-weight:700;text-decoration:none;transition:all .15s ease;line-height:1}",
      ".cws-share-btn:hover{border-color:#64748b;color:#0f172a;box-shadow:0 4px 12px rgba(15,23,42,.12)}",
      ".cws-share-icon{display:inline-flex;align-items:center;justify-content:center;width:20px;height:20px;border-radius:9999px;color:#fff;font-size:11px;font-weight:800;flex-shrink:0}",
      ".cws-share-text{white-space:nowrap}",
      ".cws-blog-share-side{position:fixed;right:18px;top:42%;transform:translateY(-50%);display:flex;flex-direction:column;align-items:center;gap:8px;padding:10px 8px;border:1px solid #cbd5e1;border-radius:14px;background:#f8fafc;box-shadow:0 12px 24px rgba(15,23,42,.12);z-index:35}",
      ".cws-blog-share-side .cws-blog-share-label{margin-right:0;font-size:11px}",
      ".cws-blog-share-side .cws-share-btn{width:40px;height:40px;padding:0;gap:0;border-radius:10px}",
      ".cws-blog-share-side .cws-share-text{display:none}",
      ".cws-nav-pricing-link{display:inline-flex;align-items:center;justify-content:center;color:#c2410c;font-size:inherit;font-weight:700;line-height:inherit;text-decoration:none;transition:color .15s ease}",
      ".cws-nav-pricing-link:hover{color:#9a3412;text-decoration:underline}",
      ".cws-nav-pricing-link-mobile{display:inline-flex;align-items:center;justify-content:center;color:#c2410c;font-size:inherit;font-weight:700;line-height:inherit;text-decoration:none;transition:color .15s ease}",
      ".cws-nav-pricing-link-mobile:hover{color:#9a3412;text-decoration:underline}",
      ".cws-footer-pricing-link{color:#c2410c;font-weight:700;text-decoration:none}",
      ".cws-footer-pricing-link:hover{color:#9a3412}",
      ".cws-cta-actions{margin-top:14px;display:flex;flex-wrap:wrap;justify-content:center;gap:12px}",
      ".cws-cta-btn{display:inline-flex;align-items:center;justify-content:center;min-height:52px;padding:14px 30px;border-radius:14px;font-size:17px;font-weight:900;line-height:1.1;text-decoration:none;transition:all .18s ease;border:1px solid transparent;box-shadow:0 10px 20px rgba(15,23,42,.12)}",
      ".cws-cta-btn-download{background:linear-gradient(135deg,#f97316,#ea580c);border-color:#ea580c;color:#fff}",
      ".cws-cta-btn-download:hover{background:linear-gradient(135deg,#fb923c,#f97316);border-color:#f97316;color:#fff;transform:translateY(-1px)}",
      ".cws-cta-btn-pricing{background:linear-gradient(135deg,#8b5cf6,#7c3aed);border-color:#7c3aed;color:#fff}",
      ".cws-cta-btn-pricing:hover{background:linear-gradient(135deg,#a78bfa,#8b5cf6);border-color:#8b5cf6;color:#fff;transform:translateY(-1px)}",
      ".cws-series-hub-line{margin-top:12px;display:flex;flex-wrap:wrap;align-items:center;gap:10px;color:#475569;font-size:14px}",
      ".cws-series-hub-sep{color:#cbd5e1;font-weight:700}",
      ".cws-series-spotlight-link{display:inline-flex;align-items:center;justify-content:center;padding:6px 12px;border-radius:9999px;border:1px solid #c7d2fe;background:#eef2ff;color:#3730a3;font-size:12px;font-weight:800;line-height:1;text-decoration:none;transition:all .15s ease}",
      ".cws-series-spotlight-link:hover{background:#e0e7ff;border-color:#a5b4fc;color:#312e81}",
      "@media (max-width:1279px){.cws-blog-share-side{display:none}}"
    ].join("");
    document.head.appendChild(style);
  }

  function normalizeSourceLabel(value, fallback) {
    var input = (value || "").toLowerCase();
    var cleaned = input.replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "");
    var resolved = cleaned || (fallback || "checkout");
    if (resolved.length > 64) return resolved.slice(0, 64);
    return resolved;
  }

  function resolvePageCheckoutSource(plan, suffix) {
    var path = (window.location.pathname || "").toLowerCase();
    var normalizedPath = path
      .replace(/\.html$/, "")
      .replace(/^\/+|\/+$/g, "")
      .replace(/\//g, "_");
    if (!normalizedPath) normalizedPath = "home";
    var tail = suffix ? "_" + suffix : "";
    return normalizeSourceLabel(normalizedPath + "_checkout_" + plan + tail, "checkout");
  }

  function parseCheckoutHref(href) {
    if (!href) return null;
    var url;
    try {
      url = new URL(href, window.location.origin);
    } catch (_) {
      return null;
    }
    var pathname = (url.pathname || "").toLowerCase();
    var isLegacyCheckout = pathname.indexOf("/checkout.html") !== -1;
    var isDirectCheckout = pathname.indexOf("/api/paddle/checkout") !== -1;
    if (!isLegacyCheckout && !isDirectCheckout) return null;

    var plan = (url.searchParams.get("plan") || "").toLowerCase();
    if (plan !== "monthly" && plan !== "yearly" && plan !== "lifetime") return null;

    var source = normalizeSourceLabel(
      url.searchParams.get("source") || resolvePageCheckoutSource(plan, "link"),
      "checkout"
    );
    return {
      plan: plan,
      source: source,
      fallbackUrl:
        "/api/paddle/checkout?plan=" +
        encodeURIComponent(plan) +
        "&source=" +
        encodeURIComponent(source)
    };
  }

  function setCheckoutStatus(target, text, kind) {
    if (!target) return;
    target.textContent = text || "";
    target.classList.remove("error");
    target.classList.remove("success");
    if (kind) target.classList.add(kind);
  }

  function bindDirectCheckoutAnchor(anchor, options) {
    if (!anchor || anchor.getAttribute("data-cws-direct-checkout-bound") === "1") return;

    var plan = options && options.plan ? String(options.plan).toLowerCase() : "";
    if (plan !== "monthly" && plan !== "yearly" && plan !== "lifetime") return;

    var source = normalizeSourceLabel(
      options && options.source ? options.source : resolvePageCheckoutSource(plan, "link"),
      "checkout"
    );
    var fallbackUrl =
      (options && options.fallbackUrl) ||
      "/api/paddle/checkout?plan=" +
        encodeURIComponent(plan) +
        "&source=" +
        encodeURIComponent(source);
    var statusTarget = (options && options.statusTarget) || null;

    anchor.setAttribute("data-cws-direct-checkout-bound", "1");
    anchor.setAttribute("data-cws-plan", plan);
    anchor.setAttribute("data-cws-source", source);

    anchor.addEventListener("click", function (event) {
      event.preventDefault();

      if (anchor.getAttribute("data-state") === "loading") return;

      if (typeof window.fetch !== "function") {
        window.location.href = fallbackUrl;
        return;
      }

      anchor.setAttribute("data-state", "loading");
      setCheckoutStatus(statusTarget, "Connecting to secure checkout...", null);

      fetch("/api/paddle/checkout-link", {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          plan: plan,
          source: source
        })
      })
        .then(function (response) {
          return response
            .json()
            .catch(function () {
              return {};
            })
            .then(function (data) {
              return {
                ok: response.ok,
                data: data
              };
            });
        })
        .then(function (result) {
          var checkoutUrl = result && result.data ? result.data.checkout_url : "";
          if (!result.ok || !checkoutUrl) {
            throw new Error("checkout_link_failed");
          }

          if (typeof window.track_event === "function") {
            window.track_event("click_buy", {
              plan: plan,
              source: source,
              target: "paddle_direct_checkout"
            });
          }

          setCheckoutStatus(statusTarget, "Redirecting to checkout...", "success");
          window.location.href = checkoutUrl;
        })
        .catch(function () {
          setCheckoutStatus(statusTarget, "Checkout API busy, opening secure fallback...", "error");
          window.location.href = fallbackUrl;
        })
        .finally(function () {
          anchor.removeAttribute("data-state");
        });
    });
  }

  function upgradeCheckoutLinksDirect() {
    var anchors = document.querySelectorAll(
      'a[href*="checkout.html?plan="],a[href*="/api/paddle/checkout?plan="],a[href*="api/paddle/checkout?plan="]'
    );
    anchors.forEach(function (anchor) {
      if (anchor.classList.contains("js-direct-checkout")) return;
      var parsed = parseCheckoutHref(anchor.getAttribute("href") || "");
      if (!parsed) return;
      bindDirectCheckoutAnchor(anchor, {
        plan: parsed.plan,
        source: parsed.source,
        fallbackUrl: parsed.fallbackUrl
      });
    });
  }

  function createShareAnchor(network, variant) {
    var meta = NETWORK_META[network];
    if (!meta) return null;
    var a = document.createElement("a");
    a.setAttribute("data-share-network", network);
    a.setAttribute("href", "#");
    a.setAttribute("target", "_blank");
    a.setAttribute("rel", "noopener noreferrer nofollow");
    a.className = "cws-share-btn";
    a.setAttribute("aria-label", meta.longLabel);
    a.innerHTML =
      '<span class="cws-share-icon" style="background:' +
      meta.color +
      ';" aria-hidden="true">' +
      meta.iconText +
      "</span>" +
      '<span class="cws-share-text">' +
      (variant === "sticky" ? meta.label : meta.label) +
      "</span>";
    return a;
  }

  function decorateShareAnchor(anchor) {
    if (!anchor) return;
    var network = (anchor.getAttribute("data-share-network") || "").toLowerCase();
    var meta = NETWORK_META[network];
    if (!meta) return;

    anchor.classList.add("cws-share-btn");
    anchor.setAttribute("aria-label", meta.longLabel);
    if (!anchor.querySelector(".cws-share-icon")) {
      anchor.innerHTML =
        '<span class="cws-share-icon" style="background:' +
        meta.color +
        ';" aria-hidden="true">' +
        meta.iconText +
        "</span>" +
        '<span class="cws-share-text">' +
        meta.label +
        "</span>";
    }
  }

  function decorateShareContainer(container) {
    if (!container) return;
    container.classList.add("cws-share-row");
    if (!container.querySelector(".cws-blog-share-label")) {
      var label = document.createElement("span");
      label.className = "cws-blog-share-label";
      label.textContent = "Share";
      container.insertBefore(label, container.firstChild);
    }
    var anchors = container.querySelectorAll("a[data-share-network]");
    anchors.forEach(function (anchor) {
      decorateShareAnchor(anchor);
    });
  }

  function normalizeInlineShareRows() {
    var rows = document.querySelectorAll(".cws-social-share");
    rows.forEach(function (row) {
      if (!row || row.closest("footer")) return;
      var previous = row.previousElementSibling;
      if (
        previous &&
        previous.tagName === "P" &&
        normalizeLinkLabel(previous.textContent) === "share"
      ) {
        previous.remove();
      }
      decorateShareContainer(row);
    });
  }

  function stripFooterShareUi() {
    var rows = document.querySelectorAll("footer .cws-social-share");
    rows.forEach(function (row) {
      row.remove();
    });
  }

  function hasPricingAnchor(container) {
    if (!container) return false;
    return !!container.querySelector(
      'a[href="/pricing.html"], a[href="pricing.html"], a[href="../pricing.html"], a[href*="/pricing.html"]'
    );
  }

  function normalizeLinkLabel(value) {
    return String(value || "")
      .replace(/\s+/g, " ")
      .trim()
      .toLowerCase();
  }

  function reorderNavLinks(container, preferredOrder) {
    if (!container || !preferredOrder || !preferredOrder.length) return;
    var anchors = Array.prototype.slice.call(container.querySelectorAll("a"));
    if (!anchors.length) return;

    var used = {};
    var ordered = [];

    preferredOrder.forEach(function (label) {
      for (var i = 0; i < anchors.length; i += 1) {
        var anchor = anchors[i];
        if (used[i]) continue;
        var anchorLabel = normalizeLinkLabel(anchor.textContent);
        if (anchorLabel === label) {
          used[i] = true;
          ordered.push(anchor);
          break;
        }
      }
    });

    for (var j = 0; j < anchors.length; j += 1) {
      if (used[j]) continue;
      var candidate = anchors[j];
      var candidateLabel = normalizeLinkLabel(candidate.textContent);
      // Drop duplicate entries for canonical nav labels.
      if (preferredOrder.indexOf(candidateLabel) !== -1) continue;
      ordered.push(candidate);
    }

    anchors.forEach(function (anchor) {
      if (anchor.parentNode === container) {
        container.removeChild(anchor);
      }
    });

    ordered.forEach(function (anchor) {
      container.appendChild(anchor);
    });
  }

  function decorateExistingPricingAnchors(container, isMobile, isFooter) {
    if (!container) return;
    var anchors = container.querySelectorAll(
      'a[href="/pricing.html"], a[href="pricing.html"], a[href="../pricing.html"], a[href*="/pricing.html"]'
    );
    anchors.forEach(function (anchor) {
      if (isFooter) {
        anchor.classList.add("cws-footer-pricing-link");
      } else if (isMobile) {
        anchor.classList.add("cws-nav-pricing-link-mobile");
      } else {
        anchor.classList.add("cws-nav-pricing-link");
      }
      anchor.setAttribute("data-cws-pricing-link", "1");
    });
  }

  function createPricingNavAnchor(isMobile) {
    var link = document.createElement("a");
    link.setAttribute("href", PRICING_PAGE_URL);
    link.textContent = "Pricing";
    link.className = isMobile ? "cws-nav-pricing-link-mobile" : "cws-nav-pricing-link";
    link.setAttribute("data-cws-pricing-link", "1");
    return link;
  }

  function injectPricingNavLink() {
    ensureBlogShareStyles();

    var desktopRows = [];
    try {
      desktopRows = document.querySelectorAll("header nav .hidden.md\\:flex");
    } catch (_) {
      desktopRows = [];
    }
    if (!desktopRows || !desktopRows.length) {
      desktopRows = document.querySelectorAll("header nav .hidden");
    }
    desktopRows.forEach(function (row) {
      if (hasPricingAnchor(row)) {
        decorateExistingPricingAnchors(row, false, false);
      } else {
        var anchor = createPricingNavAnchor(false);
        var docsLink = row.querySelector('a[href*="documentation.html"]');
        if (docsLink) {
          docsLink.insertAdjacentElement("beforebegin", anchor);
        } else {
          row.appendChild(anchor);
        }
      }
      reorderNavLinks(row, HEADER_NAV_ORDER);
    });

    var mobileRows = document.querySelectorAll("header nav .cws-mobile-nav-row .flex");
    mobileRows.forEach(function (row) {
      if (hasPricingAnchor(row)) {
        decorateExistingPricingAnchors(row, true, false);
      } else {
        var anchor = createPricingNavAnchor(true);
        var docsLink = row.querySelector('a[href*="documentation.html"]');
        if (docsLink) {
          docsLink.insertAdjacentElement("beforebegin", anchor);
        } else {
          row.appendChild(anchor);
        }
      }
      reorderNavLinks(row, HEADER_NAV_ORDER);
    });
  }

  function injectPricingFooterLink() {
    ensureBlogShareStyles();

    var footerRows = document.querySelectorAll("footer .cws-footer-links");
    footerRows.forEach(function (row) {
      if (hasPricingAnchor(row)) {
        decorateExistingPricingAnchors(row, false, true);
      } else {
        var link = document.createElement("a");
        link.setAttribute("href", PRICING_PAGE_URL);
        link.textContent = "Pricing";
        link.className = "cws-footer-pricing-link";
        link.setAttribute("data-cws-pricing-link", "1");

        var partnerLink = row.querySelector('a[href*="channel-register.html"]');
        if (partnerLink) {
          partnerLink.insertAdjacentElement("beforebegin", link);
        } else {
          row.appendChild(link);
        }
      }
      reorderNavLinks(row, FOOTER_NAV_ORDER);
    });
  }

  function shouldInjectStickyShareRail(path) {
    var currentPath = (path || "").toLowerCase();
    if (document.querySelector("main .cws-social-share, main .cws-blog-share-bottom")) {
      return true;
    }
    return (
      currentPath.indexOf("/blog/") !== -1 ||
      currentPath.indexOf("/playbooks/") !== -1 ||
      currentPath.indexOf("/faq.html") !== -1 ||
      currentPath.indexOf("/documentation.html") !== -1 ||
      currentPath.indexOf("/roadmap.html") !== -1 ||
      currentPath.indexOf("/api-token-guide.html") !== -1 ||
      currentPath.indexOf("/solutions/") !== -1
    );
  }

  function injectStickyShareRail() {
    var path = (window.location.pathname || "").toLowerCase();
    if (!shouldInjectStickyShareRail(path)) return;
    if (document.querySelector(".cws-blog-share-side")) return;

    ensureBlogShareStyles();

    var side = document.createElement("div");
    side.className = "cws-blog-share-side";
    side.setAttribute("aria-label", "Share");
    side.innerHTML = '<span class="cws-blog-share-label">Share</span>';

    ["x", "linkedin", "reddit", "facebook", "hackernews"].forEach(function (network) {
      var anchor = createShareAnchor(network, "sticky");
      if (anchor) side.appendChild(anchor);
    });

    document.body.appendChild(side);
    decorateShareContainer(side);
  }

  function injectBlogShareUi() {
    var path = (window.location.pathname || "").toLowerCase();
    if (path.indexOf("/blog/") === -1) return;

    var article = document.querySelector("main article");
    if (!article) return;
    var proseRoot = resolveProseRoot(article);

    ensureBlogShareStyles();

    if (!document.querySelector(".cws-blog-share-bottom")) {
      var seriesNav = article.querySelector("[data-cws-series-nav]");
      var bottom = document.createElement("div");
      bottom.className = "cws-blog-share-bottom";
      bottom.setAttribute("aria-label", "Share");
      bottom.innerHTML = '<span class="cws-blog-share-label">Share</span>';
      ["x", "linkedin", "reddit", "facebook", "hackernews"].forEach(function (network) {
        var anchor = createShareAnchor(network, "bottom");
        if (anchor) bottom.appendChild(anchor);
      });

      if (seriesNav && seriesNav.parentNode === article) {
        seriesNav.insertAdjacentElement("afterend", bottom);
      } else {
        insertBeforeCtaOrAfterProse(article, proseRoot, bottom);
      }
    }

    decorateShareContainer(document.querySelector(".cws-blog-share-bottom"));
  }

  function resolveSeriesContext(path) {
    for (var i = 0; i < SERIES_META.length; i += 1) {
      var series = SERIES_META[i];
      for (var j = 0; j < series.entries.length; j += 1) {
        if (series.entries[j].path === path) {
          return {
            series: series,
            index: j
          };
        }
      }
    }
    return null;
  }

  function normalizeSeriesEntry(entry) {
    return {
      path: entry.path,
      label: entry.label || "",
      title: entry.title || entry.label || "Article",
      summary: entry.summary || "Read the next practical step in this track.",
      readTime: entry.readTime || "5 min"
    };
  }

  function createSeriesTextLink(href, label, emphasized) {
    var a = document.createElement("a");
    a.setAttribute("href", href);
    a.className = emphasized
      ? "text-indigo-700 hover:text-indigo-900 hover:underline font-semibold transition-colors"
      : "text-slate-700 hover:text-slate-900 hover:underline font-medium transition-colors";
    a.textContent = label;
    return a;
  }

  function createSeriesSpotlightLink(href, label) {
    var a = document.createElement("a");
    a.setAttribute("href", href);
    a.className = "cws-series-spotlight-link";
    a.textContent = label;
    return a;
  }

  function createReadingCard(entry, eyebrow, accent) {
    var normalized = normalizeSeriesEntry(entry);
    var a = document.createElement("a");
    a.setAttribute("href", normalized.path);
    a.className = accent
      ? "group block rounded-xl border border-indigo-200 bg-indigo-50/70 p-4 hover:bg-indigo-50 hover:border-indigo-300 transition-colors"
      : "group block rounded-xl border border-slate-200 bg-white p-4 hover:border-slate-300 hover:bg-slate-50 transition-colors";

    var top = document.createElement("p");
    top.className = accent
      ? "text-xs font-bold uppercase tracking-wide text-indigo-600"
      : "text-xs font-bold uppercase tracking-wide text-slate-500";
    top.textContent = eyebrow;

    var title = document.createElement("h4");
    title.className = accent
      ? "mt-1 text-lg font-bold text-slate-900 group-hover:text-indigo-900 transition-colors"
      : "mt-1 text-base font-bold text-slate-900";
    title.textContent = normalized.title;

    var summary = document.createElement("p");
    summary.className = "mt-2 text-sm text-slate-600 leading-6";
    summary.textContent = normalized.summary;

    var meta = document.createElement("p");
    meta.className = "mt-3 text-xs font-semibold text-slate-500";
    meta.textContent = normalized.label + " · " + normalized.readTime;

    a.appendChild(top);
    a.appendChild(title);
    a.appendChild(summary);
    a.appendChild(meta);
    return a;
  }

  function resolveProseRoot(article) {
    if (!article) return null;
    var prose = article.querySelector(".prose");
    if (prose) return prose;
    if (article.classList && article.classList.contains("prose")) return article;
    return null;
  }

  function stablePathHash(input) {
    var value = input || "";
    var hash = 0;
    for (var i = 0; i < value.length; i += 1) {
      hash = (hash * 31 + value.charCodeAt(i)) >>> 0;
    }
    return hash;
  }

  function getSeriesById(seriesId) {
    for (var i = 0; i < SERIES_META.length; i += 1) {
      if (SERIES_META[i].id === seriesId) return SERIES_META[i];
    }
    return null;
  }

  function pushUniqueRecommendation(picks, seen, entry) {
    if (!entry) return false;
    var normalized = normalizeSeriesEntry(entry);
    if (!normalized.path || seen[normalized.path]) return false;
    seen[normalized.path] = true;
    picks.push(normalized);
    return true;
  }

  function pickDeterministicEntry(entries, seen, seed) {
    var candidates = [];
    var source = entries || [];
    for (var i = 0; i < source.length; i += 1) {
      var normalized = normalizeSeriesEntry(source[i]);
      if (!normalized.path || seen[normalized.path]) continue;
      candidates.push(normalized);
    }
    if (!candidates.length) return null;
    var index = stablePathHash(seed || "") % candidates.length;
    return candidates[index];
  }

  function buildFallbackPool(currentPath, seen) {
    var pool = [];
    var localSeen = {};

    function pushPool(entry) {
      var normalized = normalizeSeriesEntry(entry);
      if (!normalized.path || normalized.path === currentPath || seen[normalized.path] || localSeen[normalized.path]) return;
      localSeen[normalized.path] = true;
      pool.push(normalized);
    }

    for (var p = 0; p < POPULAR_META.length; p += 1) pushPool(POPULAR_META[p]);
    for (var i = 0; i < SERIES_META.length; i += 1) {
      var entries = SERIES_META[i].entries || [];
      for (var j = 0; j < entries.length; j += 1) pushPool(entries[j]);
    }
    return pool;
  }

  function findArticleCtaBlock(article) {
    if (!article || !article.children) return null;
    for (var i = article.children.length - 1; i >= 0; i -= 1) {
      var child = article.children[i];
      if (!child || child.nodeType !== 1) continue;

      var className = child.className || "";
      if (typeof className === "string" && className.indexOf("download-zone") !== -1) {
        return child;
      }

      var hasDownloadLink = child.querySelector(
        'a[href*="dl.cloud-waste-scanner.com/api/download/latest"], a[href*="#download"]'
      );
      if (hasDownloadLink) return child;
    }
    return null;
  }

  function normalizeCtaDownloadLinks(ctaBlock) {
    if (!ctaBlock) return;
    var links = ctaBlock.querySelectorAll("a[href]");
    links.forEach(function (link) {
      var href = (link.getAttribute("href") || "").toLowerCase();
      if (!href) return;
      if (href.indexOf("#download") === -1) return;
      if (href.indexOf("dl.cloud-waste-scanner.com/api/download/latest") !== -1) return;
      link.setAttribute("href", DOWNLOAD_LATEST_URL);
    });
  }

  function resolveCtaContentRoot(ctaBlock) {
    if (!ctaBlock) return null;
    return (
      ctaBlock.querySelector(".cws-cta-inner") ||
      ctaBlock.querySelector(".download-zone-content") ||
      ctaBlock
    );
  }

  function isCtaActionLink(link) {
    if (!link) return false;
    var href = (link.getAttribute("href") || "").toLowerCase();
    var text = (link.textContent || "").toLowerCase();
    var className = (link.className || "").toLowerCase();

    if (href.indexOf("dl.cloud-waste-scanner.com/api/download/latest") !== -1) return true;
    if (href.indexOf("#download") !== -1) return true;
    if (href.indexOf("pricing.html") !== -1 || href.indexOf("/pricing") !== -1) return true;
    if (href.indexOf("/api/paddle/checkout") !== -1 || href.indexOf("/checkout.html") !== -1) return true;
    if (href.indexOf("/feedback") !== -1) return true;
    if (className.indexOf("download-btn") !== -1 || className.indexOf("download-zone-btn") !== -1) return true;
    if (className.indexOf("btn") !== -1) return true;
    if (text.indexOf("download") !== -1 || text.indexOf("trial") !== -1 || text.indexOf("pricing") !== -1) return true;
    if (text.indexOf("plan") !== -1 || text.indexOf("feedback") !== -1) return true;
    return false;
  }

  function normalizeBlogCtaButtons() {
    var path = (window.location.pathname || "").toLowerCase();
    if (path.indexOf("/blog/") === -1) return;

    var article = document.querySelector("main article");
    if (!article) return;
    var cta = findArticleCtaBlock(article);
    if (!cta) return;
    var ctaContent = resolveCtaContentRoot(cta);
    if (!ctaContent) return;

    ensureBlogShareStyles();
    normalizeCtaDownloadLinks(cta);

    var existingActions = cta.querySelector("[data-cws-cta-actions='1']");
    if (existingActions) existingActions.remove();

    var legacyPricingBlocks = cta.querySelectorAll(".cws-cta-pricing,[data-cws-cta-pricing='1']");
    legacyPricingBlocks.forEach(function (node) {
      node.remove();
    });

    var links = cta.querySelectorAll("a[href]");
    links.forEach(function (link) {
      if (!isCtaActionLink(link)) return;
      link.style.display = "none";
      link.setAttribute("aria-hidden", "true");
      link.setAttribute("data-cws-legacy-cta-link", "1");
    });

    var section = document.createElement("div");
    section.className = "cws-cta-actions";
    section.setAttribute("data-cws-cta-actions", "1");

    var download = document.createElement("a");
    download.setAttribute("href", DOWNLOAD_LATEST_URL);
    download.className = "cws-cta-btn cws-cta-btn-primary";
    download.textContent = "Download Trial";
    section.appendChild(download);

    var pricing = document.createElement("a");
    pricing.setAttribute("href", PRICING_PAGE_URL);
    pricing.className = "cws-cta-btn cws-cta-btn-secondary";
    pricing.textContent = "View Pricing";
    section.appendChild(pricing);

    ctaContent.appendChild(section);
  }

  function insertBeforeCtaOrAfterProse(article, proseRoot, element) {
    var cta = findArticleCtaBlock(article);
    if (cta && cta.parentNode === article) {
      cta.insertAdjacentElement("beforebegin", element);
      return;
    }
    if (proseRoot && proseRoot !== article && proseRoot.parentNode === article) {
      proseRoot.insertAdjacentElement("afterend", element);
      return;
    }
    article.appendChild(element);
  }

  function buildGlobalRecommendations(currentPath, limit) {
    var picks = [];
    var seen = {};
    var productSeries = getSeriesById("product-guide-series");
    var interviewSeries = getSeriesById("interview-origins-series");
    var storySeries = getSeriesById("incident-series-series");
    var max = typeof limit === "number" && limit > 0 ? limit : 4;
    seen[currentPath] = true;

    pushUniqueRecommendation(
      picks,
      seen,
      pickDeterministicEntry(POPULAR_META, seen, (currentPath || "") + "|popular-featured")
    );
    if (productSeries) {
      pushUniqueRecommendation(
        picks,
        seen,
        pickDeterministicEntry(productSeries.entries || [], seen, (currentPath || "") + "|product-series")
      );
    }
    if (interviewSeries) {
      pushUniqueRecommendation(
        picks,
        seen,
        pickDeterministicEntry(interviewSeries.entries || [], seen, (currentPath || "") + "|interview-series")
      );
    }
    if (storySeries) {
      pushUniqueRecommendation(
        picks,
        seen,
        pickDeterministicEntry(storySeries.entries || [], seen, (currentPath || "") + "|incident-series")
      );
    }
    pushUniqueRecommendation(
      picks,
      seen,
      pickDeterministicEntry(POPULAR_META, seen, (currentPath || "") + "|popular-secondary")
    );

    var fallback = buildFallbackPool(currentPath, seen);
    if (fallback.length) {
      var start = stablePathHash((currentPath || "") + "|fallback") % fallback.length;
      for (var f = 0; f < fallback.length && picks.length < max; f += 1) {
        pushUniqueRecommendation(picks, seen, fallback[(start + f) % fallback.length]);
      }
    }

    return picks;
  }

  function buildSeriesRecommendations(series, currentIndex, currentPath, featuredPath) {
    var picks = [];
    var seen = {};
    var productSeries = getSeriesById("product-guide-series");
    var interviewSeries = getSeriesById("interview-origins-series");
    var storySeries = getSeriesById("incident-series-series");
    var max = 4;

    seen[currentPath] = true;
    if (featuredPath) seen[featuredPath] = true;

    pushUniqueRecommendation(
      picks,
      seen,
      pickDeterministicEntry(POPULAR_META, seen, (currentPath || "") + "|series-popular")
    );

    if (series.id === "product-guide-series" && interviewSeries) {
      pushUniqueRecommendation(
        picks,
        seen,
        pickDeterministicEntry(interviewSeries.entries || [], seen, (currentPath || "") + "|series-interview")
      );
      if (storySeries) {
        pushUniqueRecommendation(
          picks,
          seen,
          pickDeterministicEntry(storySeries.entries || [], seen, (currentPath || "") + "|series-story")
        );
      }
    } else if (series.id === "interview-origins-series" && productSeries) {
      pushUniqueRecommendation(
        picks,
        seen,
        pickDeterministicEntry(productSeries.entries || [], seen, (currentPath || "") + "|series-product")
      );
      if (storySeries) {
        pushUniqueRecommendation(
          picks,
          seen,
          pickDeterministicEntry(storySeries.entries || [], seen, (currentPath || "") + "|series-story")
        );
      }
    } else if (series.id === "incident-series-series") {
      if (productSeries) {
        pushUniqueRecommendation(
          picks,
          seen,
          pickDeterministicEntry(productSeries.entries || [], seen, (currentPath || "") + "|series-product")
        );
      }
      if (interviewSeries) {
        pushUniqueRecommendation(
          picks,
          seen,
          pickDeterministicEntry(interviewSeries.entries || [], seen, (currentPath || "") + "|series-interview")
        );
      }
    } else {
      if (productSeries) {
        pushUniqueRecommendation(
          picks,
          seen,
          pickDeterministicEntry(productSeries.entries || [], seen, (currentPath || "") + "|series-product-generic")
        );
      }
      if (interviewSeries) {
        pushUniqueRecommendation(
          picks,
          seen,
          pickDeterministicEntry(interviewSeries.entries || [], seen, (currentPath || "") + "|series-interview-generic")
        );
      }
    }

    pushUniqueRecommendation(
      picks,
      seen,
      pickDeterministicEntry(series.entries || [], seen, (currentPath || "") + "|series-self")
    );

    var fallback = buildFallbackPool(currentPath, seen);
    if (fallback.length) {
      var start = stablePathHash((currentPath || "") + "|series-fallback") % fallback.length;
      for (var i = 0; i < fallback.length && picks.length < max; i += 1) {
        pushUniqueRecommendation(picks, seen, fallback[(start + i) % fallback.length]);
      }
    }

    return picks.slice(0, max);
  }

  function injectSeriesNavigator() {
    var path = (window.location.pathname || "").toLowerCase();
    if (path.indexOf("/blog/") === -1) return;

    var context = resolveSeriesContext(path);
    if (document.querySelector("[data-cws-series-nav]")) return;

    var article = document.querySelector("main article");
    if (!article) return;
    var proseRoot = resolveProseRoot(article);
    if (!proseRoot) return;

    if (!context) {
      var genericPicks = buildGlobalRecommendations(path, 4);
      if (!genericPicks.length) return;

      var featured = genericPicks[0];
      var recommendations = genericPicks.slice(1, 4);

      var genericSection = document.createElement("section");
      genericSection.setAttribute("data-cws-series-nav", "1");
      genericSection.className = "mt-14 mb-12 border-t border-slate-200 pt-8";

      var genericTitle = document.createElement("h3");
      genericTitle.className = "text-2xl font-bold text-slate-900";
      genericTitle.textContent = "Recommended Next Reads";

      var genericDesc = document.createElement("p");
      genericDesc.className = "mt-2 text-base text-slate-600";
      genericDesc.textContent = "One featured guide and three practical follow-ups to keep execution moving.";

      var genericFeaturedHeading = document.createElement("h4");
      genericFeaturedHeading.className = "mt-7 text-sm font-bold uppercase tracking-wide text-slate-500";
      genericFeaturedHeading.textContent = "Featured Recommendation";

      var genericFeaturedWrap = document.createElement("div");
      genericFeaturedWrap.className = "mt-3";
      genericFeaturedWrap.appendChild(createReadingCard(featured, "Editor Pick", true));

      var genericHubLine = document.createElement("p");
      genericHubLine.className = "mt-3 text-sm text-slate-600";
      genericHubLine.appendChild(createSeriesTextLink("/blog.html", "Browse all blog posts", true));

      var genericRecHeading = document.createElement("h4");
      genericRecHeading.className = "mt-8 text-sm font-bold uppercase tracking-wide text-slate-500";
      genericRecHeading.textContent = "You May Also Like";

      var genericRecGrid = document.createElement("div");
      genericRecGrid.className = "mt-3 grid gap-3 md:grid-cols-3";
      for (var g = 0; g < recommendations.length; g += 1) {
        genericRecGrid.appendChild(createReadingCard(recommendations[g], "Recommended", false));
      }

      genericSection.appendChild(genericTitle);
      genericSection.appendChild(genericDesc);
      genericSection.appendChild(genericFeaturedHeading);
      genericSection.appendChild(genericFeaturedWrap);
      genericSection.appendChild(genericHubLine);
      genericSection.appendChild(genericRecHeading);
      genericSection.appendChild(genericRecGrid);

      insertBeforeCtaOrAfterProse(article, proseRoot, genericSection);
      return;
    }

    var series = context.series;
    var currentIndex = context.index;
    var prevEntry = currentIndex > 0 ? series.entries[currentIndex - 1] : null;
    var nextEntry = currentIndex < series.entries.length - 1 ? series.entries[currentIndex + 1] : null;
    var featuredEntry = nextEntry || prevEntry || null;
    var recommendations = buildSeriesRecommendations(
      series,
      currentIndex,
      path,
      featuredEntry ? featuredEntry.path : null
    );

    var section = document.createElement("section");
    section.setAttribute("data-cws-series-nav", "1");
    section.className = "mt-14 mb-12 border-t border-slate-200 pt-8";

    var title = document.createElement("h3");
    title.className = "text-2xl font-bold text-slate-900";
    title.textContent = series.title;

    var desc = document.createElement("p");
    desc.className = "mt-2 text-base text-slate-600";
    desc.textContent = series.description;

    var continueHeading = document.createElement("h4");
    continueHeading.className = "mt-7 text-sm font-bold uppercase tracking-wide text-slate-500";
    continueHeading.textContent = nextEntry || prevEntry ? "Continue Reading" : "Recommended Next Read";

    var continueWrap = document.createElement("div");
    continueWrap.className = "mt-3";
    if (nextEntry) {
      continueWrap.appendChild(createReadingCard(nextEntry, "Next in this series", true));
    } else if (prevEntry) {
      continueWrap.appendChild(createReadingCard(prevEntry, "Revisit from this series", true));
    } else if (recommendations.length) {
      continueWrap.appendChild(createReadingCard(recommendations[0], "Featured Recommendation", true));
      recommendations = recommendations.slice(1);
    }
    if (recommendations.length < 3) {
      var extraSeen = {};
      extraSeen[path] = true;
      if (featuredEntry) extraSeen[featuredEntry.path] = true;
      for (var ex = 0; ex < recommendations.length; ex += 1) {
        if (recommendations[ex] && recommendations[ex].path) extraSeen[recommendations[ex].path] = true;
      }
      var extraPool = buildFallbackPool(path, extraSeen);
      for (var ep = 0; ep < extraPool.length && recommendations.length < 3; ep += 1) {
        recommendations.push(extraPool[ep]);
      }
    }

    var seriesHubLine = document.createElement("p");
    seriesHubLine.className = "cws-series-hub-line";
    seriesHubLine.appendChild(createSeriesTextLink("/blog.html", "Browse all blog posts", false));
    var seriesHubSep = document.createElement("span");
    seriesHubSep.className = "cws-series-hub-sep";
    seriesHubSep.textContent = "|";
    seriesHubLine.appendChild(seriesHubSep);
    seriesHubLine.appendChild(
      createSeriesSpotlightLink(
        series.hub,
        series.id === "incident-series-series" ? "Read this story series" : "Read this series"
      )
    );
    var storySeries = getSeriesById("incident-series-series");
    if (storySeries && storySeries.id !== series.id) {
      var relatedSep = document.createElement("span");
      relatedSep.className = "cws-series-hub-sep";
      relatedSep.textContent = "|";
      seriesHubLine.appendChild(relatedSep);
      seriesHubLine.appendChild(createSeriesSpotlightLink(storySeries.hub, "Related Incident Series"));
    }

    var recHeading = document.createElement("h4");
    recHeading.className = "mt-8 text-sm font-bold uppercase tracking-wide text-slate-500";
    recHeading.textContent = "You May Also Like";

    var recGrid = document.createElement("div");
    recGrid.className = "mt-3 grid gap-3 md:grid-cols-3";
    for (var r = 0; r < recommendations.length && r < 3; r += 1) {
      var rec = recommendations[r];
      if (!rec) continue;
      recGrid.appendChild(createReadingCard(rec, "Recommended", false));
    }

    var parts = document.createElement("div");
    parts.className = "mt-8 flex flex-wrap items-center gap-x-2 gap-y-1 text-sm leading-6";
    var seriesLabel = document.createElement("span");
    seriesLabel.className = "mr-2 text-xs font-bold uppercase tracking-wide text-slate-500";
    seriesLabel.textContent = "Series Index";
    parts.appendChild(seriesLabel);

    for (var i = 0; i < series.entries.length; i += 1) {
      if (i > 0) {
        var dot = document.createElement("span");
        dot.className = "text-slate-300";
        dot.textContent = "•";
        parts.appendChild(dot);
      }
      var entry = series.entries[i];
      if (i === currentIndex) {
        var current = document.createElement("span");
        current.className = "font-semibold text-slate-900";
        current.textContent = entry.label + " (Current)";
        parts.appendChild(current);
      } else {
        var part = createSeriesTextLink(entry.path, entry.label, false);
        parts.appendChild(part);
      }
    }

    section.appendChild(title);
    section.appendChild(desc);
    section.appendChild(continueHeading);
    section.appendChild(continueWrap);
    section.appendChild(seriesHubLine);
    section.appendChild(recHeading);
    section.appendChild(recGrid);
    section.appendChild(parts);

    insertBeforeCtaOrAfterProse(article, proseRoot, section);
  }

  function hydrateShareLinks() {
    var pageUrl = currentShareUrl();
    var pageTitle = document.title || "Cloud Waste Scanner";
    var links = document.querySelectorAll("a[data-share-network]");

    links.forEach(function (link) {
      var network = (link.getAttribute("data-share-network") || "").toLowerCase();
      var shareHref = buildShareUrl(network, pageUrl, pageTitle);
      if (shareHref) {
        link.setAttribute("href", shareHref);
      }
    });
  }

  function resolveAnchorTarget(hash) {
    if (!hash || hash === "#") return null;
    var id = hash.charAt(0) === "#" ? hash.slice(1) : hash;
    if (!id) return null;
    var target = document.getElementById(id);
    if (!target) return null;
    return target;
  }

  function computeAnchorScrollOffset() {
    var nav = document.querySelector("header nav");
    var navHeight = nav ? Math.round(nav.getBoundingClientRect().height) : 0;
    var base = navHeight + 24;
    var min = window.matchMedia("(max-width: 767px)").matches ? 124 : 104;
    return Math.max(min, base);
  }

  function syncAnchorScrollOffset() {
    var offset = computeAnchorScrollOffset();
    document.documentElement.style.setProperty("--cws-anchor-offset", offset + "px");
  }

  function getAnchorScrollOffset() {
    return computeAnchorScrollOffset();
  }

  function scrollToAnchorHash(hash, updateHistory) {
    var target = resolveAnchorTarget(hash);
    if (!target) return false;
    var top = Math.max(0, window.scrollY + target.getBoundingClientRect().top - getAnchorScrollOffset());
    window.scrollTo({
      top: top,
      behavior: "smooth"
    });
    if (updateHistory) {
      if (window.history && typeof window.history.pushState === "function") {
        window.history.pushState(null, "", hash);
      } else {
        window.location.hash = hash;
      }
    }
    return true;
  }

  function bindPrecisionAnchorNavigation() {
    document.addEventListener("click", function (event) {
      var link = event.target && event.target.closest ? event.target.closest('a[href^="#"]') : null;
      if (!link) return;
      var href = link.getAttribute("href") || "";
      if (!href || href === "#") return;
      if (link.hasAttribute("data-share-network")) return;
      if (link.getAttribute("target") === "_blank") return;
      if (!resolveAnchorTarget(href)) return;
      event.preventDefault();
      scrollToAnchorHash(href, true);
    });

    if (window.location.hash) {
      window.setTimeout(function () {
        scrollToAnchorHash(window.location.hash, false);
      }, 60);
    }
  }

  function setupShareUi() {
    stripFooterShareUi();
    injectSeriesNavigator();
    normalizeInlineShareRows();
    injectStickyShareRail();
    injectBlogShareUi();
    normalizeBlogCtaButtons();
    upgradeCheckoutLinksDirect();
    hydrateShareLinks();
    syncAnchorScrollOffset();
    bindPrecisionAnchorNavigation();
  }

  var anchorOffsetBound = false;
  function bindAnchorOffsetSync() {
    if (anchorOffsetBound) return;
    anchorOffsetBound = true;
    window.addEventListener("resize", syncAnchorScrollOffset, { passive: true });
    window.addEventListener("orientationchange", syncAnchorScrollOffset, { passive: true });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () {
      setupShareUi();
      bindAnchorOffsetSync();
    });
  } else {
    setupShareUi();
    bindAnchorOffsetSync();
  }
})();
