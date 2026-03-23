//! GunSpec.io HTTP helpers and a small in-memory manufacturer cache.

use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const DEFAULT_API_BASE: &str = "https://api.gunspec.io";

#[cfg(test)]
mod test_api_base {
    use std::sync::{Mutex, OnceLock};

    static OVERRIDE: OnceLock<Mutex<Option<String>>> = OnceLock::new();

    fn slot() -> &'static Mutex<Option<String>> {
        OVERRIDE.get_or_init(|| Mutex::new(None))
    }

    pub(super) fn set(url: Option<String>) {
        *slot().lock().expect("gunspec test base lock") = url;
    }

    pub(super) fn current() -> String {
        slot()
            .lock()
            .expect("gunspec test base lock")
            .clone()
            .unwrap_or_else(|| super::DEFAULT_API_BASE.to_string())
    }
}

fn api_base() -> String {
    #[cfg(test)]
    {
        test_api_base::current()
    }
    #[cfg(not(test))]
    {
        DEFAULT_API_BASE.to_string()
    }
}

/// Serializes GunSpec HTTP tests across crates (commands + gunspec).
#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::Mutex;

    static HTTP_LOCK: Mutex<()> = Mutex::new(());

    pub(crate) fn lock_http() -> std::sync::MutexGuard<'static, ()> {
        HTTP_LOCK.lock().expect("gunspec http test lock")
    }

    pub(crate) fn reset_remote() {
        super::test_api_base::set(None);
        super::clear_cache();
    }

    pub(crate) fn set_base(url: Option<String>) {
        super::test_api_base::set(url);
        super::clear_cache();
    }
}
const CACHE_TTL: Duration = Duration::from_secs(3600);
/// Explorer tier has a very low daily cap; each page is one HTTP request — keep this small.
const MAX_MFG_PAGES: u32 = 3;
const MAX_SUGGEST: usize = 25;

const SEARCH_CACHE_TTL: Duration = Duration::from_secs(900);
const SEARCH_CACHE_MAX: usize = 48;

type MfgCacheState = Option<(Instant, Vec<(String, String)>)>;
type SearchCacheMap = HashMap<String, (Instant, Vec<FirearmListDto>)>;

static HTTP: OnceLock<Client> = OnceLock::new();
static MFG_CACHE: Mutex<MfgCacheState> = Mutex::new(None);
static SEARCH_CACHE: OnceLock<Mutex<SearchCacheMap>> = OnceLock::new();

fn search_cache() -> &'static Mutex<SearchCacheMap> {
    SEARCH_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Clear cached manufacturer data (call when the API key changes).
pub fn clear_cache() {
    if let Ok(mut g) = MFG_CACHE.lock() {
        *g = None;
    }
    if let Ok(mut s) = search_cache().lock() {
        s.clear();
    }
}

fn client() -> &'static Client {
    HTTP.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent("asset-manager/0.1 (https://github.com)")
            .build()
            .expect("reqwest client")
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    #[allow(dead_code)]
    page: i64,
    #[allow(dead_code)]
    limit: i64,
    #[allow(dead_code)]
    total: i64,
    #[serde(alias = "total_pages")]
    total_pages: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManufacturersEnvelope {
    success: bool,
    data: Option<Vec<ManufacturerDto>>,
    pagination: Option<PageInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManufacturerDto {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchEnvelope {
    success: bool,
    data: Option<Vec<FirearmListDto>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GunSpecErrorEnvelope {
    #[serde(default)]
    #[allow(dead_code)]
    success: bool,
    error: Option<GunSpecError>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GunSpecError {
    code: Option<String>,
    message: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FirearmListDto {
    name: String,
    #[serde(default, alias = "manufacturerId", alias = "manufacturer_id")]
    manufacturer_id: Option<String>,
}

/// Parse GunSpec `{"error":{"message","code"}}` JSON when present.
fn gunspec_api_error_message(text: &str, append_code_in_parens: bool) -> Option<String> {
    let env: GunSpecErrorEnvelope = serde_json::from_str(text).ok()?;
    let e = env.error?;
    let msg = e.message.unwrap_or_default();
    if msg.trim().is_empty() {
        return None;
    }
    let code = e.code.unwrap_or_default();
    if append_code_in_parens && !code.is_empty() {
        Some(format!("{msg} ({code})"))
    } else {
        Some(msg)
    }
}

fn headers(api_key: Option<&str>) -> Result<HeaderMap, String> {
    let mut h = HeaderMap::new();
    if let Some(k) = api_key {
        let t = k.trim();
        if !t.is_empty() {
            h.insert(
                "X-API-Key",
                HeaderValue::from_str(t).map_err(|e| format!("Invalid API key: {e}"))?,
            );
        }
    }
    Ok(h)
}

fn fetch_manufacturer_page(
    api_key: Option<&str>,
    page: u32,
) -> Result<ManufacturersEnvelope, String> {
    let url = format!("{}/v1/manufacturers?per_page=100&page={page}", api_base());
    let res = client()
        .get(&url)
        .headers(headers(api_key)?)
        .send()
        .map_err(|e| e.to_string())?;
    let status = res.status();
    let text = res.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        if let Some(m) = gunspec_api_error_message(&text, true) {
            return Err(m);
        }
        let snippet: String = text.chars().take(160).collect();
        return Err(format!("GunSpec manufacturers HTTP {status}: {snippet}"));
    }
    let env: ManufacturersEnvelope =
        serde_json::from_str(&text).map_err(|e| format!("GunSpec manufacturers JSON: {e}"))?;
    if !env.success {
        if let Some(m) = gunspec_api_error_message(&text, false) {
            return Err(m);
        }
        return Err("GunSpec manufacturers: success=false".into());
    }
    Ok(env)
}

/// Load (or refresh) the manufacturer id/name list into memory.
fn warm_manufacturer_cache(api_key: Option<&str>) -> Result<Vec<(String, String)>, String> {
    let mut all: Vec<(String, String)> = Vec::new();
    let mut page: u32 = 1;
    let mut total_pages: i64 = 1;

    loop {
        let env = fetch_manufacturer_page(api_key, page)?;
        if let Some(list) = env.data {
            for m in list {
                all.push((m.id, m.name));
            }
        }
        if let Some(p) = env.pagination {
            total_pages = p.total_pages.max(1);
        }
        if page as i64 >= total_pages || page >= MAX_MFG_PAGES {
            break;
        }
        page += 1;
    }

    let mut guard = MFG_CACHE
        .lock()
        .map_err(|_| "manufacturer cache lock poisoned".to_string())?;
    *guard = Some((Instant::now(), all.clone()));
    Ok(all)
}

fn manufacturers_cached(api_key: Option<&str>) -> Result<Vec<(String, String)>, String> {
    let guard = MFG_CACHE
        .lock()
        .map_err(|_| "manufacturer cache lock poisoned".to_string())?;
    let need_refresh = match &*guard {
        None => true,
        Some((t, _)) if t.elapsed() > CACHE_TTL => true,
        Some((_, v)) if v.is_empty() => true,
        _ => false,
    };
    if need_refresh {
        drop(guard);
        return warm_manufacturer_cache(api_key);
    }
    Ok(guard.as_ref().map(|(_, v)| v.clone()).unwrap_or_default())
}

/// Remote manufacturer names from the GunSpec cache, plus an optional user-visible message (e.g. rate limit).
pub fn suggest_manufacturers(query: &str, api_key: Option<&str>) -> (Vec<String>, Option<String>) {
    let q = query.trim();
    if q.is_empty() {
        return (Vec::new(), None);
    }
    let list = match manufacturers_cached(api_key) {
        Ok(l) => l,
        Err(e) => return (Vec::new(), Some(e)),
    };
    let lower = q.to_lowercase();
    let mut out: Vec<String> = list
        .into_iter()
        .map(|(_, name)| name)
        .filter(|name| name.to_lowercase().contains(&lower))
        .take(MAX_SUGGEST)
        .collect();
    out.sort_by_key(|a| a.to_lowercase());
    out.truncate(MAX_SUGGEST);
    (out, None)
}

fn search_cache_key(api_key: Option<&str>, q: &str) -> String {
    let k = api_key.unwrap_or("").trim();
    format!("{k}|{q}")
}

fn fetch_firearm_search(api_key: Option<&str>, q: &str) -> Result<Vec<FirearmListDto>, String> {
    let key = search_cache_key(api_key, q);
    if let Ok(mut guard) = search_cache().lock() {
        guard.retain(|_, (t, _)| t.elapsed() < SEARCH_CACHE_TTL);
        if let Some((t, rows)) = guard.get(&key) {
            if t.elapsed() < SEARCH_CACHE_TTL {
                return Ok(rows.clone());
            }
        }
    }

    let url = format!("{}/v1/firearms/search", api_base());
    let res = client()
        .get(&url)
        .headers(headers(api_key)?)
        .query(&[("q", q), ("per_page", "50")])
        .send()
        .map_err(|e| e.to_string())?;
    let status = res.status();
    let text = res.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        if let Some(m) = gunspec_api_error_message(&text, true) {
            return Err(m);
        }
        let snippet: String = text.chars().take(160).collect();
        return Err(format!("GunSpec search HTTP {status}: {snippet}"));
    }
    let env: SearchEnvelope =
        serde_json::from_str(&text).map_err(|e| format!("GunSpec search JSON: {e}"))?;
    if !env.success {
        if let Some(m) = gunspec_api_error_message(&text, false) {
            return Err(m);
        }
        return Err("GunSpec search: success=false".into());
    }
    let rows = env.data.unwrap_or_default();
    if let Ok(mut guard) = search_cache().lock() {
        if guard.len() >= SEARCH_CACHE_MAX {
            guard.clear();
        }
        guard.insert(key, (Instant::now(), rows.clone()));
    }
    Ok(rows)
}

pub(crate) fn resolve_manufacturer_id(
    cache: &[(String, String)],
    manufacturer_field: &str,
) -> Option<String> {
    let m = manufacturer_field.trim();
    if m.is_empty() {
        return None;
    }
    let lower = m.to_lowercase();
    for (id, name) in cache {
        if name.eq_ignore_ascii_case(m) {
            return Some(id.clone());
        }
    }
    for (id, name) in cache {
        if name.to_lowercase().contains(&lower) || id.to_lowercase() == lower {
            return Some(id.clone());
        }
    }
    None
}

/// Remote model names from GunSpec, plus an optional user-visible message (e.g. rate limit).
pub fn suggest_models(
    manufacturer_field: &str,
    model_query: &str,
    api_key: Option<&str>,
) -> (Vec<String>, Option<String>) {
    let mq = model_query.trim();
    if mq.is_empty() {
        return (Vec::new(), None);
    }

    let mut rows = match fetch_firearm_search(api_key, mq) {
        Ok(r) => r,
        Err(e) => return (Vec::new(), Some(e)),
    };

    if let Ok(cache) = manufacturers_cached(api_key) {
        if let Some(id) = resolve_manufacturer_id(&cache, manufacturer_field) {
            let filtered: Vec<FirearmListDto> = rows
                .iter()
                .filter(|r| r.manufacturer_id.as_deref() == Some(id.as_str()))
                .cloned()
                .collect();
            if !filtered.is_empty() {
                rows = filtered;
            }
        }
    }

    let mut seen = HashSet::<String>::new();
    let mut names: Vec<String> = Vec::new();
    for r in rows {
        let n = r.name.trim();
        if n.is_empty() {
            continue;
        }
        let key = n.to_lowercase();
        if seen.insert(key) {
            names.push(n.to_string());
        }
        if names.len() >= MAX_SUGGEST {
            break;
        }
    }
    names.sort_by_key(|a| a.to_lowercase());
    (names, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Matcher;

    fn reset_test_remote() {
        test_support::reset_remote();
    }

    #[test]
    fn resolve_manufacturer_id_exact_name() {
        let cache = vec![
            ("m1".to_string(), "Glock".to_string()),
            ("m2".to_string(), "Sig Sauer".to_string()),
        ];
        assert_eq!(
            resolve_manufacturer_id(&cache, "glock").as_deref(),
            Some("m1")
        );
    }

    #[test]
    fn resolve_manufacturer_id_by_partial() {
        let cache = vec![("x".to_string(), "Remington".to_string())];
        assert_eq!(
            resolve_manufacturer_id(&cache, "ming").as_deref(),
            Some("x")
        );
    }

    #[test]
    fn resolve_manufacturer_id_empty() {
        let cache: Vec<(String, String)> = vec![];
        assert!(resolve_manufacturer_id(&cache, "Anything").is_none());
    }

    #[test]
    fn suggest_manufacturers_empty_query_no_fetch() {
        let (items, notice) = suggest_manufacturers("   ", Some("fake-key"));
        assert!(items.is_empty());
        assert!(notice.is_none());
    }

    #[test]
    fn suggest_models_empty_trimmed_query() {
        let (items, notice) = suggest_models("Glock", "  \t ", Some("key"));
        assert!(items.is_empty());
        assert!(notice.is_none());
    }

    #[test]
    fn gunspec_error_envelope_deserializes() {
        let j = r#"{"success":false,"error":{"message":"Daily cap exceeded","code":"RATE_LIMIT"}}"#;
        let env: GunSpecErrorEnvelope = serde_json::from_str(j).unwrap();
        let e = env.error.unwrap();
        assert_eq!(e.message.as_deref(), Some("Daily cap exceeded"));
        assert_eq!(e.code.as_deref(), Some("RATE_LIMIT"));
    }

    #[test]
    fn gunspec_api_error_message_with_parens() {
        let j = r#"{"error":{"message":"Cap","code":"X"}}"#;
        assert_eq!(
            super::gunspec_api_error_message(j, true).as_deref(),
            Some("Cap (X)")
        );
    }

    #[test]
    fn gunspec_api_error_message_without_parens() {
        let j = r#"{"error":{"message":"Cap","code":"X"}}"#;
        assert_eq!(
            super::gunspec_api_error_message(j, false).as_deref(),
            Some("Cap")
        );
    }

    #[test]
    fn gunspec_api_error_message_invalid_json() {
        assert!(super::gunspec_api_error_message("not json", true).is_none());
    }

    #[test]
    fn headers_rejects_invalid_api_key_chars() {
        assert!(super::headers(Some("bad\0key")).is_err());
    }

    #[test]
    fn headers_accepts_trimmed_api_key() {
        let h = super::headers(Some("  abcd  ")).unwrap();
        assert_eq!(
            h.get("X-API-Key").and_then(|v| v.to_str().ok()),
            Some("abcd")
        );
    }

    #[test]
    fn headers_none_omits_key() {
        let h = super::headers(None).unwrap();
        assert!(h.get("X-API-Key").is_none());
    }

    #[test]
    fn fetch_manufacturer_page_ok() {
        let _lock = test_support::lock_http();
        let mut server = mockito::Server::new();
        let body = r#"{"success":true,"data":[{"id":"m1","name":"Acme Arms"}],"pagination":{"page":1,"limit":100,"total":1,"totalPages":1}}"#;
        let _m = server
            .mock("GET", "/v1/manufacturers")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create();
        test_support::set_base(Some(server.url()));
        let env = fetch_manufacturer_page(Some("k"), 1).unwrap();
        assert!(env.success);
        assert_eq!(env.data.as_ref().unwrap().len(), 1);
        reset_test_remote();
    }

    #[test]
    fn fetch_manufacturer_page_http_error_includes_message() {
        let _lock = test_support::lock_http();
        let mut server = mockito::Server::new();
        let err_body = r#"{"error":{"message":"Rate limited","code":"429"}}"#;
        let _m = server
            .mock("GET", "/v1/manufacturers")
            .match_query(Matcher::Any)
            .with_status(429)
            .with_body(err_body)
            .create();
        test_support::set_base(Some(server.url()));
        let e = match fetch_manufacturer_page(Some("k"), 1) {
            Err(e) => e,
            Ok(_) => panic!("expected error"),
        };
        assert!(e.contains("Rate limited"));
        reset_test_remote();
    }

    #[test]
    fn fetch_manufacturer_page_success_false() {
        let _lock = test_support::lock_http();
        let mut server = mockito::Server::new();
        let body = r#"{"success":false,"error":{"message":"Nope"}}"#;
        let _m = server
            .mock("GET", "/v1/manufacturers")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_body(body)
            .create();
        test_support::set_base(Some(server.url()));
        let e = match fetch_manufacturer_page(Some("k"), 1) {
            Err(e) => e,
            Ok(_) => panic!("expected error"),
        };
        assert!(e.contains("Nope"));
        reset_test_remote();
    }

    #[test]
    fn warm_manufacturer_cache_fetches_multiple_pages() {
        let _lock = test_support::lock_http();
        let mut server = mockito::Server::new();
        let page1 = r#"{"success":true,"data":[{"id":"a","name":"A"}],"pagination":{"page":1,"limit":100,"total":200,"totalPages":2}}"#;
        let page2 = r#"{"success":true,"data":[{"id":"b","name":"B"}],"pagination":{"page":2,"limit":100,"total":200,"totalPages":2}}"#;
        let _m1 = server
            .mock("GET", "/v1/manufacturers")
            .match_query(Matcher::Regex("page=1".to_string()))
            .with_status(200)
            .with_body(page1)
            .create();
        let _m2 = server
            .mock("GET", "/v1/manufacturers")
            .match_query(Matcher::Regex("page=2".to_string()))
            .with_status(200)
            .with_body(page2)
            .create();
        test_support::set_base(Some(server.url()));
        let rows = warm_manufacturer_cache(Some("k")).unwrap();
        assert_eq!(rows.len(), 2);
        reset_test_remote();
    }

    #[test]
    fn fetch_firearm_search_uses_cache_on_second_call() {
        let _lock = test_support::lock_http();
        let mut server = mockito::Server::new();
        let body = r#"{"success":true,"data":[{"name":"Alpha-9","manufacturerId":"m1"}]}"#;
        let m = server
            .mock("GET", "/v1/firearms/search")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_body(body)
            .expect(1)
            .create();
        test_support::set_base(Some(server.url()));
        let r1 = fetch_firearm_search(Some("k"), "alpha").unwrap();
        let r2 = fetch_firearm_search(Some("k"), "alpha").unwrap();
        assert_eq!(r1.len(), 1);
        assert_eq!(r2.len(), 1);
        m.assert();
        reset_test_remote();
    }

    #[test]
    fn suggest_manufacturers_remote_matches() {
        let _lock = test_support::lock_http();
        let mut server = mockito::Server::new();
        let body = r#"{"success":true,"data":[{"id":"x","name":"Zebra Mfg"},{"id":"y","name":"Acme Co"}],"pagination":{"page":1,"limit":100,"total":2,"totalPages":1}}"#;
        let _m = server
            .mock("GET", "/v1/manufacturers")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_body(body)
            .create();
        test_support::set_base(Some(server.url()));
        let (items, notice) = suggest_manufacturers("ac", Some("k"));
        assert!(notice.is_none());
        assert_eq!(items, vec!["Acme Co"]);
        reset_test_remote();
    }

    #[test]
    fn suggest_models_filters_by_resolved_manufacturer() {
        let _lock = test_support::lock_http();
        let mut server = mockito::Server::new();
        let search_body = r#"{"success":true,"data":[
            {"name":"Wanted","manufacturerId":"m1"},
            {"name":"Other","manufacturerId":"m2"}
        ]}"#;
        let mfg_body = r#"{"success":true,"data":[{"id":"m1","name":"Glock"}],"pagination":{"page":1,"limit":100,"total":1,"totalPages":1}}"#;
        let _ms = server
            .mock("GET", "/v1/firearms/search")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_body(search_body)
            .create();
        let _mm = server
            .mock("GET", "/v1/manufacturers")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_body(mfg_body)
            .create();
        test_support::set_base(Some(server.url()));
        let (items, notice) = suggest_models("Glock", "want", Some("k"));
        assert!(notice.is_none());
        assert_eq!(items, vec!["Wanted"]);
        reset_test_remote();
    }

    #[test]
    fn clear_cache_empties_search_results() {
        let _lock = test_support::lock_http();
        let mut server = mockito::Server::new();
        let body = r#"{"success":true,"data":[{"name":"Once"}]}"#;
        let m = server
            .mock("GET", "/v1/firearms/search")
            .match_query(Matcher::Any)
            .with_status(200)
            .with_body(body)
            .expect(2)
            .create();
        test_support::set_base(Some(server.url()));
        fetch_firearm_search(Some("k"), "q").unwrap();
        clear_cache();
        fetch_firearm_search(Some("k"), "q").unwrap();
        m.assert();
        reset_test_remote();
    }

    #[test]
    fn search_cache_key_stable() {
        assert_eq!(super::search_cache_key(Some("k"), "q"), "k|q");
        assert_eq!(super::search_cache_key(None, "q"), "|q");
    }
}
