//! The Term content contract (S09, #152): the proposal draft JSON, its deterministic Card
//! rendering, the continuation partition, and the template both the renderer and the T rules read.
//!
//! The design (§11, R0277–R0302) splits the work in two. DeepSeek supplies content as the draft
//! JSON of the proposal's Appendix C; this module validates every field of that schema and renders
//! the Markdown it must become. The compiler never asks a writer for Card layout. The same
//! template parse here is what the checker's T rules read, so a rendered draft and a manually
//! edited stored Card go through one evaluator.
//!
//! Nothing here reads a file, starts a process, or talks to an archive, so the renderer is
//! available to zt-check, zt-digest, and the zt service's in-process gate.

use std::collections::BTreeSet;
use std::fmt;

use serde_json::{Map, Value};

use crate::core::Limits;
use crate::text::{
    TermRow, measure, prose_lines, split_sentences, title_fits, utf16_form,
};

// --- the template -----------------------------------------------------------------------------

/// The exact category labels the authored `类别：` line carries.
pub const CATEGORY_CONCEPT: &str = "概念";
pub const CATEGORY_MATH: &str = "数学概念";

/// The two head-line labels of a continuation part: its category and its link to the head.
pub const CONTINUATION_LABEL: &str = "续";
/// The authored opt-out contract (design 9.4, R0231): absence means automatic linking is enabled.
pub const AUTO_LINK_LABEL: &str = "自动链接";
pub const AUTO_LINK_REASON_LABEL: &str = "自动链接理由";
/// The one value the `自动链接：` line may carry.
pub const AUTO_LINK_OFF: &str = "否";

/// The head labels, in template order. `原文` is optional; `自动链接` and its reason are the
/// authored opt-out pair, present only together.
pub const HEAD_LABELS: [&str; 7] = [
    "术语",
    "原文",
    "不用",
    "类别",
    "首次定义",
    AUTO_LINK_LABEL,
    AUTO_LINK_REASON_LABEL,
];

/// The part head labels, in template order.
pub const PART_LABELS: [&str; 2] = ["类别", CONTINUATION_LABEL];

/// The source trailer label.
pub const SOURCE_LABEL: &str = "来源";

/// The fixed section order of the Term template (design 11.1). `推导` may repeat; its heading
/// carries the claim after `：`.
pub const SECTIONS: [&str; 11] = [
    "释义",
    "书中的说法",
    "数学定义",
    "等价形式",
    "推导",
    "极限与特例",
    "例子",
    "直觉",
    "区别",
    "常见误读",
    "在本书中的作用",
];

/// The sections a concept term must not carry (the formal derivation sections, R0293).
pub const FORMAL_SECTIONS: [&str; 4] = ["数学定义", "等价形式", "推导", "极限与特例"];

/// The continuation title prefix. A part's title is `推导（续 k/n）：<label>` (D14, R0047).
pub const CONTINUATION_PREFIX: &str = "推导（续 ";

/// The line that says a derivation continues on the next Card (R0289): a Card holds at most 12
/// steps, so a longer derivation is split at a whole step boundary, and the chunk that is not the
/// last names its continuation. T5 accepts it in place of the final `结果：` line.
pub const CONTINUES: &str = "（续见下卡）";

/// The two categories of one term (design 11.1).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    Concept,
    Math,
}

impl Category {
    /// The authored `类别：` value.
    pub fn label(self) -> &'static str {
        match self {
            Category::Concept => CATEGORY_CONCEPT,
            Category::Math => CATEGORY_MATH,
        }
    }

    /// Read the draft's `category` value: exactly `concept` or `math`.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "concept" => Some(Category::Concept),
            "math" => Some(Category::Math),
            _ => None,
        }
    }

    /// Read an authored `类别：` value: exactly `概念` or `数学概念`.
    pub fn from_label(value: &str) -> Option<Self> {
        match value.trim() {
            CATEGORY_CONCEPT => Some(Category::Concept),
            CATEGORY_MATH => Some(Category::Math),
            _ => None,
        }
    }
}

// --- the draft schema (proposal Appendix C) ---------------------------------------------------

/// One Draft fault: a JSON path and what is wrong there. The writer fixes content, so the path
/// names the exact field rather than a rendered line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftFault {
    pub path: String,
    pub message: String,
}

impl fmt::Display for DraftFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// Every fault of one draft file. The check step reports all of them, not the first, so one
/// writer fix can satisfy the schema (design 12.3).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftFaults(pub Vec<DraftFault>);

impl fmt::Display for DraftFaults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, fault) in self.0.iter().enumerate() {
            if index > 0 {
                write!(f, "; ")?;
            }
            write!(f, "{fault}")?;
        }
        Ok(())
    }
}

impl std::error::Error for DraftFaults {}

impl DraftFaults {
    fn one(path: &str, message: impl Into<String>) -> Self {
        Self(vec![DraftFault {
            path: path.to_string(),
            message: message.into(),
        }])
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BookDefinition {
    pub text: Vec<String>,
    pub section: String,
    pub quote: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hypothesis {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Definition {
    pub formula: String,
    pub reading: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Symbol {
    pub symbol: String,
    pub meaning: String,
    pub r#type: String,
    pub auxiliary: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EquivalentForm {
    pub formula: String,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step {
    pub text: String,
    pub basis: String,
    pub formula: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Derivation {
    pub claim: String,
    pub steps: Vec<Step>,
    pub result: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LimitCase {
    pub case: String,
    pub text: String,
    pub formula: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Distinction {
    pub term: String,
    pub text: String,
}

/// One term's content, exactly the fields the proposal's Appendix C schema defines (R0302,
/// AC10). No field is optional in the type: an absent one is a fault, not a silent default.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Draft {
    pub term: String,
    pub original: String,
    pub category: Category,
    pub link: bool,
    pub link_reason: String,
    pub first_defined: String,
    pub explanation: Vec<String>,
    pub book_definition: BookDefinition,
    pub hypotheses: Vec<Hypothesis>,
    pub definition: Definition,
    pub symbols: Vec<Symbol>,
    pub equivalent_forms: Vec<EquivalentForm>,
    pub derivations: Vec<Derivation>,
    pub limits: Vec<LimitCase>,
    pub example: Vec<String>,
    pub intuition: Vec<String>,
    pub distinctions: Vec<Distinction>,
    pub misreadings: Vec<String>,
    pub role_in_book: Vec<String>,
}

/// The known keys of each draft object, so a misspelt or extra field is refused rather than
/// silently dropped (AC09: every field is rendered, used, or validated).
const ROOT_KEYS: [&str; 19] = [
    "term",
    "original",
    "category",
    "link",
    "link_reason",
    "first_defined",
    "explanation",
    "book_definition",
    "hypotheses",
    "definition",
    "symbols",
    "equivalent_forms",
    "derivations",
    "limits",
    "example",
    "intuition",
    "distinctions",
    "misreadings",
    "role_in_book",
];

/// Parse the proposal's draft document (`{"terms":[…]}`) and validate every field. `rows` are the
/// exact table-row spellings of the book, so a draft cannot name a term, or a neighbouring term in
/// a distinction, that no row holds (design 9.3, R0292).
pub fn parse_terms(value: &Value, rows: &[String]) -> Result<Vec<Draft>, DraftFaults> {
    let mut reader = Reader::default();
    let Some(root) = value.as_object() else {
        return Err(DraftFaults::one("draft", "must be a JSON object with a `terms` array"));
    };
    let terms = reader.array(root, "terms", "draft", true);
    let mut drafts = Vec::new();
    for (index, item) in terms.iter().enumerate() {
        let path = format!("terms[{index}]");
        if let Some(draft) = Draft::read(item, &path, rows, &mut reader) {
            drafts.push(draft);
        }
    }
    if reader.faults.is_empty() {
        Ok(drafts)
    } else {
        Err(DraftFaults(reader.faults))
    }
}

/// Read one Term Card's content contract (`{"term":…,…}`, Appendix C).
pub fn parse_draft(value: &Value, rows: &[String]) -> Result<Draft, DraftFaults> {
    let mut reader = Reader::default();
    let draft = Draft::read(value, "term", rows, &mut reader);
    match (draft, reader.faults.is_empty()) {
        (Some(draft), true) => Ok(draft),
        (_, false) => Err(DraftFaults(reader.faults)),
        (None, true) => Err(DraftFaults::one("term", "must be a JSON object")),
    }
}

impl Draft {
    fn read(value: &Value, path: &str, rows: &[String], r: &mut Reader) -> Option<Self> {
        let object = r.object(Some(value), path)?;
        r.known(object, &ROOT_KEYS, path);
        let term = r.string(object, "term", path, true);
        let term = term?;
        r.subject = term.clone();
        if !rows.is_empty() && !rows.iter().any(|row| row == &term) {
            r.fault(
                &format!("{path}.term"),
                format!("`{term}` is no row of the book's term table"),
            );
        }
        let category = match r.string(object, "category", path, true) {
            Some(raw) => match Category::parse(&raw) {
                Some(category) => category,
                None => {
                    r.fault(
                        &format!("{path}.category"),
                        format!("`{raw}` is no category; expected `concept` or `math`"),
                    );
                    Category::Concept
                }
            },
            None => Category::Concept,
        };
        let link = match object.get("link") {
            None => true,
            Some(Value::Bool(flag)) => *flag,
            Some(_) => {
                r.fault(&format!("{path}.link"), "must be true or false");
                true
            }
        };
        let link_reason = r.string(object, "link_reason", path, false).unwrap_or_default();
        if !link {
            if link_reason.is_empty() {
                r.fault(
                    &format!("{path}.link_reason"),
                    "a term with `link: false` must say why",
                );
            }
        } else if !link_reason.is_empty() {
            r.fault(
                &format!("{path}.link_reason"),
                "a reason is used only when `link` is false; set `link` to false or drop it",
            );
        }
        let explanation = r.sentences(object, "explanation", path, 3, 8, true);
        let book_definition = object
            .get("book_definition")
            .and_then(|value| {
                let at = format!("{path}.book_definition");
                let book = r.object(Some(value), &at)?;
                r.known(book, &["text", "section", "quote"], &at);
                Some(BookDefinition {
                    text: r.sentences(book, "text", &at, 1, 4, true),
                    section: r.string(book, "section", &at, true).unwrap_or_default(),
                    quote: r.string(book, "quote", &at, false).unwrap_or_default(),
                })
            })
            .unwrap_or_default();
        if !book_definition.quote.is_empty() && split_sentences(&book_definition.quote).len() > 1 {
            r.fault(
                &format!("{path}.book_definition.quote"),
                "a quotation is at most one sentence",
            );
        }
        let hypotheses = read_objects(object, "hypotheses", path, r, &["id", "text"], |item, at, r| {
            Some(Hypothesis {
                id: r.string(item, "id", at, true)?,
                text: r.string(item, "text", at, true)?,
            })
        });
        let mut seen_ids = BTreeSet::new();
        for (index, hypothesis) in hypotheses.iter().enumerate() {
            if !valid_hypothesis_id(&hypothesis.id) {
                r.fault(
                    &format!("{path}.hypotheses[{index}].id"),
                    format!("`{}` is no labelled hypothesis; expected H1, H2, …", hypothesis.id),
                );
            } else if !seen_ids.insert(hypothesis.id.clone()) {
                r.fault(
                    &format!("{path}.hypotheses[{index}].id"),
                    format!("`{}` is used twice", hypothesis.id),
                );
            }
        }
        let mut definition = Definition::default();
        let mut has_definition = false;
        if let Some(value) = object.get("definition") {
            let at = format!("{path}.definition");
            if let Some(item) = r.object(Some(value), &at) {
                r.known(item, &["formula", "reading"], &at);
                has_definition = true;
                definition = Definition {
                    formula: r.string(item, "formula", &at, true).unwrap_or_default(),
                    reading: r.sentences(item, "reading", &at, 1, 3, true),
                };
                if !definition.formula.contains("\\tag") {
                    r.fault(
                        &format!("{at}.formula"),
                        "the definition is a display formula with a tag; add `\\tag{T1}`",
                    );
                }
            }
        }
        let symbols = read_objects(
            object,
            "symbols",
            path,
            r,
            &["symbol", "meaning", "type", "auxiliary"],
            |item, at, r| {
                Some(Symbol {
                    symbol: r.string(item, "symbol", at, true)?,
                    meaning: r.string(item, "meaning", at, true)?,
                    r#type: r.string(item, "type", at, true)?,
                    auxiliary: match item.get("auxiliary") {
                        None => false,
                        Some(Value::Bool(flag)) => *flag,
                        Some(_) => {
                            r.fault(&format!("{at}.auxiliary"), "must be true or false");
                            false
                        }
                    },
                })
            },
        );
        let equivalent_forms = read_objects(
            object,
            "equivalent_forms",
            path,
            r,
            &["formula", "reason"],
            |item, at, r| {
                Some(EquivalentForm {
                    formula: r.string(item, "formula", at, false).unwrap_or_default(),
                    reason: r.string(item, "reason", at, true)?,
                })
            },
        );
        let derivations = read_objects(
            object,
            "derivations",
            path,
            r,
            &["claim", "steps", "result"],
            |item, at, r| {
                let steps = read_objects(
                    item,
                    "steps",
                    at,
                    r,
                    &["text", "basis", "formula"],
                    |step, at, r| {
                        Some(Step {
                            text: r.string(step, "text", at, true)?,
                            basis: r.string(step, "basis", at, true)?,
                            formula: r.string(step, "formula", at, false).unwrap_or_default(),
                        })
                    },
                );
                if steps.is_empty() {
                    r.fault(&format!("{at}.steps"), "a derivation holds at least one step");
                }
                Some(Derivation {
                    claim: r.string(item, "claim", at, true)?,
                    steps,
                    result: r.string(item, "result", at, true)?,
                })
            },
        );
        let limits = read_objects(
            object,
            "limits",
            path,
            r,
            &["case", "text", "formula"],
            |item, at, r| {
                Some(LimitCase {
                    case: r.string(item, "case", at, true)?,
                    text: r.string(item, "text", at, true)?,
                    formula: r.string(item, "formula", at, false).unwrap_or_default(),
                })
            },
        );
        let example = r.sentences(object, "example", path, 0, 0, false);
        let intuition = r.sentences(object, "intuition", path, 0, 0, false);
        let distinctions = read_objects(
            object,
            "distinctions",
            path,
            r,
            &["term", "text"],
            |item, at, r| {
                let term = r.string(item, "term", at, true)?;
                if !rows.is_empty() && !rows.iter().any(|row| row == &term) {
                    r.fault(
                        &format!("{at}.term"),
                        format!("`{term}` is no row of the book's term table"),
                    );
                }
                if !term.is_empty() && term == r.subject {
                    r.fault(&format!("{at}.term"), "a term cannot be its own neighbour");
                }
                Some(Distinction {
                    term,
                    text: r.string(item, "text", at, true)?,
                })
            },
        );
        let misreadings = r.sentences(object, "misreadings", path, 0, 0, false);
        let role_in_book = r.sentences(object, "role_in_book", path, 1, 4, true);

        // The category decides which fields are required and which are refused (R0280, R0293).
        match category {
            Category::Math => {
                if hypotheses.is_empty() {
                    r.fault(&format!("{path}.hypotheses"), "a mathematical term states its hypotheses");
                }
                if !has_definition {
                    r.fault(&format!("{path}.definition"), "a mathematical term has a formal definition");
                }
                if symbols.is_empty() {
                    r.fault(&format!("{path}.symbols"), "a mathematical term has a symbol table");
                }
                if equivalent_forms.is_empty() {
                    r.fault(
                        &format!("{path}.equivalent_forms"),
                        "a mathematical term says every equivalent form, or that the book gives none",
                    );
                }
                if derivations.is_empty() {
                    r.fault(&format!("{path}.derivations"), "a mathematical term carries its derivation");
                }
                if limits.is_empty() {
                    r.fault(
                        &format!("{path}.limits"),
                        "a mathematical term states its limit cases, or that it has none",
                    );
                }
                if example.is_empty() {
                    r.fault(&format!("{path}.example"), "a mathematical term carries a computed example");
                }
                if intuition.len() < 2 || intuition.len() > 5 {
                    r.fault(
                        &format!("{path}.intuition"),
                        "a mathematical term carries 2 to 5 sentences of intuition",
                    );
                }
                if misreadings.is_empty() {
                    r.fault(&format!("{path}.misreadings"), "a mathematical term names its common misreadings");
                }
            }
            Category::Concept => {
                for (name, present) in [
                    ("hypotheses", !hypotheses.is_empty()),
                    ("definition", has_definition),
                    ("symbols", !symbols.is_empty()),
                    ("equivalent_forms", !equivalent_forms.is_empty()),
                    ("derivations", !derivations.is_empty()),
                    ("limits", !limits.is_empty()),
                    ("example", !example.is_empty()),
                    ("intuition", !intuition.is_empty()),
                ] {
                    if present {
                        r.fault(
                            &format!("{path}.{name}"),
                            "a concept term carries no formal mathematics; set the category to math or drop the field",
                        );
                    }
                }
            }
        }
        if distinctions.is_empty() {
            r.fault(&format!("{path}.distinctions"), "every term says how it differs from its neighbours");
        }
        Some(Self {
            term,
            original: r.string(object, "original", path, false).unwrap_or_default(),
            category,
            link,
            link_reason,
            first_defined: r.string(object, "first_defined", path, true)?,
            explanation,
            book_definition,
            hypotheses,
            definition,
            symbols,
            equivalent_forms,
            derivations,
            limits,
            example,
            intuition,
            distinctions,
            misreadings,
            role_in_book,
        })
    }
}

/// The LaTeX hypothesis id grammar: `H1`, `H2`, … (design 11.1).
fn valid_hypothesis_id(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next() == Some('H')
        && chars.next().is_some_and(|ch| ('1'..='9').contains(&ch))
        && chars.all(|ch| ch.is_ascii_digit())
}

fn read_objects<T>(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
    r: &mut Reader,
    keys: &[&str],
    mut item: impl FnMut(&Map<String, Value>, &str, &mut Reader) -> Option<T>,
) -> Vec<T> {
    let at = format!("{path}.{key}");
    let mut out = Vec::new();
    for (index, value) in r.array(object, key, path, false).iter().enumerate() {
        let item_path = format!("{at}[{index}]");
        if let Some(value) = r.object(Some(value), &item_path) {
            r.known(value, keys, &item_path);
            if let Some(read) = item(value, &item_path, r) {
                out.push(read);
            }
        }
    }
    out
}

/// One reader accumulates every fault of a draft instead of stopping at the first.
#[derive(Default)]
struct Reader {
    faults: Vec<DraftFault>,
    /// The draft's term name, so a distinction can refuse to name its own Card.
    subject: String,
}

impl Reader {
    fn fault(&mut self, path: &str, message: impl Into<String>) {
        self.faults.push(DraftFault {
            path: path.to_string(),
            message: message.into(),
        });
    }

    /// Refuse every key the schema does not define, so a field is never silently dropped.
    fn known(&mut self, object: &Map<String, Value>, keys: &[&str], path: &str) {
        for key in object.keys() {
            if !keys.contains(&key.as_str()) {
                self.fault(&format!("{path}.{key}"), "is not a draft field");
            }
        }
    }

    fn object<'a>(&mut self, value: Option<&'a Value>, path: &str) -> Option<&'a Map<String, Value>> {
        match value {
            Some(Value::Object(object)) => Some(object),
            Some(_) => {
                self.fault(path, "must be a JSON object");
                None
            }
            None => {
                self.fault(path, "is required");
                None
            }
        }
    }

    fn string(
        &mut self,
        object: &Map<String, Value>,
        key: &str,
        path: &str,
        required: bool,
    ) -> Option<String> {
        let at = format!("{path}.{key}");
        let value = match object.get(key) {
            Some(Value::String(text)) => text.clone(),
            Some(Value::Null) | None => String::new(),
            Some(_) => {
                self.fault(&at, "must be a string");
                String::new()
            }
        };
        let value = value.trim().to_string();
        if value.is_empty() && required {
            self.fault(&at, "is required");
            return None;
        }
        if value.contains(['\n', '\r']) {
            self.fault(&at, "must be one line");
        }
        Some(value)
    }

    fn array<'a>(
        &mut self,
        object: &'a Map<String, Value>,
        key: &str,
        path: &str,
        required: bool,
    ) -> &'a [Value] {
        match object.get(key) {
            Some(Value::Array(items)) => items,
            Some(Value::Null) | None => {
                if required {
                    self.fault(&format!("{path}.{key}"), "is required");
                }
                &[]
            }
            Some(_) => {
                self.fault(&format!("{path}.{key}"), "must be an array");
                &[]
            }
        }
    }

    /// One array of one-line sentences. `max == 0` means no bound beyond the minimum.
    fn sentences(
        &mut self,
        object: &Map<String, Value>,
        key: &str,
        path: &str,
        min: usize,
        max: usize,
        required: bool,
    ) -> Vec<String> {
        let at = format!("{path}.{key}");
        let mut out = Vec::new();
        for (index, value) in self.array(object, key, path, required).iter().enumerate() {
            let item = match value {
                Value::String(text) => text.trim().to_string(),
                _ => {
                    self.fault(&format!("{at}[{index}]"), "must be a string");
                    continue;
                }
            };
            if item.is_empty() {
                self.fault(&format!("{at}[{index}]"), "must not be empty");
                continue;
            }
            if item.contains(['\n', '\r']) {
                self.fault(&format!("{at}[{index}]"), "must be one line");
            }
            out.push(item);
        }
        if out.len() < min {
            self.fault(&at, format!("needs at least {min} sentence(s), has {}", out.len()));
        }
        if max > 0 && out.len() > max {
            self.fault(&at, format!("holds at most {max} sentences, has {}", out.len()));
        }
        out
    }
}

// --- the template a rendered or stored Card carries -------------------------------------------

/// One recognized head line: its label, value, and 1-based body line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateLine {
    pub label: String,
    pub value: String,
    pub line: usize,
}

/// One `##` section: its canonical name, its full heading, and its content lines.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateSection {
    pub name: String,
    pub heading: String,
    pub line: usize,
    pub content: Vec<String>,
}

/// The template structure of one authored Term Card body (head or part). It is derived from the
/// text alone; a fault names the line, never the Card field.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Template {
    pub lines: Vec<TemplateLine>,
    pub sections: Vec<TemplateSection>,
    /// Nonblank lines before the first section that are no recognized head line.
    pub unknown_lines: Vec<(usize, String)>,
    /// `##` headings outside the known template.
    pub unknown_sections: Vec<(usize, String)>,
    /// The `来源：` line, when it is the body's last nonblank line.
    pub trailer: Option<TemplateLine>,
    /// A `来源：` line anywhere before the last nonblank line.
    pub misplaced_trailer: bool,
    /// A nonblank line after the trailer.
    pub after_trailer: bool,
}

impl Template {
    /// One head line, if present.
    pub fn line(&self, label: &str) -> Option<&TemplateLine> {
        self.lines.iter().find(|line| line.label == label)
    }

    /// Every section with one canonical name, in source order.
    pub fn named(&self, name: &str) -> Vec<&TemplateSection> {
        self.sections
            .iter()
            .filter(|section| section.name == name)
            .collect()
    }

    /// The one section with this name, when it appears exactly once.
    pub fn one(&self, name: &str) -> Option<&TemplateSection> {
        let mut found = self.named(name);
        (found.len() == 1).then(|| found.remove(0))
    }

    /// The `符号表` table under the math definition, as `(symbol, meaning, type)` rows.
    pub fn symbol_table(&self) -> Vec<(String, String, String)> {
        let Some(section) = self.one("数学定义") else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        let mut inside = false;
        for line in &section.content {
            let trimmed = line.trim();
            if trimmed == "符号表" || trimmed == "**符号表**" {
                inside = true;
                continue;
            }
            if !inside {
                continue;
            }
            if !trimmed.starts_with('|') {
                if !trimmed.is_empty() {
                    break;
                }
                continue;
            }
            if let Some(cells) = table_cells(trimmed) {
                if cells[0] == "符号"
                    || cells
                        .iter()
                        .all(|cell| cell.starts_with('-') || cell.starts_with(':'))
                {
                    continue;
                }
                if cells.len() >= 3 && !cells[0].is_empty() {
                    rows.push((cells[0].clone(), cells[1].clone(), cells[2].clone()));
                }
            }
        }
        rows
    }
}

/// A table line's cells, trimmed; `None` for a line that is no table row.
fn table_cells(line: &str) -> Option<Vec<String>> {
    if !line.starts_with('|') {
        return None;
    }
    Some(
        line.trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect(),
    )
}

/// Parse one authored Term Card body into the template structure (head labels, ordered sections,
/// and the source trailer). `body` may be raw or in UTF-16 form; a fence hides its contents.
pub fn parse_template(body: &str) -> Template {
    let mut template = Template::default();
    let lines: Vec<&str> = body.lines().collect();
    let mut fenced = false;
    let mut headings: Vec<(usize, String)> = Vec::new();
    let mut trailer_at: Option<usize> = None;
    let mut misplaced = false;
    let last = lines.iter().rposition(|line| !line.trim().is_empty());
    for (index, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(heading) = trimmed.strip_prefix("## ") {
            headings.push((index, heading.trim().to_string()));
        }
        if (trimmed.starts_with("来源：") || trimmed.starts_with("来源:")) && trailer_at.is_none() {
            if Some(index) == last {
                trailer_at = Some(index);
            } else {
                misplaced = true;
            }
        }
    }
    let first_section = headings.first().map(|(index, _)| *index).unwrap_or(lines.len());
    for (index, raw) in lines.iter().enumerate().take(first_section) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        match head_line(trimmed) {
            Some(line) => template.lines.push(TemplateLine {
                label: line.0,
                value: line.1,
                line: index + 1,
            }),
            None => template.unknown_lines.push((index + 1, trimmed.to_string())),
        }
    }
    for (position, (index, heading)) in headings.iter().enumerate() {
        let end = headings
            .get(position + 1)
            .map(|(next, _)| *next)
            .unwrap_or_else(|| trailer_at.unwrap_or(lines.len()));
        let name = section_name(heading);
        let content = lines[index + 1..end]
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        if name.is_none() {
            template
                .unknown_sections
                .push((index + 1, heading.to_string()));
        }
        template.sections.push(TemplateSection {
            name: name.unwrap_or_else(|| heading.clone()),
            heading: heading.clone(),
            line: index + 1,
            content,
        });
    }
    if let Some(index) = trailer_at {
        let trimmed = lines[index].trim();
        let value = trimmed
            .strip_prefix("来源：")
            .or_else(|| trimmed.strip_prefix("来源:"))
            .unwrap_or_default()
            .trim()
            .to_string();
        template.trailer = Some(TemplateLine {
            label: SOURCE_LABEL.to_string(),
            value,
            line: index + 1,
        });
        template.after_trailer = lines[index + 1..]
            .iter()
            .any(|line| !line.trim().is_empty());
    }
    template.misplaced_trailer = misplaced;
    template
}

/// The canonical section name of one `##` heading. A derivation heading is `推导：<claim>`.
fn section_name(heading: &str) -> Option<String> {
    if heading == "推导" || heading.starts_with("推导：") {
        return Some("推导".to_string());
    }
    SECTIONS
        .iter()
        .find(|name| **name == heading)
        .map(|name| (*name).to_string())
}

/// A recognized `<label>：<value>` line.
pub fn head_line(trimmed: &str) -> Option<(String, String)> {
    let (label, value) = trimmed.split_once('：')?;
    if label.is_empty() || label.chars().count() > 12 {
        return None;
    }
    let known = HEAD_LABELS.contains(&label) || PART_LABELS.contains(&label);
    known.then(|| (label.to_string(), value.trim().to_string()))
}

/// The numbered steps of one `## 推导` section, in source order. A step is a numbered line and
/// every following line until the next numbered line or a section boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepBlock {
    pub number: i64,
    pub lines: Vec<String>,
}

impl StepBlock {
    /// Whether the step holds a display formula.
    pub fn has_formula(&self) -> bool {
        self.lines.iter().any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("$$") && trimmed.ends_with("$$") && trimmed.len() > 4
        })
    }

    /// The step's basis after `依据：`, when it names one (R0295).
    pub fn basis(&self) -> Option<&str> {
        self.lines
            .iter()
            .find_map(|line| line.split_once("依据：").map(|(_, rest)| rest.trim()))
    }

    /// The step's own text: its lines before the basis.
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|line| line.split_once("依据：").map_or(line.trim(), |(before, _)| before.trim()))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Parse the numbered steps of one derivation section. Lines outside a numbered step are not a
/// step; the result line (`结果：`) belongs to no step.
pub fn step_blocks(section: &TemplateSection) -> Vec<StepBlock> {
    let mut steps: Vec<StepBlock> = Vec::new();
    for line in &section.content {
        let trimmed = line.trim();
        if trimmed.starts_with("结果：") {
            continue;
        }
        if let Some(number) = numbered_step(trimmed) {
            steps.push(StepBlock {
                number,
                lines: vec![line.clone()],
            });
        } else if let Some(last) = steps.last_mut()
            && !trimmed.is_empty()
        {
            last.lines.push(line.clone());
        }
    }
    steps
}

/// The number of a line that opens a derivation step: `1.`, `2．`, `3)`, or `4、`.
fn numbered_step(trimmed: &str) -> Option<i64> {
    let digits = trimmed
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .count();
    if digits == 0 || digits > 3 {
        return None;
    }
    let rest = trimmed[digits..].trim_start();
    let marker = rest.chars().next()?;
    if !matches!(marker, '.' | '．' | ')' | '）' | '、') {
        return None;
    }
    trimmed[..digits].parse().ok()
}

/// The `结果：` line of one derivation section, if present.
pub fn result_line(section: &TemplateSection) -> Option<String> {
    section.content.iter().find_map(|line| {
        line.trim()
            .strip_prefix("结果：")
            .map(str::trim)
            .map(str::to_string)
    })
}

/// The prose units of one Card by the shared measurement: every prose sentence, formulas and
/// tables excluded (design 11.2, R0289; T9). A source line is a locator, not prose.
pub fn prose_units(body: &str) -> i64 {
    let body = utf16_form(body);
    let mut total = 0;
    for line in prose_lines(&body) {
        if is_source_line(&line.text) {
            continue;
        }
        for sentence in split_sentences(&line.text) {
            total += measure(&sentence).size;
        }
    }
    total
}

fn is_source_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("来源：") || trimmed.starts_with("来源:")
}

/// Every LaTeX identifier a formula text names, normalized for comparison with a symbol table
/// (T4, advisory because the scan is approximate). Standard operators and layout commands are
/// ignored; Greek commands, `\mathcal`-style names, and single letters are identifiers.
pub fn latex_identifiers(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '\\' {
            let start = index + 1;
            let mut end = start;
            while end < chars.len() && chars[end].is_ascii_alphabetic() {
                end += 1;
            }
            if end == start {
                index += 1;
                continue;
            }
            let name = chars[start..end].iter().collect::<String>();
            let lower = name.to_lowercase();
            if matches!(
                lower.as_str(),
                "tag" | "text" | "mathrm" | "operatorname" | "begin" | "end"
            ) {
                // The argument of these is not a symbol.
                index = skip_argument(&chars, end);
                continue;
            }
            if is_standard_latex(&lower) {
                index = end;
                continue;
            }
            out.push(format!("\\{name}"));
            index = end;
            continue;
        }
        if ch.is_ascii_alphabetic() {
            out.push(ch.to_string());
        }
        index += 1;
    }
    dedup(out)
}

fn skip_argument(chars: &[char], mut index: usize) -> usize {
    while index < chars.len() && chars[index].is_whitespace() {
        index += 1;
    }
    if chars.get(index) != Some(&'{') {
        return index;
    }
    let mut depth = 0;
    while index < chars.len() {
        match chars[index] {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return index + 1;
                }
            }
            _ => {}
        }
        index += 1;
    }
    index
}

fn dedup(values: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for value in values {
        if !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

fn is_standard_latex(name: &str) -> bool {
    matches!(
        name,
        "frac" | "ln" | "log" | "exp" | "sum" | "prod" | "min" | "max" | "arg" | "lim"
            | "sup" | "inf" | "det" | "left" | "right" | "cdot" | "qquad" | "quad" | "bar"
            | "hat" | "tilde" | "mathbf" | "mathcal" | "le" | "leq" | "ge" | "geq" | "to"
            | "longrightarrow" | "Longrightarrow" | "equiv" | "partial" | "infty" | "ldots"
            | "dots" | "sqrt" | "cup" | "cap" | "subset" | "in" | "forall" | "exists" | "times"
            | "pm" | "mp" | "approx" | "sim" | "mid" | "langle" | "rangle" | "big" | "Big"
            | "bigg" | "Bigg" | "displaystyle" | "limits" | "not"
    )
}

// --- rendering --------------------------------------------------------------------------------

/// Where a rendered block belongs in the template.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Block {
    Identity,
    Explanation,
    BookDefinition,
    MathDefinition,
    EquivalentForms,
    Derivation,
    Limits,
    Example,
    Intuition,
    Distinctions,
    Misreadings,
    RoleInBook,
}

impl Block {
    fn heading(self) -> &'static str {
        match self {
            Block::Explanation => "释义",
            Block::BookDefinition => "书中的说法",
            Block::MathDefinition => "数学定义",
            Block::EquivalentForms => "等价形式",
            Block::Derivation => "推导",
            Block::Limits => "极限与特例",
            Block::Example => "例子",
            Block::Intuition => "直觉",
            Block::Distinctions => "区别",
            Block::Misreadings => "常见误读",
            Block::RoleInBook => "在本书中的作用",
            Block::Identity => "",
        }
    }

    fn is_mandatory(self) -> bool {
        matches!(
            self,
            Block::Identity | Block::Explanation | Block::BookDefinition | Block::MathDefinition
        )
    }
}

/// One rendered unit the partition places. A whole section is one unit; a derivation is one unit
/// per step (the result travels with the last step), so a long derivation can continue at a whole
/// step boundary (R0289).
struct Unit {
    block: Block,
    /// The `##` heading this unit opens when the card has no open section with it.
    heading: String,
    /// The unit's lines. A derivation step holds no leading number; assembly numbers it per Card.
    text: String,
    prose: i64,
    /// The derivation this unit belongs to, so the assembly restarts numbering per section.
    derivation: Option<usize>,
    /// Whether this unit carries the derivation's final `结果：` line.
    has_result: bool,
    /// What a part starting here names after its `推导（续 k/n）：` prefix.
    label: String,
}

impl Unit {
    fn steps(&self) -> i64 {
        i64::from(self.derivation.is_some())
    }
}

/// One rendered Card.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedCard {
    /// `card` for a head, `part` for a continuation.
    pub role: &'static str,
    pub title: String,
    pub body: String,
    /// The numbered derivation steps this Card holds (T5's 12-step bound).
    pub steps: i64,
    /// The measured prose units this Card holds (T9's 1,600-unit bound).
    pub prose: i64,
}

impl RenderedCard {
    /// The exact Card text: the title heading, a blank line, and the body.
    pub fn text(&self) -> String {
        format!("# {}\n\n{}\n", self.title, self.body)
    }
}

/// One term's complete rendered chain: its head and its continuation parts, in order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rendered {
    pub head: RenderedCard,
    pub parts: Vec<RenderedCard>,
}

impl Rendered {
    /// Every Card of the chain, the head first.
    pub fn cards(&self) -> Vec<&RenderedCard> {
        std::iter::once(&self.head).chain(self.parts.iter()).collect()
    }
}

/// Render one validated draft into its head and continuation parts (design 11.1–11.2, R0277–
/// R0290). `row` is the exact table row the term publishes, `book` is the Citation key of the
/// book, and `head` is the address (or the batch reference `@@term_head`) a part's continuation
/// Link names. `limits` supplies the per-Card step and prose bounds.
///
/// The partition is deterministic: whole sections stay together except a derivation, which
/// continues at a whole step boundary. A single unit that exceeds a bound, and a head whose
/// mandatory material exceeds the prose bound, are refused for a writer fix (R0289–R0290); this
/// function never truncates content or raises a bound.
pub fn render(
    draft: &Draft,
    row: &TermRow,
    book: &str,
    head: &str,
    limits: &Limits,
) -> Result<Rendered, DraftFaults> {
    if row.term != draft.term {
        return Err(DraftFaults::one(
            "term",
            format!("`{}` is not the table row `{}`", draft.term, row.term),
        ));
    }
    let units = build_units(draft, row);
    let trailer = format!("来源：[[{book}]] {}", draft.first_defined);
    let trailer_prose = prose_units(&trailer);

    // The head must hold its mandatory material; if that alone is too large, no split can help
    // (R0290). A single unit larger than a Card is a writer fix, never a truncation (R0289).
    let mandatory = units
        .iter()
        .take_while(|unit| unit.block.is_mandatory())
        .count();
    if mandatory == 0 {
        return Err(DraftFaults::one("render.head", "the draft renders no identity line"));
    }
    let mandatory_prose = units[..mandatory].iter().map(|unit| unit.prose).sum::<i64>();
    if mandatory_prose + trailer_prose > limits.max_term_prose {
        return Err(DraftFaults::one(
            "render.head",
            format!(
                "the head's mandatory material holds {} prose units > {}; split the content or shorten it",
                mandatory_prose + trailer_prose,
                limits.max_term_prose
            ),
        ));
    }
    for unit in &units {
        if unit.prose > limits.max_term_prose {
            return Err(DraftFaults::one(
                "render.block",
                format!(
                    "one {} block holds {} prose units > {}; split it at a whole block boundary or shorten it",
                    unit.block.heading(),
                    unit.prose,
                    limits.max_term_prose
                ),
            ));
        }
    }

    // The head keeps its mandatory units plus as many whole later units as fit. Every later unit
    // either extends the head or starts/extends a continuation part (design 11.2).
    let mut head_end = mandatory;
    let mut head_steps = 0;
    let mut head_prose = mandatory_prose;
    for (index, unit) in units.iter().enumerate().skip(mandatory) {
        let steps = head_steps + unit.steps();
        if steps > limits.max_term_steps
            || head_prose + unit.prose + trailer_prose > limits.max_term_prose
        {
            break;
        }
        head_end = index + 1;
        head_steps = steps;
        head_prose += unit.prose;
    }

    // Fill each continuation part greedily at whole units. A unit that cannot fit a fresh part is
    // refused rather than written alone or clipped.
    let part_identity = part_identity_unit(draft, head);
    let part_prefix = part_identity.prose + trailer_prose;
    let mut parts: Vec<(usize, usize)> = Vec::new();
    let mut start = head_end;
    while start < units.len() {
        let mut end = start;
        let mut steps = 0;
        let mut prose = part_prefix;
        while end < units.len() {
            let unit = &units[end];
            if steps + unit.steps() > limits.max_term_steps || prose + unit.prose > limits.max_term_prose
            {
                break;
            }
            steps += unit.steps();
            prose += unit.prose;
            end += 1;
        }
        if end == start {
            return Err(DraftFaults::one(
                "render.part",
                format!(
                    "one {} block cannot fit a continuation part ({} prose units + {} identity > {})",
                    units[start].block.heading(),
                    units[start].prose,
                    part_prefix,
                    limits.max_term_prose
                ),
            ));
        }
        parts.push((start, end));
        start = end;
    }

    let head_card = assemble("card", &draft.term, &units, 0, head_end, None, &trailer, limits)?;
    let total = parts.len() + 1;
    let mut rendered_parts = Vec::new();
    for (position, (start, end)) in parts.iter().enumerate() {
        let number = position + 2;
        let label = part_label(&units, *start, *end);
        let title = format!("{CONTINUATION_PREFIX}{number}/{total}）：{label}");
        if !title_fits(&title) {
            return Err(DraftFaults::one(
                "render.part.title",
                format!(
                    "the continuation title `{title}` is longer than a Card title; shorten the claim"
                ),
            ));
        }
        let mut card = assemble(
            "part",
            &title,
            &units,
            *start,
            *end,
            Some(&part_identity.text),
            &trailer,
            limits,
        )?;
        card.title = title;
        card.role = "part";
        rendered_parts.push(card);
    }
    Ok(Rendered {
        head: head_card,
        parts: rendered_parts,
    })
}

fn build_units(draft: &Draft, row: &TermRow) -> Vec<Unit> {
    let mut units = vec![identity_unit(draft, row)];
    units.push(text_unit(Block::Explanation, &draft.explanation));
    units.push(text_unit(
        Block::BookDefinition,
        &book_definition_lines(&draft.book_definition),
    ));
    if draft.category == Category::Math {
        units.push(math_definition_unit(draft));
    }
    if !draft.equivalent_forms.is_empty() {
        let lines = draft
            .equivalent_forms
            .iter()
            .map(|form| {
                if form.formula.trim().is_empty() {
                    format!("- {}", form.reason)
                } else {
                    format!("- {}：{}", inline_math(&form.formula), form.reason)
                }
            })
            .collect::<Vec<_>>();
        units.push(text_unit(Block::EquivalentForms, &lines));
    }
    for (index, derivation) in draft.derivations.iter().enumerate() {
        units.extend(derivation_units(index, derivation));
    }
    if !draft.limits.is_empty() {
        let lines = draft
            .limits
            .iter()
            .map(|case| {
                let text = format!("- {}：{}", case.case, case.text);
                if case.formula.trim().is_empty() {
                    text
                } else {
                    format!("{text} {}", inline_math(&case.formula))
                }
            })
            .collect::<Vec<_>>();
        units.push(text_unit(Block::Limits, &lines));
    }
    if !draft.example.is_empty() {
        units.push(text_unit(Block::Example, &draft.example));
    }
    if !draft.intuition.is_empty() {
        units.push(text_unit(Block::Intuition, &draft.intuition));
    }
    if !draft.distinctions.is_empty() {
        let lines = draft
            .distinctions
            .iter()
            .map(|distinction| format!("- 与 {}：{}", distinction.term, distinction.text))
            .collect::<Vec<_>>();
        units.push(text_unit(Block::Distinctions, &lines));
    }
    if !draft.misreadings.is_empty() {
        let lines = draft
            .misreadings
            .iter()
            .map(|misreading| format!("- {misreading}"))
            .collect::<Vec<_>>();
        units.push(text_unit(Block::Misreadings, &lines));
    }
    units.push(text_unit(Block::RoleInBook, &draft.role_in_book));
    units
}

fn part_label(units: &[Unit], start: usize, end: usize) -> String {
    let first = &units[start];
    if first.block == Block::Derivation {
        return first.label.clone();
    }
    let mut labels: Vec<String> = Vec::new();
    for unit in &units[start..end] {
        let label = unit.block.heading().to_string();
        if !label.is_empty() && !labels.contains(&label) {
            labels.push(label);
        }
        if labels.len() == 3 {
            break;
        }
    }
    if labels.is_empty() {
        "内容".to_string()
    } else {
        labels.join("、")
    }
}

/// Assemble one Card's body: the optional part prefix, then each unit (its heading when it opens a
/// new section, its numbered step text), then the source trailer.
#[allow(clippy::too_many_arguments)]
fn assemble(
    role: &'static str,
    title: &str,
    units: &[Unit],
    start: usize,
    end: usize,
    prefix: Option<&str>,
    trailer: &str,
    limits: &Limits,
) -> Result<RenderedCard, DraftFaults> {
    let mut body = String::new();
    if let Some(prefix) = prefix {
        body.push_str(prefix);
        body.push('\n');
    }
    let mut current_heading = String::new();
    let mut derivation: Option<usize> = None;
    let mut step_number = 0i64;
    let mut steps = 0i64;
    for unit in &units[start..end] {
        if !unit.heading.is_empty() && unit.heading != current_heading {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(&format!("## {}\n\n", unit.heading));
            current_heading = unit.heading.clone();
        }
        if let Some(index) = unit.derivation {
            if derivation != Some(index) {
                derivation = Some(index);
                step_number = 0;
            }
            step_number += 1;
            steps += 1;
            if !body.is_empty() && !body.ends_with('\n') {
                body.push('\n');
            }
            body.push_str(&format!("{step_number}. {}\n", unit.text));
        } else {
            if !body.is_empty() && !body.ends_with("\n\n") {
                body.push('\n');
            }
            body.push_str(&unit.text);
            body.push('\n');
        }
    }
    // A chunk that ends before its derivation's last step says so; the next Card carries the rest,
    // and T5 accepts the continuation line in place of the final result (R0289).
    if units[start..end]
        .last()
        .is_some_and(|unit| unit.derivation.is_some() && !unit.has_result)
    {
        if !body.ends_with("\n\n") {
            body.push('\n');
        }
        body.push_str(CONTINUES);
        body.push('\n');
    }
    if !body.is_empty() && !body.ends_with("\n\n") {
        body.push('\n');
    }
    body.push_str(trailer);
    body.push('\n');
    let prose = prose_units(&body);
    if prose > limits.max_term_prose {
        return Err(DraftFaults::one(
            "render.card",
            format!(
                "the rendered Card holds {prose} prose units > {}; split it at a whole block boundary",
                limits.max_term_prose
            ),
        ));
    }
    if steps > limits.max_term_steps {
        return Err(DraftFaults::one(
            "render.card",
            format!(
                "the rendered Card holds {steps} derivation steps > {}; split it at a step boundary",
                limits.max_term_steps
            ),
        ));
    }
    Ok(RenderedCard {
        role,
        title: title.to_string(),
        body,
        steps,
        prose,
    })
}

fn identity_unit(draft: &Draft, row: &TermRow) -> Unit {
    let avoid = if row.avoid.is_empty() {
        "—".to_string()
    } else {
        row.avoid.join("、")
    };
    let mut lines = vec![format!("术语：{}", draft.term)];
    if !draft.original.is_empty() {
        lines.push(format!("原文：{}", draft.original));
    }
    lines.push(format!("不用：{avoid}"));
    lines.push(format!("类别：{}", draft.category.label()));
    lines.push(format!("首次定义：{}", draft.first_defined));
    if !draft.link {
        lines.push(format!("{AUTO_LINK_LABEL}：{AUTO_LINK_OFF}"));
        lines.push(format!("{AUTO_LINK_REASON_LABEL}：{}", draft.link_reason));
    }
    let text = lines.join("\n");
    Unit {
        block: Block::Identity,
        heading: String::new(),
        prose: prose_units(&text),
        text,
        derivation: None,
        has_result: false,
        label: String::new(),
    }
}

fn part_identity_unit(draft: &Draft, head: &str) -> Unit {
    let inheritance = match draft.category {
        Category::Math => "本卡沿用该卡的符号表。",
        Category::Concept => "本卡承接该术语的说明。",
    };
    let text = format!(
        "类别：{}\n{CONTINUATION_LABEL}：[[{head}]] {inheritance}",
        draft.category.label()
    );
    Unit {
        block: Block::Identity,
        heading: String::new(),
        prose: prose_units(&text),
        text,
        derivation: None,
        has_result: false,
        label: String::new(),
    }
}

fn book_definition_lines(book: &BookDefinition) -> Vec<String> {
    let mut lines = Vec::new();
    let section = book.section.trim();
    for (index, sentence) in book.text.iter().enumerate() {
        if index == 0 && !section.is_empty() && !sentence.starts_with(section) {
            lines.push(format!("{section} {sentence}"));
        } else {
            lines.push(sentence.clone());
        }
    }
    if !book.quote.is_empty() {
        lines.push(format!("「{}」", book.quote));
    }
    lines
}

fn math_definition_unit(draft: &Draft) -> Unit {
    let definition = &draft.definition;
    let mut parts = vec!["前提：".to_string()];
    for hypothesis in &draft.hypotheses {
        parts.push(format!("- ({}) {}", hypothesis.id, hypothesis.text));
    }
    parts.push(String::new());
    parts.push(format!("$${}$$", definition.formula.trim_matches('$')));
    if !definition.reading.is_empty() {
        parts.push(String::new());
        parts.extend(definition.reading.iter().cloned());
    }
    parts.push(String::new());
    parts.push("符号表".to_string());
    parts.push(String::new());
    parts.push("| 符号 | 含义 | 类型与取值 |".to_string());
    parts.push("| --- | --- | --- |".to_string());
    for symbol in &draft.symbols {
        let kind = if symbol.auxiliary {
            format!("辅助，{}", symbol.r#type)
        } else {
            symbol.r#type.clone()
        };
        parts.push(format!("| {} | {} | {} |", symbol.symbol, symbol.meaning, kind));
    }
    text_unit(Block::MathDefinition, &parts)
}

fn derivation_units(index: usize, derivation: &Derivation) -> Vec<Unit> {
    let heading = format!("推导：{}", derivation.claim);
    let last = derivation.steps.len();
    let mut units = Vec::new();
    for (position, step) in derivation.steps.iter().enumerate() {
        let mut text = sentence(&step.text);
        text.push_str("依据：");
        text.push_str(&sentence(&step.basis));
        if !step.formula.is_empty() {
            text.push('\n');
            text.push_str(&format!("   $${}$$", step.formula.trim_matches('$')));
        }
        if position + 1 == last {
            text.push_str("\n\n");
            text.push_str(&format!("结果：{}", sentence(&derivation.result)));
        }
        units.push(Unit {
            block: Block::Derivation,
            heading: heading.clone(),
            prose: prose_units(&text),
            text,
            derivation: Some(index),
            has_result: position + 1 == last,
            label: derivation.claim.clone(),
        });
    }
    units
}

/// A sentence with one end mark: an existing `。`, `！` or `？` is kept.
fn sentence(text: &str) -> String {
    let text = text.trim();
    if text.ends_with(['。', '！', '？', '!', '?', '.']) {
        text.to_string()
    } else {
        format!("{text}。")
    }
}

fn text_unit(block: Block, lines: &[String]) -> Unit {
    let text = lines.join("\n");
    Unit {
        block,
        heading: block.heading().to_string(),
        prose: prose_units(&text),
        text,
        derivation: None,
        has_result: false,
        label: String::new(),
    }
}

/// Inline math with exactly one pair of `$` delimiters.
fn inline_math(formula: &str) -> String {
    let formula = formula.trim();
    if formula.starts_with('$') && formula.ends_with('$') && formula.len() > 1 {
        formula.to_string()
    } else {
        format!("${formula}$")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(term: &str) -> TermRow {
        TermRow {
            term: term.to_string(),
            avoid: vec!["平衡".to_string()],
            meaning: "双方都不改变策略".to_string(),
            defined: "§1.1".to_string(),
        }
    }

    fn math_draft() -> Value {
        json!({
            "term": "均衡",
            "original": "equilibrium",
            "category": "math",
            "link": false,
            "link_reason": "这是过于一般的词。",
            "first_defined": "§1.1",
            "explanation": [
                "均衡是双方都不改变选择的状态。",
                "本书用它说明市场的稳定点。",
                "均衡不等于最优。"
            ],
            "book_definition": { "text": ["§1.1 把均衡定义为一个不动点。"], "section": "§1.1", "quote": "" },
            "hypotheses": [
                { "id": "H1", "text": "策略集是紧凸集。" },
                { "id": "H2", "text": "收益函数连续。" }
            ],
            "definition": {
                "formula": "x^* \\in \\arg\\max_{x} u(x) \\tag{T1}",
                "reading": ["$x^*$ 是没有人愿意偏离的策略。"]
            },
            "symbols": [
                { "symbol": "$x^*$", "meaning": "均衡策略", "type": "紧凸集", "auxiliary": false },
                { "symbol": "$\\rho$", "meaning": "辅助参数", "type": "$\\rho<1$", "auxiliary": true }
            ],
            "equivalent_forms": [
                { "formula": "u(x^*)\\ge u(x)", "reason": "由 (T1) 直接得到。" }
            ],
            "derivations": [
                {
                    "claim": "均衡是纳什均衡",
                    "steps": [
                        { "text": "起点是定义 (T1)。", "basis": "定义 (T1)", "formula": "x^*=x" },
                        { "text": "两边比较收益。", "basis": "H2", "formula": "u(x^*)\\ge u(x)" }
                    ],
                    "result": "均衡是纳什均衡。"
                }
            ],
            "limits": [ { "case": "无", "text": "本定义没有需要说明的极限情形。", "formula": "" } ],
            "example": ["取 $u(x)=-x^2$。", "唯一均衡是 $x^*=0$。"],
            "intuition": ["没有人愿意单方面偏离。", "均衡是相互一致的选择。"],
            "distinctions": [ { "term": "弹性", "text": "弹性度量反应程度。" } ],
            "misreadings": ["均衡不是最优。"],
            "role_in_book": ["本书第一章用均衡定义市场结果。"]
        })
    }

    fn concept_draft() -> Value {
        json!({
            "term": "均衡",
            "category": "concept",
            "first_defined": "§1.1",
            "explanation": ["均衡是双方都不改变选择的状态。", "本书用它说明市场的稳定点。", "均衡不等于最优。"],
            "book_definition": { "text": ["§1.1 把均衡定义为一个不动点。"], "section": "§1.1", "quote": "" },
            "distinctions": [ { "term": "弹性", "text": "弹性度量反应程度。" } ],
            "role_in_book": ["本书第一章用均衡定义市场结果。"]
        })
    }

    fn rows() -> Vec<String> {
        vec!["均衡".to_string(), "弹性".to_string()]
    }

    #[test]
    fn a_valid_math_draft_reads_every_field_and_renders_the_head_template() {
        let draft = parse_draft(&math_draft(), &rows()).expect("a valid draft");
        assert_eq!(draft.term, "均衡");
        assert_eq!(draft.category, Category::Math);
        assert!(!draft.link);
        let rendered = render(
            &draft,
            &row("均衡"),
            "Test2026",
            "Test2026/1|a|a",
            &Limits::default(),
        )
        .expect("renders");
        assert!(rendered.parts.is_empty(), "the example fits one Card");
        let head = &rendered.head;
        assert_eq!(head.role, "card");
        assert_eq!(head.title, "均衡");
        let body = &head.body;
        for expected in [
            "术语：均衡",
            "原文：equilibrium",
            "不用：平衡",
            "类别：数学概念",
            "首次定义：§1.1",
            "自动链接：否",
            "自动链接理由：这是过于一般的词。",
            "## 释义",
            "## 书中的说法",
            "§1.1 把均衡定义为一个不动点。",
            "## 数学定义",
            "前提：",
            "- (H1) 策略集是紧凸集。",
            "$$x^* \\in \\arg\\max_{x} u(x) \\tag{T1}$$",
            "符号表",
            "| 符号 | 含义 | 类型与取值 |",
            "| $\\rho$ | 辅助参数 | 辅助，$\\rho<1$ |",
            "## 等价形式",
            "## 推导：均衡是纳什均衡",
            "1. 起点是定义 (T1)。依据：定义 (T1)",
            "2. 两边比较收益。依据：H2",
            "结果：均衡是纳什均衡。",
            "## 极限与特例",
            "## 例子",
            "## 直觉",
            "## 区别",
            "- 与 弹性：弹性度量反应程度。",
            "## 常见误读",
            "- 均衡不是最优。",
            "## 在本书中的作用",
            "来源：[[Test2026]] §1.1",
        ] {
            assert!(body.contains(expected), "missing {expected:?} in:\n{body}");
        }
        assert_eq!(head.steps, 2);
        assert!(head.prose > 0);
        let template = parse_template(body);
        assert!(template.unknown_sections.is_empty(), "{:?}", template.unknown_sections);
        assert!(template.unknown_lines.is_empty(), "{:?}", template.unknown_lines);
        assert!(template.trailer.is_some());
        assert_eq!(template.line("术语").map(|line| line.value.as_str()), Some("均衡"));
        assert!(head.text().starts_with("# 均衡\n\n术语：均衡"));
    }

    #[test]
    fn a_concept_draft_renders_only_the_common_sections_and_refuses_math_fields() {
        let draft = parse_draft(&concept_draft(), &rows()).expect("a valid concept draft");
        let rendered = render(
            &draft,
            &row("均衡"),
            "Test2026",
            "Test2026/1|a|a",
            &Limits::default(),
        )
        .expect("renders");
        let body = &rendered.head.body;
        assert!(body.contains("类别：概念"));
        assert!(body.contains("## 释义"));
        assert!(body.contains("## 区别"));
        assert!(!body.contains("## 数学定义"));
        assert!(!body.contains("## 推导"));
        assert!(!body.contains("符号表"));

        let mut with_math = concept_draft();
        with_math["derivations"] = math_draft()["derivations"].clone();
        let faults = parse_draft(&with_math, &rows()).expect_err("a concept carries no derivation");
        assert!(
            faults
                .to_string()
                .contains("concept term carries no formal mathematics"),
            "{faults}"
        );
    }

    #[test]
    fn a_disabled_link_renders_the_two_lines_and_a_missing_reason_is_a_fault() {
        let mut draft = concept_draft();
        draft["link"] = json!(false);
        draft["link_reason"] = json!("这是过于一般的词。");
        let text = render(
            &parse_draft(&draft, &rows()).expect("valid"),
            &row("均衡"),
            "Test2026",
            "Test2026/1|a|a",
            &Limits::default(),
        )
        .expect("renders")
        .head
        .body;
        assert!(
            text.contains("自动链接：否\n自动链接理由：这是过于一般的词。"),
            "{text}"
        );
        let enabled = render(
            &parse_draft(&concept_draft(), &rows()).expect("valid"),
            &row("均衡"),
            "Test2026",
            "Test2026/1|a|a",
            &Limits::default(),
        )
        .expect("renders")
        .head
        .body;
        assert!(!enabled.contains("自动链接"), "{enabled}");
        let mut missing = concept_draft();
        missing["link"] = json!(false);
        let faults = parse_draft(&missing, &rows()).expect_err("no reason");
        assert!(faults.to_string().contains("term.link_reason"), "{faults}");
    }

    #[test]
    fn a_long_derivation_partitions_at_whole_steps_into_same_name_parts() {
        let mut draft = parse_draft(&math_draft(), &rows()).expect("valid");
        draft.derivations = vec![Derivation {
            claim: "均衡是纳什均衡".to_string(),
            steps: (0..24)
                .map(|index| Step {
                    text: format!("第 {} 步做一次变换。", index + 1),
                    basis: "代数".to_string(),
                    formula: format!("x_{{{}}}=1", index + 1),
                })
                .collect(),
            result: "均衡是纳什均衡。".to_string(),
        }];
        let rendered = render(
            &draft,
            &row("均衡"),
            "Test2026",
            "@@term_head",
            &Limits::default(),
        )
        .expect("partitions");
        assert_eq!(rendered.parts.len(), 1, "{rendered:#?}");
        assert_eq!(rendered.head.steps, 12);
        assert_eq!(rendered.parts[0].steps, 12);
        assert_eq!(rendered.head.title, "均衡");
        assert_eq!(rendered.parts[0].title, "推导（续 2/2）：均衡是纳什均衡");
        let part = &rendered.parts[0];
        assert!(
            part.body
                .starts_with("类别：数学概念\n续：[[@@term_head]] 本卡沿用该卡的符号表。"),
            "{}",
            part.body
        );
        assert!(part.body.contains("## 推导：均衡是纳什均衡"));
        assert!(!part.body.contains("## 释义"));
        assert!(!part.body.contains("术语：均衡"));
        assert!(part.body.contains("来源：[[Test2026]] §1.1"));
        assert!(part.body.contains("结果：均衡是纳什均衡。"));
        let mut numbers = Vec::new();
        for card in rendered.cards() {
            assert!(card.prose <= 1600, "{} > 1600", card.prose);
            assert!(card.steps <= 12);
            for line in card.body.lines() {
                if let Some(number) = numbered_step(line.trim()) {
                    numbers.push(number);
                }
            }
        }
        // Each Card numbers its own section from 1; the chain keeps every step exactly once.
        assert_eq!(numbers, (1..=12).chain(1..=12).collect::<Vec<i64>>());
    }

    #[test]
    fn an_oversized_block_or_mandatory_head_is_refused_not_truncated() {
        let draft = parse_draft(&math_draft(), &rows()).expect("valid");
        let small = Limits {
            max_term_prose: 20,
            ..Limits::default()
        };
        let error = render(&draft, &row("均衡"), "Test2026", "@@term_head", &small)
            .expect_err("an oversized mandatory head is refused");
        assert!(error.to_string().contains("mandatory material"), "{error}");
        let mut long = parse_draft(&concept_draft(), &rows()).expect("valid");
        long.explanation = vec!["甲".repeat(200), "乙".repeat(200), "丙".repeat(200)];
        let error = render(&long, &row("均衡"), "Test2026", "@@term_head", &small)
            .expect_err("an oversized block is refused");
        let message = error.to_string();
        assert!(message.contains("prose units >"), "{message}");
    }

    #[test]
    fn every_draft_field_is_rendered_or_explicitly_validated() {
        let draft = parse_draft(&math_draft(), &rows()).expect("valid");
        let rendered = render(
            &draft,
            &row("均衡"),
            "Test2026",
            "@@term_head",
            &Limits::default(),
        )
        .expect("renders");
        let text = rendered.head.body;
        for expected in [
            "均衡",
            "equilibrium",
            "平衡",
            "数学概念",
            "§1.1",
            "均衡是双方都不改变选择的状态。",
            "§1.1 把均衡定义为一个不动点。",
            "策略集是紧凸集。",
            "均衡策略",
            "由 (T1) 直接得到。",
            "起点是定义 (T1)。",
            "取 $u(x)=-x^2$。",
            "没有人愿意单方面偏离。",
            "与 弹性",
            "均衡不是最优。",
            "本书第一章用均衡定义市场结果。",
        ] {
            assert!(text.contains(expected), "missing {expected:?}");
        }
        let mut unknown = math_draft();
        unknown["surprise"] = json!("content");
        let faults = parse_draft(&unknown, &rows()).expect_err("an unknown field is refused");
        assert!(faults.to_string().contains("surprise"), "{faults}");
    }

    #[test]
    fn draft_faults_name_the_exact_path_and_collect_every_fault() {
        let mut draft = math_draft();
        draft["explanation"] = json!(["只有一句。"]);
        draft["definition"]["formula"] = json!("no tag");
        draft["symbols"] = json!([]);
        draft["misreadings"] = json!([]);
        let faults = parse_draft(&draft, &rows()).expect_err("faulty");
        let text = faults.to_string();
        for expected in ["term.explanation", "term.symbols", "term.misreadings"] {
            assert!(text.contains(expected), "missing {expected} in {text}");
        }
        assert!(faults.0.len() >= 3, "{:?}", faults.0);
    }

    #[test]
    fn a_missing_or_mismatched_row_is_a_fault() {
        let mut draft = math_draft();
        draft["term"] = json!("利率");
        let faults = parse_draft(&draft, &rows()).expect_err("no such row");
        assert!(
            faults
                .to_string()
                .contains("is no row of the book's term table"),
            "{faults}"
        );
        let draft = parse_draft(&math_draft(), &rows()).expect("valid");
        let error = render(
            &draft,
            &TermRow {
                term: "利率".to_string(),
                ..row("利率")
            },
            "Test2026",
            "@@term_head",
            &Limits::default(),
        )
        .expect_err("another row");
        assert!(error.to_string().contains("not the table row"), "{error}");
    }

    #[test]
    fn the_prose_measure_excludes_formulas_and_tables_and_counts_utf16_units() {
        let body = "第一句。\n\n$$x=1$$\n\n| 甲 | 乙 |\n| --- | --- |\n| 一 | 二 |\n\n第二句。";
        assert_eq!(prose_units(body), 8);
        assert_eq!(prose_units("😀。"), 3);
        assert_eq!(
            prose_units("来源：[[Test2026]] §1.1"),
            0,
            "a source locator is no prose"
        );
    }

    #[test]
    fn the_template_parser_names_unknown_and_misordered_sections() {
        let body = "术语：均衡\n类别：数学概念\n未知：x\n\n## 释义\n\n说明。\n\n## 神秘\n\n内容。\n\n## 在本书中的作用\n\n作用。\n\n来源：[[Test2026]] §1.1\n";
        let template = parse_template(body);
        assert_eq!(template.lines.len(), 2);
        assert_eq!(template.unknown_lines.len(), 1);
        assert_eq!(template.unknown_sections.len(), 1);
        assert_eq!(template.unknown_sections[0].1, "神秘");
        let names = template
            .sections
            .iter()
            .map(|section| section.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["释义", "神秘", "在本书中的作用"]);
        assert!(template.trailer.is_some());
        assert!(!template.after_trailer);
        let fenced = parse_template("术语：均衡\n\n```\n## 释义\n```\n\n来源：[[Test2026]] §1\n");
        assert!(fenced.sections.is_empty(), "{fenced:#?}");
    }

    #[test]
    fn the_step_parser_reads_numbers_bases_formulas_and_the_result() {
        let template = parse_template(
            "类别：数学概念\n\n## 推导：均衡是纳什均衡\n\n1. 起点是定义。依据：定义 (T1)。\n   $$x=1$$\n2. 比较收益。依据：H2。\n\n结果：均衡是纳什均衡。\n\n来源：[[Test2026]] §1\n",
        );
        let section = template.one("推导").expect("one derivation");
        let steps = step_blocks(section);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].number, 1);
        assert!(steps[0].has_formula());
        assert_eq!(steps[0].basis(), Some("定义 (T1)。"));
        assert_eq!(steps[1].number, 2);
        assert!(!steps[1].has_formula());
        assert_eq!(result_line(section).as_deref(), Some("均衡是纳什均衡。"));
    }

    #[test]
    fn the_symbol_scan_ignores_standard_operators_and_reads_greek_and_scripts() {
        let ids = latex_identifiers("\\sigma=\\frac{d\\ln Z}{d\\ln L}+\\rho_x \\tag{T1}");
        assert!(ids.contains(&"\\sigma".to_string()), "{ids:?}");
        assert!(ids.contains(&"\\rho".to_string()), "{ids:?}");
        assert!(ids.contains(&"Z".to_string()), "{ids:?}");
        assert!(ids.contains(&"L".to_string()), "{ids:?}");
        for standard in ["\\frac", "\\ln", "\\tag", "T"] {
            assert!(!ids.contains(&standard.to_string()), "{standard} in {ids:?}");
        }
    }
}
