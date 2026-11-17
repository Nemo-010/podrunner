//! The CacheService (cache service v2) of the results API (T-120): the
//! crate `pod-actions-results` of podcrates (GitHub #95). Its tests are in
//! the crate.

pub use pod_actions_results::cache::{CacheStore, LIMIT, Routed};

/// The CacheService of a run with no cache (`--no-cache`, T-338): each
/// lookup is a miss and each save is refused, as twirp answers that the
/// cache client takes at once. With no answer, the client retries each
/// call for about a minute.
pub fn off(method: &str, path: &str) -> Option<Routed> {
    let call = path.split('?').next()?.rsplit_once("CacheService/")?.1;
    let body = match (method, call) {
        ("POST", "GetCacheEntryDownloadURL") => {
            serde_json::json!({ "ok": false, "signed_download_url": "", "matched_key": "" })
        }
        ("POST", "CreateCacheEntry" | "FinalizeCacheEntryUpload") => {
            serde_json::json!({ "ok": false, "message": "the cache of this run is off (--no-cache)" })
        }
        _ => return None,
    };
    Some((200, body.to_string().into_bytes(), Vec::new()))
}

#[cfg(test)]
mod tests {
    /// T-338: with no cache, a lookup misses and a save is refused.
    #[test]
    fn a_run_with_no_cache_misses_and_refuses() {
        let p = "/twirp/github.actions.results.api.v1.CacheService/";
        let (status, body, _) = super::off("POST", &format!("{p}GetCacheEntryDownloadURL")).unwrap();
        assert_eq!((status, String::from_utf8(body).unwrap().contains(r#""ok":false"#)), (200, true));
        assert!(super::off("POST", &format!("{p}CreateCacheEntry")).is_some());
        assert!(super::off("GET", "/_apis/other").is_none());
    }
}
