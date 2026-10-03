use serde_json::json;
use sqlx::{Pool, Postgres};

fn canonical_ip_expr(ip_expr: &str) -> String {
    format!(
        "COALESCE(NULLIF(split_part({expr}, ',', 1), ''), NULLIF({expr}, ''), 'unknown')",
        expr = ip_expr
    )
}

fn session_key_expr(meta_expr: &str, ip_expr: &str, created_expr: &str) -> String {
    let canonical_ip = canonical_ip_expr(ip_expr);
    format!(
        "
        CASE
            WHEN NULLIF({meta_expr} ->> 'sid', '') IS NOT NULL THEN
                'sid:' || ({meta_expr} ->> 'sid') || ':' || CAST({created_expr} / 1800 AS TEXT)
            WHEN NULLIF({meta_expr} ->> 'machine_id', '') IS NOT NULL THEN
                'mid:' || ({meta_expr} ->> 'machine_id') || ':' || CAST({created_expr} / 1800 AS TEXT)
            ELSE
                'ip:' || {canonical_ip} || ':' || CAST({created_expr} / 1800 AS TEXT)
        END
        "
    )
}

fn overseas_country_predicate(country_expr: &str) -> String {
    format!(
        "
        CASE
            WHEN lower(trim(COALESCE({country_expr}, ''))) IN (
                'hong kong', 'hong kong sar', 'hk',
                'macau', 'macao', 'macao sar', 'mo',
                'taiwan', 'tw', 'taiwan, province of china'
            ) THEN TRUE
            WHEN lower(trim(COALESCE({country_expr}, ''))) IN (
                '', 'unknown', 'cached not available', 'utc', 'etc/utc', 'gmt',
                'n/a', '-', 'null', 'undefined',
                'china', 'mainland china', 'china mainland', 'cn',
                'people''s republic of china', 'prc'
            ) THEN FALSE
            ELSE TRUE
        END
        "
    )
}

fn automated_traffic_predicate(meta_expr: &str) -> String {
    let ua = format!("trim(lower(COALESCE({meta_expr} ->> 'ua', '')))");
    format!(
        "
        (
            {ua} IN ('', '-', 'null', 'undefined', 'curl', 'wget', 'node')
            OR {ua} LIKE '%headlesschrome%'
            OR {ua} LIKE 'cloudwastescanner-qa/%'
            OR {ua} LIKE 'cloudwastescanner-navcheck/%'
            OR {ua} LIKE 'cws-health-check/%'
            OR {ua} LIKE '%opsprobe%'
            OR {ua} LIKE '%okhttp%'
            OR {ua} LIKE '%curl/%'
            OR {ua} LIKE '%wget/%'
            OR {ua} LIKE '%python-requests%'
            OR {ua} LIKE 'python/%'
            OR {ua} LIKE 'python-%'
            OR {ua} LIKE '%urllib%'
            OR {ua} LIKE '%httpx%'
            OR {ua} LIKE '%aiohttp%'
            OR {ua} LIKE '%go-http-client%'
            OR {ua} LIKE '%fasthttp%'
            OR {ua} LIKE 'req/v%'
            OR {ua} LIKE '%node-fetch%'
            OR {ua} LIKE 'node/%'
            OR {ua} LIKE 'node.js/%'
            OR {ua} LIKE '%undici%'
            OR {ua} LIKE '%axios%'
            OR {ua} LIKE 'java/%'
            OR {ua} LIKE '%apache-httpclient%'
            OR {ua} LIKE '%jakarta commons-httpclient%'
            OR {ua} LIKE '%bot%'
            OR {ua} LIKE '%crawler%'
            OR {ua} LIKE '%spider%'
            OR {ua} LIKE '%scraper%'
            OR {ua} LIKE '%scrapy%'
            OR {ua} LIKE '%slurp%'
            OR {ua} LIKE '%ahrefs%'
            OR {ua} LIKE '%semrush%'
            OR {ua} LIKE '%dataforseo%'
            OR {ua} LIKE '%seranking%'
            OR {ua} LIKE '%anthropic%'
            OR {ua} LIKE '%bytespider%'
            OR {ua} LIKE '%cohere-ai%'
            OR {ua} LIKE '%censys%'
            OR {ua} LIKE '%nutch%'
            OR {ua} LIKE '%cms-checker%'
            OR {ua} LIKE '%webapp-mapper%'
            OR {ua} LIKE '%zgrab%'
            OR {ua} LIKE '%masscan%'
            OR {ua} LIKE '%nmap%'
            OR {ua} LIKE '%nuclei%'
            OR {ua} LIKE '%nikto%'
            OR {ua} LIKE '%sqlmap%'
            OR {ua} LIKE '%palo alto networks%'
            OR {ua} LIKE '%onyphe%'
            OR {ua} LIKE '%netcraftsurveyagent%'
            OR {ua} LIKE '%rootevidence%'
            OR {ua} LIKE '%panscient%'
            OR {ua} LIKE '%statuscake%'
            OR {ua} LIKE '%uptimerobot%'
            OR {ua} LIKE '%saashub%'
            OR {ua} LIKE '%slackbot%'
            OR {ua} LIKE '%telegrambot%'
        )
        "
    )
}

fn non_public_ip_predicate(ip_expr: &str) -> String {
    let ip = format!("lower(trim({}))", canonical_ip_expr(ip_expr));
    format!(
        "
        (
            {ip} IN ('0.0.0.0', '::', '::1')
            OR {ip} ~ '^127\\.'
            OR {ip} ~ '^10\\.'
            OR {ip} ~ '^172\\.(1[6-9]|2[0-9]|3[01])\\.'
            OR {ip} ~ '^192\\.168\\.'
            OR {ip} ~ '^169\\.254\\.'
            OR {ip} ~ '^f[cd][0-9a-f]{{2}}:'
            OR {ip} ~ '^fe[89ab][0-9a-f]:'
            OR {ip} ~ '^::ffff:(127|10|192\\.168|169\\.254)\\.'
            OR {ip} ~ '^::ffff:172\\.(1[6-9]|2[0-9]|3[01])\\.'
        )
        "
    )
}

fn analytics_geo_join(alias: &str, overseas_only: bool) -> String {
    if overseas_only {
        format!(
            " LEFT JOIN geoip_cache g ON g.ip = {} ",
            canonical_ip_expr(&format!("{alias}.ip_address"))
        )
    } else {
        String::new()
    }
}

fn analytics_geo_filter(overseas_only: bool) -> String {
    if overseas_only {
        format!(
            " AND {} ",
            overseas_country_predicate("COALESCE(g.json::jsonb ->> 'country', '')")
        )
    } else {
        String::new()
    }
}

fn analytics_automation_filter(meta_expr: &str, ip_expr: &str, exclude_automated: bool) -> String {
    if exclude_automated {
        format!(
            "
            AND NOT (
                {automation_predicate}
                OR {non_public_ip_predicate}
            )
            ",
            automation_predicate = automated_traffic_predicate(meta_expr),
            non_public_ip_predicate = non_public_ip_predicate(ip_expr)
        )
    } else {
        String::new()
    }
}

fn proxy_backfill_predicate(meta_expr: &str, ip_expr: &str) -> String {
    let canonical_ip = canonical_ip_expr(ip_expr);
    format!(
        "
        (
            lower(COALESCE({meta_expr} ->> 'origin', '')) = 'nginx_backfill'
            AND EXISTS (
                SELECT 1
                FROM geoip_cache gc
                WHERE gc.ip = {canonical_ip}
                  AND (
                      lower(COALESCE(gc.json::jsonb ->> 'org', '')) LIKE '%cloudflare%'
                      OR lower(COALESCE(gc.json::jsonb ->> 'as', '')) LIKE '%cloudflare%'
                  )
            )
        )
        "
    )
}

fn analytics_proxy_backfill_filter(
    meta_expr: &str,
    ip_expr: &str,
    exclude_proxy_backfill: bool,
) -> String {
    if exclude_proxy_backfill {
        format!(" AND NOT {} ", proxy_backfill_predicate(meta_expr, ip_expr))
    } else {
        String::new()
    }
}

fn nonhuman_backfill_predicate(meta_expr: &str) -> String {
    format!(
        "
        (
            lower(COALESCE({meta_expr} ->> 'origin', '')) = 'nginx_backfill'
            AND (
                COALESCE({meta_expr} ->> 'ua', '') ~* '^(CloudWasteScanner-(QA|NavCheck)/|cws-health-check/)'
                OR lower(COALESCE({meta_expr} ->> 'ua', '')) ~ '(curl|wget|python-|urllib|httpx|aiohttp|go-http-client|fasthttp|node|req/v3)'
                OR lower(COALESCE({meta_expr} ->> 'ua', '')) ~ '(saashub|censysinspect|palo alto networks|onyphe|abuse\\.xmco\\.fr|security\\.ipip\\.net|visionheight\\.com/scan|facebookexternalhit|slackbot|telegrambot|internetmeasurement|modatscanner|l9tcpid|zgrab|netcraftsurveyagent|cms-checker)'
                OR lower(COALESCE({meta_expr} ->> 'ua', '')) ~ '^java/'
                OR lower(trim(COALESCE({meta_expr} ->> 'ua', ''))) IN ('', '-', 'null', 'undefined')
                OR lower(COALESCE({meta_expr} ->> 'ua', '')) ~ '(bot|spider|crawler|scrapy)'
                OR lower(COALESCE({meta_expr} ->> 'url', '')) ~ '(boaform|wp-|xmlrpc|\\.env|\\.git|phpmyadmin)'
            )
        )
        "
    )
}

fn analytics_nonhuman_backfill_filter(meta_expr: &str, exclude_nonhuman_backfill: bool) -> String {
    if exclude_nonhuman_backfill {
        format!(" AND NOT {} ", nonhuman_backfill_predicate(meta_expr))
    } else {
        String::new()
    }
}

pub async fn build_admin_content_payload(
    pg_pool: &Pool<Postgres>,
    start_time: i64,
    end_time: i64,
    overseas_only: bool,
    exclude_automated: bool,
    exclude_proxy_backfill: bool,
    exclude_nonhuman_backfill: bool,
    include_search_interest: bool,
    page_detail: Option<&str>,
) -> serde_json::Value {
    let geo_join = analytics_geo_join("a", overseas_only);
    let geo_filter = analytics_geo_filter(overseas_only);
    let automation_filter =
        analytics_automation_filter("a.meta::jsonb", "a.ip_address", exclude_automated);
    let proxy_backfill_filter =
        analytics_proxy_backfill_filter("a.meta::jsonb", "a.ip_address", exclude_proxy_backfill);
    let nonhuman_backfill_filter =
        analytics_nonhuman_backfill_filter("a.meta::jsonb", exclude_nonhuman_backfill);
    let analytics_session_key = session_key_expr("a.meta::jsonb", "a.ip_address", "a.created_at");
    let analytics_ip = canonical_ip_expr("a.ip_address");

    let total_pv_query = format!(
        "SELECT COUNT(*)::bigint
         FROM analytics a
         {geo_join}
         WHERE a.event_type = 'page_view'
           AND a.created_at BETWEEN $1 AND $2
           {geo_filter}
           {automation_filter}
           {proxy_backfill_filter}
           {nonhuman_backfill_filter}"
    );
    let total_pv = sqlx::query_scalar::<_, i64>(&total_pv_query)
        .bind(start_time)
        .bind(end_time)
        .fetch_one(pg_pool)
        .await
        .unwrap_or(0);

    let pages_query = format!(
        "
        WITH raw AS (
            SELECT
                CASE
                    WHEN a.event_type = 'api_download_latest' THEN '/api/download/latest'
                    WHEN a.event_type = 'click_download' THEN lower(trim(COALESCE(a.meta::jsonb ->> 'url', '/download')))
                    ELSE lower(trim(COALESCE(a.meta::jsonb ->> 'url', '/')))
                END AS raw_url
            FROM analytics a
            {geo_join}
            WHERE a.event_type = 'page_view'
              AND a.created_at BETWEEN $1 AND $2
              {geo_filter}
              {automation_filter}
              {proxy_backfill_filter}
           {nonhuman_backfill_filter}
        ),
        normalized AS (
            SELECT CASE
                WHEN raw_url = '' OR raw_url = 'null' THEN '/'
                WHEN raw_url LIKE 'http://%' OR raw_url LIKE 'https://%' THEN COALESCE(NULLIF(substring(raw_url FROM 'https?://[^/]+(/.*)$'), ''), '/')
                ELSE raw_url
            END AS path_with_suffix
            FROM raw
        ),
        stripped AS (
            SELECT split_part(path_with_suffix, '?', 1) AS path_no_query FROM normalized
        ),
        final AS (
            SELECT split_part(path_no_query, '#', 1) AS path FROM stripped
        )
        SELECT CASE WHEN path = '' THEN '/' ELSE path END AS page, COUNT(*)::bigint as hits
        FROM final
        GROUP BY page
        ORDER BY hits DESC
        LIMIT 50"
    );
    let pages = sqlx::query_as::<_, (Option<String>, i64)>(&pages_query)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(pg_pool)
        .await
        .unwrap_or_default()
        .iter()
        .map(|r| json!({ "page": r.0.clone().unwrap_or("/".to_string()), "hits": r.1 }))
        .collect::<Vec<_>>();
    let displayed_pages = pages
        .iter()
        .filter_map(|row| row.get("page").and_then(|v| v.as_str()).map(str::to_string))
        .collect::<Vec<_>>();

    let page_engagement_rows = if displayed_pages.is_empty() {
        Vec::new()
    } else {
        let page_engagement_query = format!(
            "
            WITH selected_pages AS (
                SELECT unnest($3::text[]) AS page
            ),
            event_base AS (
                SELECT
                    a.created_at,
                    {analytics_session_key} AS session_key,
                    a.event_type,
                    CASE
                        WHEN COALESCE(a.meta::jsonb ->> 'duration', '') ~ '^[0-9]+(\\.[0-9]+)?$'
                            THEN (a.meta::jsonb ->> 'duration')::DOUBLE PRECISION
                        ELSE 0.0
                    END AS leave_duration,
                    CASE
                        WHEN a.event_type = 'api_download_latest' THEN '/api/download/latest'
                        WHEN a.event_type = 'click_download' THEN lower(trim(COALESCE(a.meta::jsonb ->> 'url', '/download')))
                        ELSE lower(trim(COALESCE(a.meta::jsonb ->> 'url', '/')))
                    END AS raw_url
                FROM analytics a
                {geo_join}
                WHERE a.created_at BETWEEN $1 AND $2
                  AND a.event_type IN ('page_view', 'page_leave')
                  {geo_filter}
                  {automation_filter}
                  {proxy_backfill_filter}
           {nonhuman_backfill_filter}
            ),
            normalized AS (
                SELECT created_at, session_key, event_type, leave_duration,
                    CASE
                        WHEN raw_url = '' OR raw_url = 'null' THEN '/'
                        WHEN raw_url LIKE 'http://%' OR raw_url LIKE 'https://%' THEN COALESCE(NULLIF(substring(raw_url FROM 'https?://[^/]+(/.*)$'), ''), '/')
                        ELSE raw_url
                    END AS path_with_suffix
                FROM event_base
            ),
            stripped AS (
                SELECT created_at, session_key, event_type, leave_duration, split_part(path_with_suffix, '?', 1) AS path_no_query
                FROM normalized
            ),
            event_norm AS (
                SELECT created_at, session_key, event_type, leave_duration, split_part(path_no_query, '#', 1) AS page
                FROM stripped
            ),
            page_views AS (
                SELECT created_at, session_key, CASE WHEN page = '' THEN '/' ELSE page END AS page
                FROM event_norm
                WHERE event_type = 'page_view'
            ),
            session_pages AS (
                SELECT session_key, page, ROW_NUMBER() OVER (PARTITION BY session_key ORDER BY created_at ASC) AS rn
                FROM page_views
            ),
            landing AS (
                SELECT sp.session_key, sp.page
                FROM session_pages sp
                JOIN selected_pages p ON p.page = sp.page
                WHERE sp.rn = 1
            ),
            session_stats AS (
                SELECT e.session_key,
                    SUM(CASE WHEN event_type = 'page_view' THEN 1 ELSE 0 END) AS page_views,
                    SUM(CASE WHEN event_type = 'page_leave' AND leave_duration > 0 THEN leave_duration ELSE 0 END) AS leave_duration_total,
                    MIN(created_at) AS min_ts,
                    MAX(created_at) AS max_ts
                FROM event_norm e
                JOIN landing l ON l.session_key = e.session_key
                GROUP BY e.session_key
            )
            SELECT l.page AS page, COUNT(*)::bigint AS sessions,
                COALESCE(
                    AVG(
                        CASE
                            WHEN COALESCE(s.page_views, 0) <= 1 THEN 1.0::DOUBLE PRECISION
                            ELSE 0.0::DOUBLE PRECISION
                        END
                    ),
                    0.0::DOUBLE PRECISION
                ) AS bounce_rate,
                COALESCE(
                    AVG(
                        CASE
                            WHEN COALESCE(s.leave_duration_total, 0) > 0 THEN LEAST(s.leave_duration_total, 1800.0)
                            WHEN COALESCE(s.max_ts, 0) > COALESCE(s.min_ts, 0) THEN LEAST(CAST(s.max_ts - s.min_ts AS DOUBLE PRECISION), 1800.0)
                            ELSE 0.0::DOUBLE PRECISION
                        END
                    ),
                    0.0::DOUBLE PRECISION
                ) AS avg_duration
            FROM landing l
            JOIN session_stats s ON s.session_key = l.session_key
            GROUP BY l.page
            ORDER BY sessions DESC, l.page ASC
            LIMIT 50
            "
        );
        sqlx::query_as::<_, (Option<String>, i64, f64, f64)>(&page_engagement_query)
            .bind(start_time)
            .bind(end_time)
            .bind(&displayed_pages)
            .fetch_all(pg_pool)
            .await
            .unwrap_or_else(|err| {
                eprintln!("admin_content page_engagement query failed: {err}");
                Vec::new()
            })
            .iter()
            .map(|r| {
                json!({
                    "page": r.0.clone().unwrap_or_else(|| "/".to_string()),
                    "sessions": r.1,
                    "bounce_rate": r.2,
                    "avg_duration": r.3
                })
            })
            .collect::<Vec<_>>()
    };

    let (
        top_search_queries,
        zero_result_queries,
        top_faq_opens,
        search_totals,
        search_trend_labels,
        search_trend_searches,
        search_trend_zero_results,
        search_trend_faq_opens,
    ) = if include_search_interest {
        let top_search_query_sql = format!(
            "
            SELECT
                NULLIF(TRIM(COALESCE(a.meta::jsonb ->> 'query', '')), '') AS query_text,
                COUNT(*)::bigint AS searches,
                SUM(CASE WHEN lower(COALESCE(NULLIF(a.meta::jsonb ->> 'zero_results', ''), 'false')) IN ('1', 'true', 't', 'yes') THEN 1 ELSE 0 END)::bigint AS zero_result_hits,
                AVG(CAST(COALESCE(NULLIF(a.meta::jsonb ->> 'results', ''), '0') AS DOUBLE PRECISION)) AS avg_results,
                MAX(a.created_at) AS last_seen,
                NULLIF(TRIM(COALESCE(a.meta::jsonb ->> 'section', '')), '') AS section_name
            FROM analytics a
            {geo_join}
            WHERE a.event_type = 'site_search'
              AND a.created_at BETWEEN $1 AND $2
              AND NULLIF(TRIM(COALESCE(a.meta::jsonb ->> 'query', '')), '') IS NOT NULL
              {geo_filter}
              {automation_filter}
              {proxy_backfill_filter}
               {nonhuman_backfill_filter}
            GROUP BY query_text, section_name
            ORDER BY searches DESC, last_seen DESC
            LIMIT 25
            "
        );
        let zero_result_query_sql = format!(
            "
            SELECT NULLIF(TRIM(COALESCE(a.meta::jsonb ->> 'query', '')), '') AS query_text,
                   COUNT(*)::bigint AS zero_result_hits,
                   MAX(a.created_at) AS last_seen
            FROM analytics a
            {geo_join}
            WHERE a.event_type = 'site_search'
              AND a.created_at BETWEEN $1 AND $2
              AND lower(COALESCE(NULLIF(a.meta::jsonb ->> 'zero_results', ''), 'false')) IN ('1', 'true', 't', 'yes')
              AND NULLIF(TRIM(COALESCE(a.meta::jsonb ->> 'query', '')), '') IS NOT NULL
              {geo_filter}
              {automation_filter}
              {proxy_backfill_filter}
               {nonhuman_backfill_filter}
            GROUP BY query_text
            ORDER BY zero_result_hits DESC, last_seen DESC
            LIMIT 25
            "
        );
        let faq_opens_sql = format!(
            "
            SELECT
                NULLIF(TRIM(COALESCE(a.meta::jsonb ->> 'faq_id', '')), '') AS faq_id,
                NULLIF(TRIM(COALESCE(a.meta::jsonb ->> 'faq_title', '')), '') AS faq_title,
                NULLIF(TRIM(COALESCE(a.meta::jsonb ->> 'faq_category', '')), '') AS faq_category,
                COUNT(*)::bigint AS opens,
                MAX(a.created_at) AS last_seen
            FROM analytics a
            {geo_join}
            WHERE a.event_type = 'faq_open'
              AND a.created_at BETWEEN $1 AND $2
              {geo_filter}
              {automation_filter}
              {proxy_backfill_filter}
               {nonhuman_backfill_filter}
            GROUP BY faq_id, faq_title, faq_category
            ORDER BY opens DESC, last_seen DESC
            LIMIT 25
            "
        );
        let search_totals_sql = format!(
            "
            SELECT
                COALESCE(SUM(CASE WHEN a.event_type = 'site_search' THEN 1 ELSE 0 END), 0)::bigint AS total_searches,
                COALESCE(SUM(CASE WHEN a.event_type = 'site_search' AND lower(COALESCE(NULLIF(a.meta::jsonb ->> 'zero_results', ''), 'false')) IN ('1', 'true', 't', 'yes') THEN 1 ELSE 0 END), 0)::bigint AS zero_result_searches,
                COALESCE(SUM(CASE WHEN a.event_type = 'faq_open' THEN 1 ELSE 0 END), 0)::bigint AS faq_opens
            FROM analytics a
            {geo_join}
            WHERE a.created_at BETWEEN $1 AND $2
              AND a.event_type IN ('site_search', 'faq_open')
              {geo_filter}
              {automation_filter}
              {proxy_backfill_filter}
               {nonhuman_backfill_filter}
            "
        );
        let search_trend_sql = format!(
            "
            WITH days AS (
                SELECT gs.day_ts,
                       to_char((to_timestamp(gs.day_ts) AT TIME ZONE 'UTC' + interval '8 hour'), 'YYYY-MM-DD') AS day
                FROM generate_series($1::bigint, $2::bigint, 86400) AS gs(day_ts)
            ),
            usage_daily AS (
                SELECT to_char((to_timestamp(a.created_at) AT TIME ZONE 'UTC' + interval '8 hour'), 'YYYY-MM-DD') AS day,
                       SUM(CASE WHEN a.event_type = 'site_search' THEN 1 ELSE 0 END)::bigint AS searches,
                       SUM(CASE WHEN a.event_type = 'site_search' AND lower(COALESCE(NULLIF(a.meta::jsonb ->> 'zero_results', ''), 'false')) IN ('1', 'true', 't', 'yes') THEN 1 ELSE 0 END)::bigint AS zero_results,
                       SUM(CASE WHEN a.event_type = 'faq_open' THEN 1 ELSE 0 END)::bigint AS faq_opens
                FROM analytics a
                {geo_join}
                WHERE a.created_at BETWEEN $1 AND $2
                  AND a.event_type IN ('site_search', 'faq_open')
                  {geo_filter}
                  {automation_filter}
                  {proxy_backfill_filter}
               {nonhuman_backfill_filter}
                GROUP BY day
            )
            SELECT d.day, COALESCE(u.searches, 0), COALESCE(u.zero_results, 0), COALESCE(u.faq_opens, 0)
            FROM days d
            LEFT JOIN usage_daily u ON u.day = d.day
            ORDER BY d.day ASC
            "
        );
        let top_search_future = sqlx::query_as::<
            _,
            (Option<String>, i64, i64, f64, i64, Option<String>),
        >(&top_search_query_sql)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(pg_pool);
        let zero_result_future =
            sqlx::query_as::<_, (Option<String>, i64, i64)>(&zero_result_query_sql)
                .bind(start_time)
                .bind(end_time)
                .fetch_all(pg_pool);
        let faq_open_future = sqlx::query_as::<
            _,
            (Option<String>, Option<String>, Option<String>, i64, i64),
        >(&faq_opens_sql)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(pg_pool);
        let search_totals_future = sqlx::query_as::<_, (i64, i64, i64)>(&search_totals_sql)
            .bind(start_time)
            .bind(end_time)
            .fetch_one(pg_pool);
        let search_trend_future = sqlx::query_as::<_, (String, i64, i64, i64)>(&search_trend_sql)
            .bind(start_time)
            .bind(end_time)
            .fetch_all(pg_pool);

        let (
            top_search_rows_res,
            zero_result_rows_res,
            faq_open_rows_res,
            search_totals_res,
            search_trend_rows_res,
        ) = tokio::join!(
            top_search_future,
            zero_result_future,
            faq_open_future,
            search_totals_future,
            search_trend_future
        );

        let top_search_queries = top_search_rows_res
            .unwrap_or_default()
            .into_iter()
            .map(
                |(query_text, searches, zero_result_hits, avg_results, last_seen, section_name)| {
                    json!({
                        "query": query_text.unwrap_or_default(),
                        "searches": searches,
                        "zero_result_hits": zero_result_hits,
                        "avg_results": ((avg_results.max(0.0) * 10.0).round()) / 10.0,
                        "last_seen": last_seen,
                        "section": section_name.unwrap_or_else(|| "All".to_string())
                    })
                },
            )
            .collect::<Vec<_>>();
        let zero_result_queries = zero_result_rows_res
            .unwrap_or_default()
            .into_iter()
            .map(|(query_text, zero_result_hits, last_seen)| {
                json!({
                    "query": query_text.unwrap_or_default(),
                    "zero_result_hits": zero_result_hits,
                    "last_seen": last_seen
                })
            })
            .collect::<Vec<_>>();
        let top_faq_opens = faq_open_rows_res
            .unwrap_or_default()
            .into_iter()
            .map(|(faq_id, faq_title, faq_category, opens, last_seen)| {
                json!({
                    "faq_id": faq_id.unwrap_or_default(),
                    "faq_title": faq_title.unwrap_or_else(|| "Untitled FAQ".to_string()),
                    "faq_category": faq_category.unwrap_or_else(|| "General".to_string()),
                    "opens": opens,
                    "last_seen": last_seen
                })
            })
            .collect::<Vec<_>>();
        let search_totals = search_totals_res.unwrap_or((0, 0, 0));
        let search_trend_rows = search_trend_rows_res.unwrap_or_default();
        let search_trend_labels = search_trend_rows
            .iter()
            .map(|r| r.0.clone())
            .collect::<Vec<_>>();
        let search_trend_searches = search_trend_rows.iter().map(|r| r.1).collect::<Vec<_>>();
        let search_trend_zero_results = search_trend_rows.iter().map(|r| r.2).collect::<Vec<_>>();
        let search_trend_faq_opens = search_trend_rows.iter().map(|r| r.3).collect::<Vec<_>>();
        (
            top_search_queries,
            zero_result_queries,
            top_faq_opens,
            search_totals,
            search_trend_labels,
            search_trend_searches,
            search_trend_zero_results,
            search_trend_faq_opens,
        )
    } else {
        (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            (0_i64, 0_i64, 0_i64),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    };

    let page_visits = if let Some(page) = page_detail.filter(|value| !value.trim().is_empty()) {
        let page_visit_sql = format!(
            "
            WITH event_base AS (
                SELECT
                    a.created_at,
                    {analytics_ip} AS ip_address,
                    a.meta::jsonb AS meta_json,
                    g.json AS geo_json,
                    CASE
                        WHEN a.event_type = 'api_download_latest' THEN '/api/download/latest'
                        WHEN a.event_type = 'click_download' THEN lower(trim(COALESCE(a.meta::jsonb ->> 'url', '/download')))
                        ELSE lower(trim(COALESCE(a.meta::jsonb ->> 'url', '/')))
                    END AS raw_url
                FROM analytics a
                LEFT JOIN geoip_cache g ON g.ip = {analytics_ip}
                WHERE a.event_type = 'page_view'
                  AND a.created_at BETWEEN $1 AND $2
                  {geo_filter}
                  {automation_filter}
                  {proxy_backfill_filter}
           {nonhuman_backfill_filter}
            ),
            normalized AS (
                SELECT created_at, ip_address, meta_json, geo_json,
                    CASE
                        WHEN raw_url = '' OR raw_url = 'null' THEN '/'
                        WHEN raw_url LIKE 'http://%' OR raw_url LIKE 'https://%' THEN COALESCE(NULLIF(substring(raw_url FROM 'https?://[^/]+(/.*)$'), ''), '/')
                        ELSE raw_url
                    END AS path_with_suffix
                FROM event_base
            ),
            stripped AS (
                SELECT created_at, ip_address, meta_json, geo_json, split_part(path_with_suffix, '?', 1) AS path_no_query
                FROM normalized
            ),
            final AS (
                SELECT created_at, ip_address, meta_json, geo_json, split_part(path_no_query, '#', 1) AS page
                FROM stripped
            )
            SELECT
                ip_address,
                created_at,
                geo_json,
                NULLIF(
                    TRIM(
                        COALESCE(
                            meta_json ->> 'referrer',
                            meta_json ->> 'referer',
                            meta_json ->> 'ref',
                            meta_json ->> 'source',
                            meta_json ->> 'utm_source',
                            ''
                        )
                    ),
                    ''
                ) AS source_label
            FROM final
            WHERE CASE WHEN page = '' THEN '/' ELSE page END = $3
            ORDER BY created_at DESC
            LIMIT 200
            "
        );
        sqlx::query_as::<_, (String, i64, Option<String>, Option<String>)>(&page_visit_sql)
            .bind(start_time)
            .bind(end_time)
            .bind(page.trim())
            .fetch_all(pg_pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|(ip, visited_at, geo_json, source_label)| {
                json!({
                    "ip": ip,
                    "visited_at": visited_at,
                    "geo_info": geo_json,
                    "source": source_label.unwrap_or_else(|| "-".to_string())
                })
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    json!({
        "overview": { "pageviews": total_pv },
        "pages": pages,
        "page_engagement": { "rows": page_engagement_rows },
        "page_visits": {
            "page": page_detail.unwrap_or(""),
            "rows": page_visits
        },
        "search_interest": {
            "summary": {
                "searches": search_totals.0,
                "zero_result_searches": search_totals.1,
                "faq_opens": search_totals.2
            },
            "top_queries": top_search_queries,
            "zero_result_queries": zero_result_queries,
            "top_faq": top_faq_opens,
            "trends": {
                "labels": search_trend_labels,
                "searches": search_trend_searches,
                "zero_results": search_trend_zero_results,
                "faq_opens": search_trend_faq_opens
            }
        }
    })
}

pub async fn build_admin_content_summary_payload(
    pg_pool: &Pool<Postgres>,
    start_time: i64,
    end_time: i64,
    overseas_only: bool,
    exclude_automated: bool,
    exclude_proxy_backfill: bool,
    exclude_nonhuman_backfill: bool,
) -> serde_json::Value {
    let geo_join = analytics_geo_join("a", overseas_only);
    let geo_filter = analytics_geo_filter(overseas_only);
    let automation_filter =
        analytics_automation_filter("a.meta::jsonb", "a.ip_address", exclude_automated);
    let proxy_backfill_filter =
        analytics_proxy_backfill_filter("a.meta::jsonb", "a.ip_address", exclude_proxy_backfill);
    let nonhuman_backfill_filter =
        analytics_nonhuman_backfill_filter("a.meta::jsonb", exclude_nonhuman_backfill);

    let total_pv_query = format!(
        "SELECT COUNT(*)::bigint
         FROM analytics a
         {geo_join}
         WHERE a.event_type = 'page_view'
           AND a.created_at BETWEEN $1 AND $2
           {geo_filter}
           {automation_filter}
           {proxy_backfill_filter}
           {nonhuman_backfill_filter}"
    );
    let total_pv = sqlx::query_scalar::<_, i64>(&total_pv_query)
        .bind(start_time)
        .bind(end_time)
        .fetch_one(pg_pool)
        .await
        .unwrap_or(0);

    let pages_query = format!(
        "
        WITH raw AS (
            SELECT
                CASE
                    WHEN a.event_type = 'api_download_latest' THEN '/api/download/latest'
                    WHEN a.event_type = 'click_download' THEN lower(trim(COALESCE(a.meta::jsonb ->> 'url', '/download')))
                    ELSE lower(trim(COALESCE(a.meta::jsonb ->> 'url', '/')))
                END AS raw_url
            FROM analytics a
            {geo_join}
            WHERE a.event_type = 'page_view'
              AND a.created_at BETWEEN $1 AND $2
              {geo_filter}
              {automation_filter}
              {proxy_backfill_filter}
              {nonhuman_backfill_filter}
        ),
        normalized AS (
            SELECT CASE
                WHEN raw_url = '' OR raw_url = 'null' THEN '/'
                WHEN raw_url LIKE 'http://%' OR raw_url LIKE 'https://%' THEN COALESCE(NULLIF(substring(raw_url FROM 'https?://[^/]+(/.*)$'), ''), '/')
                ELSE raw_url
            END AS path_with_suffix
            FROM raw
        ),
        stripped AS (
            SELECT split_part(path_with_suffix, '?', 1) AS path_no_query FROM normalized
        ),
        final AS (
            SELECT split_part(path_no_query, '#', 1) AS path FROM stripped
        )
        SELECT CASE WHEN path = '' THEN '/' ELSE path END AS page, COUNT(*)::bigint as hits
        FROM final
        GROUP BY page
        ORDER BY hits DESC
        LIMIT 50"
    );
    let pages = sqlx::query_as::<_, (Option<String>, i64)>(&pages_query)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(pg_pool)
        .await
        .unwrap_or_default()
        .iter()
        .map(|r| json!({ "page": r.0.clone().unwrap_or("/".to_string()), "hits": r.1 }))
        .collect::<Vec<_>>();

    json!({
        "overview": { "pageviews": total_pv },
        "pages": pages,
        "page_engagement": { "rows": [] },
        "page_visits": {
            "page": "",
            "rows": []
        },
        "search_interest": {
            "summary": {
                "searches": 0,
                "zero_result_searches": 0,
                "faq_opens": 0
            },
            "top_queries": [],
            "zero_result_queries": [],
            "top_faq": [],
            "trends": {
                "labels": [],
                "searches": [],
                "zero_results": [],
                "faq_opens": []
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        analytics_automation_filter, analytics_proxy_backfill_filter, automated_traffic_predicate,
        non_public_ip_predicate, proxy_backfill_predicate,
    };

    #[test]
    fn automation_filter_can_be_disabled() {
        assert_eq!(
            analytics_automation_filter("a.meta::jsonb", "a.ip_address", false),
            ""
        );
    }

    #[test]
    fn automation_filter_contains_bot_markers_when_enabled() {
        let sql = analytics_automation_filter("a.meta::jsonb", "a.ip_address", true);
        assert!(sql.contains("headlesschrome"));
        assert!(sql.contains("telegrambot"));
        assert!(sql.contains("cloudwastescanner-qa"));
        assert!(sql.contains("curl"));
        assert!(sql.contains("wget"));
        assert!(sql.contains("opsprobe"));
        assert!(sql.contains("dataforseo"));
        assert!(sql.contains("censys"));
        assert!(sql.contains("AND NOT"));
        assert!(!sql.contains("SELECT DISTINCT"));
    }

    #[test]
    fn proxy_backfill_filter_can_be_disabled() {
        assert_eq!(
            analytics_proxy_backfill_filter("a.meta::jsonb", "a.ip_address", false),
            ""
        );
    }

    #[test]
    fn proxy_backfill_filter_targets_nginx_backfill_and_cloudflare_org() {
        let predicate = proxy_backfill_predicate("a.meta::jsonb", "a.ip_address");
        assert!(predicate.contains("nginx_backfill"));
        assert!(predicate.contains("cloudflare"));
        assert!(predicate.contains("geoip_cache"));
    }

    #[test]
    fn base_predicates_keep_expected_dimensions() {
        let automation = automated_traffic_predicate("a.meta::jsonb");
        assert!(automation.contains("a.meta::jsonb ->> 'ua'"));
        let private_ip = non_public_ip_predicate("a.ip_address");
        assert!(private_ip.contains("127\\."));
        assert!(private_ip.contains("192\\.168"));
        assert!(private_ip.contains("::ffff"));
    }
}
