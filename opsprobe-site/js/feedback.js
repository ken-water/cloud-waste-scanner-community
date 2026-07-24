(() => {
  const form = document.querySelector("[data-feedback-form]");
  if (!form) return;

  const status = document.querySelector("[data-feedback-status]");
  const submit = form.querySelector('button[type="submit"]');
  const config = window.CWS_SITE || {};
  const loops = config.loops || {};
  const endpoint = (loops.formEndpoint || "").trim();
  const pageInput = form.querySelector('input[name="source"]');
  const groupInput = form.querySelector('input[name="userGroup"]');
  const listInput = form.querySelector('input[name="mailingLists"]');

  if (pageInput) pageInput.value = window.location.pathname || "feedback";
  if (groupInput) groupInput.value = loops.feedbackUserGroup || "website-feedback";
  if (listInput) listInput.value = loops.mailingLists || "";

  const params = new URLSearchParams(window.location.search);
  const topic = params.get("topic");
  const plan = params.get("plan");
  const topicSelect = form.querySelector('select[name="topic"]');
  const notes = form.querySelector('textarea[name="notes"]');
  if (topic && topicSelect && Array.from(topicSelect.options).some((option) => option.value === topic)) {
    topicSelect.value = topic;
  }
  if (plan && notes && !notes.value.trim()) {
    notes.value = `Plan: ${plan}\n\n`;
  }

  const setStatus = (message, tone = "neutral") => {
    if (!status) return;
    status.textContent = message;
    status.dataset.tone = tone;
  };

  if (!endpoint) {
    submit.disabled = true;
    setStatus("Feedback collection is ready, but the Loops form endpoint has not been configured yet.", "warn");
    return;
  }

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    submit.disabled = true;
    setStatus("Sending feedback...", "neutral");

    const data = new FormData(form);
    const topic = data.get("topic") || "unspecified";
    const contactAllowed = data.get("contactAllowed") === "yes" ? "yes" : "no";
    const notes = [
      `Topic: ${topic}`,
      `Contact allowed: ${contactAllowed}`,
      "",
      data.get("notes") || ""
    ].join("\n");

    const payload = new URLSearchParams();
    payload.set("email", data.get("email") || "");
    payload.set("source", data.get("source") || "feedback");
    payload.set("userGroup", data.get("userGroup") || "website-feedback");
    payload.set("notes", notes);
    if (data.get("mailingLists")) payload.set("mailingLists", data.get("mailingLists"));

    try {
      const response = await fetch(endpoint, {
        method: "POST",
        body: payload.toString(),
        headers: { "Content-Type": "application/x-www-form-urlencoded" }
      });
      const result = await response.json().catch(() => ({}));
      if (!response.ok || result.success === false) {
        throw new Error(result.message || "Feedback could not be sent.");
      }
      form.reset();
      setStatus("Feedback sent. We will review it with the product support queue.", "success");
    } catch (error) {
      submit.disabled = false;
      setStatus(error.message || "Feedback could not be sent. Please try again later.", "warn");
    }
  });
})();
