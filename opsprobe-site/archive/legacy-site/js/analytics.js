(function() {
    // Default to same-origin first. If unavailable, retry the primary site endpoint so
    // external domains without /api/track proxy won't silently drop telemetry.
    const overrideApiUrl = (window.CWS_TRACK_API_URL || '').trim();
    const fallbackApiUrl = (window.CWS_TRACK_FALLBACK_API_URL || 'https://cloud-waste-scanner.com/api/track').trim();
    const apiUrl = overrideApiUrl || `${window.location.origin}/api/track`;
    const fallbackEnabled = String(window.CWS_TRACK_ENABLE_FALLBACK || '1').trim() !== '0';
    const canFallback = fallbackEnabled && apiUrl !== fallbackApiUrl;
    const explicitEnv = (window.CWS_TRACK_ENV || '').trim();
    const logTrackWarning = (...args) => {
        try {
            console.warn('[cws-track]', ...args);
        } catch (_) {}
    };
    
    // 1. Session Management
    let sessionId = sessionStorage.getItem('cws_sid');
    if (!sessionId) {
        sessionId = crypto.randomUUID();
        sessionStorage.setItem('cws_sid', sessionId);
        sessionStorage.setItem('cws_ref', document.referrer);
    }
    const referrer = sessionStorage.getItem('cws_ref');
    const startTime = Date.now();

    // Capture UTM params (persist for the session)
    const urlParams = new URLSearchParams(window.location.search);
    const sourceParam = urlParams.get('source');
    const utmSource = urlParams.get('utm_source');
    const utmMedium = urlParams.get('utm_medium');
    const utmCampaign = urlParams.get('utm_campaign');
    const utmContent = urlParams.get('utm_content');
    
    if (sourceParam) sessionStorage.setItem('cws_source', sourceParam);
    if (utmSource) sessionStorage.setItem('cws_utm_source', utmSource);
    if (utmMedium) sessionStorage.setItem('cws_utm_medium', utmMedium);
    if (utmCampaign) sessionStorage.setItem('cws_utm_campaign', utmCampaign);
    if (utmContent) sessionStorage.setItem('cws_utm_content', utmContent);

    function getExperimentContext() {
        const runtime = window.__cwsExperiment || {};
        const root = document.documentElement ? document.documentElement.dataset : {};
        const experiment = runtime.experiment || root.experiment || null;
        const variant = runtime.variant || root.variant || null;
        return { experiment, variant };
    }

    function getPageGroup() {
        const path = window.location.pathname || '/';
        if (path === '/' || path.endsWith('/index.html')) return 'home';
        if (path.endsWith('/pricing.html')) return 'pricing';
        if (path.endsWith('/sample-report.html')) return 'sample_report';
        if (path.endsWith('/download/') || path.endsWith('/download/index.html')) return 'download';
        if (path.endsWith('/feedback.html')) return 'feedback';
        return 'other';
    }

    function getTargetPageGroup(parsedUrl, rawHref) {
        const href = rawHref || '';
        const path = parsedUrl ? parsedUrl.pathname : href;
        if (!path) return 'other';
        if (path === '/' || path.endsWith('/index.html')) return 'home';
        if (path.endsWith('/pricing.html') || href === '#paid-plans') return 'pricing';
        if (path.endsWith('/sample-report.html')) return 'sample_report';
        if (path.endsWith('/download/') || path.endsWith('/download/index.html')) return 'download';
        if (path.endsWith('/security.html') || path.endsWith('/privacy.html') || path.endsWith('/refund-policy.html') || path.endsWith('/recover.html')) return 'trust';
        return 'other';
    }

    // 2. Tracking Function
    function sendPayload(payload) {
        if (navigator.sendBeacon && (payload.event === 'page_leave' || payload.event === 'page_exit')) {
            const blob = new Blob([JSON.stringify(payload)], {type: 'application/json'});
            const queued = navigator.sendBeacon(apiUrl, blob);
            if (!queued && canFallback) {
                navigator.sendBeacon(fallbackApiUrl, blob);
            }
            return;
        }

        fetch(apiUrl, {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify(payload)
        })
            .then((res) => {
                if (!res.ok && canFallback) {
                    throw new Error(`track_primary_http_${res.status}`);
                }
            })
            .catch(() => {
                if (!canFallback) return;
                fetch(fallbackApiUrl, {
                    method: 'POST',
                    headers: {'Content-Type': 'application/json'},
                    body: JSON.stringify(payload)
                }).catch((err) => {
                    logTrackWarning('fallback request failed', {
                        event: payload.event,
                        apiUrl,
                        fallbackApiUrl,
                        error: String(err || 'unknown')
                    });
                });
            });
    }

    function track(event, meta = {}) {
        const payload = {
            event: event,
            meta: {
                ...meta,
                sid: sessionId,
                url: window.location.pathname,
                ref: referrer || 'direct',
                ua: navigator.userAgent, // New: Browser/OS
                tz: Intl.DateTimeFormat().resolvedOptions().timeZone, // New: Region
                lang: navigator.language, // New: Language
                page_group: meta.page_group || getPageGroup(),
                env: explicitEnv || null,
                source: sessionStorage.getItem('cws_source') || sessionStorage.getItem('cws_utm_source') || null,
                utm_source: sessionStorage.getItem('cws_utm_source') || null,
                utm_medium: sessionStorage.getItem('cws_utm_medium') || null,
                utm_campaign: sessionStorage.getItem('cws_utm_campaign') || null,
                utm_content: sessionStorage.getItem('cws_utm_content') || null,
                prev_page_group: sessionStorage.getItem('cws_prev_page_group') || null,
                prev_path: sessionStorage.getItem('cws_prev_path') || null
            }
        };
        sendPayload(payload);
        sendGtagEvent(event, payload.meta);
    }

    window.CWSAnalytics = window.CWSAnalytics || {};
    window.CWSAnalytics.track = track;

    function sendGtagEvent(event, meta) {
        if (typeof window.gtag !== 'function') {
            return;
        }

        const mappedName = {
            page_view: 'page_view',
            click_download: 'download_click',
            click_buy: 'checkout_start',
            click_start_audit: 'start_audit_click',
            click_sample_report: 'sample_report_view',
            view_pricing: 'pricing_intent',
            view_trust_doc: 'trust_doc_view',
            experiment_exposure: 'experiment_exposure',
            cta_impression: 'cta_impression',
            checkout_link_request: 'checkout_start',
            checkout_open: 'checkout_open',
            checkout_fallback: 'checkout_fallback',
            faq_open: 'faq_expand',
            site_search: 'search_submit',
            docs_search: 'search_submit',
            consent_granted: 'consent_granted'
        }[event];

        if (!mappedName) {
            return;
        }

        const params = {
            page_path: meta.url || window.location.pathname,
            page_title: document.title
        };

        if (meta.plan) params.plan = meta.plan;
        if (meta.target) params.target = meta.target;
        if (meta.file) params.file_name = meta.file;
        if (meta.source) params.source = meta.source;
        if (meta.query) params.search_term = meta.query;
        if (meta.section) params.search_section = meta.section;
        if (meta.sort) params.sort_order = meta.sort;
        if (typeof meta.results === 'number') params.result_count = meta.results;
        if (meta.experiment) params.experiment = meta.experiment;
        if (meta.variant) params.variant = meta.variant;
        if (meta.placement) params.placement = meta.placement;
        if (meta.cta_label) params.cta_label = meta.cta_label;
        if (meta.target_page_group) params.target_page_group = meta.target_page_group;
        if (meta.prev_page_group) params.prev_page_group = meta.prev_page_group;
        if (meta.faq_id) params.faq_id = meta.faq_id;
        if (meta.faq_title) params.faq_title = meta.faq_title;
        if (meta.faq_category) params.faq_category = meta.faq_category;

        window.gtag('event', mappedName, params);
    }

    // 3. Auto-Track Page View
    const expContext = getExperimentContext();
    track('page_view', {
        title: document.title,
        experiment: expContext.experiment,
        variant: expContext.variant
    });
    sessionStorage.setItem('cws_prev_page_group', getPageGroup());
    sessionStorage.setItem('cws_prev_path', window.location.pathname || '/');
    if (expContext.experiment && expContext.variant) {
        const exposureKey = `cws_exp_seen_${expContext.experiment}_${expContext.variant}`;
        if (!sessionStorage.getItem(exposureKey)) {
            track('experiment_exposure', {
                experiment: expContext.experiment,
                variant: expContext.variant
            });
            sessionStorage.setItem(exposureKey, '1');
        }
    }

    function trackCtaImpressions() {
        const seen = new Set();
        const elements = document.querySelectorAll('[data-track-impression]');
        if (!elements.length) return;

        const emitImpression = (el) => {
            const eventName = (el.dataset.trackImpression || 'cta_impression').trim();
            const placement = (el.dataset.placement || '').trim();
            const ctaLabel = (el.dataset.ctaLabel || el.textContent || '').trim();
            const experiment = (el.dataset.experiment || expContext.experiment || '').trim();
            const variant = (el.dataset.variant || expContext.variant || '').trim();
            const target = el.getAttribute('href') || el.dataset.target || null;
            const key = [eventName, placement || ctaLabel, target || ''].join('::');
            if (seen.has(key)) return;
            seen.add(key);
            track(eventName, {
                placement: placement || null,
                cta_label: ctaLabel || null,
                experiment: experiment || null,
                variant: variant || null,
                target: target || null
            });
        };

        if (!('IntersectionObserver' in window)) {
            elements.forEach(emitImpression);
            return;
        }

        const observer = new IntersectionObserver((entries) => {
            entries.forEach((entry) => {
                if (!entry.isIntersecting) return;
                emitImpression(entry.target);
                observer.unobserve(entry.target);
            });
        }, { threshold: 0.35 });

        elements.forEach((el) => observer.observe(el));
    }

    // 4. Heartbeat (Every 30s) - Keeps session alive & tracks duration roughly
    setInterval(() => {
        if (document.visibilityState === 'visible') {
            track('heartbeat', { duration: Math.round((Date.now() - startTime) / 1000) });
        }
    }, 30000);

    // 5. Page Leave / Unload (Time on Page)
    const handleLeave = () => {
        const duration = Math.round((Date.now() - startTime) / 1000);
        if (duration > 0) {
            track('page_leave', { duration: duration });
        }
    };
    window.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'hidden') handleLeave();
    });
    // Fallback for close
    window.addEventListener('beforeunload', handleLeave);

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', trackCtaImpressions, { once: true });
    } else {
        trackCtaImpressions();
    }

    // Keep hash anchors visible under the fixed top nav across all pages.
    function decodeHashId(hashValue) {
        const raw = String(hashValue || '').trim();
        if (!raw || raw === '#') return '';
        const token = raw.charAt(0) === '#' ? raw.slice(1) : raw;
        if (!token) return '';
        try {
            return decodeURIComponent(token);
        } catch (_) {
            return token;
        }
    }

    function computeAnchorOffset() {
        const nav = document.querySelector('header nav');
        const navHeight = nav ? Math.round(nav.getBoundingClientRect().height) : 0;
        const base = navHeight + 24;
        const min = window.matchMedia('(max-width: 767px)').matches ? 124 : 104;
        return Math.max(min, base);
    }

    function alignHashAnchor(maxAttempts = 3, attempt = 0) {
        const id = decodeHashId(window.location.hash);
        if (!id) return;
        const target = document.getElementById(id);
        if (!target) return;
        const top = Math.max(0, window.scrollY + target.getBoundingClientRect().top - computeAnchorOffset());
        window.scrollTo({ top, behavior: 'auto' });
        if (attempt < maxAttempts - 1) {
            window.setTimeout(() => alignHashAnchor(maxAttempts, attempt + 1), 90);
        }
    }

    function initAnchorAlignment() {
        alignHashAnchor();
        window.addEventListener('hashchange', () => alignHashAnchor(), { passive: true });
        window.addEventListener('resize', () => {
            if (window.location.hash) alignHashAnchor(1);
        }, { passive: true });
        window.addEventListener('orientationchange', () => {
            if (window.location.hash) alignHashAnchor(1);
        }, { passive: true });
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', initAnchorAlignment, { once: true });
    } else {
        initAnchorAlignment();
    }

    // 5. Track Clicks on Key Elements
    document.addEventListener('click', (e) => {
        const link = e.target.closest('a');
        if (link) {
            const href = link.getAttribute('href');
            if (!href) return;

            let parsedUrl = null;
            try {
                parsedUrl = new URL(href, window.location.origin);
            } catch (_) {
                parsedUrl = null;
            }

            const source =
                (parsedUrl && parsedUrl.searchParams.get('source')) ||
                sessionStorage.getItem('cws_source') ||
                sessionStorage.getItem('cws_utm_source') ||
                null;
            const currentPageGroup = getPageGroup();
            const targetPageGroup = getTargetPageGroup(parsedUrl, href);

            const manualEvent = link.dataset.trackEvent || null;
            if (manualEvent) {
                track(manualEvent, {
                    source,
                    target: parsedUrl ? parsedUrl.pathname : href,
                    source_page_group: currentPageGroup,
                    target_page_group: targetPageGroup,
                    experiment: link.dataset.experiment || null,
                    variant: link.dataset.variant || null,
                    placement: link.dataset.placement || null,
                    cta_label: link.dataset.ctaLabel || (link.textContent || '').trim() || null
                });
            }

            if (!manualEvent && targetPageGroup === 'pricing') {
                track('view_pricing', {
                    source,
                    target: parsedUrl ? parsedUrl.pathname : href,
                    source_page_group: currentPageGroup,
                    target_page_group: targetPageGroup,
                    placement: link.dataset.placement || null,
                    cta_label: link.dataset.ctaLabel || (link.textContent || '').trim() || null
                });
            } else if (!manualEvent && targetPageGroup === 'sample_report') {
                track('click_sample_report', {
                    source,
                    target: parsedUrl ? parsedUrl.pathname : href,
                    source_page_group: currentPageGroup,
                    target_page_group: targetPageGroup,
                    placement: link.dataset.placement || null,
                    cta_label: link.dataset.ctaLabel || (link.textContent || '').trim() || null
                });
            } else if (!manualEvent && targetPageGroup === 'trust') {
                track('view_trust_doc', {
                    source,
                    target: parsedUrl ? parsedUrl.pathname : href,
                    source_page_group: currentPageGroup,
                    target_page_group: targetPageGroup,
                    placement: link.dataset.placement || null,
                    cta_label: link.dataset.ctaLabel || (link.textContent || '').trim() || null
                });
            }

            const isCheckoutLink =
                href.includes('checkout.html') ||
                (parsedUrl && parsedUrl.pathname.includes('/api/paddle/checkout'));

            if (isCheckoutLink) {
                const plan = parsedUrl ? (parsedUrl.searchParams.get('plan') || 'unknown') : 'unknown';
                track('click_buy', { plan, source, target: parsedUrl ? parsedUrl.pathname : href });
            } else if (
                href.includes('.exe') ||
                href.includes('.deb') ||
                (parsedUrl && parsedUrl.pathname.includes('/api/download/latest'))
            ) {
                track('click_download', { file: href.split('/').pop(), source });
            }
        }
    });

    // Expose track to global window for manual events
    window.track_event = track;

    // 6. Cookie Consent Banner (Auto-Inject)
    (function injectCookieBanner() {
        if (localStorage.getItem('cws_cookie_consent')) return;

        // Ensure DOM is ready
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', injectCookieBanner);
            return;
        }

        // Inject Styles
        const style = document.createElement('style');
        style.innerHTML = `
            .cws-cookie-banner {
                position: fixed; bottom: 20px; right: 20px; width: 320px;
                background: white; border: 1px solid #e2e8f0; border-radius: 16px;
                box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.1), 0 8px 10px -6px rgba(0, 0, 0, 0.1);
                padding: 20px; z-index: 9999; 
                font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
                animation: cws-slide-up 0.5s ease-out;
            }
            @keyframes cws-slide-up { from { transform: translateY(20px); opacity: 0; } to { transform: translateY(0); opacity: 1; } }
            .cws-cookie-title { font-weight: 700; color: #0f172a; margin-bottom: 8px; font-size: 15px; }
            .cws-cookie-text { font-size: 13px; color: #64748b; line-height: 1.5; margin-bottom: 16px; }
            .cws-cookie-actions { display: flex; gap: 10px; }
            .cws-btn-accept { flex: 1; background: #0f172a; color: white; border: none; padding: 8px 16px; border-radius: 8px; font-size: 13px; font-weight: 600; cursor: pointer; transition: background 0.2s; }
            .cws-btn-accept:hover { background: #1e293b; }
            .cws-link-details { color: #64748b; text-decoration: none; font-size: 13px; font-weight: 600; padding: 8px 12px; border-radius: 8px; transition: color 0.2s; text-align: center; }
            .cws-link-details:hover { color: #0f172a; background: #f1f5f9; }
        `;
        document.head.appendChild(style);

        // Inject HTML
        const banner = document.createElement('div');
        banner.className = 'cws-cookie-banner';
        banner.innerHTML = `
            <div class="cws-cookie-title">Privacy Notice</div>
            <div class="cws-cookie-text">We use local/session storage and analytics signals (including IP-based measurement) to run and improve this site. See details in our Privacy page.</div>
            <div class="cws-cookie-actions">
                <button class="cws-btn-accept" id="cws-accept-btn">Accept</button>
                <a href="/privacy.html" class="cws-link-details">Details</a>
            </div>
        `;
        document.body.appendChild(banner);

        // Handle Click
        document.getElementById('cws-accept-btn').onclick = function() {
            localStorage.setItem('cws_cookie_consent', 'true');
            track('consent_granted'); // NEW: Track the click
            banner.style.opacity = '0';
            banner.style.transform = 'translateY(20px)';
            setTimeout(() => banner.remove(), 300);
        };
    })();

})();
