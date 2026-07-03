//! Wallpaper catalog client (ROADMAP 3.1) — metadata only. Portable brain
//! code: fetch + parse + cache; no GTK, no Linux-isms (callers supply dirs).
//!
//! The catalog is a Supabase table read with the anon key (RLS: published
//! rows only). `FRESCO_CATALOG_URL` overrides the endpoint — tests point it
//! at a local fixture server; it also de-risks a future host move.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const SUPABASE_URL: &str = "https://mmoxgmvrpiaflfnsrynx.supabase.co";
const ANON_KEY: &str = "sb_publishable_eWKJzAuME5rstSxGyCBoHA_8hrTwkQM";

/// One published catalog item, as served by PostgREST.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub media_url: String,
    #[serde(default)]
    pub thumb_url: Option<String>,
    #[serde(default)]
    pub size_bytes: u64,
    /// License + author are ALWAYS displayed (legal attribution is a launch
    /// requirement, not a nice-to-have).
    pub license: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub source_url: Option<String>,
}

/// Endpoint returning the JSON array of published items.
pub fn catalog_url() -> String {
    std::env::var("FRESCO_CATALOG_URL").unwrap_or_else(|_| {
        format!(
            "{SUPABASE_URL}/rest/v1/catalog_items?select=*&published=eq.true&order=created_at.desc"
        )
    })
}

/// Fetch the catalog. Blocking — call from a worker thread.
pub fn fetch(url: &str) -> Result<Vec<CatalogItem>> {
    let items: Vec<CatalogItem> = ureq::get(url)
        .set("apikey", ANON_KEY)
        .set("Authorization", &format!("Bearer {ANON_KEY}"))
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .context("fetching catalog")?
        .into_json()
        .context("parsing catalog JSON")?;
    Ok(items)
}

/// Cache the parsed catalog so the gallery renders offline (atomic write).
pub fn save_cache(cache_dir: &Path, items: &[CatalogItem]) -> Result<()> {
    std::fs::create_dir_all(cache_dir)?;
    let tmp = cache_dir.join("catalog.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(items)?)?;
    std::fs::rename(&tmp, cache_dir.join("catalog.json"))?;
    Ok(())
}

pub fn load_cache(cache_dir: &Path) -> Option<Vec<CatalogItem>> {
    let bytes = std::fs::read(cache_dir.join("catalog.json")).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Bump the server-side install counter (the only "analytics" — no client
/// identifiers, fire-and-forget). No-op when a custom endpoint is active.
pub fn record_install(item_id: &str) {
    if std::env::var("FRESCO_CATALOG_URL").is_ok() {
        return;
    }
    let _ = ureq::post(&format!("{SUPABASE_URL}/rest/v1/rpc/catalog_count_install"))
        .set("apikey", ANON_KEY)
        .set("Authorization", &format!("Bearer {ANON_KEY}"))
        .set("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(10))
        .send_json(serde_json::json!({ "item": item_id }));
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"[
        {
            "id": "0b70e5f3-3f9e-4d0a-9c94-000000000001",
            "created_at": "2026-07-01T00:00:00Z",
            "content_type": "video",
            "title": "Rainy Window",
            "category": "nature",
            "tags": ["rain", "cozy"],
            "media_url": "https://cdn.example.com/rainy.mp4",
            "thumb_url": "https://cdn.example.com/rainy.jpg",
            "size_bytes": 20000000,
            "width": 3840, "height": 2160, "duration_s": 12.0,
            "checksum": "sha256:abc",
            "license": "CC0-1.0",
            "author": "Jane Doe",
            "source_url": "https://example.com/original",
            "published": true,
            "install_count": 5
        }
    ]"#;

    #[test]
    fn parses_postgrest_rows_and_tolerates_extra_fields() {
        let items: Vec<CatalogItem> = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(items.len(), 1);
        let it = &items[0];
        assert_eq!(it.title, "Rainy Window");
        assert_eq!(it.license, "CC0-1.0");
        assert_eq!(it.author, "Jane Doe");
        assert_eq!(it.size_bytes, 20_000_000);
        assert_eq!(it.tags, ["rain", "cozy"]);
    }

    #[test]
    fn cache_round_trips() {
        let items: Vec<CatalogItem> = serde_json::from_str(FIXTURE).unwrap();
        let dir = std::env::temp_dir().join(format!("fresco-cat-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        save_cache(&dir, &items).unwrap();
        assert_eq!(load_cache(&dir).unwrap(), items);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fetch_works_against_a_local_fixture_server() {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf);
                let body = FIXTURE.as_bytes();
                let _ = sock.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                );
                let _ = sock.write_all(body);
            }
        });
        let items = fetch(&format!("http://{addr}/catalog")).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].category, "nature");
    }
}
