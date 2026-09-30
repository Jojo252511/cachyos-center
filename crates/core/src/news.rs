//! Arch Linux / CachyOS news shown before larger upgrades.
//!
//! V1 does not judge whether a news item is relevant. Every item newer than the
//! last acknowledgement counts as unread; unattended preparation is blocked
//! while unread news exist or when the news could not be fetched.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NewsSource {
    ArchLinux,
    CachyOs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NewsItem {
    pub source: NewsSource,
    pub title: String,
    /// `https://` link to the official announcement.
    pub link: String,
    #[ts(type = "number")]
    pub published_at: Timestamp,
    pub unread: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NewsFeedError {
    pub source: NewsSource,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NewsStatus {
    /// News fetching is disabled in the settings.
    pub disabled: bool,
    pub items: Vec<NewsItem>,
    #[ts(type = "number | null")]
    pub fetched_at: Option<Timestamp>,
    /// Feeds that could not be fetched ("News-Check nicht möglich").
    pub errors: Vec<NewsFeedError>,
    #[ts(type = "number | null")]
    pub acknowledged_until: Option<Timestamp>,
    pub unread_count: u32,
}

impl NewsStatus {
    /// The news check is complete: all feeds were fetched and nothing is unread.
    pub fn clear_for_unattended(&self) -> bool {
        !self.disabled
            && self.errors.is_empty()
            && self.fetched_at.is_some()
            && self.unread_count == 0
    }
}

/// Official feed URLs. Only these URLs are ever fetched.
pub const ARCH_NEWS_FEED: &str = "https://archlinux.org/feeds/news/";
pub const CACHYOS_NEWS_FEED: &str = "https://cachyos.org/rss.xml";

/// Only news published within this window are shown (30 days).
pub const NEWS_WINDOW_SECS: i64 = 30 * 24 * 60 * 60;
