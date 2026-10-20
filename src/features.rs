//! The GUI feature switches (S28, #121; design §15-release, D380).
//!
//! The release can turn the three reader features that arrived with the Luhmann comparison on and
//! off independently: GUI search, Ask and Reply. A switch lives in the config file as one line
//! (`gui_search=off`), so an operator can turn one feature off without stopping the service, and
//! the other reader paths keep working: Cards, trees, the map, the Table and saved Spreads stay
//! readable whichever switch is off. The switches change no Card.

/// The config key of each switch, in the order `zt config show` prints them.
pub(crate) const KEYS: [&str; 3] = ["gui_search", "gui_ask", "gui_reply"];

/// Which of the three independent reader features the GUI currently offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Features {
    pub(crate) search: bool,
    pub(crate) ask: bool,
    pub(crate) reply: bool,
}

impl Default for Features {
    /// Every feature on: the default when no switch is written.
    fn default() -> Self {
        Self {
            search: true,
            ask: true,
            reply: true,
        }
    }
}

impl Features {
    /// The features the config file declares, read fresh so a switch takes effect without a
    /// restart. A missing or unreadable config file means every feature is on.
    pub(crate) fn load() -> Self {
        let text = crate::config_file()
            .ok()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .unwrap_or_default();
        Self::from_config(&text)
    }

    /// The features one config file's text declares. Only the exact word `off` (in any case)
    /// turns a feature off; every other value, malformed or missing, leaves it on.
    pub(crate) fn from_config(text: &str) -> Self {
        Self {
            search: declared(text, KEYS[0]).unwrap_or(true),
            ask: declared(text, KEYS[1]).unwrap_or(true),
            reply: declared(text, KEYS[2]).unwrap_or(true),
        }
    }

    /// The switch states as the `/api/overview` answer carries them.
    pub(crate) fn json(&self) -> serde_json::Value {
        serde_json::json!({ "search": self.search, "ask": self.ask, "reply": self.reply })
    }

    /// The switch that turns this API route off, when the route is one of the three features and
    /// that feature is off. Every other route is not gated: Cards, trees, the map, the Table and
    /// saved Spreads stay readable whichever switch is off.
    pub(crate) fn gate(&self, path: &str) -> Option<&'static str> {
        let (key, on) = match path {
            "/api/search" => (KEYS[0], self.search),
            "/api/ask" => (KEYS[1], self.ask),
            "/api/reply" => (KEYS[2], self.reply),
            _ => return None,
        };
        (!on).then_some(key)
    }
}

/// The readable name of one switch, for the API's refusal message and the CLI's output.
pub(crate) fn label(key: &str) -> &'static str {
    match key {
        "gui_search" => "GUI search",
        "gui_ask" => "Ask",
        "gui_reply" => "Reply",
        _ => "the feature",
    }
}

/// The declared state of one switch in a config file's text: `Some(false)` for the exact word
/// `off`, `Some(true)` for any other written value, `None` when the line is absent. The last
/// line wins, as it does for `archive_root`.
pub(crate) fn declared(text: &str, key: &str) -> Option<bool> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(name, _)| name.trim() == key)
        .map(|(_, value)| !value.trim().eq_ignore_ascii_case("off"))
        .next_back()
}

/// Whether `key` names one of the three switches.
pub(crate) fn is_key(key: &str) -> bool {
    KEYS.contains(&key)
}

/// The value `zt config set <key> <value>` accepts: `on` or `off`, in any case.
pub(crate) fn parse_switch(value: &str) -> anyhow::Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => anyhow::bail!("expected on or off, got `{value}`"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_feature_is_on_without_a_switch() {
        assert_eq!(Features::from_config(""), Features::default());
        assert_eq!(
            Features::from_config("archive_root=D:\\zt\n"),
            Features::default()
        );
    }

    #[test]
    fn one_switch_turns_off_only_its_own_feature() {
        let search = Features::from_config("archive_root=D:\\zt\ngui_search=off\n");
        assert!(!search.search && search.ask && search.reply);
        let ask = Features::from_config("gui_ask=off\n");
        assert!(!ask.ask && ask.search && ask.reply);
        let reply = Features::from_config("gui_reply=off");
        assert!(!reply.reply && reply.search && reply.ask);
        let every = Features::from_config("gui_search=off\ngui_ask=off\ngui_reply=off\n");
        assert_eq!(every, Features { search: false, ask: false, reply: false });
    }

    #[test]
    fn only_the_exact_word_off_turns_a_feature_off() {
        assert!(Features::from_config("gui_search=maybe").search);
        assert!(Features::from_config("gui_search=\n").search);
        assert!(Features::from_config("gui_search off").search);
        assert!(!Features::from_config(" gui_search = OFF \n").search);
        assert!(!Features::from_config("gui_search=Off\n").search);
    }

    #[test]
    fn the_change_command_accepts_only_on_or_off() {
        assert_eq!(parse_switch("on").unwrap(), true);
        assert_eq!(parse_switch("OFF").unwrap(), false);
        assert_eq!(parse_switch("  on  ").unwrap(), true);
        assert!(parse_switch("maybe").is_err());
        assert!(parse_switch("").is_err());
        assert!(is_key(KEYS[0]) && is_key(KEYS[1]) && is_key(KEYS[2]));
        assert!(!is_key("archive_root"));
    }

    #[test]
    fn each_route_is_gated_by_its_own_switch() {
        let every_off = Features { search: false, ask: false, reply: false };
        assert_eq!(every_off.gate("/api/search"), Some(KEYS[0]));
        assert_eq!(every_off.gate("/api/ask"), Some(KEYS[1]));
        assert_eq!(every_off.gate("/api/reply"), Some(KEYS[2]));
        for path in ["/api/card", "/api/spreads", "/api/graph", "/api/overview", "/api/hub/draft"] {
            assert_eq!(every_off.gate(path), None, "{path} is not one of the three switches");
        }
        let search_off = Features::from_config("gui_search=off");
        assert_eq!(search_off.gate("/api/search"), Some(KEYS[0]));
        assert_eq!(search_off.gate("/api/ask"), None);
        assert_eq!(search_off.gate("/api/reply"), None);
    }
}

