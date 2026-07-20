//! Resolve a user-pasted link (Pinterest pin, pin.it short link, or a direct
//! media URL) into a directly downloadable media URL. Shared by the GUI's
//! paste-a-link flow; all network calls are blocking, so callers run this on a
//! background thread.

use anyhow::{bail, Context};

/// Desktop-browser UA — Pinterest's unauth endpoints refuse obviously
/// non-browser clients.
const BROWSER_UA: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:127.0) Gecko/20100101 Firefox/127.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaKind {
    Video,
    Image,
    Gif,
}

#[derive(Debug, Clone)]
pub struct ResolvedMedia {
    /// Direct downloadable URL.
    pub media_url: String,
    pub kind: MediaKind,
    /// Human name for the library entry.
    pub title: Option<String>,
    /// Thumbnail/poster image if known.
    pub poster_url: Option<String>,
    /// Pinterest pin id for dedupe; `None` for direct URLs.
    pub pin_id: Option<String>,
}

/// Resolve a pasted URL into downloadable media. Blocking — call from a
/// background thread. Errors carry user-readable messages.
pub fn resolve(url: &str) -> anyhow::Result<ResolvedMedia> {
    let url = url.trim();
    if url.is_empty() {
        bail!("Paste a link first.");
    }
    if let Some(direct) = classify_direct(url) {
        return Ok(direct);
    }
    if let Some(code) = pin_it_code(url) {
        let expanded = expand_pin_it(url, &code)?;
        let pin_id = extract_pin_id(&expanded).with_context(|| {
            "That pin.it link didn't lead to a Pinterest pin — try copying the pin's full URL."
        })?;
        return resolve_pin(&pin_id);
    }
    if let Some(pin_id) = extract_pin_id(url) {
        return resolve_pin(&pin_id);
    }
    bail!("This link isn't supported yet — paste a Pinterest pin or a direct video/image link.")
}

/// Media kind implied by a URL path's extension, ignoring any query string.
fn kind_from_path(path: &str) -> Option<MediaKind> {
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        "mp4" | "webm" | "mkv" | "mov" => Some(MediaKind::Video),
        "gif" => Some(MediaKind::Gif),
        "jpg" | "jpeg" | "png" | "webp" => Some(MediaKind::Image),
        _ => None,
    }
}

/// The path component of a URL, without scheme/host/query/fragment.
fn url_path(url: &str) -> &str {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    match rest.find('/') {
        Some(i) => &rest[i..],
        None => "",
    }
}

/// A URL whose path already points at a media file needs no resolution.
fn classify_direct(url: &str) -> Option<ResolvedMedia> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return None;
    }
    let path = url_path(url);
    let kind = kind_from_path(path)?;
    let title = path
        .rsplit('/')
        .next()
        .and_then(|f| f.rsplit_once('.'))
        .map(|(stem, _)| stem.to_string())
        .filter(|s| !s.is_empty());
    Some(ResolvedMedia {
        media_url: url.to_string(),
        kind,
        title,
        poster_url: None,
        pin_id: None,
    })
}

/// The short code from a pin.it URL, if this is one.
fn pin_it_code(url: &str) -> Option<String> {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let rest = rest.strip_prefix("pin.it/")?;
    let code: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    (!code.is_empty()).then_some(code)
}

/// The numeric pin id from a pinterest.com/pin/<id> URL. Pin paths sometimes
/// carry a slug (`…/pin/some-slug--123456/`), so only the trailing digit run
/// counts.
fn extract_pin_id(url: &str) -> Option<String> {
    let idx = url.find("/pin/")?;
    let after = &url[idx + "/pin/".len()..];
    let segment = after.split(['/', '?', '#']).next()?;
    let id: String = segment
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    (!id.is_empty()).then_some(id)
}

/// Expand a pin.it short link to the full pin URL. Pinterest's URL-shortener
/// redirect endpoint answers without auth; if it doesn't, following the short
/// link itself and taking the final URL still works.
fn expand_pin_it(original: &str, code: &str) -> anyhow::Result<String> {
    let no_redirect = ureq::builder().redirects(0).build();
    let endpoint = format!("https://api.pinterest.com/url_shortener/{code}/redirect/");
    let resp = no_redirect
        .get(&endpoint)
        .set("User-Agent", BROWSER_UA)
        .call();
    // ureq treats 3xx-without-follow as Ok, but be liberal: some error paths
    // still carry a response with a Location header.
    let location = match resp {
        Ok(r) => r.header("Location").map(str::to_string),
        Err(ureq::Error::Status(_, r)) => r.header("Location").map(str::to_string),
        Err(_) => None,
    };
    if let Some(loc) = location {
        return Ok(loc);
    }
    let followed = ureq::agent()
        .get(original)
        .set("User-Agent", BROWSER_UA)
        .call()
        .context("Couldn't open that pin.it link — check your connection and try again.")?;
    Ok(followed.get_url().to_string())
}

/// Resolve a pin id via Pinterest's unauth JSON API, falling back to scraping
/// the pin page HTML when the API answer is missing or unexpectedly shaped.
fn resolve_pin(pin_id: &str) -> anyhow::Result<ResolvedMedia> {
    match resolve_pin_api(pin_id) {
        Ok(m) => Ok(m),
        Err(api_err) => resolve_pin_html(pin_id).map_err(|html_err| {
            log::warn!("pin {pin_id}: API failed ({api_err:#}); HTML failed ({html_err:#})");
            anyhow::anyhow!(
                "Couldn't read that Pinterest pin. It may be private or deleted — \
                 try another pin, or a direct video/image link."
            )
        }),
    }
}

fn resolve_pin_api(pin_id: &str) -> anyhow::Result<ResolvedMedia> {
    let data =
        format!(r#"{{"options":{{"id":"{pin_id}","field_set_key":"unauth_react_main_pin"}}}}"#);
    let resp = ureq::agent()
        .get("https://www.pinterest.com/resource/PinResource/get/")
        .query("data", &data)
        .set("User-Agent", BROWSER_UA)
        .set("X-Pinterest-PWS-Handler", "www/[username].js")
        .call()
        .context("Pinterest didn't answer")?;
    let json: serde_json::Value = resp.into_json().context("Pinterest sent unreadable data")?;
    let media = parse_pin_json(pin_id, &json)?;
    // Story-pin videos only advertise HLS; parse_pin_json derives the
    // progressive-mp4 twin, which is a convention, not a contract — probe it
    // before trusting, so a miss falls back to the HTML scan.
    if media.kind == MediaKind::Video {
        ureq::agent()
            .head(&media.media_url)
            .set("User-Agent", BROWSER_UA)
            .call()
            .context("derived video URL didn't answer")?;
    }
    Ok(media)
}

/// Interpret a PinResource response. Split from the network call so tests can
/// feed canned JSON.
fn parse_pin_json(pin_id: &str, json: &serde_json::Value) -> anyhow::Result<ResolvedMedia> {
    let data = &json["resource_response"]["data"];
    if data.is_null() {
        bail!("no pin data in response");
    }

    let title = [&data["title"], &data["grid_title"], &data["description"]]
        .iter()
        .filter_map(|v| v.as_str())
        .map(str::trim)
        .find(|s| !s.is_empty())
        .map(|s| s.chars().take(60).collect::<String>());
    let orig_image = data["images"]["orig"]["url"].as_str().map(str::to_string);

    // Video pins keep renditions either directly on the pin or, for story
    // pins, nested per-page/per-block.
    let mut video_lists = vec![&data["videos"]["video_list"]];
    if let Some(pages) = data["story_pin_data"]["pages"].as_array() {
        for page in pages {
            if let Some(blocks) = page["blocks"].as_array() {
                for block in blocks {
                    video_lists.push(&block["video"]["video_list"]);
                }
            }
        }
    }
    // Prefer a progressive mp4; story pins often expose ONLY an HLS playlist,
    // whose progressive twin lives at a derivable expMp4 URL.
    let video_url = video_lists
        .iter()
        .filter_map(|l| best_rendition(l))
        .next()
        .or_else(|| {
            video_lists
                .iter()
                .filter_map(|l| first_hls_url(l))
                .filter_map(|hls| hls_to_progressive(&hls))
                .next()
        });
    if let Some(url) = video_url {
        return Ok(ResolvedMedia {
            media_url: url,
            kind: MediaKind::Video,
            title,
            poster_url: orig_image,
            pin_id: Some(pin_id.to_string()),
        });
    }

    if let Some(url) = orig_image {
        let kind = if url_path(&url).to_ascii_lowercase().ends_with(".gif") {
            MediaKind::Gif
        } else {
            MediaKind::Image
        };
        return Ok(ResolvedMedia {
            media_url: url.clone(),
            kind,
            title,
            poster_url: None,
            pin_id: Some(pin_id.to_string()),
        });
    }
    bail!("pin response had neither video nor image")
}

/// The largest progressive .mp4 rendition in a `video_list` map. HLS playlists
/// (.m3u8) are skipped — the downloader wants a single progressive file.
fn best_rendition(video_list: &serde_json::Value) -> Option<String> {
    let map = video_list.as_object()?;
    map.values()
        .filter_map(|v| {
            let url = v["url"].as_str()?;
            if !url_path(url).to_ascii_lowercase().ends_with(".mp4") {
                return None;
            }
            let area = v["width"].as_u64().unwrap_or(0) * v["height"].as_u64().unwrap_or(0);
            Some((area, url.to_string()))
        })
        .max_by_key(|(area, _)| *area)
        .map(|(_, url)| url)
}

/// The first HLS playlist URL in a `video_list` map.
fn first_hls_url(video_list: &serde_json::Value) -> Option<String> {
    let map = video_list.as_object()?;
    map.values()
        .filter_map(|v| v["url"].as_str())
        .find(|url| url_path(url).to_ascii_lowercase().ends_with(".m3u8"))
        .map(str::to_string)
}

/// Derive the progressive-mp4 twin of a pinimg HLS playlist URL
/// (`…/hls/<hash>.m3u8` → `…/expMp4/<hash>_720w.mp4`). Pinterest hosts both
/// for every video; the caller probes the result before trusting it.
fn hls_to_progressive(hls_url: &str) -> Option<String> {
    if !hls_url.contains("/hls/") {
        return None;
    }
    hls_url
        .strip_suffix(".m3u8")
        .map(|base| format!("{}_720w.mp4", base.replacen("/hls/", "/expMp4/", 1)))
}

fn resolve_pin_html(pin_id: &str) -> anyhow::Result<ResolvedMedia> {
    let url = format!("https://www.pinterest.com/pin/{pin_id}/");
    let html = ureq::agent()
        .get(&url)
        .set("User-Agent", BROWSER_UA)
        .call()
        .context("couldn't load the pin page")?
        .into_string()
        .context("couldn't read the pin page")?;
    scan_pin_html(pin_id, &html).context("no media found in the pin page")
}

/// Find media URLs embedded in the pin page HTML with a plain string scan (the
/// repo deliberately has no regex dependency). Prefers the video rendition
/// with the largest `_NNNw` width marker when several appear.
fn scan_pin_html(pin_id: &str, html: &str) -> Option<ResolvedMedia> {
    if let Some(url) = best_scanned_url(html, "https://v1.pinimg.com/videos/", &["mp4"]) {
        return Some(ResolvedMedia {
            media_url: url,
            kind: MediaKind::Video,
            title: None,
            poster_url: None,
            pin_id: Some(pin_id.to_string()),
        });
    }
    let image_exts = ["jpg", "jpeg", "png", "gif", "webp"];
    let url = best_scanned_url(html, "https://i.pinimg.com/originals/", &image_exts)?;
    let kind = if url.to_ascii_lowercase().ends_with(".gif") {
        MediaKind::Gif
    } else {
        MediaKind::Image
    };
    Some(ResolvedMedia {
        media_url: url,
        kind,
        title: None,
        poster_url: None,
        pin_id: Some(pin_id.to_string()),
    })
}

/// All prefix-anchored URLs in `html` ending in one of `exts` (sliced at the
/// closing quote or backslash), ranked by any `_NNNw` width marker so the
/// largest rendition wins; falls back to the first match.
fn best_scanned_url(html: &str, prefix: &str, exts: &[&str]) -> Option<String> {
    let mut best: Option<(u32, String)> = None;
    let mut from = 0;
    while let Some(rel) = html[from..].find(prefix) {
        let start = from + rel;
        let tail = &html[start..];
        let end = tail.find(['"', '\\', '\'', '<', ' ']).unwrap_or(tail.len());
        let candidate = &tail[..end];
        from = start + prefix.len();
        let lower = candidate.to_ascii_lowercase();
        if !exts.iter().any(|e| lower.ends_with(&format!(".{e}"))) {
            continue;
        }
        let width = width_marker(candidate);
        if best.as_ref().is_none_or(|(w, _)| width > *w) {
            best = Some((width, candidate.to_string()));
        }
    }
    best.map(|(_, url)| url)
}

/// The `NNN` from a `_NNNw` marker in a pinimg URL, 0 when absent.
fn width_marker(url: &str) -> u32 {
    let mut from = 0;
    let mut best = 0;
    while let Some(rel) = url[from..].find('_') {
        let start = from + rel + 1;
        let digits: String = url[start..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        from = start;
        if !digits.is_empty() && url[start + digits.len()..].starts_with('w') {
            best = best.max(digits.parse().unwrap_or(0));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_urls_classify_by_extension() {
        let m = resolve("https://example.com/clips/ocean-waves.mp4?token=abc").unwrap();
        assert_eq!(m.kind, MediaKind::Video);
        assert_eq!(
            m.media_url,
            "https://example.com/clips/ocean-waves.mp4?token=abc"
        );
        assert_eq!(m.title.as_deref(), Some("ocean-waves"));
        assert_eq!(m.pin_id, None);

        assert_eq!(
            resolve("https://example.com/a/b/loop.GIF").unwrap().kind,
            MediaKind::Gif
        );
        assert_eq!(
            resolve("https://example.com/pic.jpeg#frag").unwrap().kind,
            MediaKind::Image
        );
    }

    #[test]
    fn unsupported_urls_fail_with_friendly_message() {
        let err = resolve("https://example.com/some/page").unwrap_err();
        assert!(err.to_string().contains("isn't supported"));
    }

    #[test]
    fn pin_id_extraction_handles_plain_and_slugged_urls() {
        assert_eq!(
            extract_pin_id("https://www.pinterest.com/pin/1234567890/").as_deref(),
            Some("1234567890")
        );
        assert_eq!(
            extract_pin_id("https://in.pinterest.com/pin/cozy-rain-loop--987654321?x=1").as_deref(),
            Some("987654321")
        );
        assert_eq!(extract_pin_id("https://www.pinterest.com/ideas/"), None);
    }

    #[test]
    fn pin_it_code_extraction() {
        assert_eq!(
            pin_it_code("https://pin.it/AbC123xy").as_deref(),
            Some("AbC123xy")
        );
        assert_eq!(pin_it_code("pin.it/AbC123xy/").as_deref(), Some("AbC123xy"));
        assert_eq!(pin_it_code("https://pinterest.com/pin/1/"), None);
    }

    #[test]
    fn picks_largest_mp4_rendition_and_skips_hls() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"resource_response":{"data":{
                "title":"  Rainy Street  ",
                "images":{"orig":{"url":"https://i.pinimg.com/originals/aa/poster.jpg"}},
                "videos":{"video_list":{
                    "HLS":{"url":"https://v1.pinimg.com/videos/hls/x.m3u8","width":9999,"height":9999},
                    "V_720P":{"url":"https://v1.pinimg.com/videos/mc/720p.mp4","width":720,"height":1280},
                    "V_480P":{"url":"https://v1.pinimg.com/videos/mc/480p.mp4","width":480,"height":854}
                }}
            }}}"#,
        )
        .unwrap();
        let m = parse_pin_json("42", &json).unwrap();
        assert_eq!(m.kind, MediaKind::Video);
        assert_eq!(m.media_url, "https://v1.pinimg.com/videos/mc/720p.mp4");
        assert_eq!(m.title.as_deref(), Some("Rainy Street"));
        assert_eq!(
            m.poster_url.as_deref(),
            Some("https://i.pinimg.com/originals/aa/poster.jpg")
        );
        assert_eq!(m.pin_id.as_deref(), Some("42"));
    }

    #[test]
    fn image_pin_json_falls_back_to_orig_image() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"resource_response":{"data":{
                "grid_title":"Forest",
                "images":{"orig":{"url":"https://i.pinimg.com/originals/bb/forest.gif"}}
            }}}"#,
        )
        .unwrap();
        let m = parse_pin_json("7", &json).unwrap();
        assert_eq!(m.kind, MediaKind::Gif);
        assert_eq!(m.media_url, "https://i.pinimg.com/originals/bb/forest.gif");
        assert_eq!(m.title.as_deref(), Some("Forest"));
    }

    #[test]
    fn story_pin_hls_derives_progressive_mp4() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"resource_response":{"data":{
                "title":"Story",
                "images":{"orig":{"url":"https://i.pinimg.com/originals/aa/poster.jpg"}},
                "videos":null,
                "story_pin_data":{"pages":[{"blocks":[{"video":{"video_list":{
                    "V_HLSV3_MOBILE":{"url":"https://v1.pinimg.com/videos/iht/hls/2d/37/52/abc.m3u8","width":640,"height":480}
                }}}]}]}
            }}}"#,
        )
        .unwrap();
        let m = parse_pin_json("11", &json).unwrap();
        assert_eq!(m.kind, MediaKind::Video);
        assert_eq!(
            m.media_url,
            "https://v1.pinimg.com/videos/iht/expMp4/2d/37/52/abc_720w.mp4"
        );
    }

    #[test]
    fn html_scanner_finds_video_and_prefers_largest_width() {
        let html = concat!(
            r#"<script>{"u":"https://v1.pinimg.com/videos/mc/expMp4/a_360w.mp4","#,
            r#""v":"https://v1.pinimg.com/videos/mc/expMp4/a_720w.mp4",""#,
            r#"hls":"https://v1.pinimg.com/videos/mc/expMp4/a.m3u8"}</script>"#,
        );
        let m = scan_pin_html("9", html).unwrap();
        assert_eq!(m.kind, MediaKind::Video);
        assert_eq!(
            m.media_url,
            "https://v1.pinimg.com/videos/mc/expMp4/a_720w.mp4"
        );
        assert_eq!(m.pin_id.as_deref(), Some("9"));
    }

    #[test]
    fn html_scanner_falls_back_to_original_image() {
        let html = r#"<img src="https://i.pinimg.com/originals/cc/dd/pic.jpg" alt="x">"#;
        let m = scan_pin_html("3", html).unwrap();
        assert_eq!(m.kind, MediaKind::Image);
        assert_eq!(m.media_url, "https://i.pinimg.com/originals/cc/dd/pic.jpg");
        assert!(scan_pin_html("3", "<html>nothing here</html>").is_none());
    }
}
