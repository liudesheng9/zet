use anyhow::{Context, Result, bail};
use nom_bibtex::{Bibtex, Entry};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LiteratureMetadata {
    pub(crate) citation_key: String,
    pub(crate) title: String,
}

pub(crate) fn parse_metadata(raw: &str) -> Result<LiteratureMetadata> {
    let entries = Bibtex::raw_parse(raw).context("invalid BibTeX metadata")?;
    if entries.len() != 1 {
        bail!("BibTeX metadata must contain exactly one bibliographic entry");
    }
    let Entry::Bibliography(_, citation_key, fields) = &entries[0] else {
        bail!("BibTeX metadata must contain exactly one ordinary bibliographic entry");
    };
    validate_citation_key(citation_key)?;

    let titles = fields
        .iter()
        .filter(|field| field.key.eq_ignore_ascii_case("title"))
        .collect::<Vec<_>>();
    if titles.len() != 1 {
        bail!("BibTeX metadata must contain exactly one title field");
    }
    let mut literal_title = String::new();
    for chunk in &titles[0].value {
        let nom_bibtex::model::StringValueType::Str(value) = chunk else {
            bail!("BibTeX title must be a literal value, not a string macro");
        };
        literal_title.push_str(value);
    }
    let title = normalize_title(&literal_title);
    if title.is_empty() {
        bail!("BibTeX title must be nonempty");
    }

    Ok(LiteratureMetadata {
        citation_key: citation_key.clone(),
        title,
    })
}

pub(crate) fn validate_citation_key(key: &str) -> Result<()> {
    let valid = (1..=128).contains(&key.len())
        && key
            .bytes()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
    if !valid {
        bail!("invalid Citation key `{key}`; expected ^[A-Za-z][A-Za-z0-9_-]{{0,127}}$");
    }
    let upper = key.to_ascii_uppercase();
    let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || reserved_numbered_stem(&upper, "COM")
        || reserved_numbered_stem(&upper, "LPT");
    if reserved {
        bail!("Citation key `{key}` is a Windows reserved filename stem");
    }
    Ok(())
}

fn reserved_numbered_stem(value: &str, prefix: &str) -> bool {
    value
        .strip_prefix(prefix)
        .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'))
}

fn normalize_title(raw: &str) -> String {
    let mut output = String::new();
    let mut chars = raw.chars().peekable();
    let mut pending_space = false;
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if pending_space && !output.is_empty() {
                output.push(' ');
            }
            pending_space = false;
            output.push(ch);
            if let Some(next) = chars.next() {
                output.push(next);
            }
        } else if ch == '{' || ch == '}' {
            continue;
        } else if ch.is_whitespace() {
            pending_space = !output.is_empty();
        } else {
            if pending_space && !output.is_empty() {
                output.push(' ');
            }
            pending_space = false;
            output.push(ch);
        }
    }
    output.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_parser_preserves_identity_and_normalizes_only_the_display_title() {
        let raw = concat!(
            "@ARTICLE{Smith2024,\n",
            "  author = {Smith, Alice},\n",
            "  TiTlE = {{A}   \\LaTeX{}\n    研究},\n",
            "  year = {2024}\n",
            "}\n",
        );

        assert_eq!(
            parse_metadata(raw).unwrap(),
            LiteratureMetadata {
                citation_key: "Smith2024".to_string(),
                title: "A \\LaTeX 研究".to_string(),
            }
        );
        assert!(
            raw.ends_with("}\n"),
            "the caller retains the exact raw string"
        );
    }

    #[test]
    fn metadata_parser_rejects_non_entries_multiple_entries_and_nonliteral_titles() {
        let invalid = [
            "@comment{not a work}",
            "@preamble{\"not a work\"}",
            "@string{work_title = \"A title\"}",
            "@book{One, title={One}}\n@book{Two, title={Two}}",
            "@book{NoTitle, author={Someone}}",
            "@book{EmptyTitle, title={}}",
            "@book{MacroTitle, title=work_title}",
        ];

        for raw in invalid {
            assert!(parse_metadata(raw).is_err(), "unexpectedly accepted: {raw}");
        }
    }

    #[test]
    fn citation_keys_are_ascii_filename_safe_and_never_locations() {
        for valid in ["A", "Smith2024", "alpha-beta_2"] {
            validate_citation_key(valid).unwrap();
        }
        for invalid in [
            "1StartsWithDigit",
            "0/1",
            "has.dot",
            "has space",
            "con",
            "COM9",
            "lpt1",
        ] {
            assert!(
                validate_citation_key(invalid).is_err(),
                "unexpectedly accepted: {invalid}"
            );
        }
        assert!(validate_citation_key(&format!("A{}", "x".repeat(128))).is_err());
    }
}
