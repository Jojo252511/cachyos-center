//! Arch Linux and CachyOS news (RSS).
//!
//! Only the two official feed URLs are fetched, with `curl` and fixed
//! arguments (HTTPS only, size and time limits). V1 does not rate the
//! relevance of an item: everything newer than the last acknowledgement is
//! unread.

use std::path::Path;
use std::process::{Command, Stdio};

use cachyos_center_core::news::{
    ARCH_NEWS_FEED, CACHYOS_NEWS_FEED, NEWS_WINDOW_SECS, NewsFeedError, NewsItem, NewsSource,
    NewsStatus,
};
use cachyos_center_core::{Timestamp, timefmt};
use quick_xml::Reader;
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};

/// Maximum accepted feed size.
const MAX_FEED_BYTES: &str = "4000000";
/// Maximum number of items per feed.
const MAX_ITEMS: usize = 30;

/// Parsed feed item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedItem {
    pub source: NewsSource,
    pub title: String,
    pub link: String,
    pub published_at: Timestamp,
}

/// Cached news (per user or in the helper state directory).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsCache {
    pub fetched_at: Option<Timestamp>,
    pub items: Vec<FeedItem>,
    pub errors: Vec<NewsFeedError>,
}

impl NewsCache {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).unwrap_or_default())?;
        std::fs::rename(tmp, path)
    }
}

fn feed_url(source: NewsSource) -> &'static str {
    match source {
        NewsSource::ArchLinux => ARCH_NEWS_FEED,
        NewsSource::CachyOs => CACHYOS_NEWS_FEED,
    }
}

/// Downloads a feed with curl (HTTPS only, 15 s, 4 MB).
pub fn download(source: NewsSource) -> Result<String, String> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "15",
            "--max-filesize",
            MAX_FEED_BYTES,
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--",
            feed_url(source),
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("download failed ({})", output.status)
        } else {
            err
        });
    }
    String::from_utf8(output.stdout).map_err(|_| "feed is not valid UTF-8".to_string())
}

/// Parses an RSS 2.0 document. Only `https://` links are accepted.
pub fn parse_rss(xml: &str, source: NewsSource) -> Result<Vec<FeedItem>, String> {
    let mut reader = Reader::from_str(xml);
    let mut path: Vec<String> = Vec::new();
    let mut items = Vec::new();
    let (mut title, mut link, mut date) = (String::new(), String::new(), String::new());
    let mut saw_rss = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.local_name().as_ref().to_string();
                if path.is_empty() {
                    if name != "rss" {
                        return Err("not an RSS document".into());
                    }
                    saw_rss = true;
                }
                if name == "item" {
                    title.clear();
                    link.clear();
                    date.clear();
                }
                path.push(name);
            }
            Ok(Event::End(_)) => {
                if path.last().map(String::as_str) == Some("item") {
                    let published = timefmt::parse_rfc2822(date.trim());
                    let link = link.trim();
                    if let Some(published_at) = published
                        && link.starts_with("https://")
                        && !title.is_empty()
                        && items.len() < MAX_ITEMS
                    {
                        items.push(FeedItem {
                            source,
                            title: title
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ")
                                .chars()
                                .take(300)
                                .collect(),
                            link: link.to_string(),
                            published_at,
                        });
                    }
                }
                path.pop();
            }
            Ok(Event::Text(t)) => {
                append(&path, &t.xml10_content(), &mut title, &mut link, &mut date);
            }
            Ok(Event::CData(t)) => {
                append(&path, &t.xml10_content(), &mut title, &mut link, &mut date);
            }
            Ok(Event::GeneralRef(r)) => {
                let resolved = match r.resolve_char_ref() {
                    Ok(Some(c)) => c.to_string(),
                    _ => match &*r {
                        "lt" => "<".into(),
                        "gt" => ">".into(),
                        "amp" => "&".into(),
                        "apos" => "'".into(),
                        "quot" => "\"".into(),
                        _ => String::new(),
                    },
                };
                append(&path, &resolved, &mut title, &mut link, &mut date);
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("invalid feed: {e}")),
            _ => {}
        }
    }
    if !saw_rss {
        return Err("not an RSS document".into());
    }
    Ok(items)
}

fn append(path: &[String], text: &str, title: &mut String, link: &mut String, date: &mut String) {
    let n = path.len();
    if n < 2 || path[n - 2] != "item" {
        return;
    }
    match path[n - 1].as_str() {
        "title" => title.push_str(text),
        "link" => link.push_str(text),
        "pubDate" => date.push_str(text),
        _ => {}
    }
}

/// Fetches both feeds. Items of failed feeds are kept from `previous`.
pub fn fetch_all(previous: &NewsCache, now: Timestamp) -> NewsCache {
    let mut items = Vec::new();
    let mut errors = Vec::new();
    for source in [NewsSource::ArchLinux, NewsSource::CachyOs] {
        match download(source).and_then(|xml| parse_rss(&xml, source)) {
            Ok(list) => items.extend(list),
            Err(message) => {
                errors.push(NewsFeedError { source, message });
                items.extend(
                    previous
                        .items
                        .iter()
                        .filter(|i| i.source == source)
                        .cloned(),
                );
            }
        }
    }
    items.sort_by_key(|i| std::cmp::Reverse(i.published_at));
    NewsCache {
        fetched_at: Some(now),
        items,
        errors,
    }
}

/// Status for the UI based on a cache and the acknowledgement watermark.
pub fn status(
    cache: &NewsCache,
    acknowledged_until: Option<Timestamp>,
    disabled: bool,
    now: Timestamp,
) -> NewsStatus {
    let items: Vec<NewsItem> = cache
        .items
        .iter()
        .filter(|i| now.saturating_sub(i.published_at) <= NEWS_WINDOW_SECS)
        .map(|i| NewsItem {
            source: i.source,
            title: i.title.clone(),
            link: i.link.clone(),
            published_at: i.published_at,
            unread: acknowledged_until.is_none_or(|a| i.published_at > a),
        })
        .collect();
    NewsStatus {
        disabled,
        unread_count: items.iter().filter(|i| i.unread).count() as u32,
        items,
        fetched_at: cache.fetched_at,
        errors: cache.errors.clone(),
        acknowledged_until,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0"><channel><title>Arch Linux: Recent news updates</title>
<item><title>Mkinitcpio &gt;=42 requires manual intervention</title><link>https://archlinux.org/news/mkinitcpio-42/</link><pubDate>Tue, 22 Sep 2026 09:09:27 +0000</pubDate></item>
<item><title><![CDATA[Old & plain]]></title><link>http://insecure.example/</link><pubDate>Tue, 21 Jul 2026 13:01:46 +0000</pubDate></item>
<item><title>No date</title><link>https://archlinux.org/news/x/</link></item>
</channel></rss>"#;

    #[test]
    fn parses_items() {
        let items = parse_rss(RSS, NewsSource::ArchLinux).unwrap();
        assert_eq!(
            items.len(),
            1,
            "http links and items without date are dropped"
        );
        assert_eq!(
            items[0].title,
            "Mkinitcpio >=42 requires manual intervention"
        );
        assert_eq!(items[0].link, "https://archlinux.org/news/mkinitcpio-42/");
    }

    #[test]
    fn rejects_html() {
        assert!(
            parse_rss(
                "<!DOCTYPE html><html><body>x</body></html>",
                NewsSource::CachyOs
            )
            .is_err()
        );
        assert!(parse_rss("<html><body>x</body></html>", NewsSource::CachyOs).is_err());
    }

    #[test]
    fn unread_and_window() {
        let now = timefmt::parse_rfc2822("Wed, 30 Sep 2026 12:00:00 +0000").unwrap();
        let cache = NewsCache {
            fetched_at: Some(now),
            items: vec![
                FeedItem {
                    source: NewsSource::ArchLinux,
                    title: "new".into(),
                    link: "https://a/".into(),
                    published_at: now - 3600,
                },
                FeedItem {
                    source: NewsSource::ArchLinux,
                    title: "old".into(),
                    link: "https://b/".into(),
                    published_at: now - 40 * 24 * 3600,
                },
            ],
            errors: vec![],
        };
        let s = status(&cache, None, false, now);
        assert_eq!(s.items.len(), 1);
        assert_eq!(s.unread_count, 1);
        assert!(!s.clear_for_unattended());
        let s = status(&cache, Some(now - 60), false, now);
        assert_eq!(s.unread_count, 0);
        assert!(s.clear_for_unattended());
    }

    #[test]
    #[ignore = "network access"]
    fn fetches_the_official_feeds() {
        let cache = fetch_all(&NewsCache::default(), cachyos_center_core::now());
        assert!(cache.errors.is_empty(), "{:?}", cache.errors);
        assert!(
            cache
                .items
                .iter()
                .any(|i| i.source == NewsSource::ArchLinux)
        );
        assert!(cache.items.iter().any(|i| i.source == NewsSource::CachyOs));
        assert!(cache.items.iter().all(|i| i.link.starts_with("https://")));
    }
}
