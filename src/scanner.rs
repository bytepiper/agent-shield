use regex::Regex;

use crate::events::Alert;

pub(crate) struct Scanner(pub(crate) Vec<(String, Regex, String)>);

impl Scanner {
    pub(crate) fn new() -> Self {
        let patterns = vec![
            ("aws-key", r"AKIA[0-9A-Z]{16}", "block"),
            ("private-key", r"-----BEGIN.+PRIVATE KEY-----", "block"),
            ("github-token", r"ghp_[a-zA-Z0-9]{36}", "block"),
            ("stripe-key", r"sk_live_[a-zA-Z0-9]{24,}", "block"),
        ];
        Self(
            patterns
                .into_iter()
                .filter_map(|(name, regex, action)| {
                    Regex::new(regex)
                        .ok()
                        .map(|re| (name.into(), re, action.into()))
                })
                .collect(),
        )
    }

    pub(crate) fn scan(&self, body: &[u8]) -> Vec<Alert> {
        let text = String::from_utf8_lossy(body);
        self.0
            .iter()
            .flat_map(|(name, re, action)| {
                re.find_iter(&text).take(2).map(|m| {
                    let matched = m.as_str();
                    let truncated = if matched.len() > 50 {
                        format!("{}...", &matched[..40])
                    } else {
                        matched.into()
                    };
                    Alert {
                        pattern: name.clone(),
                        action: action.clone(),
                        matched: truncated,
                    }
                })
            })
            .collect()
    }

    pub(crate) fn blocks(alerts: &[Alert]) -> bool {
        alerts.iter().any(|alert| alert.action == "block")
    }
}
