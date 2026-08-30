use chrono::{Duration, Utc};
use email_core::models::SyncWindow;

pub struct DateWindowQuery;

impl DateWindowQuery {
    /// Formats the cutoff date for RFC 3501 IMAP query (e.g. "31-Jul-2026")
    pub fn format_imap_date(window: SyncWindow) -> Option<String> {
        let days = window.days()?;
        let cutoff = Utc::now() - Duration::days(days);
        Some(cutoff.format("%d-%b-%Y").to_string())
    }

    /// Constructs the IMAP UID SEARCH query string
    pub fn build_search_query(window: SyncWindow, last_synced_uid: u32) -> String {
        let mut parts = Vec::new();

        if let Some(date_str) = Self::format_imap_date(window) {
            parts.push(format!("SINCE {}", date_str));
        }

        if last_synced_uid > 0 {
            parts.push(format!("UID {}:*", last_synced_uid + 1));
        }

        if parts.is_empty() {
            "ALL".to_string()
        } else {
            parts.join(" ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_window_query() {
        let q = DateWindowQuery::build_search_query(SyncWindow::Days30, 0);
        assert!(q.starts_with("SINCE "));

        let q_inc = DateWindowQuery::build_search_query(SyncWindow::Days30, 500);
        assert!(q_inc.contains("SINCE "));
        assert!(q_inc.contains("UID 501:*"));


        let q_all = DateWindowQuery::build_search_query(SyncWindow::All, 0);
        assert_eq!(q_all, "ALL");
    }
}
