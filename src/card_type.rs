//! Card types: what a Card holds.
//!
//! Most Cards are Idea Cards. A Regular Card may instead be an Event Card,
//! which records one real-world event, a Finding Card, which states one
//! empirical result from a source, a Model Card, which presents one whole
//! model, a Hub Card, which opens one subject, or a Term Card, which defines
//! one term. The type is stored on the Card row, so it stays with the Card when
//! it moves; Topic and Literature Cards are always Idea Cards.

use anyhow::{Result, bail};

use crate::Card;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum CardType {
    #[default]
    Idea,
    Event,
    Finding,
    Model,
    Hub,
    Term,
}

impl CardType {
    /// The stored and typed name.
    pub(crate) fn id(self) -> &'static str {
        match self {
            CardType::Idea => "idea",
            CardType::Event => "event",
            CardType::Finding => "finding",
            CardType::Model => "model",
            CardType::Hub => "hub",
            CardType::Term => "term",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "idea" => Ok(CardType::Idea),
            "event" => Ok(CardType::Event),
            "finding" => Ok(CardType::Finding),
            "model" => Ok(CardType::Model),
            "hub" => Ok(CardType::Hub),
            "term" => Ok(CardType::Term),
            other => bail!(
                "unknown card type `{other}`; expected idea, event, finding, model, hub, or term"
            ),
        }
    }

    /// Read a stored value; anything unexpected is treated as an idea.
    pub(crate) fn from_stored(value: &str) -> Self {
        CardType::parse(value).unwrap_or_default()
    }
}

impl std::fmt::Display for CardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// Only Regular Cards may be Event, Finding, Model, Hub, or Term Cards.
pub(crate) fn ensure_allowed(card: &Card, card_type: CardType) -> Result<()> {
    if card_type != CardType::Idea && crate::is_tree_root(card) {
        bail!("only Regular Cards can be Event, Finding, Model, Hub, or Term Cards");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_type_names_parse_and_round_trip() {
        for card_type in [
            CardType::Idea,
            CardType::Event,
            CardType::Finding,
            CardType::Model,
            CardType::Hub,
            CardType::Term,
        ] {
            assert_eq!(CardType::parse(card_type.id()).unwrap(), card_type);
        }
        assert_eq!(CardType::parse(" Event ").unwrap(), CardType::Event);
        assert_eq!(CardType::parse("Finding").unwrap(), CardType::Finding);
        assert_eq!(CardType::parse("TERM ").unwrap(), CardType::Term);
        assert_eq!(
            CardType::parse("fact").unwrap_err().to_string(),
            "unknown card type `fact`; expected idea, event, finding, model, hub, or term"
        );
        assert_eq!(CardType::from_stored("bogus"), CardType::Idea);
        assert_eq!(CardType::default(), CardType::Idea);
    }
}
