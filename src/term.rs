//! The one authored field of a Term Card: its exact term name (S07, #149).
//!
//! A Term Card stores the term it defines in `cards.term_name`, a nullable single-line string of
//! at most 40 Unicode scalar values. No name means the Card is a candidate Term list; a name is
//! the definition it carries. The name is authored identity, so it travels in the Lock
//! fingerprint. One parser decides what a supplied name normalizes to, and the client field
//! protocol (`None` absent, `Some(None)` clear, `Some(Some(name))` set) is shared by every route.

use anyhow::{Result, bail};
use rusqlite::{Connection, params};
use serde_json::Value;

use crate::CardType;

/// The longest term name, and the same limit a Term-link label gets: Unicode scalar values.
pub(crate) const MAX_NAME_CHARS: usize = 40;

/// The refusal for a name supplied beside another Card type.
pub(crate) const NAME_NEEDS_TERM_CARD: &str =
    "only Term Cards can have a term name; set `type` to `term` first";

/// The characters a term name cannot hold: Link macro syntax and the Term-link separator.
const FORBIDDEN: [char; 5] = ['[', ']', '@', '|', '`'];

/// Normalize one supplied term name: trim the outer whitespace, treat an empty result as no name,
/// and refuse more than one line, more than [`MAX_NAME_CHARS`] scalar values, or macro syntax.
pub(crate) fn normalize_name(raw: &str) -> Result<Option<String>> {
    let name = raw.trim();
    if name.is_empty() {
        return Ok(None);
    }
    if name.contains(['\n', '\r']) {
        bail!("a term name must be one line");
    }
    if name.chars().count() > MAX_NAME_CHARS {
        bail!("a term name holds at most {MAX_NAME_CHARS} characters");
    }
    if name.contains(FORBIDDEN) {
        bail!("a term name cannot contain `[`, `]`, `@`, `|`, or a backtick");
    }
    Ok(Some(name.to_string()))
}

/// The client field protocol: absent leaves the stored name alone, `null` or a trimmed-empty
/// string clears it, and a string sets it.
pub(crate) fn parse_field(value: Option<&Value>) -> Result<Option<Option<String>>> {
    match value {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(raw)) => Ok(Some(normalize_name(raw)?)),
        Some(_) => bail!("argument `term` must be a string"),
    }
}

/// Refuse a supplied name whose Card type is not `term`.
pub(crate) fn ensure_on_type(name: Option<&str>, card_type: CardType) -> Result<()> {
    if name.is_some() && card_type != CardType::Term {
        bail!(NAME_NEEDS_TERM_CARD);
    }
    Ok(())
}

/// Write one Card's term name inside the caller's transaction. The caller has already checked the
/// final type and the Lock gate; the table's own check backs the same rule.
pub(crate) fn set_name(conn: &Connection, row_id: i64, name: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE cards SET term_name = ?1 WHERE row_id = ?2",
        params![name, row_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_name_is_trimmed_and_an_empty_one_means_no_name() {
        assert_eq!(normalize_name(" 均衡 ").unwrap().as_deref(), Some("均衡"));
        assert_eq!(normalize_name("a b").unwrap().as_deref(), Some("a b"));
        for empty in ["", "   ", "\t\n "] {
            assert_eq!(normalize_name(empty).unwrap(), None, "{empty:?}");
        }
    }

    #[test]
    fn a_name_is_one_line_of_at_most_forty_scalar_values() {
        let forty = "概".repeat(40);
        assert_eq!(normalize_name(&forty).unwrap().as_deref(), Some(forty.as_str()));
        let forty_one = "概".repeat(41);
        assert_eq!(
            normalize_name(&forty_one).unwrap_err().to_string(),
            "a term name holds at most 40 characters"
        );
        // Counting is by Unicode scalar value, not bytes: 40 two-byte characters still fit.
        assert!(normalize_name(&"é".repeat(40)).is_ok());
        assert!(normalize_name(&"é".repeat(41)).is_err());
        assert_eq!(
            normalize_name("two\nlines").unwrap_err().to_string(),
            "a term name must be one line"
        );
        assert_eq!(
            normalize_name("carriage\rreturn").unwrap_err().to_string(),
            "a term name must be one line"
        );
    }

    #[test]
    fn a_name_holds_no_macro_or_term_link_syntax() {
        for name in ["[均衡]", "a]b", "a@b", "a|b", "a`b"] {
            assert_eq!(
                normalize_name(name).unwrap_err().to_string(),
                "a term name cannot contain `[`, `]`, `@`, `|`, or a backtick",
                "{name}"
            );
        }
        // Other Markdown punctuation is ordinary term text.
        for name in ["(均衡)", "均衡*", "a-b", "a/b", "a_b"] {
            assert!(normalize_name(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn the_client_field_distinguishes_omitted_cleared_and_set() {
        assert_eq!(parse_field(None).unwrap(), None);
        assert_eq!(parse_field(Some(&Value::Null)).unwrap(), Some(None));
        assert_eq!(parse_field(Some(&json!(""))).unwrap(), Some(None));
        assert_eq!(parse_field(Some(&json!("   "))).unwrap(), Some(None));
        assert_eq!(
            parse_field(Some(&json!(" 均衡 "))).unwrap(),
            Some(Some("均衡".to_string()))
        );
        assert_eq!(
            parse_field(Some(&json!(5))).unwrap_err().to_string(),
            "argument `term` must be a string"
        );
    }

    #[test]
    fn a_name_is_only_valid_on_a_term_card() {
        assert!(ensure_on_type(Some("均衡"), CardType::Term).is_ok());
        assert!(ensure_on_type(None, CardType::Idea).is_ok());
        assert!(ensure_on_type(None, CardType::Term).is_ok());
        for card_type in [
            CardType::Idea,
            CardType::Event,
            CardType::Finding,
            CardType::Model,
            CardType::Hub,
        ] {
            assert_eq!(
                ensure_on_type(Some("均衡"), card_type)
                    .unwrap_err()
                    .to_string(),
                NAME_NEEDS_TERM_CARD,
                "{card_type}"
            );
        }
    }
}
