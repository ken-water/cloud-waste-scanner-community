const element = (id) => document.getElementById(id);
const charts = new Map();
const chartDefinitions = new Map();

async function request(path, options = {}) {
  const response = await fetch(`/api/admin/${path}`, {
    credentials: "same-origin",
    headers: { "Content-Type": "application/json", ...(options.headers || {}) },
    ...options,
  });
  const body = await response.json().catch(() => ({}));
  if (!response.ok) {
    const error = new Error(body.message || "The request failed. Try again.");
    error.status = response.status;
    throw error;
  }
  return body;
}

function escapeHtml(value) {
  return String(value ?? "").replace(/[&<>"']/g, (character) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  })[character]);
}

function formatNumber(value) {
  return Number(value || 0).toLocaleString("en-US");
}

function formatPercent(value) {
  return `${(Number(value || 0) * 100).toFixed(1)}%`;
}

function formatDuration(value) {
  const seconds = Math.max(0, Math.round(Number(value || 0)));
  if (seconds < 60) return `${seconds}s`;
  return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
}

function formatTime(value) {
  const numeric = Number(value || 0);
  if (!numeric) return "Unknown";
  return new Date(numeric * 1000).toLocaleString("en-US", { hour12: false });
}

function empty(message) {
  return `<p class="empty">${escapeHtml(message)}</p>`;
}

function table(rows, columns) {
  if (!rows.length) return empty("No data for this period.");
  return `<table><thead><tr>${columns.map(([label]) => `<th>${escapeHtml(label)}</th>`).join("")}</tr></thead><tbody>${rows.map((row) => `<tr>${columns.map(([, key, formatter, className]) => `<td class="${className || ""}">${escapeHtml(formatter ? formatter(row[key], row) : row[key])}</td>`).join("")}</tr>`).join("")}</tbody></table>`;
}

function renderMetrics(target, items) {
  element(target).innerHTML = items.map(([label, value]) => `<article class="metric"><span>${escapeHtml(label)}</span><strong>${escapeHtml(value)}</strong></article>`).join("");
}

function reportingWindow() {
  const selectedDays = Number(element("days").value);
  const endDate = Math.floor(Date.now() / 1000);
  return {
    start_date: endDate - selectedDays * 86400,
    end_date: endDate,
    lite: false,
    include_search_interest: false,
    exclude_automated: true,
    exclude_proxy_backfill: true,
    exclude_nonhuman_backfill: true,
  };
}

function renderLineChart(target, labels, series) {
  const container = element(target);
  chartDefinitions.set(target, { labels, series });
  charts.get(target)?.destroy();
  charts.delete(target);
  const normalized = series.map((item) => ({ ...item, values: (item.values || []).map(Number) }));
  if (!labels?.length || normalized.every((item) => !item.values.some((value) => value > 0))) {
    container.innerHTML = empty("No human activity in this period.");
    return;
  }
  const description = normalized.map((item) => `${item.label}: ${item.values.reduce((sum, value) => sum + value, 0)}`).join("; ");
  if (typeof Chart === "undefined") {
    container.innerHTML = empty("The traffic chart could not be loaded.");
    return;
  }
  if (container.offsetParent === null) {
    container.innerHTML = "";
    return;
  }

  container.innerHTML = '<canvas role="img"></canvas>';
  const canvas = container.querySelector("canvas");
  canvas.setAttribute("aria-label", description);
  canvas.textContent = description;
  const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const chart = new Chart(canvas, {
    type: "line",
    data: {
      labels: labels.map((label) => String(label || "")),
      datasets: normalized.map((item) => ({
        label: item.label,
        data: item.values,
        borderColor: item.color,
        backgroundColor: item.fill,
        borderWidth: 2.5,
        pointRadius: labels.length > 45 ? 0 : 2.5,
        pointHoverRadius: 5,
        pointBackgroundColor: item.color,
        tension: 0.3,
        fill: false,
      })),
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      animation: reduceMotion ? false : { duration: 250 },
      interaction: { mode: "index", intersect: false },
      plugins: {
        legend: { display: false },
        tooltip: {
          backgroundColor: "#173f36",
          padding: 12,
          displayColors: true,
        },
      },
      scales: {
        x: {
          grid: { display: false },
          ticks: { color: "#63706d", maxTicksLimit: 7, maxRotation: 0 },
        },
        y: {
          beginAtZero: true,
          suggestedMax: 1,
          grid: { color: "#e8eeec" },
          ticks: { color: "#63706d", precision: 0, padding: 8 },
        },
      },
    },
  });
  charts.set(target, chart);
}

function renderRankList(target, rows) {
  const safeRows = Array.isArray(rows) ? rows.slice(0, 8) : [];
  if (!safeRows.length) {
    element(target).innerHTML = empty("No data for this period.");
    return;
  }
  const max = Math.max(...safeRows.map((row) => Number(row.count || 0)), 1);
  element(target).innerHTML = safeRows.map((row) => `<div class="rank-row"><span title="${escapeHtml(row.name || "Other")}">${escapeHtml(row.name || "Other")}</span><strong>${formatNumber(row.count)}</strong><div class="rank-track"><i style="width:${Math.max(3, Number(row.count || 0) / max * 100)}%"></i></div></div>`).join("");
}

function parseMeta(raw) {
  if (!raw) return {};
  if (typeof raw === "object") return raw;
  try { return JSON.parse(raw); } catch { return {}; }
}

function renderActivity(rows) {
  const visible = (rows || []).slice(0, 12).map((row) => {
    const meta = parseMeta(row.meta);
    return {
      date: row.date,
      event: row.event === "page_view" ? "Page view" : row.event,
      path: meta.url || meta.path || "/",
      source: meta.referrer || meta.referer || meta.ref || "Direct",
    };
  });
  element("overview-activity").innerHTML = table(visible, [
    ["Time", "date", formatTime],
    ["Event", "event"],
    ["Page", "path"],
    ["Source", "source"],
  ]);
}

function renderContent(content) {
  const engagement = new Map((content.page_engagement?.rows || []).map((row) => [row.page, row]));
  const rows = (content.pages || []).map((row) => ({ ...row, ...(engagement.get(row.page) || {}) }));
  renderMetrics("content-metrics", [["Human page views", formatNumber(content.overview?.pageviews)]]);
  element("content-pages").innerHTML = table(rows, [
    ["Page", "page"],
    ["Views", "hits", formatNumber, "numeric"],
    ["Sessions", "sessions", (value) => value == null ? "-" : formatNumber(value), "numeric"],
    ["Bounce rate", "bounce_rate", (value) => value == null ? "-" : formatPercent(value), "numeric"],
    ["Avg. duration", "avg_duration", (value) => value == null ? "-" : formatDuration(value), "numeric"],
  ]);
  element("overview-pages").innerHTML = table(rows.slice(0, 8), [
    ["Page", "page"],
    ["Views", "hits", formatNumber, "numeric"],
  ]);
}

function renderFunnel(funnel) {
  const steps = [
    ["Homepage views", funnel.view_home],
    ["Download actions", funnel.click_download],
    ["Pricing interest", funnel.view_pricing],
    ["Purchases", funnel.purchase],
  ];
  const first = Math.max(Number(steps[0][1] || 0), 1);
  element("conversion-funnel").innerHTML = steps.map(([label, value], index) => {
    const rate = index === 0 ? "Entry point" : `${(Number(value || 0) / first * 100).toFixed(1)}% of homepage views`;
    return `<div class="funnel-step"><span>${escapeHtml(label)}</span><strong>${formatNumber(value)}</strong><small>${escapeHtml(rate)}</small></div>`;
  }).join("");
}

function renderGovernance(scorecard) {
  const runs = Number(scorecard.scan_runs || 0);
  if (!runs) {
    element("governance-content").innerHTML = `<section class="governance-empty"><div><h2>No scan activity in this period</h2><p>Governance metrics appear after a Cloud Waste Scanner client completes a scan. Website traffic remains available in the other sections.</p></div></section>`;
    return;
  }
  element("governance-content").innerHTML = `<section class="metrics metrics-four" aria-label="Governance metrics">${[
    ["Scan runs", scorecard.scan_runs],
    ["Findings", scorecard.findings],
    ["Active machines", scorecard.active_scanning_machines],
    ["Identified savings", `$${formatNumber(scorecard.identified_savings)}`],
  ].map(([label, value]) => `<article class="metric"><span>${escapeHtml(label)}</span><strong>${escapeHtml(value)}</strong></article>`).join("")}</section>`;
}

function renderDashboard(dashboard, content, logs) {
  const overview = dashboard.overview || {};
  renderMetrics("overview-metrics", [
    ["Page views", formatNumber(overview.pageviews)],
    ["Visitors", formatNumber(overview.visitors)],
    ["Sessions", formatNumber(overview.sessions)],
    ["Downloads", formatNumber(overview.downloads)],
    ["Orders", formatNumber(overview.orders)],
  ]);
  renderMetrics("traffic-metrics", [
    ["Visitors", formatNumber(overview.visitors)],
    ["Sessions", formatNumber(overview.sessions)],
    ["Bounce rate", formatPercent(overview.bounce_rate)],
    ["Avg. duration", formatDuration(overview.avg_duration)],
  ]);

  const daily = dashboard.chart || {};
  renderLineChart("overview-trend", daily.labels || [], [
    { label: "Page views", values: daily.views || daily.pageviews || [], color: "#0f7664", fill: "rgba(15, 118, 100, .12)" },
    { label: "Visitors", values: daily.visitors || [], color: "#b96f0b", fill: "rgba(185, 111, 11, .12)" },
  ]);
  const hourly = dashboard.chart_24h || {};
  renderLineChart("traffic-hourly", hourly.labels || [], [
    { label: "Page views", values: hourly.views || [], color: "#0f7664", fill: "rgba(15, 118, 100, .12)" },
    { label: "Sessions", values: hourly.sessions || [], color: "#2563a6", fill: "rgba(37, 99, 166, .12)" },
  ]);

  renderRankList("regions", dashboard.demographics?.region);
  renderRankList("browsers", dashboard.demographics?.browser);
  renderRankList("operating-systems", dashboard.demographics?.os);
  renderContent(content || {});
  renderActivity(logs?.logs || []);
  renderFunnel(dashboard.funnel || {});
  renderGovernance(dashboard.governance_scorecard || {});
  element("freshness").textContent = `Updated ${new Date().toLocaleString("en-US", { hour12: false })} · Human traffic only`;
}

async function loadDashboard() {
  const error = element("dashboard-error");
  const refresh = element("refresh");
  error.hidden = true;
  refresh.disabled = true;
  refresh.textContent = "Refreshing...";
  const body = JSON.stringify(reportingWindow());
  try {
    const [dashboard, content, logs] = await Promise.all([
      request("dashboard", { method: "POST", body }),
      request("content", { method: "POST", body }),
      request("logs", { method: "POST", body }),
    ]);
    renderDashboard(dashboard, content, logs);
  } catch (requestError) {
    if (requestError.status === 401) return showLogin();
    error.textContent = requestError.message;
    error.hidden = false;
  } finally {
    refresh.disabled = false;
    refresh.textContent = "Refresh";
  }
}

function showLogin() {
  element("dashboard").hidden = true;
  element("login").hidden = false;
  element("username").focus();
}

async function showDashboard() {
  element("login").hidden = true;
  element("dashboard").hidden = false;
  await loadDashboard();
}

function selectTab(tab) {
  const selected = tab.dataset.tab;
  document.querySelectorAll("[data-tab]").forEach((item) => {
    const active = item === tab;
    item.classList.toggle("is-active", active);
    item.setAttribute("aria-selected", String(active));
    item.tabIndex = active ? 0 : -1;
  });
  document.querySelectorAll("[data-panel]").forEach((panel) => {
    const active = panel.dataset.panel === selected;
    panel.classList.toggle("is-active", active);
    panel.hidden = !active;
  });
  window.requestAnimationFrame(() => chartDefinitions.forEach((definition, target) => {
    const container = element(target);
    if (!container || container.offsetParent === null) return;
    if (!charts.has(target)) {
      renderLineChart(target, definition.labels, definition.series);
      return;
    }
    charts.get(target).resize();
  }));
}

element("login-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const button = element("login-button");
  const error = element("login-error");
  error.textContent = "";
  button.disabled = true;
  try {
    await request("session/login", {
      method: "POST",
      body: JSON.stringify({ username: element("username").value, password: element("password").value }),
    });
    element("password").value = "";
    await showDashboard();
  } catch (requestError) {
    error.textContent = requestError.status === 401 ? "Incorrect username or password." : requestError.message;
    element("password").focus();
  } finally {
    button.disabled = false;
  }
});

element("days").addEventListener("change", () => void loadDashboard());
element("refresh").addEventListener("click", () => void loadDashboard());
element("logout").addEventListener("click", async () => {
  await request("session/logout", { method: "POST", body: "{}" }).catch(() => {});
  showLogin();
});

const tabs = [...document.querySelectorAll("[data-tab]")];
tabs.forEach((tab, index) => {
  tab.addEventListener("click", () => selectTab(tab));
  tab.addEventListener("keydown", (event) => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    let nextIndex = index;
    if (event.key === "ArrowLeft") nextIndex = (index - 1 + tabs.length) % tabs.length;
    if (event.key === "ArrowRight") nextIndex = (index + 1) % tabs.length;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = tabs.length - 1;
    selectTab(tabs[nextIndex]);
    tabs[nextIndex].focus();
  });
});

request("session/me")
  .then((session) => session.authenticated ? showDashboard() : showLogin())
  .catch(showLogin);
