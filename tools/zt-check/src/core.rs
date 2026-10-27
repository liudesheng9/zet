//! The process-free rule core: the Card language rules (R2 to R22) and the Card
//! layer rules (I, E, F, L), evaluated from the entries and the bounded context
//! a caller materialized. It reads no file, starts no process, and imports no
//! archive or platform binding, so the standalone checker, zt-digest, and the
//! zt service evaluate one definition of Card legality.

use crate::text::{
    TitleContext, TitleRow, Terms, bigrams, book_place, culture, culture_cmp,
    culture_starts_with, entry_section, from_utf16_form, is_high_stand_in, is_low_stand_in, key,
    leads_to, length, link_occurrences, literature_tree, measure, open_math, passive, plain, prop,
    prose_lines, ps_eq, ps_in, ps_int, ps_string, read_terms, similarity, split_sentences,
    strings, structure_lines, term_table, text, truthy, utf16_form, LinkForm, BOLD, CJK, CODE,
    DOCUMENT_LINE, HEADING, INLINE_MATH, LINK, LINK_TARGET, PLACEHOLDERS, REASON_STRIP,
    SOURCE_LINE, SPACE, SPACES, WORD, pattern,
};
use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

// The rules.
pattern!(SEMICOLON, r"[；;]");
pattern!(LIGHT_VERB, r"(进行|加以|予以|作出|做出)[㐀-鿿豈-﫿]");
pattern!(
    NOUN_PHRASE,
    r"的[^，。、：；！？,.;:!?\s的]{0,7}的[^，。、：；！？,.;:!?\s的]{0,7}的"
);
pattern!(DERIVATION, r"(?i)^derivation");
// The standalone text rules (R20, R21). Not every digit and not every 第 is a
// local reference: 第一福利定理 is a name, and a label with the content beside
// it is a statement.
pattern!(LOCAL_EQUATION, r"式\s*[（(]\s*[0-9A-Za-z][0-9A-Za-z.]*\s*[)）]");
pattern!(
    LOCAL_NAMED_NUMBER,
    r"(命题|引理|定理|推论|模型)\s*[（(]?\s*\d+(?:[.．][0-9A-Za-z]+)*"
);
pattern!(LOCAL_CHAPTER, r"第\s*\d+\s*[章节]");
pattern!(LOCAL_THIS, r"本节|本章");
pattern!(LOCAL_SIGN, r"§\s*\d+(?:[.．][0-9A-Za-z]+)*");
pattern!(SECTION, r"^\*{0,2}(思路|起点|步骤|结果|解读)\*\*\s*$");
pattern!(POSITION, r"^第 \d+ 张，共 \d+ 张");
pattern!(NOTE, r"^说明\s*[：:]");
pattern!(NOTE_LABEL, r"^说明\s*[：:]\s*");
pattern!(SOURCE_LABEL, r"^来源\s*[：:]");
// The label a Bridge apply puts before its sentence (`跨书关联：对照 [[x]]：…`): navigation, not prose.
pattern!(BRIDGE_LABEL, r"^跨书关联\s*[：:]\s*");

// The Term content rules (S09, #152, design 11.3).
// A skipped derivation step names its shortcut instead of the step it skips (R0296, AC12).
pattern!(SKIPPED_STEP, r"易知|显然|不难看出|容易证明|同理可得");
// A named earlier step or tagged definition: `步骤 3`, `第 2 步`, `同步骤 2–3`, `(T1)`, `式 (4)`.
pattern!(
    NAMED_STEP,
    r"步骤\s*\d|第\s*\d+\s*步|同步骤|[(（]\s*[A-Z]\d+\s*[)）]|式\s*[(（]\s*[0-9A-Za-z][0-9A-Za-z.\-]*\s*[)）]"
);
// A continuation title: `推导（续 k/n）：<what continues>` (D14, R0047).
pattern!(CONTINUATION_TITLE, r"^推导（续 (\d+)/(\d+)）：(.+)$");
// A labelled hypothesis line: `- (H1) …`.
pattern!(HYPOTHESIS_LINE, r"^[-*]\s*\(H\d+\)\s*\S");
// A quotation is exempt from the Card language (T6 does not read inside it).
pattern!(QUOTED, r"「[^」]*」|“[^”]*”");

// The Card layer rules.
pattern!(
    CLAIM_VERB,
    r"是|会|能|让|使|要|不|没|把|被|决定|导致|取决|依赖|提高|降低|减少|增加|等于|大于|小于|高于|低于|来自|需要|带来|引起|产生|改变|保证|排除|允许|阻止|证明|说明|表明"
);

pattern!(
    DATE,
    r"(1[0-9]{3}|20[0-9]{2})|[0-9一二三四五六七八九十]+\s*(年|世纪)"
);
pattern!(YEAR, r"(1[0-9]{3}|20[0-9]{2})");

pattern!(LIST_ITEM, r"^\s*([-*+]|\d+[.、)])\s+");

/// The title and prose bounds of a Literature note (issue #184): a title of at most 60 characters
/// and a body of at least 80. Both advise; neither blocks a write, and a note has no upper size
/// bound, because it carries the source's argument rather than one claim.
pub const LITERATURE_TITLE_CHARS: i64 = 60;
pub const LITERATURE_MIN_CHARS: i64 = 80;

const HEDGES: [&str; 18] = [
    "值得注意的是",
    "需要注意的是",
    "在某种程度上",
    "某种意义上",
    "在一定程度上",
    "不难看出",
    "显而易见",
    "毋庸置疑",
    "众所周知",
    "可以说",
    "精妙",
    "巧妙",
    "深刻",
    "精彩",
    "极其重要",
    "非常重要",
    "十分重要",
    "优雅",
];

/// The labels of an Event Card (`时间：…`).
static EVENT_LABELS: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| {
    ["时间", "地点", "参与者", "经过", "来源"]
        .into_iter()
        .map(|label| {
            let pattern = format!(r"(?m)^\s*[-*]?\s*(\*\*)?{label}(\*\*)?\s*[：:]");
            (label, Regex::new(&pattern).expect("a label pattern"))
        })
        .collect()
});

/// The labels of a Finding Card (`测量：…`): what was measured, the data
/// (sample, period, place), the method, the result, its limits, the source.
static FINDING_LABELS: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| {
    ["测量", "数据", "方法", "结果", "局限", "来源"]
        .into_iter()
        .map(|label| {
            let pattern = format!(r"^\s*[-*]?\s*(\*\*)?{label}(\*\*)?\s*[：:]");
            (label, Regex::new(&pattern).expect("a label pattern"))
        })
        .collect()
});

const SECTIONS: [&str; 5] = ["思路", "起点", "步骤", "结果", "解读"];

/// Whether a facet marks a Derivation part (R14 to R17 read it, and the
/// applicability signature compares the same fact).
pub fn is_derivation_facet(facet: &str) -> bool {
    culture_starts_with(facet, "推导") || DERIVATION.is_match(facet)
}

/// R20's named constant: the characters of content a sentence must carry beside
/// a local label. `式 (12) 的加成项来自需求弹性` states content; `式 (12) 有该项`
/// does not. It is calibrated on annotated examples before it can block (D294).
const LOCAL_REFERENCE_CONTENT: i64 = 5;

/// The deictic phrases R21 reports when no named Link is in the same sentence.
/// It is a detector of two things it can see, never a Chinese coreference engine.
const DEICTIC: [&str; 6] = ["那里", "那儿", "那张卡", "这张卡", "该卡", "该卡片"];

fn corruption(body: &str, title: &str, address: &str, faults: &mut Faults) {
    fn whole_quote(text: &str) -> bool {
        [
            ("“", "”"),
            ("「", "」"),
            ("『", "』"),
            ("«", "»"),
            ("\"", "\""),
            ("'", "'"),
        ]
        .iter()
        .any(|(open, close)| {
            text.starts_with(open) && text.ends_with(close) && text.len() > open.len() + close.len()
        })
    }
    fn heading(line: &str) -> Option<&str> {
        let rest = line.strip_prefix("# ")?.trim();
        let without = rest.trim_end_matches('#');
        if without.len() < rest.len() && without.chars().last().is_some_and(char::is_whitespace) {
            Some(without.trim_end())
        } else {
            Some(rest)
        }
    }
    fn flush(
        paragraph: &mut Vec<(usize, &str)>,
        seen: &mut HashMap<String, (usize, usize)>,
        address: &str,
        faults: &mut Faults,
    ) {
        if paragraph.is_empty() {
            return;
        }
        let start = paragraph.first().unwrap().0;
        let end = paragraph.last().unwrap().0;
        let text = paragraph
            .iter()
            .map(|(_, line)| *line)
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
        paragraph.clear();
        let probe = INLINE_MATH.replace_all(&text, "");
        let probe = LINK.replace_all(&probe, "");
        let probe = CODE.replace_all(&probe, "");
        let probe = BOLD.replace_all(&probe, "");
        if matches!(probe.trim(), "待写" | "待补" | "TODO" | "TBD" | "..." | "…")
            || (text.lines().count() == 1
                && (text.starts_with("来源：")
                    || text.starts_with("来源:")
                    || text.to_ascii_lowercase().starts_with("source:")))
            || !(CJK.is_match(&probe) || WORD.is_match(&probe))
        {
            return;
        }
        if let Some((first, last)) = seen.get(&text) {
            faults.add(
                address,
                "R18",
                "repeated_prose",
                &text,
                &format!("repeated prose paragraph at lines {first}-{last} and {start}-{end}"),
            );
        } else {
            seen.insert(text, (start, end));
        }
    }
    let normalized = body.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<_> = normalized.split('\n').collect();
    let mut quoted = HashSet::new();
    let mut start = 0;
    for end in 0..=lines.len() {
        if end == lines.len() || lines[end].trim().is_empty() {
            if whole_quote(lines[start..end].join("\n").trim()) {
                quoted.extend(start..end);
            }
            start = end + 1;
        }
    }
    let mut paragraph = Vec::new();
    let mut seen = HashMap::new();
    let mut repeated_titles = 0usize;
    let mut fence: Option<(char, usize)> = None;
    let mut math: Option<String> = None;
    let mut html_quote = 0usize;
    let mut lazy_quote = false;
    for (index, line) in lines.iter().enumerate() {
        let trim = line.trim();
        if let Some((mark, length)) = fence {
            let run = trim.chars().take_while(|c| *c == mark).count();
            if run >= length && trim.chars().skip(run).all(char::is_whitespace) {
                fence = None;
            }
            continue;
        }
        if let Some(end) = &math {
            if trim.contains(end.as_str()) {
                math = None;
            }
            continue;
        }
        let lower = trim.to_ascii_lowercase();
        if html_quote > 0 {
            html_quote = (html_quote + lower.matches("<blockquote").count())
                .saturating_sub(lower.matches("</blockquote").count());
            continue;
        }
        if trim.is_empty() {
            flush(&mut paragraph, &mut seen, address, faults);
            lazy_quote = false;
            continue;
        }
        if trim == "<!-- zt:reverse-links -->" {
            break;
        }
        if lazy_quote {
            continue;
        }
        if quoted.contains(&index) {
            flush(&mut paragraph, &mut seen, address, faults);
            continue;
        }
        if let Some(mark) = trim.chars().next().filter(|c| *c == '`' || *c == '~') {
            let length = trim.chars().take_while(|c| *c == mark).count();
            if length >= 3 {
                flush(&mut paragraph, &mut seen, address, faults);
                fence = Some((mark, length));
                continue;
            }
        }
        let ending = if trim.starts_with("$$") {
            Some(("$$".to_string(), 2))
        } else if trim.starts_with("\\[") {
            Some(("\\]".to_string(), 2))
        } else if let Some(rest) = trim.strip_prefix("\\begin{") {
            rest.split_once('}').and_then(|(env, _)| {
                [
                    "equation",
                    "equation*",
                    "align",
                    "align*",
                    "aligned",
                    "gather",
                    "gather*",
                    "displaymath",
                    "math",
                    "split",
                    "eqnarray",
                    "eqnarray*",
                ]
                .contains(&env)
                .then(|| (format!("\\end{{{env}}}"), env.len() + 8))
            })
        } else {
            None
        };
        if let Some((end, offset)) = ending {
            flush(&mut paragraph, &mut seen, address, faults);
            if !trim.get(offset..).unwrap_or_default().contains(&end) {
                math = Some(end);
            }
            continue;
        }
        if lower.starts_with("<blockquote") {
            flush(&mut paragraph, &mut seen, address, faults);
            html_quote = lower
                .matches("<blockquote")
                .count()
                .saturating_sub(lower.matches("</blockquote").count());
            continue;
        }
        if trim.starts_with('>') {
            flush(&mut paragraph, &mut seen, address, faults);
            lazy_quote = true;
            continue;
        }
        if line.starts_with("    ") || line.starts_with('\t') {
            if !paragraph.is_empty() {
                paragraph.push((index + 1, *line));
            }
            continue;
        }
        if trim.starts_with('|') || trim.starts_with("<!--") || trim.starts_with("![") {
            flush(&mut paragraph, &mut seen, address, faults);
            continue;
        }
        if !title.is_empty() && heading(trim) == Some(title) {
            flush(&mut paragraph, &mut seen, address, faults);
            repeated_titles += 1;
            faults.add(
                address,
                "R19",
                "repeated_title",
                &format!("heading:{repeated_titles}"),
                &format!("repeated Card title at body line {}", index + 1),
            );
            continue;
        }
        if HEADING.is_match(trim) || SECTION.is_match(trim) {
            flush(&mut paragraph, &mut seen, address, faults);
            continue;
        }
        paragraph.push((index + 1, *line));
    }
    flush(&mut paragraph, &mut seen, address, faults);
}

/// The limits of card-style-check.ps1 and self-check.ps1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_sentences: i64,
    pub max_chars: i64,
    pub max_steps: i64,
    pub max_explain_sentences: i64,
    pub max_explain_chars: i64,
    pub min_idea_chars: i64,
    pub max_idea_chars: i64,
    pub min_reason: i64,
    /// The most derivation steps one Term Card holds (design 11.2, R0289).
    pub max_term_steps: i64,
    /// The most measured prose units one Term Card holds, formulas and tables excluded; 0 (the
    /// default since 2026-10-26, the user's request) sets no bound: a long term is not restricted.
    pub max_term_prose: i64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_sentences: 12,
            max_chars: 400,
            max_steps: 6,
            max_explain_sentences: 16,
            max_explain_chars: 600,
            min_idea_chars: 40,
            max_idea_chars: 300,
            min_reason: 8,
            max_term_steps: 12,
            max_term_prose: 0,
        }
    }
}

impl Limits {
    /// The prose bound a Term Card is held to: `max_term_prose`, or none when it is 0.
    pub fn term_prose_bound(&self) -> i64 {
        if self.max_term_prose > 0 { self.max_term_prose } else { i64::MAX }
    }

    /// The eight limits under their flag names, as the check contract publishes
    /// them (design 8.3, R0202).
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "max_sentences": self.max_sentences,
            "max_chars": self.max_chars,
            "max_steps": self.max_steps,
            "max_explain_sentences": self.max_explain_sentences,
            "max_explain_chars": self.max_explain_chars,
            "min_idea_chars": self.min_idea_chars,
            "max_idea_chars": self.max_idea_chars,
            "min_reason": self.min_reason,
            "max_term_steps": self.max_term_steps,
            "max_term_prose": self.max_term_prose,
        })
    }

    /// Read the published limits. A missing or non-integer limit is a refusal:
    /// a check with unknown limits cannot be compared with the gate's.
    pub fn from_json(value: &serde_json::Value) -> Result<Self, String> {
        let number = |name: &str| -> Result<i64, String> {
            value
                .get(name)
                .and_then(serde_json::Value::as_i64)
                .ok_or_else(|| format!("no `{name}`"))
        };
        Ok(Self {
            max_sentences: number("max_sentences")?,
            max_chars: number("max_chars")?,
            max_steps: number("max_steps")?,
            max_explain_sentences: number("max_explain_sentences")?,
            max_explain_chars: number("max_explain_chars")?,
            min_idea_chars: number("min_idea_chars")?,
            max_idea_chars: number("max_idea_chars")?,
            min_reason: number("min_reason")?,
            max_term_steps: number("max_term_steps")?,
            max_term_prose: number("max_term_prose")?,
        })
    }
}

/// What one diagnostic does if the policy enforces its rule for the Card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// A configured blocking rule: the gate refuses while the fault is not inherited.
    Block,
    /// Advice: the line is reported, never refused.
    Advise,
}

impl Severity {
    /// The policy name, as the published contract spells it.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Block => "block",
            Severity::Advise => "advise",
        }
    }
}

/// What evidence one diagnostic reads: the Card and its bounded context, or a
/// whole-archive graph fact that stays an audit path (design 6.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// The Card's own text and the bounded context a caller materialized for it.
    Local,
    /// Other Cards' Links or titles: reported by the audit, not decided per write.
    Graph,
}

impl Scope {
    /// The policy name, as the published contract spells it.
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Local => "local",
            Scope::Graph => "graph",
        }
    }
}

/// One fault under the structured contract (design 5.4): the human line is
/// formatted from it, and the legacy comparison uses `rule`, `fault`, and `key`
/// rather than the clipped line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// The Card the fault belongs to.
    pub address: String,
    /// The rule id, as the existing reports spell it (`R2`, `E2`, ...).
    pub rule: String,
    /// The rule-specific fault code (`sentence_length`, `missing_labels`, ...).
    pub fault: String,
    /// The rule-specific semantic key, built from the full offending fragment or
    /// missing requirement and never from a line number or the clipped display.
    pub key: String,
    /// The full message, before the display clip.
    pub message: String,
    /// The severity the rule's default policy gives this fault.
    pub severity: Severity,
    /// Whether an old version of the same Card already carried this occurrence.
    pub legacy: bool,
    /// Whether this is an advisory subcase of a rule whose default is blocking
    /// (I7's unknown source): the policy can never promote it (design 6.2).
    pub subcase: bool,
    /// What evidence the diagnostic read.
    pub scope: Scope,
}

impl Diagnostic {
    /// `<address> <rule> [(advisory) ]<message>`, the text on one line, white
    /// space collapsed and clipped to 90 UTF-16 units with `…`.
    pub fn line(&self) -> String {
        let mut display = String::new();
        if self.severity == Severity::Advise {
            display.push_str("(advisory) ");
        }
        display.push_str(&self.message);
        let units = utf16_form(&display);
        let collapsed = SPACES.replace_all(&units, " ");
        let trimmed = collapsed.trim();
        let units = trimmed.chars().collect::<Vec<_>>();
        let clip = if units.len() > 90 {
            let mut kept = units[..89].iter().collect::<String>();
            // A cut through a surrogate pair leaves half a character; .NET writes U+FFFD for it.
            if is_high_stand_in(units[88]) && is_low_stand_in(units[89]) {
                kept.pop();
                kept.push('\u{FFFD}');
            }
            kept.push('…');
            kept
        } else {
            trimmed.to_string()
        };
        from_utf16_form(&format!(
            "{} {} {}",
            utf16_form(&self.address),
            self.rule,
            clip
        ))
    }

    /// Whether this diagnostic blocks a write under its current severity.
    pub fn blocking(&self) -> bool {
        self.severity == Severity::Block
    }

    /// The published structured contract (design 5.4/R0111): the eight fields
    /// with the policy names of the severity and the scope, so the service gate,
    /// an MCP reply, a batch result and the CLI report carry one shape.
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "address": self.address,
            "rule": self.rule,
            "fault": self.fault,
            "key": self.key,
            "message": self.message,
            "severity": self.severity.as_str(),
            "legacy": self.legacy,
            "scope": self.scope.as_str(),
        })
    }
}

/// One run of card-style-check.ps1: the limits, the level, the term table, and
/// the shared parts that follow another model's term table.
pub struct Check {
    pub(crate) limits: Limits,
    pub(crate) require_explain: bool,
    pub(crate) level: String,
    /// The Workspace title context I6 compares with; empty means one tree only.
    context: TitleContext,
    terms: Terms,
    /// Every term the table keeps (its left column, rows without a rejected word too), longest first.
    kept: Vec<String>,
    shared: Vec<String>,
    /// The archive's effective policy, loaded once per run (S03). Without one
    /// the rules keep the checker's own severity, the pre-S03 behaviour.
    policies: Option<crate::policy::PolicySet>,
}

impl Check {
    pub fn new(limits: Limits, require_explain: bool, level: &str) -> Self {
        Self {
            limits,
            require_explain,
            level: level.to_string(),
            context: TitleContext::default(),
            terms: Vec::new(),
            kept: Vec::new(),
            shared: Vec::new(),
            policies: None,
        }
    }

    /// The Workspace title context from the mode's bulk reader, or a bundle's context.
    pub fn with_context(mut self, context: TitleContext) -> Self {
        self.context = context;
        self
    }

    /// The archive's effective policy snapshot. One value decides the whole run,
    /// including both sides of a legacy comparison (S03-AC05).
    pub fn with_policies(mut self, policies: crate::policy::PolicySet) -> Self {
        self.policies = Some(policies);
        self
    }

    /// The term table from the setting Card's body.
    pub fn with_terms(mut self, setting_body: Option<&str>) -> Self {
        self.terms = setting_body
            .map(|body| read_terms(&utf16_form(body)))
            .unwrap_or_default();
        let mut kept: Vec<String> = setting_body
            .map(|body| term_table(&utf16_form(body)).into_iter().map(|row| row.term).collect())
            .unwrap_or_default();
        kept.sort_by_key(|term| std::cmp::Reverse(term.chars().count()));
        self.kept = kept;
        self
    }

    pub fn with_shared(mut self, shared: Vec<String>) -> Self {
        self.shared = shared.iter().map(|address| utf16_form(address)).collect();
        self
    }

    /// Every fault of the Cards (`{"address","title","body","facet",...}`) as
    /// structured diagnostics, in the order card-style-check.ps1 writes them.
    /// An entry may name its own `level`: the old authored version is checked
    /// with its old metadata, the proposed version with the proposed metadata
    /// (design 6.3). Without one, the run's level applies.
    pub fn report(&self, cards: &[Value]) -> Vec<Diagnostic> {
        let mut faults = Faults::default();
        let mut prose_chars = HashMap::new();
        for card in cards {
            let address = text(prop(card, "address"));
            let chars = self.language(card, &address, &mut faults);
            // The Term content rules read the same authored text a draft renderer wrote or a
            // hand edit stored, so both go through one evaluator (design 11.3, S09-AC08).
            self.term_rules(card, &address, &mut faults);
            // T7's label and retained-role advice applies to a Term use on any Card, not only a
            // Term Card; the bounded target facts travel with the entry.
            self.term_link_advice(
                &address,
                &text(prop(card, "body")),
                card,
                &mut faults,
            );
            prose_chars.insert(key(&address), chars);
        }
        self.layer(cards, &prose_chars, &mut faults);
        match &self.policies {
            Some(policies) => policies.apply(cards, faults.diagnostics),
            None => faults.diagnostics,
        }
    }

    /// Every fault of the Cards, in the order card-style-check.ps1 writes them.
    pub fn run(&self, cards: &[Value]) -> Vec<String> {
        self.report(cards)
            .iter()
            .map(Diagnostic::line)
            .collect::<Vec<_>>()
    }

    /// The level this Card is checked at: its own metadata when the entry names
    /// it, otherwise the run's level.
    fn level_of(&self, card: &Value) -> String {
        let own = text(prop(card, "level"));
        if own.is_empty() {
            self.level.clone()
        } else {
            own
        }
    }

    /// R2 to R19 for one Card; returns its prose characters (I2 reads them).
    fn language(&self, card: &Value, address: &str, faults: &mut Faults) -> i64 {
        let level = self.level_of(card);
        let title = text(prop(card, "title"));
        let authored_body = text(prop(card, "body"));
        // A Term Card reports its balanced-formula fault as T10, the Term-facing label of R22
        // (design 11.3): one fault, not two labels for one predicate.
        let is_term = ps_eq(&text(prop(card, "type")), "term");
        let formula_rule = if is_term { "T10" } else { "R22" };
        corruption(&authored_body, &title, address, faults);
        if open_math(&authored_body) {
            faults.add(
                address,
                formula_rule,
                "open_formula",
                &format!("body:{authored_body}"),
                "the body leaves an inline formula open (an odd number of $; write a currency as 美元 or \\$)",
            );
        }
        let facet = text(prop(card, "facet"));
        // A root, an anchor's section index, a hub's navigation list, and a register part's term
        // rows are not claims; the standalone rules read the authored prose that states one
        // (D126's Literature prose excludes a locator, and an index line is a locator). A Term
        // Card's `首次定义：§3` line is a locator too, and the pilot rules never covered it.
        let navigation = ps_in(
            &text(prop(card, "category")),
            &["anchor", "root", "hub", "register-part", "restatement-entry"],
        ) || is_term;
        let shared = ps_in(address, &self.shared);
        let derivation = is_derivation_facet(&facet);
        // A Literature note carries the source's argument, not one claim: its row turns the
        // claim-title and sentence rules off and only advises on a long title (issue #184).
        let literature = is_literature_note(card);
        if !title.is_empty() {
            let measure = measure(&title);
            let limit = if literature {
                LITERATURE_TITLE_CHARS
            } else if measure.words {
                25
            } else {
                40
            };
            if measure.size > limit {
                faults.add_row(
                    address,
                    "R12",
                    "title_size",
                    &format!("{} {} > {limit}: {title}", measure.size, measure.unit()),
                    &format!("({} {} > {limit}) {title}", measure.size, measure.unit()),
                    literature,
                );
            }
            if SEMICOLON.is_match(&plain(&title)) {
                faults.add_row(
                    address,
                    "R12",
                    "two_claims",
                    &title,
                    &format!("two claims: {title}"),
                    literature,
                );
            }
            if open_math(&title) {
                faults.add(
                    address,
                    formula_rule,
                    "open_formula",
                    &format!("title:{title}"),
                    &format!("the title leaves an inline formula open (an odd number of $): {title}"),
                );
            }
            // A title names its content, so a bare local number is the same fault there.
            local_reference(faults, address, &title);
            unnamed_referent(faults, address, &title);
        }
        let mut sentences = 0;
        let mut chars = 0;
        let mut steps = 0;
        // A Derivation part: its sections, the explanation (思路, 说明, 解读), and the 说明 lines per step.
        let mut section = String::new();
        let mut order = Vec::new();
        let mut explain_sentences = 0;
        let mut explain_chars = 0;
        let mut thought = 0;
        let mut reading = 0;
        let mut notes = BTreeMap::new();
        // A Model Card's `**结构**` list is the compiler's navigation over its parts (a facet, a Link and the
        // part's title per line), not authored prose: each title is checked as its part's own title. On
        // grossman2002integration trials 10 and 11 a long Derivation facet made a 结构 line an R2 fault in a done
        // Model Card that the bounded correction may not reopen.
        let structure = structure_lines(&text(prop(card, "body")));
        for line in prose_lines(&text(prop(card, "body"))) {
            if structure.contains(line.text.trim()) {
                continue;
            }
            // A Term Card's `术语：`, `不用：`, `首次定义：` … lines are authored metadata, not
            // prose: the language rules read the explanation and sections, and T1/T2 read the
            // head lines. Without this, R6 would reject the term's own `不用：` list.
            if is_term && crate::term::head_line(&line.text).is_some() {
                continue;
            }
            if derivation && let Some(caps) = SECTION.captures(&line.text) {
                section = caps[1].to_string();
                order.push(section.clone());
                continue;
            }
            // The position line of 思路 (`第 2 张，共 3 张：…`) is the compiler's, not the writer's text.
            if derivation && section == "思路" && POSITION.is_match(&line.text) {
                continue;
            }
            let is_step = derivation && line.step;
            if is_step {
                steps += 1;
            }
            let is_note = derivation && !is_step && section == "步骤" && NOTE.is_match(&line.text);
            let line_text = if is_note {
                NOTE_LABEL.replace(&line.text, "").into_owned()
            } else {
                line.text.clone()
            };
            // The 来源 line after 解读 is not part of the explanation.
            let is_explain = is_note
                || (derivation
                    && (section == "思路" || section == "解读")
                    && !SOURCE_LABEL.is_match(&line.text));
            if is_note {
                if steps == 0 {
                    faults.add(
                        address,
                        "R17",
                        "note_before_step",
                        &line_text,
                        &format!("a 说明 line before the first step: {line_text}"),
                    );
                } else if notes.contains_key(&steps) {
                    faults.add(
                        address,
                        "R17",
                        "second_note",
                        &line_text,
                        &format!("a second 说明 line under step {steps}: {line_text}"),
                    );
                }
                notes.insert(steps, 0);
            }
            // grossman2002integration trial 10: the generated document line and the index line naming the book
            // were the Literature Card's two R2 faults, and the bounded correction may not reopen a root.
            if DOCUMENT_LINE.is_match(&line_text) {
                continue;
            }
            // acemoglu2007equilibrium: the Bridge judge measured its sentence without the label it is applied under.
            let line_text = BRIDGE_LABEL.replace(&line_text, "").into_owned();
            for sentence in split_sentences(&line_text) {
                self.sentence(faults, address, shared, &sentence, is_step, literature);
                // A source locator (`来源：[[book]] §11.D`) may carry the number.
                if !navigation && !SOURCE_LINE.is_match(&line.text) {
                    local_reference(faults, address, &sentence);
                    unnamed_referent(faults, address, &sentence);
                }
                if is_step {
                    continue;
                }
                let measure = measure(&sentence);
                let size = if measure.words { 0 } else { measure.size };
                if is_explain {
                    explain_sentences += 1;
                    explain_chars += size;
                    if is_note {
                        *notes.entry(steps).or_insert(0) += 1;
                    } else if !ps_eq(sentence.trim(), "待写") {
                        if section == "思路" {
                            thought += 1;
                        } else {
                            reading += 1;
                        }
                    }
                } else {
                    sentences += 1;
                    chars += size;
                }
            }
        }
        // R15 to R17: a formal model's Derivation part with an explanation, or any one with --require-explain.
        if derivation && !ps_eq(&level, "concept") {
            let has = order.iter().any(|name| name == "思路" || name == "解读");
            if has || self.require_explain {
                if order.join(",") != SECTIONS.join(",") {
                    let found = if order.is_empty() {
                        "none".to_string()
                    } else {
                        order.join(", ")
                    };
                    faults.add(
                        address,
                        "R15",
                        "sections",
                        &order.join(","),
                        &format!("sections {found}; want {}", SECTIONS.join(", ")),
                    );
                }
                if order.iter().any(|name| name == "思路") && thought > 5 {
                    faults.add(
                        address,
                        "R16",
                        "thought_sentences",
                        &format!("{thought} sentences > 5"),
                        &format!("(思路 has {thought} sentences > 5)"),
                    );
                }
                if order.iter().any(|name| name == "解读") && reading > 6 {
                    faults.add(
                        address,
                        "R16",
                        "reading_sentences",
                        &format!("{reading} sentences > 6"),
                        &format!("(解读 has {reading} sentences > 6)"),
                    );
                }
                for (step, count) in &notes {
                    if *count > 2 {
                        faults.add(
                            address,
                            "R17",
                            "note_sentences",
                            &format!("{step}:{count} sentences > 2"),
                            &format!("(说明 under step {step} has {count} sentences > 2)"),
                        );
                    }
                }
                let limits = &self.limits;
                if explain_sentences > limits.max_explain_sentences {
                    faults.add(
                        address,
                        "R17",
                        "explanation_size",
                        &format!("sentences:{explain_sentences}/{}", limits.max_explain_sentences),
                        &format!(
                            "({explain_sentences} explanation sentences > {}) shorten the explanation",
                            limits.max_explain_sentences
                        ),
                    );
                } else if explain_chars > limits.max_explain_chars {
                    faults.add(
                        address,
                        "R17",
                        "explanation_size",
                        &format!("characters:{explain_chars}/{}", limits.max_explain_chars),
                        &format!(
                            "({explain_chars} explanation characters > {}) shorten the explanation",
                            limits.max_explain_chars
                        ),
                    );
                }
            }
        }
        // Card size: only a Model part of a formal model; the Model Card is the model's entry
        // point, and a concept model keeps its few parts whole.
        if ps_eq(&level, "concept") {
            return chars;
        }
        let limits = &self.limits;
        if !facet.is_empty() {
            if sentences > limits.max_sentences {
                faults.add(
                    address,
                    "R13",
                    "part_size",
                    &format!("sentences:{sentences}/{}", limits.max_sentences),
                    &format!(
                        "({sentences} sentences > {}) split the part into two or more parts",
                        limits.max_sentences
                    ),
                );
            } else if chars > limits.max_chars {
                faults.add(
                    address,
                    "R13",
                    "part_size",
                    &format!("characters:{chars}/{}", limits.max_chars),
                    &format!(
                        "({chars} characters > {}) split the part into two or more parts",
                        limits.max_chars
                    ),
                );
            }
        }
        if steps > limits.max_steps {
            faults.add(
                address,
                "R14",
                "steps",
                &format!("steps:{steps}/{}", limits.max_steps),
                &format!(
                    "({steps} steps > {}) split the derivation into a chain of Derivation parts",
                    limits.max_steps
                ),
            );
        }
        chars
    }

    /// The Term content rules T1–T10 for one named Term Card (design 11.3, S09 / #152). A Term
    /// list has no term content template; the book audit (`get_terms`) reports its coverage.
    fn term_rules(&self, card: &Value, address: &str, faults: &mut Faults) {
        if !ps_eq(&text(prop(card, "type")), "term") {
            return;
        }
        let term = text(prop(card, "term"));
        if term.is_empty() {
            return;
        }
        let title = text(prop(card, "title"));
        let body = text(prop(card, "body"));
        let role = {
            let role = ps_string(prop(card, "term_role")).to_lowercase();
            if role.is_empty() { "card".to_string() } else { role }
        };
        let template = crate::term::parse_template(&body);
        self.term_template(address, &role, &template, card, faults);
        self.term_identity(address, &role, &title, &term, &template, card, faults);
        self.term_math(address, &role, &template, card, faults);
        self.term_steps(address, &template, faults);
        self.term_symbols(address, &template, faults);
        self.term_skipped_steps(address, &body, faults);
        let prose = crate::term::prose_units(&body);
        if prose > self.limits.term_prose_bound() {
            faults.add(
                address,
                "T9",
                "prose_size",
                &format!("prose:{prose}/{}", self.limits.max_term_prose),
                &format!(
                    "({prose} prose units > {}) split the term into a head and continuation parts",
                    self.limits.max_term_prose
                ),
            );
        }
    }

    /// T1: the known ordered labels and sections, the required common fields, and the
    /// role-specific template (R0291, R0292, R0298, R0302).
    fn term_template(
        &self,
        address: &str,
        role: &str,
        template: &crate::term::Template,
        card: &Value,
        faults: &mut Faults,
    ) {
        for (line, text) in &template.unknown_lines {
            faults.add(
                address,
                "T1",
                "unknown_line",
                text,
                &format!("line {line} is no Term head line: {text}"),
            );
        }
        for (line, heading) in &template.unknown_sections {
            faults.add(
                address,
                "T1",
                "unknown_section",
                heading,
                &format!("line {line} is no Term section: ## {heading}"),
            );
        }
        // A repeated label is ambiguous: the reader cannot tell which value decides.
        let mut labels: BTreeMap<&str, usize> = BTreeMap::new();
        for line in &template.lines {
            *labels.entry(line.label.as_str()).or_default() += 1;
        }
        for (label, count) in &labels {
            if *count > 1 {
                faults.add(
                    address,
                    "T1",
                    "duplicate_label",
                    label,
                    &format!("{count} `{label}：` lines in one Term Card"),
                );
            }
        }
        let mut positions = Vec::new();
        for line in &template.lines {
            if let Some(position) = head_position(&line.label) {
                positions.push((line.label.clone(), position));
            }
        }
        if positions
            .windows(2)
            .any(|pair| pair[0].1 > pair[1].1)
        {
            let found = positions
                .iter()
                .map(|(label, _)| label.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            faults.add(
                address,
                "T1",
                "label_order",
                &found,
                &format!("head lines out of template order: {found}"),
            );
        }
        let required: &[&str] = if role == "part" {
            &["类别", crate::term::CONTINUATION_LABEL]
        } else {
            &["术语", "不用", "类别", "首次定义"]
        };
        for label in required {
            if template.line(label).is_none() {
                faults.add(
                    address,
                    "T1",
                    "missing_label",
                    label,
                    &format!("missing the `{label}：` line"),
                );
            }
        }
        if let Some(line) = template.line("类别")
            && crate::term::Category::from_label(&line.value).is_none()
        {
            faults.add(
                address,
                "T1",
                "category",
                &line.value,
                &format!(
                    "`类别：{}` is no Term category; expected `{}` or `{}`",
                    line.value,
                    crate::term::CATEGORY_CONCEPT,
                    crate::term::CATEGORY_MATH
                ),
            );
        }
        // The authored opt-out: absence means enabled, and the two lines travel together.
        let off = template.line(crate::term::AUTO_LINK_LABEL);
        let reason = template.line(crate::term::AUTO_LINK_REASON_LABEL);
        match (off, reason) {
            (Some(line), _) if line.value != crate::term::AUTO_LINK_OFF => faults.add(
                address,
                "T1",
                "auto_link_value",
                &line.value,
                &format!(
                    "`{}：{}` is no opt-out; absence means enabled and only `{}` disables it",
                    crate::term::AUTO_LINK_LABEL,
                    line.value,
                    crate::term::AUTO_LINK_OFF
                ),
            ),
            (Some(_), None) => faults.add(
                address,
                "T1",
                "auto_link_reason",
                crate::term::AUTO_LINK_REASON_LABEL,
                "a disabled automatic link needs its `自动链接理由：` line",
            ),
            (Some(_), Some(line)) if line.value.is_empty() => faults.add(
                address,
                "T1",
                "auto_link_reason",
                crate::term::AUTO_LINK_REASON_LABEL,
                "the `自动链接理由：` line is empty",
            ),
            (None, Some(_)) => faults.add(
                address,
                "T1",
                "auto_link_reason_orphan",
                crate::term::AUTO_LINK_REASON_LABEL,
                "an `自动链接理由：` line without `自动链接：否` decides nothing",
            ),
            _ => {}
        }
        // A part's `续：` line names the head it continues and says what it inherits (R0283).
        if role == "part"
            && let Some(line) = template.line(crate::term::CONTINUATION_LABEL)
        {
            let head = text(prop(card, "term_head"));
            match link_occurrences(&line.value)
                .into_iter()
                .find(|occurrence| occurrence.form == LinkForm::Ordinary)
            {
                None => faults.add(
                    address,
                    "T1",
                    "continuation_link",
                    crate::term::CONTINUATION_LABEL,
                    "the `续：` line carries no Link to the head it continues",
                ),
                Some(occurrence) if !head.is_empty() && occurrence.target != head => faults.add(
                    address,
                    "T1",
                    "continuation_target",
                    occurrence.target,
                    &format!(
                        "the `续：` line names `{}`, not the head `{head}`",
                        occurrence.target
                    ),
                ),
                Some(_) => {}
            }
        }
        // Sections: known order, no repeat outside 推导, and the common section.
        let mut section_positions = Vec::new();
        let mut section_counts: BTreeMap<&str, usize> = BTreeMap::new();
        for section in &template.sections {
            let Some(position) = section_position(&section.name) else {
                continue;
            };
            section_positions.push((section.name.clone(), position));
            *section_counts.entry(section.name.as_str()).or_default() += 1;
        }
        if section_positions
            .windows(2)
            .any(|pair| pair[0].1 > pair[1].1)
        {
            let found = section_positions
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            faults.add(
                address,
                "T1",
                "section_order",
                &found,
                &format!("sections out of template order: {found}"),
            );
        }
        for (name, count) in &section_counts {
            if *count > 1 && *name != "推导" {
                faults.add(
                    address,
                    "T1",
                    "duplicate_section",
                    name,
                    &format!("{count} `## {name}` sections in one Term Card"),
                );
            }
        }
        if role == "part" {
            if template.sections.is_empty() {
                faults.add(
                    address,
                    "T1",
                    "missing_section",
                    "续",
                    "a continuation part carries at least one section",
                );
            }
        } else if !template.sections.iter().any(|section| section.name == "释义") {
            faults.add(
                address,
                "T1",
                "missing_section",
                "释义",
                "a Term Card carries the `## 释义` section",
            );
        }
        for section in &template.sections {
            if section.content.iter().all(|line| line.trim().is_empty()) {
                faults.add(
                    address,
                    "T1",
                    "empty_section",
                    &section.name,
                    &format!("`## {}` holds no content; leave an empty section out", section.heading),
                );
            }
        }
        // The source trailer is the last line and names a book Card.
        match &template.trailer {
            None => {
                faults.add(
                    address,
                    "T1",
                    "missing_source",
                    crate::term::SOURCE_LABEL,
                    "a Term Card ends with its `来源：[[book]] locator` line",
                );
                if template.misplaced_trailer {
                    faults.add(
                        address,
                        "T1",
                        "source_not_last",
                        crate::term::SOURCE_LABEL,
                        "the `来源：` line must be the last line of the Card",
                    );
                }
            }
            Some(line) => {
                if template.misplaced_trailer || template.after_trailer {
                    faults.add(
                        address,
                        "T1",
                        "source_not_last",
                        crate::term::SOURCE_LABEL,
                        "the `来源：` line must be the last line of the Card",
                    );
                }
                if !link_occurrences(&line.value)
                    .iter()
                    .any(|occurrence| occurrence.form == LinkForm::Ordinary)
                {
                    faults.add(
                        address,
                        "T1",
                        "source_link",
                        &line.value,
                        &format!("the `来源：` line names no book Link: {}", line.value),
                    );
                }
            }
        }
    }

    /// T2: the head title, stored name, `术语：` line and table row agree; a part title uses the
    /// continuation format and keeps the same stored term (R0292, R0285).
    fn term_identity(
        &self,
        address: &str,
        role: &str,
        title: &str,
        term: &str,
        template: &crate::term::Template,
        card: &Value,
        faults: &mut Faults,
    ) {
        if let Some(line) = template.line("术语")
            && line.value != term
        {
            faults.add(
                address,
                "T2",
                "term_line",
                &line.value,
                &format!("`术语：{}` differs from the stored term `{term}`", line.value),
            );
        }
        if role == "part" {
            if !continuation_title(title) {
                faults.add(
                    address,
                    "T2",
                    "part_title",
                    title,
                    &format!(
                        "`{title}` is no continuation title; expected `推导（续 k/n）：…`"
                    ),
                );
            }
            return;
        }
        if title != term {
            faults.add(
                address,
                "T2",
                "title",
                title,
                &format!("the title `{title}` is not the exact term `{term}`"),
            );
        }
        if role == "list" {
            return;
        }
        let rows = strings(prop(card, "term_rows"));
        if !rows.is_empty() && !rows.iter().any(|row| row == term) {
            faults.add(
                address,
                "T2",
                "row",
                term,
                &format!("`{term}` is no row of this list's term table"),
            );
        }
    }

    /// T3: a math head establishes its hypotheses, displayed definition, symbol table and
    /// derivation; a concept term carries no formal derivation section (R0293, R0286, AC07).
    fn term_math(
        &self,
        address: &str,
        role: &str,
        template: &crate::term::Template,
        card: &Value,
        faults: &mut Faults,
    ) {
        let Some(line) = template.line("类别") else {
            return;
        };
        let Some(category) = crate::term::Category::from_label(&line.value) else {
            return;
        };
        match category {
            crate::term::Category::Concept => {
                for name in crate::term::FORMAL_SECTIONS {
                    if !template.named(name).is_empty() {
                        faults.add(
                            address,
                            "T3",
                            "formal_section_on_concept",
                            name,
                            &format!("a concept term carries no formal `## {name}` section"),
                        );
                    }
                }
                let symbol_table = template
                    .sections
                    .iter()
                    .any(|section| section.content.iter().any(|line| is_symbol_table_line(line)));
                if symbol_table {
                    faults.add(
                        address,
                        "T3",
                        "formal_section_on_concept",
                        "符号表",
                        "a concept term carries no symbol table",
                    );
                }
            }
            crate::term::Category::Math => {
                let definitions = template.named("数学定义");
                if definitions.is_empty() {
                    if role != "part" {
                        faults.add(
                            address,
                            "T3",
                            "missing_section",
                            "数学定义",
                            "a mathematical term carries the `## 数学定义` section",
                        );
                    }
                } else {
                    let definition = definitions[0];
                    if !definition
                        .content
                        .iter()
                        .any(|line| HYPOTHESIS_LINE.is_match(line.trim()))
                    {
                        faults.add(
                            address,
                            "T3",
                            "missing_hypotheses",
                            "前提",
                            "a mathematical definition lists labelled hypotheses `- (H1) …`",
                        );
                    }
                    if !definition.content.iter().any(|line| display_formula(line)) {
                        faults.add(
                            address,
                            "T3",
                            "missing_definition",
                            "$$",
                            "a mathematical definition is a display formula",
                        );
                    }
                    if template.symbol_table().is_empty() {
                        faults.add(
                            address,
                            "T3",
                            "missing_symbols",
                            "符号表",
                            "a mathematical definition carries its `符号表` table",
                        );
                    }
                }
                if template.named("推导").is_empty() {
                    faults.add(
                        address,
                        "T3",
                        "missing_derivation",
                        "推导",
                        "a mathematical term carries its `## 推导：…` section",
                    );
                }
            }
        }
        // A continuation part inherits its head's category; a different one is a chain fault,
        // whichever of the two categories the part itself claims (R0286, R0288).
        let head_category = crate::term::Category::from_label(&text(prop(
            card,
            "term_head_category",
        )));
        if role == "part"
            && let Some(head) = head_category
            && head != category
        {
            faults.add(
                address,
                "T3",
                "chain_category",
                &line.value,
                &format!(
                    "this part says `类别：{}` but its head says `{}`",
                    line.value,
                    head.label()
                ),
            );
        }
    }

    /// T5: numbered steps, a basis per step, a final result line, and the per-Card step bound
    /// (R0295, R0289).
    fn term_steps(&self, address: &str, template: &crate::term::Template, faults: &mut Faults) {
        let mut total = 0i64;
        for section in template.named("推导") {
            let claim = section
                .heading
                .strip_prefix("推导：")
                .unwrap_or_default()
                .trim()
                .to_string();
            if claim.is_empty() {
                faults.add(
                    address,
                    "T5",
                    "missing_claim",
                    &section.heading,
                    "a derivation heading names what it derives: `## 推导：<claim>`",
                );
            }
            let steps = crate::term::step_blocks(section);
            if steps.is_empty() {
                faults.add(
                    address,
                    "T5",
                    "no_steps",
                    &section.heading,
                    &format!("`## {}` holds no numbered step", section.heading),
                );
                continue;
            }
            let numbers = steps.iter().map(|step| step.number).collect::<Vec<_>>();
            let expected = (1..=steps.len() as i64).collect::<Vec<_>>();
            if numbers != expected {
                faults.add(
                    address,
                    "T5",
                    "step_numbering",
                    &numbers
                        .iter()
                        .map(i64::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                    &format!(
                        "the steps of `## {}` are numbered {:?}; expected 1…{}",
                        section.heading,
                        numbers,
                        steps.len()
                    ),
                );
            }
            for step in &steps {
                total += 1;
                if basis_missing(step.basis()) {
                    faults.add(
                        address,
                        "T5",
                        "missing_basis",
                        &format!("step:{}", step.number),
                        &format!("step {} names no `依据：` basis", step.number),
                    );
                }
                if step.text().trim().is_empty() {
                    faults.add(
                        address,
                        "T5",
                        "empty_step",
                        &format!("step:{}", step.number),
                        &format!("step {} holds no text", step.number),
                    );
                }
            }
            if crate::term::result_line(section).is_none()
                && !section
                    .content
                    .iter()
                    .any(|line| line.trim() == crate::term::CONTINUES)
            {
                faults.add(
                    address,
                    "T5",
                    "missing_result",
                    &section.heading,
                    &format!("`## {}` ends without a `结果：` line", section.heading),
                );
            }
        }
        if total > self.limits.max_term_steps {
            faults.add(
                address,
                "T5",
                "steps",
                &format!("steps:{total}/{}", self.limits.max_term_steps),
                &format!(
                    "({total} derivation steps > {}) split the derivation into continuation parts",
                    self.limits.max_term_steps
                ),
            );
        }
    }

    /// T4 (advice): every formula identifier is covered by the head's symbol table. The LaTeX
    /// scan is approximate, so the fault never blocks a write (R0294).
    fn term_symbols(&self, address: &str, template: &crate::term::Template, faults: &mut Faults) {
        let Some(line) = template.line("类别") else {
            return;
        };
        if crate::term::Category::from_label(&line.value) != Some(crate::term::Category::Math) {
            return;
        }
        let symbols = template.symbol_table();
        if symbols.is_empty() {
            return;
        }
        let mut known = BTreeSet::new();
        for (symbol, _, _) in &symbols {
            known.extend(crate::term::latex_identifiers(symbol));
        }
        let mut formulas = Vec::new();
        for section in template.named("数学定义") {
            formulas.extend(math_formulas(&section.content));
        }
        for section in template.named("推导") {
            formulas.extend(math_formulas(&section.content));
        }
        let mut reported = BTreeSet::new();
        for formula in formulas {
            for identifier in crate::term::latex_identifiers(&formula) {
                if known.contains(&identifier) || !reported.insert(identifier.clone()) {
                    continue;
                }
                faults.advise(
                    address,
                    "T4",
                    "symbol_uncovered",
                    &identifier,
                    &format!(
                        "the formula symbol `{identifier}` is not in the symbol table; a LaTeX scan is approximate"
                    ),
                );
            }
        }
    }

    /// T6: no unexpanded `易知`, `显然`, `不难看出`, `容易证明` or `同理可得` without a named
    /// step; a named earlier-step reference is allowed (R0296, AC12).
    fn term_skipped_steps(&self, address: &str, body: &str, faults: &mut Faults) {
        for line in prose_lines(body) {
            if SOURCE_LINE.is_match(&line.text) {
                continue;
            }
            let masked = QUOTED.replace_all(&line.text, "");
            for sentence in split_sentences(&masked) {
                let plain = plain(&sentence);
                if NAMED_STEP.is_match(&plain) {
                    continue;
                }
                for phrase in SKIPPED_STEP.find_iter(&plain) {
                    faults.add(
                        address,
                        "T6",
                        "skipped_step",
                        &format!("{}|{sentence}", phrase.as_str()),
                        &format!(
                            "`{}` without the named step it skips: {sentence}",
                            phrase.as_str()
                        ),
                    );
                }
            }
        }
    }

    /// T7 (advice): a Term link's label agrees with the head's name, and a retained legacy
    /// target is not a head. A new invalid target is already a mandatory Link invariant (R0297).
    fn term_link_advice(&self, address: &str, body: &str, card: &Value, faults: &mut Faults) {
        let invalid = strings(prop(card, "term_invalid_targets"));
        let targets = prop(card, "term_targets").and_then(Value::as_object);
        if invalid.is_empty() && targets.is_none() {
            return;
        }
        for occurrence in link_occurrences(body) {
            if occurrence.form != LinkForm::Term {
                continue;
            }
            if invalid.iter().any(|target| target == occurrence.target) {
                faults.advise(
                    address,
                    "T7",
                    "target_role",
                    occurrence.target,
                    &format!(
                        "the retained Term link target `{}` is no Term head",
                        occurrence.target
                    ),
                );
                continue;
            }
            let Some(name) = targets
                .and_then(|targets| targets.get(occurrence.target))
                .map(|value| text(Some(value)))
            else {
                continue;
            };
            let Some(label) = occurrence.label.filter(|label| !label.is_empty()) else {
                continue;
            };
            if label != name {
                faults.advise(
                    address,
                    "T7",
                    "label_mismatch",
                    &format!("{label}|{}", occurrence.target),
                    &format!(
                        "the Term link label `{label}` differs from the term `{name}`"
                    ),
                );
            }
        }
    }

    /// R2 to R9 for one sentence. A Literature note's row turns the sentence rules R2, R3, R4,
    /// R5 and R7 off and lets R9 advise (issue #184); a rule the row does not name keeps its own
    /// severity.
    pub(crate) fn sentence(
        &self,
        faults: &mut Faults,
        address: &str,
        shared: bool,
        sentence: &str,
        step: bool,
        literature: bool,
    ) {
        let plain = plain(sentence);
        let measure = measure(sentence);
        let limit = match (measure.words, step) {
            (false, true) => 30,
            (false, false) => 40,
            (true, true) => 20,
            (true, false) => 25,
        };
        if measure.size > limit {
            faults.add_row(
                address,
                "R2",
                "sentence_length",
                &format!("{} {} > {limit}: {sentence}", measure.size, measure.unit()),
                &format!("({} {} > {limit}) {sentence}", measure.size, measure.unit()),
                literature,
            );
        }
        if SEMICOLON.is_match(&plain) {
            faults.add_row(address, "R3", "semicolon", sentence, sentence, literature);
        }
        // `被收购方`, `被投资方`: a party, not a passive (and a 被 that names the party has its actor).
        if passive(&plain) {
            faults.add_row(address, "R4", "passive", sentence, sentence, literature);
        }
        if LIGHT_VERB.is_match(&plain) {
            faults.add_row(address, "R5", "light_verb", sentence, sentence, literature);
        }
        if !shared {
            // A rejected word inside a term the table keeps is that term, not the word: 税率差 is
            // rejected for 幅度, and 税率差异化 is a term of its own (ebrill2001modern: 66 of 100 R6
            // faults read 税率差 inside 税率差异化). Each kept term becomes one mark, so no word forms
            // across the gap it leaves.
            let mut masked = plain.clone();
            for kept in &self.kept {
                masked = masked.replace(kept.as_str(), "");
            }
            for (term, words) in &self.terms {
                // A rejected word inside the term itself is the term, not the word.
                let rest = masked.replace(term.as_str(), "");
                for word in words {
                    if rest.contains(word.as_str()) {
                        faults.add(
                            address,
                            "R6",
                            "forbidden_word",
                            &format!("{term}|{word}|{sentence}"),
                            &format!("uses `{word}` for `{term}`: {sentence}"),
                        );
                    }
                }
            }
        }
        if NOUN_PHRASE.is_match(&plain) {
            faults.add_row(address, "R7", "noun_phrase", sentence, sentence, literature);
        }
        if let Some(word) = HEDGES.iter().find(|word| plain.contains(*word)) {
            faults.add_row(
                address,
                "R9",
                "hedge",
                &format!("{word}|{sentence}"),
                &format!("`{word}`: {sentence}"),
                literature,
            );
        }
    }

    /// I6: the checked Idea titles against every title of their Workspace (D129). The
    /// candidates come from an inverted bigram index, the existing Dice function decides,
    /// and an unordered pair is reported once, on the later checked Card. A copied title
    /// is advice: it never merges, deletes, or edits either Card.
    fn similar_titles(&self, cards: &[&Value], faults: &mut Faults) {
        let mut checked: Vec<TitleRow> = Vec::new();
        for card in cards {
            if !ps_eq(&text(prop(card, "category")), "idea") {
                continue;
            }
            let address = text(prop(card, "address"));
            if address.is_empty() {
                continue;
            }
            let row = TitleRow {
                address,
                title: text(prop(card, "title")),
                workspace: text(prop(card, "workspace")),
                restatement: false,
            };
            if checked.iter().any(|known| key(&known.address) == key(&row.address)) {
                continue;
            }
            checked.push(row);
        }
        if checked.is_empty() {
            return;
        }
        // The context rows, then the checked rows: a checked Card is its own row.
        let mut rows = self.context.titles.clone();
        for row in &checked {
            match rows.iter_mut().find(|known| key(&known.address) == key(&row.address)) {
                Some(known) => *known = row.clone(),
                None => rows.push(row.clone()),
            }
        }
        let checked_at: HashMap<String, usize> =
            checked.iter().enumerate().map(|(index, row)| (key(&row.address), index)).collect();
        let row_at: HashMap<String, usize> =
            rows.iter().enumerate().map(|(index, row)| (key(&row.address), index)).collect();
        let mut index: HashMap<String, Vec<usize>> = HashMap::new();
        let mut titles: Vec<HashSet<String>> = Vec::with_capacity(rows.len());
        // A title of one character has no bigram; it can still be an exact copy.
        let mut single: Vec<usize> = Vec::new();
        for (at, row) in rows.iter().enumerate() {
            let grams = bigrams(&row.title);
            if grams.is_empty() {
                single.push(at);
            }
            // The index folds case only, so the inverted index alone finds an exact copy
            // that differs in case — the Dice function keeps case in a bigram (D137).
            for gram in &grams {
                index.entry(gram.to_lowercase()).or_default().push(at);
            }
            titles.push(grams);
        }
        for (at, row) in checked.iter().enumerate() {
            // The context is one Workspace's title projection (D135): an entry that names
            // no Workspace cannot be compared without crossing that boundary, so I6 is
            // unknown for it rather than silently reporting a pair from the other half.
            if self.context.supplied && row.workspace.is_empty() {
                faults.add(
                    &row.address,
                    "I6",
                    "workspace_scope_unknown",
                    "",
                    "Workspace scope unknown: the entry names no Workspace, so nothing is compared",
                );
                continue;
            }
            // A context the reader could not complete makes this Card's I6 unknown; the
            // known rows are still compared, and the line says so (D136).
            if self.context.supplied && !self.context.complete {
                let known = self.context.titles.len();
                faults.add(
                    &row.address,
                    "I6",
                    "context_incomplete",
                    &format!("known:{known}"),
                    &format!(
                        "Workspace title context incomplete: {known} known {} compared, others unknown",
                        if known == 1 { "title" } else { "titles" }
                    ),
                );
            }
            let own = &titles[row_at[&key(&row.address)]];
            let mut candidates: BTreeSet<usize> = BTreeSet::new();
            for gram in own {
                for other in index.get(&gram.to_lowercase()).into_iter().flatten() {
                    candidates.insert(*other);
                }
            }
            if own.is_empty() {
                candidates.extend(single.iter().copied());
            }
            for other_at in candidates {
                let other = &rows[other_at];
                if other.restatement {
                    continue;
                }
                if key(&other.address) == key(&row.address) {
                    continue;
                }
                // One Workspace only: a same title in the other Workspace is no candidate.
                if !row.workspace.is_empty() && !other.workspace.is_empty() && !ps_eq(&row.workspace, &other.workspace) {
                    continue;
                }
                // One report per unordered pair: the later checked Card names the earlier.
                if checked_at.get(&key(&other.address)).is_some_and(|earlier| *earlier > at) {
                    continue;
                }
                let same = key(&other.title) == key(&row.title);
                if !same && similarity(own, &titles[other_at]) < 0.8 {
                    continue;
                }
                faults.add(
                    &row.address,
                    "I6",
                    "similar_title",
                    &format!("[[{}]]", other.address),
                    &format!("a title very similar to [[{}]]", other.address),
                );
            }
        }
    }

    /// The Card layer rules (I, E, F, L): only for entries that name their category.
    fn layer(&self, cards: &[Value], prose_chars: &HashMap<String, i64>, faults: &mut Faults) {
        let layer = cards
            .iter()
            .filter(|card| prop(card, "category").is_some_and(truthy))
            .collect::<Vec<_>>();
        let limits = &self.limits;
        for card in &layer {
            let address = text(prop(card, "address"));
            let category = text(prop(card, "category"));
            let is = |names: &[&str]| ps_in(&category, names);
            if !is(&["idea", "anchor", "event", "finding", "restatement-entry"]) {
                continue;
            }
            let title = text(prop(card, "title"));
            let body = text(prop(card, "body"));
            let has = |name: &str| prop(card, name).is_some();
            let parent = text(prop(card, "parent"));
            let links = strings(prop(card, "links"));
            let kinds = strings(prop(card, "kinds"));
            // A Literature note's row decides I1, I2, I4, I6, I7, L1, L2 and L3 (issue #184).
            let literature = is_literature_note(card);
            let title_plain = SPACE
                .replace_all(&PLACEHOLDERS.replace_all(&plain(&title), ""), "")
                .into_owned();
            if is(&["idea", "anchor"])
                && length(&title_plain) <= 6
                && !CLAIM_VERB.is_match(&title_plain)
            {
                faults.add_row(
                    &address,
                    "I1",
                    "no_claim_title",
                    &title,
                    &format!("the title states no claim: {title}"),
                    literature,
                );
            }
            if is(&["idea"]) && literature {
                // A note carries the source's argument, so only a body too thin to hold one is
                // advised, and no upper bound applies.
                let size = prose_chars.get(&key(&address)).copied().unwrap_or(0);
                if size < LITERATURE_MIN_CHARS {
                    faults.add_row(
                        &address,
                        "I2",
                        "idea_size",
                        &format!("{size} characters < {LITERATURE_MIN_CHARS}"),
                        &format!("({size} characters < {LITERATURE_MIN_CHARS}) merge candidate"),
                        literature,
                    );
                }
            } else if is(&["idea"]) {
                let size = prose_chars.get(&key(&address)).copied().unwrap_or(0);
                if size > limits.max_idea_chars {
                    faults.add(
                        &address,
                        "I2",
                        "idea_size",
                        &format!("{size} characters > {}", limits.max_idea_chars),
                        &format!(
                            "({size} characters > {}) split candidate",
                            limits.max_idea_chars
                        ),
                    );
                } else if size < limits.min_idea_chars {
                    faults.add(
                        &address,
                        "I2",
                        "idea_size",
                        &format!("{size} characters < {}", limits.min_idea_chars),
                        &format!(
                            "({size} characters < {}) merge candidate",
                            limits.min_idea_chars
                        ),
                    );
                }
            }
            // A restatement entry keeps the source check too (§4.5: the appropriate source Link),
            // while the claim-title and Idea-size rules stay with a substantive Idea (D086).
            if is(&["idea", "event", "restatement-entry"])
                && has("book")
                && prop(card, "book").is_some_and(truthy)
                && has("links")
            {
                let book = text(prop(card, "book"));
                let anchor = Regex::new(&format!("(?i)^{}/\\d+$", regex::escape(&book)))
                    .expect("an escaped Citation key");
                if !links
                    .iter()
                    .any(|link| ps_eq(link, &book) || anchor.is_match(link))
                {
                    faults.add(
                        &address,
                        "I3",
                        "no_source",
                        &book,
                        &format!("no source line: no Link to [[{book}]] or one of its anchors"),
                    );
                }
            }
            if is(&["idea"]) && has("kinds") && !kinds.iter().any(|kind| !ps_eq(kind, &parent)) {
                faults.add_row(
                    &address,
                    "I4",
                    "no_kinded_link",
                    "",
                    "no kinded Link to a Card other than the parent",
                    literature,
                );
            }
            if has("inbound") {
                let inbound = prop(card, "inbound")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                if is_orphan(&address, &text(prop(card, "category")), &inbound) {
                    faults.add(
                        &address,
                        "I5",
                        "no_inbound",
                        "",
                        "no other Card has an authored Link here",
                    );
                }
            }
            if is(&["event"]) {
                if !DATE.is_match(&title_plain) {
                    faults.add(
                        &address,
                        "E1",
                        "no_date",
                        &title,
                        &format!("no date in the title: {title}"),
                    );
                }
                let missing = EVENT_LABELS
                    .iter()
                    .filter(|(_, pattern)| !pattern.is_match(&body))
                    .map(|(label, _)| *label)
                    .collect::<Vec<_>>();
                if !missing.is_empty() {
                    faults.add(
                        &address,
                        "E2",
                        "missing_labels",
                        &format!("labels:{}", missing.join(",")),
                        &format!("missing labels: {}", missing.join("、")),
                    );
                }
                if has("inbound_kinded") && ps_int(prop(card, "inbound_kinded")) == 0 {
                    faults.add(
                        &address,
                        "E4",
                        "no_kinded_inbound",
                        "",
                        "no Idea Card links it with a kind word",
                    );
                }
            }
            if is(&["finding"]) {
                finding_rules(card, &address, &body, faults);
            }
            // A restatement entry's `## 同述入口` section is navigation: its one Link names the Card
            // that holds the statement, so it is no semantic Link and needs no Relation kind (§4.5,
            // D086). L1 and L2 skip it; L3, I3 and the Link target checks still apply.
            let entry = if is(&["restatement-entry"]) {
                entry_section(&body)
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };
            if has("links") {
                // A Term use is a vocabulary reference, not a reasoning Link (S16, #158; design
                // 10.2). The lexical scan is this checker's own, so L1 needs no caller to
                // classify an edge: a target the body reaches only by Term uses is no Link the
                // reason rules read, while an ordinary Link to the same target still is.
                let term_only = term_only_targets(&body);
                let mut refers: Vec<String> = Vec::new();
                for link in &links {
                    if term_only.contains(link.as_str()) {
                        continue;
                    }
                    if !ps_in(link, &kinds)
                        && !book_place(link)
                        && !(entry.0.iter().any(|target| ps_eq(target, link))
                            && !entry.2.iter().any(|target| ps_eq(target, link)))
                        && !leads_to(&body, link)
                        && !refers.iter().any(|seen| culture(seen) == culture(link))
                    {
                        refers.push(link.clone());
                    }
                }
                if !refers.is_empty() {
                    let shown = refers
                        .iter()
                        .take(5)
                        .map(|link| format!("[[{link}]]"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    // The identity is the set of unkinded Links, so reordering them is no new fault.
                    let mut identity = refers.clone();
                    identity.sort();
                    faults.add_row(
                        &address,
                        "L1",
                        "unkinded_links",
                        &identity.join(","),
                        &format!("({} Links read as refers) {shown}", refers.len()),
                        literature,
                    );
                }
            }
            // L2 and L3 read the body: the sentence of each Link, and the paragraphs it appears in.
            // A Term use reads as its visible word, so it starts no Link sentence and no repeated
            // Link (S16, #158; design 10.2) — the same projection the size rules already use.
            let reasoning_body = crate::text::strip_term_links(&body);
            let mut paragraphs: Vec<(String, BTreeSet<usize>)> = Vec::new();
            let mut paragraph = 0;
            for raw in reasoning_body.split('\n') {
                let trim = raw.trim();
                if trim.is_empty() {
                    paragraph += 1;
                    continue;
                }
                for caps in LINK_TARGET.captures_iter(trim) {
                    let target = caps[1].trim();
                    match paragraphs
                        .iter_mut()
                        .find(|(known, _)| key(known) == key(target))
                    {
                        Some((_, seen)) => {
                            seen.insert(paragraph);
                        }
                        None => paragraphs.push((target.to_string(), BTreeSet::from([paragraph]))),
                    }
                }
            }
            for line in prose_lines(&reasoning_body) {
                if entry.1.iter().any(|known| ps_eq(known, line.text.trim())) {
                    continue;
                }
                for sentence in split_sentences(&line.text) {
                    if !sentence.contains("[[") || SOURCE_LINE.is_match(&sentence) {
                        continue;
                    }
                    let reason = PLACEHOLDERS.replace_all(&plain(&sentence), "").into_owned();
                    let reason = REASON_STRIP.replace_all(&reason, "");
                    // A formula states the reason too (`代入 [[…]] 的定义得 $MRTS=…$`).
                    if length(&reason) < limits.min_reason && !INLINE_MATH.is_match(&sentence) {
                        faults.add_row(
                            &address,
                            "L2",
                            "no_reason",
                            &format!(
                                "{} characters < {}: {sentence}",
                                length(&reason),
                                limits.min_reason
                            ),
                            &format!(
                                "({} characters < {}) a Link with no reason: {sentence}",
                                length(&reason),
                                limits.min_reason
                            ),
                            literature,
                        );
                    }
                }
            }
            paragraphs.sort_by(|a, b| culture_cmp(&a.0, &b.0));
            for (target, seen) in &paragraphs {
                if seen.len() >= 2 {
                    faults.add_row(
                        &address,
                        "L3",
                        "repeated_paragraph_link",
                        target,
                        &format!("[[{target}]] is linked in {} paragraphs", seen.len()),
                        literature,
                    );
                }
            }
            // I7 reads the relationship evidence, so only a Card that carries it is in scope.
            if is(&["idea"]) {
                own_source_dependency(card, &address, faults, literature);
            }
        }
        // I6: the checked Idea titles against their Workspace. It reads the context, not
        // only this run, so a tree-only read is never reported as a Workspace-wide check. A
        // Literature note's row turns the duplicate-title rule off (issue #184), so the note is
        // neither checked nor offered as a pair.
        let idea_titles = layer
            .iter()
            .copied()
            .filter(|card| !is_literature_note(card))
            .collect::<Vec<_>>();
        self.similar_titles(&idea_titles, faults);
        let events = layer
            .iter()
            .filter(|card| ps_eq(&text(prop(card, "category")), "event"))
            .map(|card| {
                let title = text(prop(card, "title"));
                let year = YEAR
                    .captures(&title)
                    .map(|caps| caps[1].to_string())
                    .unwrap_or_default();
                (text(prop(card, "address")), year, bigrams(&title))
            })
            .collect::<Vec<_>>();
        for i in 0..events.len() {
            for j in i + 1..events.len() {
                if !events[i].1.is_empty()
                    && ps_eq(&events[i].1, &events[j].1)
                    && similarity(&events[i].2, &events[j].2) >= 0.5
                {
                    faults.add(
                        &events[j].0,
                        "E5",
                        "same_event",
                        &format!("[[{}]]|{}", events[i].0, events[i].1),
                        &format!("the same event as [[{}]]?", events[i].0),
                    );
                }
            }
        }
    }
}

/// Whether a rule's Literature-note row turns the rule off for this entry (issue #184): the fault
/// is not reported at all, so no archive policy can bring it back.
fn row_hides(rule: &str, literature: bool) -> bool {
    literature
        && crate::policy::literature_setting(rule) == Some(crate::policy::Setting::Off)
}

/// The targets a body reaches only by Term uses (S16, #158; design 10.2): a vocabulary reference,
/// not a reasoning Link. A target that an ordinary Link also names is not in the set, so L1 still
/// reports it when no relation word announces it. One shared lexical scan, outside code and math.
fn term_only_targets(body: &str) -> BTreeSet<String> {
    let mut term = BTreeSet::new();
    let mut ordinary = BTreeSet::new();
    for occurrence in link_occurrences(body) {
        let target = occurrence.target.to_string();
        match occurrence.form {
            LinkForm::Term => {
                term.insert(target);
            }
            LinkForm::Ordinary => {
                ordinary.insert(target);
            }
        }
    }
    term.difference(&ordinary).cloned().collect()
}

/// The template position of a Term head label, or `None` for a label outside the template.
fn head_position(label: &str) -> Option<usize> {
    crate::term::HEAD_LABELS.iter().position(|known| *known == label)
}

/// The template position of a Term section; every derivation shares one position (a term may
/// carry more than one derivation, design 11.1).
fn section_position(name: &str) -> Option<usize> {
    crate::term::SECTIONS.iter().position(|known| *known == name)
}

/// Whether a title is a continuation title: `推导（续 k/n）：<what continues>` with `2 ≤ k ≤ n`.
fn continuation_title(title: &str) -> bool {
    let Some(captures) = CONTINUATION_TITLE.captures(title) else {
        return false;
    };
    let Ok(k) = captures[1].parse::<i64>() else {
        return false;
    };
    let Ok(n) = captures[2].parse::<i64>() else {
        return false;
    };
    k >= 2 && n >= k && !captures[3].trim().is_empty()
}

/// A line that opens the `符号表` table: the plain label or a bold one.
fn is_symbol_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed == "符号表" || trimmed == "**符号表**"
}

/// A display formula line: `$$…$$` with content between the delimiters.
fn display_formula(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("$$") && trimmed.ends_with("$$") && trimmed.len() > 4
}

/// Whether a derivation step names no basis: absent, or only whitespace and end marks.
fn basis_missing(basis: Option<&str>) -> bool {
    basis.is_none_or(|basis| {
        basis
            .trim_matches(|ch: char| ch.is_whitespace() || "。．.；;，,、".contains(ch))
            .is_empty()
    })
}

/// Every math span of one section: its display formulas and the inline formulas on its lines.
/// Tables and prose are not formulas, so a word there is no symbol (T4 stays approximate).
fn math_formulas(content: &[String]) -> Vec<String> {
    let mut formulas = Vec::new();
    for line in content {
        let trimmed = line.trim();
        if trimmed.starts_with('|') {
            continue;
        }
        if display_formula(trimmed) {
            formulas.push(trimmed.trim_matches('$').to_string());
            continue;
        }
        for found in INLINE_MATH.find_iter(trimmed) {
            formulas.push(found.as_str().trim_matches('$').to_string());
        }
    }
    formulas
}

/// R20: a local label that stands in for its content (`式 (12) 有该项`). The
/// content is what the rest of the text says once the labels are taken out, so
/// `式 (12) 的加成项来自需求弹性` passes and a source locator is tested by its
/// caller only. Advice in the pilot: the label may stay as a secondary name.
fn local_reference(faults: &mut Faults, address: &str, text: &str) {
    let text_plain = plain(text);
    let mut labels: Vec<String> = Vec::new();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for pattern in [
        &*LOCAL_EQUATION,
        &*LOCAL_NAMED_NUMBER,
        &*LOCAL_CHAPTER,
        &*LOCAL_THIS,
        &*LOCAL_SIGN,
    ] {
        for found in pattern.find_iter(&text_plain) {
            if spans
                .iter()
                .any(|(start, end)| *start < found.end() && found.start() < *end)
            {
                continue;
            }
            spans.push((found.start(), found.end()));
            labels.push(found.as_str().to_string());
        }
    }
    if labels.is_empty() {
        return;
    }
    let mut order = (0..labels.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| spans[*index].0);
    let mut rest = String::new();
    let mut at = 0;
    for index in order {
        rest.push_str(&text_plain[at..spans[index].0]);
        at = spans[index].1;
    }
    rest.push_str(&text_plain[at..]);
    if length(&REASON_STRIP.replace_all(&rest, "")) >= LOCAL_REFERENCE_CONTENT {
        return;
    }
    for label in labels {
        faults.add(
            address,
            "R20",
            "local_reference",
            &format!("{label}|{text}"),
            &format!("local reference `{label}` names no content: {text}"),
        );
    }
}

/// R21: a deictic phrase with no named Link in its own sentence (`该卡` alone,
/// `那里` alone). A Link in the same sentence names the referent; a Link in the
/// sentence before does not. It decides only what it can see.
fn unnamed_referent(faults: &mut Faults, address: &str, text: &str) {
    let text_plain = plain(text);
    let Some(word) = DEICTIC.iter().find(|word| text_plain.contains(**word)) else {
        return;
    };
    if LINK.is_match(text) {
        return;
    }
    faults.add(
        address,
        "R21",
        "unnamed_referent",
        &format!("{word}|{text}"),
        &format!("unnamed referent `{word}` with no named Link in its sentence: {text}"),
    );
}

// The shared classification of an authored Card (``§4.5``, D086, D188): one definition serves
// the audit, the compilers, the standalone checker and the service write gate.
pattern!(CATEGORY_ANCHOR, r"^[^/]+/\d+$");
pattern!(CATEGORY_TERMS, r"(?i)^术语表");
pattern!(CATEGORY_NOTATION, r"(?i)^(符号表|notation)");
pattern!(CATEGORY_STRUCTURE, r"(?i)^本卡是《.*》的结构卡");

/// Whether a Card body holds the `## 同述入口` section: the derived navigation role of an
/// accepted restatement entry (§4.5, D086). The role is derived from the written text on every
/// read, never stored as Card metadata; a heading inside a code fence is not a section.
pub fn is_restatement_entry(body: &str) -> bool {
    let mut fenced = false;
    for raw in body.lines() {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(heading) = trimmed
            .strip_prefix("## ")
            .or_else(|| trimmed.strip_prefix("### "))
            && heading.trim() == "同述入口"
        {
            return true;
        }
    }
    false
}

/// Classify the same metadata for audit, compiler and standalone checks. A scope that does not
/// read the register's declaration classifies no Card as a register part; the register read does.
pub fn category(
    card: &Value,
    literature: &HashSet<String>,
    anchors: &HashMap<String, usize>,
) -> String {
    category_with_parts(card, literature, anchors, &HashSet::new())
}

/// Whether one entry is a Literature note (issue #184): a Regular Card of type `idea` filed in a
/// Literature tree, other than the tree's root, with a Section anchor counting as one too. The
/// layer category names the Idea role; the entry's own `tree` — or, for a draft that names no
/// tree, the `book` it belongs to — says whether that tree is a book. An Event, Finding, Model or
/// Term Card keeps its category and its rules.
pub fn is_literature_note(entry: &Value) -> bool {
    if !ps_in(
        &text(prop(entry, "category")),
        &["idea", "anchor", "restatement-entry"],
    ) {
        return false;
    }
    let tree = text(prop(entry, "tree"));
    if !tree.is_empty() {
        return literature_tree(&tree).is_some();
    }
    let book = text(prop(entry, "book"));
    !book.is_empty() && literature_tree(&book).is_some()
}

/// The same classification, with the addresses a register's root declares under `词条分册` (§8.5,
/// D188): a register part is a navigation Card holding term rows, not a substantive Idea, so it
/// triggers no claim-title or Idea-size rule and needs no topic hub of its own.
pub fn category_with_parts(
    card: &Value,
    literature: &HashSet<String>,
    anchors: &HashMap<String, usize>,
    register_parts: &HashSet<String>,
) -> String {
    if register_parts.contains(&text(prop(card, "address"))) {
        return "register-part".into();
    }
    if ps_in(&text(prop(card, "kind")), &["topic", "literature"]) {
        return "root".into();
    }
    match text(prop(card, "type")).as_str() {
        // A Term Card is terminology, not a claim: its list is navigation and its named Cards
        // take the T rules instead of the Idea rules (S09, #152; design 11.3).
        "term" => {
            return if text(prop(card, "term")).is_empty() {
                "terms"
            } else {
                "term"
            }
            .into();
        }
        "finding" => return "finding".into(),
        "event" => return "event".into(),
        "hub" => return "hub".into(),
        "model" => {
            return if text(prop(card, "facet")).is_empty() {
                "model"
            } else {
                "part"
            }
            .into();
        }
        _ => {}
    }
    let tree = text(prop(card, "tree"));
    let in_book = literature.contains(&tree);
    if in_book && CATEGORY_ANCHOR.is_match(&text(prop(card, "address"))) {
        return "anchor".into();
    }
    if CATEGORY_TERMS.is_match(&text(prop(card, "title"))) {
        return "terms".into();
    }
    if CATEGORY_NOTATION.is_match(&text(prop(card, "title"))) {
        return "notation".into();
    }
    let prose = prop(card, "excerpt")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| text(prop(card, "body")));
    if in_book && CATEGORY_STRUCTURE.is_match(&prose) {
        return "structure".into();
    }
    // An accepted restatement entry names the substantive statement at another address: it is
    // navigation, not a claim of its own (§4.5, D086). The role comes from the authored section
    // (or the flag the archive derives from it), never from a stored Card field.
    if prop(card, "restatement") == Some(&Value::Bool(true))
        || prop(card, "body")
            .and_then(Value::as_str)
            .is_some_and(is_restatement_entry)
    {
        return "restatement-entry".into();
    }
    let parent = text(prop(card, "parent"));
    let parent = if parent.is_empty() {
        text(prop(card, "parent_address"))
    } else {
        parent
    };
    if in_book
        && parent == format!("{tree}/1")
        && text(prop(card, "edge")) == "side"
        && text(prop(card, "type")) == "idea"
    {
        let own = Regex::new(&format!(r"^{}/\d+$", regex::escape(&tree))).unwrap();
        let count = card["links"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|target| own.is_match(target))
            .collect::<HashSet<_>>()
            .len();
        if count >= 3 && anchors.get(&tree).is_none_or(|total| count * 2 >= *total) {
            return "structure".into();
        }
    }
    "idea".into()
}

/// The source evidence of one Card as the audit derives it (design §4.3, D068–D072): an
/// explicit `sources` set, else the Citation keys its `来源` lines name, else the Literature
/// tree it is filed in. Empty is unknown, which is not a pass.
///
/// This is the checker's one derivation, shared with a report that reports the same source
/// sets, so the checker and the report can never disagree about what a Card's source is.
pub fn source_evidence(card: &Value) -> Vec<String> {
    let explicit = strings(prop(card, "sources"));
    if !explicit.is_empty() {
        return explicit;
    }
    fn push(sources: &mut Vec<String>, tree: String) {
        if !sources.iter().any(|seen| key(seen) == key(&tree)) {
            sources.push(tree);
        }
    }
    let mut sources: Vec<String> = Vec::new();
    for line in text(prop(card, "body")).split('\n') {
        if !SOURCE_LINE.is_match(line.trim()) {
            continue;
        }
        for caps in LINK_TARGET.captures_iter(line) {
            if let Some(tree) = literature_tree(caps[1].trim()) {
                push(&mut sources, tree);
            }
        }
    }
    if sources.is_empty()
        && let Some(tree) = literature_tree(&text(prop(card, "tree")))
    {
        push(&mut sources, tree);
    }
    sources
}

/// I7: a Topic Idea Card whose `rests-on` Link enters its own source's Literature
/// tree. The claim has to carry its necessary conditions instead. A Card filed in
/// that tree rests on it by design, and an `illustrates`, `corresponds`, or
/// `contrasts` Link is no dependency.
/// I7: an Idea Card of a Topic tree resting on its own source's Literature tree. A Literature
/// note's row turns the rule off (issue #184), so the note reports nothing.
fn own_source_dependency(card: &Value, address: &str, faults: &mut Faults, literature: bool) {
    if row_hides("I7", literature) {
        return;
    }
    let Some(relations) = card.get("relations").and_then(Value::as_object) else {
        return;
    };
    let tree = text(prop(card, "tree"));
    let sources = source_evidence(card);
    for (target, kind) in relations {
        if !ps_eq(&ps_string(Some(kind)), "rests-on") {
            continue;
        }
        let Some(book) = literature_tree(target) else {
            continue;
        };
        if !tree.is_empty() && key(&tree) == key(&book) {
            continue;
        }
        if sources.is_empty() {
            faults.advise(
                address,
                "I7",
                "own_source_unknown",
                target,
                &format!("source evidence unknown, so [[{target}]] may be an own-source premise"),
            );
        } else if sources.iter().any(|source| key(source) == key(&book)) {
            faults.add(
                address,
                "I7",
                "own_source",
                target,
                &format!("`rests-on` [[{target}]] enters its own source `{book}`"),
            );
        }
    }
}

/// The Finding Card rules. F1: the fixed labels. F2: the `来源` line names its
/// source: a Link to the book or one of its anchors when the book is known,
/// else any text. F3: one result per Card: one `结果` label, with at most one
/// list item under it. F4: an Idea Card links it with a kind word.
fn finding_rules(card: &Value, address: &str, body: &str, faults: &mut Faults) {
    let lines = body
        .split('\n')
        .map(|line| line.trim_end_matches('\r'))
        .collect::<Vec<_>>();
    let is_label = |line: &str| {
        FINDING_LABELS
            .iter()
            .any(|(_, pattern)| pattern.is_match(line))
    };
    let labelled = |name: &str| {
        let pattern = &FINDING_LABELS
            .iter()
            .find(|(label, _)| *label == name)
            .expect("a Finding label")
            .1;
        lines
            .iter()
            .enumerate()
            .filter(|(_, line)| pattern.is_match(line))
            .map(|(index, line)| (index, pattern.replace(line, "").trim().to_string()))
            .collect::<Vec<_>>()
    };
    let missing = FINDING_LABELS
        .iter()
        .filter(|(label, _)| labelled(label).is_empty())
        .map(|(label, _)| *label)
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        faults.add(
            address,
            "F1",
            "missing_labels",
            &format!("labels:{}", missing.join(",")),
            &format!("missing labels: {}", missing.join("、")),
        );
    }
    let sources = labelled("来源");
    if !sources.is_empty() {
        if prop(card, "book").is_some_and(truthy) {
            let book = text(prop(card, "book"));
            let anchor = Regex::new(&format!("(?i)^{}/\\d+$", regex::escape(&book)))
                .expect("an escaped Citation key");
            let linked = sources.iter().any(|(_, rest)| {
                LINK_TARGET.captures_iter(rest).any(|caps| {
                    let target = caps[1].trim();
                    ps_eq(target, &book) || anchor.is_match(target)
                })
            });
            if !linked {
                faults.add(
                    address,
                    "F2",
                    "source_wrong_book",
                    &book,
                    &format!("the source line links neither [[{book}]] nor one of its anchors"),
                );
            }
        } else if sources.iter().all(|(_, rest)| rest.is_empty()) {
            faults.add(address, "F2", "source_missing", "", "the source line names no source");
        }
    }
    let results = labelled("结果");
    if results.len() > 1 {
        faults.add(
            address,
            "F3",
            "result_labels",
            &format!("{} labels", results.len()),
            &format!(
                "({} 结果 labels) one result per Card: split it",
                results.len()
            ),
        );
    } else if let Some((index, _)) = results.first() {
        let items = lines[index + 1..]
            .iter()
            .take_while(|line| !line.trim().is_empty() && !is_label(line))
            .filter(|line| LIST_ITEM.is_match(line))
            .count();
        if items > 1 {
            faults.add(
                address,
                "F3",
                "result_items",
                &format!("{items} items"),
                &format!("({items} results under 结果) one result per Card: split it"),
            );
        }
    }
    if prop(card, "inbound_kinded").is_some() && ps_int(prop(card, "inbound_kinded")) == 0 {
        faults.add(
            address,
            "F4",
            "no_kinded_inbound",
            "",
            "no Idea Card links it with a kind word",
        );
    }
}

#[derive(Default)]
pub(crate) struct Faults {
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl Faults {
    /// One fault at the rule's default severity and scope.
    pub(crate) fn add(&mut self, address: &str, rule: &str, fault: &str, key: &str, message: &str) {
        self.push(
            address,
            rule,
            fault,
            key,
            message,
            crate::policy::severity(rule),
            crate::policy::scope(rule),
            false,
        );
    }

    /// One advisory subcase of a rule whose default severity is otherwise blocking.
    pub(crate) fn advise(&mut self, address: &str, rule: &str, fault: &str, key: &str, message: &str) {
        self.push(
            address,
            rule,
            fault,
            key,
            message,
            Severity::Advise,
            crate::policy::scope(rule),
            true,
        );
    }

    /// One fault of a rule whose Literature-note row may set its level (issue #184): `off` reports
    /// nothing, `advise` is advice whatever the archive policy decides for the rule, and a rule
    /// with no row — or a Card that is no Literature note — keeps the rule's own severity.
    pub(crate) fn add_row(
        &mut self,
        address: &str,
        rule: &str,
        fault: &str,
        key: &str,
        message: &str,
        literature: bool,
    ) {
        if literature {
            match crate::policy::literature_setting(rule) {
                Some(crate::policy::Setting::Off) => return,
                Some(crate::policy::Setting::Advise) => {
                    self.advise(address, rule, fault, key, message);
                    return;
                }
                _ => {}
            }
        }
        self.add(address, rule, fault, key, message);
    }

    fn push(
        &mut self,
        address: &str,
        rule: &str,
        fault: &str,
        key: &str,
        message: &str,
        severity: Severity,
        scope: Scope,
        subcase: bool,
    ) {
        self.diagnostics.push(Diagnostic {
            address: from_utf16_form(address),
            rule: rule.to_string(),
            fault: fault.to_string(),
            key: from_utf16_form(key),
            message: from_utf16_form(message),
            severity,
            legacy: false,
            subcase,
            scope,
        });
    }
}

/// The categories an authored inbound Link can make substantive (I5 and the
/// orphan audit read one definition).
pub const SUBSTANTIVE: &[&str] = &["idea", "event", "finding", "model", "part"];

/// The navigation categories that are never orphan candidates.
pub const EXCLUDED: &[&str] = &[
    "root",
    "anchor",
    "structure",
    "terms",
    "term",
    "notation",
    "hub",
    "register-part",
    "restatement-entry",
];

/// Filing, outbound citations and Reverse text are not inbound authored Links.
/// An actual authored Link from a parent is evidence; a self-Link is not.
pub fn is_orphan(address: &str, category: &str, inbound: &[String]) -> bool {
    SUBSTANTIVE.contains(&category)
        && !inbound
            .iter()
            .any(|from| !from.is_empty() && from != address)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{CheckReport, DEFAULT_RULES, filter_rules};
    use serde_json::json;

    const CARD_STYLE_FIXTURE: &str =
        include_str!("../../../scripts/book-digest/tests/card-style-fixture.json");
    const CARD_STYLE_EXPECTED: &str = include_str!("../tests/fixtures/card-style-expected.txt");

    /// S02-AC03/AC04: the display clips two long faults to the same prefix, but
    /// their semantic keys keep the full offending fragment, so they stay two
    /// distinct occurrences. A duplicate fault is not hidden behind an old one.
    #[test]
    fn faults_sharing_a_clipped_line_keep_distinct_semantic_keys() {
        let head = "甲".repeat(80);
        let cards = json!([{
            "address": "K/5", "title": "均衡稳定",
            "body": format!("{head}乙。\n\n{head}丙。")
        }]);
        let report = Check::new(Limits::default(), false, "").report(cards.as_array().unwrap());
        let r2 = report.iter().filter(|d| d.rule == "R2").collect::<Vec<_>>();
        assert_eq!(r2.len(), 2, "{report:#?}");
        assert_eq!(r2[0].line(), r2[1].line(), "both lines clip to the same prefix");
        assert_ne!(r2[0].key, r2[1].key, "the keys keep the full fragment");
        assert!(
            r2[0].key.contains('乙') && r2[1].key.contains('丙'),
            "the full sentence is the identity: {:?}",
            [&r2[0].key, &r2[1].key]
        );
        assert!(r2[0].message.len() > 90, "the message is not clipped either");
        let retained = crate::policy::ratchet(&[r2[0].clone()], report, |_| true);
        assert!(retained.diagnostics.iter().any(|d| d.rule == "R2" && d.legacy));
        assert_eq!(retained.blockers, 1, "the second occurrence is new");
    }

    /// S02-AC04: a size fault's identity is the measured value and the limit, not
    /// the clipped line; a missing-label fault names the whole missing set.
    #[test]
    fn size_and_label_faults_identify_their_measured_requirement() {
        let part = |sentences: usize| {
            let body = (1..=sentences)
                .map(|n| format!("第 {n} 句说明这一步的推理。"))
                .collect::<String>();
            json!({ "address": "K/6|a", "facet": "设定", "title": "设定", "body": body })
        };
        let key = |sentences: usize| {
            let cards = json!([part(sentences)]);
            let report = Check::new(Limits::default(), false, "").report(cards.as_array().unwrap());
            report
                .iter()
                .find(|d| d.rule == "R13")
                .map(|d| d.key.clone())
                .expect("R13")
        };
        assert_eq!(key(13), "sentences:13/12");
        assert_eq!(key(14), "sentences:14/12");
        assert_ne!(key(13), key(14), "a larger part is a new fault");

        let event = |labels: &str| {
            json!({ "address": "K/6|b", "category": "event", "title": "1999 年的一笔交易",
                "body": format!("时间：1999 年。\n地点：上海。\n{labels}") })
        };
        let e2 = |labels: &str| {
            let cards = json!([event(labels)]);
            let report = Check::new(Limits::default(), false, "").report(cards.as_array().unwrap());
            report
                .iter()
                .find(|d| d.rule == "E2")
                .map(|d| d.key.clone())
                .expect("E2")
        };
        let full = "参与者：甲。\n经过：双方签约。\n来源：[[K]]。";
        assert_eq!(
            {
                let cards = json!([event(full)]);
                let report =
                    Check::new(Limits::default(), false, "").report(cards.as_array().unwrap());
                report.iter().filter(|d| d.rule == "E2").count()
            },
            0,
            "all five labels are present"
        );
        assert_eq!(e2("参与者：甲。\n经过：双方签约。"), "labels:来源");
        assert_eq!(e2("经过：双方签约。\n来源：[[K]]。"), "labels:参与者");
        assert_eq!(e2("经过：双方签约。"), "labels:参与者,来源");
        assert_ne!(
            e2("经过：双方签约。\n来源：[[K]]。"),
            e2("经过：双方签约。"),
            "two missing labels are another requirement"
        );
    }

    /// S02-AC01/R0111: the structured public contract is the design's example
    /// document, field for field, including the policy names of the severity and
    /// the scope and the legacy flag. A caller that returns check data over MCP,
    /// the GUI or a batch result reads this one projection.
    #[test]
    fn a_diagnostic_serialises_the_published_contract() {
        let cards = json!([{
            "address": "K/14", "category": "event", "title": "1999 年的一笔交易",
            "body": "时间：1999 年。\n地点：上海。\n经过：双方签约。"
        }]);
        let report = Check::new(Limits::default(), false, "").report(cards.as_array().unwrap());
        assert_eq!(
            report[0].to_json(),
            json!({
                "address": "K/14",
                "rule": "E2",
                "fault": "missing_labels",
                "key": "labels:参与者,来源",
                "message": "missing labels: 参与者、来源",
                "severity": "block",
                "legacy": false,
                "scope": "local",
            })
        );
        // A retained occurrence is advice in the same projection.
        let mut legacy = report[0].clone();
        legacy.legacy = true;
        legacy.severity = Severity::Advise;
        let value = legacy.to_json();
        assert_eq!(value["legacy"], json!(true));
        assert_eq!(value["severity"], json!("advise"));
        assert_eq!(value["key"], json!("labels:参与者,来源"));
    }

    /// S02-AC01: one structured diagnostic carries the address, the rule, the
    /// rule-specific fault code, the unclipped semantic identity, the full
    /// message, the severity, the legacy flag, and the scope; the familiar line
    /// is formatted from it at the boundary.
    #[test]
    fn a_diagnostic_carries_its_structured_identity_and_formats_the_familiar_line() {
        let cards = json!([{ "address": "K/1", "title": "均衡稳定", "body": "甲；乙。" }]);
        let cards = cards.as_array().expect("entries");
        let report = Check::new(Limits::default(), false, "").report(cards);
        assert_eq!(report.len(), 1);
        let diagnostic = &report[0];
        assert_eq!(diagnostic.address, "K/1");
        assert_eq!(diagnostic.rule, "R3");
        assert_eq!(diagnostic.fault, "semicolon");
        assert_eq!(diagnostic.key, "甲；乙。");
        assert_eq!(diagnostic.message, "甲；乙。");
        assert_eq!(diagnostic.severity, Severity::Block);
        assert!(!diagnostic.legacy);
        assert_eq!(diagnostic.scope, Scope::Local);
        assert_eq!(diagnostic.line(), "K/1 R3 甲；乙。");
    }

    /// The in-process adapter the service check uses: the process-free core over
    /// the entries and the context the caller already materialized. `--json`
    /// prints one `faults:` line after the same lines, so the helper appends its
    /// count and the core and the CLI stay comparable line for line.
    fn run_json(cards: &Value, setting: &str, limits: Limits, require_explain: bool, level: &str) -> Vec<String> {
        let cards = cards.as_array().expect("an array of entries");
        let setting = utf16_form(setting);
        let setting_body = (!setting.is_empty())
            .then(|| {
                cards
                    .iter()
                    .find(|card| ps_eq(&text(prop(card, "address")), &setting))
                    .map(|card| ps_string(prop(card, "body")))
            })
            .flatten();
        faulted(
            Check::new(limits, require_explain, level)
                .with_terms(setting_body.as_deref())
                .run(cards),
        )
    }

    /// The core with a supplied Workspace title context: the gate adapter shape.
    fn run_with_context(cards: &Value, context: TitleContext, limits: Limits) -> Vec<String> {
        let cards = cards.as_array().expect("an array of entries");
        faulted(Check::new(limits, false, "").with_context(context).run(cards))
    }

    fn faulted(mut lines: Vec<String>) -> Vec<String> {
        let count = lines.len();
        lines.push(format!("faults: {count}"));
        lines
    }

    /// The report's lines with the count every mode publishes: the blockers only
    /// (design 8.1, R0187).
    fn reported(mut lines: Vec<String>, blockers: usize) -> Vec<String> {
        lines.push(format!("faults: {blockers}"));
        lines
    }

    fn title_context(titles: &Value, complete: bool) -> TitleContext {
        let rows = titles.as_array().cloned().unwrap_or_default();
        let mut context = TitleContext::from_titles(&rows);
        context.complete = complete;
        context
    }

    fn assert_prefixes(lines: &[String], expected: &[&str]) {
        assert_eq!(lines.len(), expected.len(), "lines: {lines:#?}");
        for (line, want) in lines.iter().zip(expected) {
            assert!(line.starts_with(want), "expected `{want}`, got `{line}`");
        }
    }

    fn of_rule<'a>(lines: &'a [String], rule: &str) -> Vec<&'a str> {
        lines
            .iter()
            .map(String::as_str)
            .filter(|line| line.split(' ').nth(1) == Some(rule))
            .collect()
    }

    /// The recorded `card-style-fixture.json` run: one fault for each rule, the same
    /// 14 ordered lines the standalone tool printed before the extraction. The pure
    /// core raises them with no file, no process, and no archive client.
    #[test]
    fn the_fixture_raises_one_fault_for_each_rule() {
        let fixture: Value = serde_json::from_str(CARD_STYLE_FIXTURE).unwrap();
        let cards = fixture.as_array().expect("the fixture is an array of entries");
        let setting = utf16_form("K/1|a|a");
        let setting_body = cards
            .iter()
            .find(|card| ps_eq(&text(prop(card, "address")), &setting))
            .and_then(|card| prop(card, "body"))
            .map(|body| ps_string(Some(body)));
        let shared = cards
            .iter()
            .filter(|card| ps_eq(&text(prop(card, "line")), "shared"))
            .map(|card| ps_string(prop(card, "address")))
            .collect::<Vec<_>>();
        let check = Check::new(Limits::default(), false, "")
            .with_terms(setting_body.as_deref())
            .with_shared(shared)
            // The recorded lines are the ones the coordinated policy produces (S06):
            // an entry that names no Workspace is a draft in the agent Workspace.
            .with_policies(crate::policy::PolicySet::new());
        let report = check.report(cards);
        // S02-AC01: every fault carries the structured contract, its severity and
        // scope come from the rule catalog and the effective policy, and the
        // familiar lines are exactly the formatted report, in the same order.
        let policies = crate::policy::PolicySet::new();
        let agent = policies.policy(crate::policy::Workspace::Agent);
        for diagnostic in &report {
            assert!(!diagnostic.address.is_empty(), "{diagnostic:?}");
            assert!(!diagnostic.rule.is_empty(), "{diagnostic:?}");
            assert!(!diagnostic.fault.is_empty(), "{diagnostic:?}");
            assert!(!diagnostic.message.is_empty(), "{diagnostic:?}");
            assert!(!diagnostic.legacy);
            let decided = agent
                .effective(&diagnostic.rule)
                .and_then(crate::policy::Setting::severity);
            assert!(
                diagnostic.subcase || decided.is_none_or(|severity| severity == diagnostic.severity),
                "{diagnostic:?}"
            );
            assert_eq!(diagnostic.scope, crate::policy::scope(&diagnostic.rule));
        }
        let lines = reported(
            report.iter().map(Diagnostic::line).collect(),
            CheckReport::of(report.clone()).blockers,
        );
        let expected = CARD_STYLE_EXPECTED.lines().collect::<Vec<_>>();
        assert_eq!(lines, expected, "the fixture lines, in order: {lines:#?}");
        assert_eq!(check.run(cards), report.iter().map(Diagnostic::line).collect::<Vec<_>>());
        // The setting's own tables, the quotation, chart, and formulas, and a shared
        // part's words under another model's term table raise nothing. The shared
        // part's one bare deictic is the pilot's advice, which its own text earns.
        let skipped = Regex::new(r"引文|图里|清算价值；|^Other/").unwrap();
        for line in &lines {
            let shared_advice = line.starts_with("Other/") && line.contains(" R21 (advisory) ");
            assert!(
                !skipped.is_match(line) || shared_advice,
                "checked text it must skip: {line}"
            );
        }
        // A graph rule's diagnostic says so; a text rule's stays local.
        let graph = json!([{
            "address": "K/2", "category": "idea", "workspace": "agent",
            "title": "剩余控制权决定契约空白处", "body": "所有者能拒绝合作，所以契约的空白处由所有者决定。",
            "inbound": []
        }]);
        let report = Check::new(Limits::default(), false, "").report(graph.as_array().unwrap());
        let i5 = report.iter().find(|diagnostic| diagnostic.rule == "I5").expect("I5");
        assert_eq!(i5.scope, Scope::Graph);
        assert_eq!(i5.severity, Severity::Block);
    }


    // R22: a formula a lost `$A_I` left open is a fault in a title or a body; closed inline math, a
    // display block, code, and an escaped dollar are not.
    #[test]
    fn an_open_inline_formula_is_r22() {
        assert!(open_math("普遍纵向一体化均衡稳定当且仅当 <A_O$"));
        assert!(open_math("当 $A_I<A_O 时稳定"));
        assert!(!open_math("均衡稳定当且仅当 $A_I<A_O$"));
        assert!(!open_math("$$\na=b$c\n$$ 之后 $x$"));
        assert!(!open_math("代码 `$x` 与价格 \\$5"));
        let cards = json!([{ "address": "K/4", "title": "均衡稳定当且仅当 <A_O$", "body": "成立条件是 $A_I<A_O$。\n\n来源：[[K]] §4" },
            { "address": "K/4|a", "title": "两个门槛决定稳定性", "body": "稳定要求 $A_I<A_O。\n\n来源：[[K]] §4" }]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        assert!(lines.iter().any(|l| l.starts_with("K/4 R22 the title leaves an inline formula open")), "{lines:#?}");
        assert!(lines.iter().any(|l| l.starts_with("K/4|a R22 the body leaves an inline formula open")), "{lines:#?}");
        assert!(!lines.iter().any(|l| l.starts_with("K/4 R22 the body")), "{lines:#?}");
    }

    // R2: a book title in 《…》 counts as one name, and the Literature Card's document line is a locator; a
    // long prose sentence stays a fault.
    #[test]
    fn a_book_title_and_the_document_line_do_not_make_r2() {
        let cards = json!([{ "address": "K/1|f", "title": "结构卡给出全书的论证顺序",
            "body": "本卡是《Integration versus Outsourcing in Industry Equilibrium》的结构卡：按阅读顺序列出论证。\n\n原书文档：[原书 PDF](scholar:78) · [中英对照 Markdown](scholar:75) · [原文 Markdown](scholar:76)\n\n企业在进入阶段比较两种组织模式的预期利润，然后选择利润更高的那一种组织模式进入行业。\n\n来源：[[K]] §1" }]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        let r2: Vec<&String> = lines.iter().filter(|l| l.contains(" R2 ")).collect();
        assert_eq!(r2.len(), 1, "{lines:#?}");
        assert!(r2[0].contains("企业在进入阶段"), "{lines:#?}");
    }

    // A Model Card's 结构 list is navigation: its long part lines raise no prose fault; prose after it still does.
    #[test]
    fn the_structure_list_of_a_model_card_is_not_prose() {
        let cards = json!([{ "address": "K/7|a", "type": "model", "title": "匹配规模报酬递增模型", "level": "formal",
            "body": "问题：经济规模怎样改变均衡组织模式？\n\n**结构**\n\n- 推导：μL 增大时两个交点的移动（2/2） [[K/7|a|a|15]]：推导：μL 增大，两个交点沿 VV 线移动，外包均衡出现\n- 设定 [[K/7|a|a]]：设定\n\n**来源**\n\n企业在进入阶段比较两种组织模式的预期利润，然后选择利润更高的那一种组织模式进入行业。\n\n来源：[[K]] §7" }]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        let r2: Vec<&String> = lines.iter().filter(|l| l.contains(" R2 ")).collect();
        assert_eq!(r2.len(), 1, "{lines:#?}");
        assert!(r2[0].contains("企业在进入阶段"), "{lines:#?}");
    }

    fn small() -> Value {
        json!([
            { "address": "S/1|a|a", "facet": "设定", "title": "设定", "body": "E 有一个项目，项目在时点 2 产生收益。C 出资，换取时点 1 的还款。E 可以挪用一部分收益。" },
            { "address": "S/1|a|a|1", "facet": "推导", "title": "推导", "body": "1. E 在时点 1 还款以后，手里剩下项目的收益。\n2. E 在时点 2 挪用一部分收益，留下其余部分。\n3. 两边加上初始财富，得到参与约束。" },
        ])
    }

    /// R13 counts prose characters too; a Derivation part's steps count under R14 only.
    #[test]
    fn card_size_counts_characters_and_steps() {
        let limits = Limits {
            max_chars: 30,
            max_steps: 2,
            ..Limits::default()
        };
        let size = run_json(&small(), "", limits.clone(), false, "");
        assert_prefixes(
            &size,
            &["S/1|a|a R13 (", "S/1|a|a|1 R14 (3 steps > 2)", "faults: 2"],
        );
        assert!(
            size[0].contains("characters > 30"),
            "R13 must count characters: {}",
            size[0]
        );
        // A concept model keeps its few parts whole: no size faults.
        let concept = run_json(&small(), "", limits, false, "concept");
        assert_eq!(concept, ["faults: 0"]);
    }

    fn explain_cards() -> Value {
        let steps = "**步骤**\n1. 参与约束：E 的所得高于 $w$，即 $R+fy_2>w$。\n2. 情形 $f=1$：$R=c-P$。\n   说明：不清算时，现金够还款，E 留下差额。\n3. 两边加 $K-w$，得 $y_1+fy_2>K$。";
        let good = format!(
            "**思路**\nE 只在还款后所得超过 $w$ 时参与。先求 E 还款后剩下的现金 $R$。\n\n**起点**\n- 依赖设定 [[X/1|a|a]]：E 参与，须所得高于 $w$。\n\n{steps}\n\n**结果**\n$$y_1+fy_2>K$$\n\n**解读**\n$w$ 在两边相消，所以 E 的财富不影响参与。检验：$f=1$ 时它是普通的净现值条件。"
        );
        let long = (1..=17)
            .map(|n| format!("第 {n} 句解释推导的一步。"))
            .collect::<String>();
        let without_reading = good[..good.find("**解读**").unwrap()].trim().to_string();
        json!([
            { "address": "X/1|a|a|1", "facet": "推导：参与约束", "title": "推导：参与约束", "body": good },
            { "address": "X/1|a|a|2", "facet": "推导：缺解读", "title": "推导：缺解读", "body": without_reading },
            { "address": "X/1|a|a|3", "facet": "推导：两行说明", "title": "推导：两行说明", "body": good.replace("留下差额。", "留下差额。\n   说明：这一行多余。") },
            { "address": "X/1|a|a|4", "facet": "推导：太长", "title": "推导：太长", "body": good.replace("**解读**\n", &format!("**解读**\n{long}\n")) },
            { "address": "X/1|a|a|5", "facet": "推导：旧格式", "title": "推导：旧格式", "body": format!("**起点**\n- 依赖设定 [[X/1|a|a]]。\n\n{steps}\n\n**结果**\n$$y_1+fy_2>K$$") },
        ])
    }

    /// R15 to R17: the explanation of a Derivation part.
    #[test]
    fn a_derivation_part_holds_its_explanation_sections() {
        let cards = explain_cards();
        let plain_run = run_json(&cards, "", Limits::default(), false, "");
        assert_prefixes(
            &plain_run,
            &[
                "X/1|a|a|2 R15 sections 思路, 起点, 步骤, 结果; want 思路, 起点, 步骤, 结果, 解读",
                "X/1|a|a|3 R17 a second 说明 line under step 2",
                "X/1|a|a|4 R16 (解读 has 19 sentences > 6)",
                "X/1|a|a|4 R17 (22 explanation sentences > 16)",
                "faults: 4",
            ],
        );
        // --require-explain flags the older part too.
        let strict = run_json(&cards, "", Limits::default(), true, "");
        assert!(
            strict
                .iter()
                .any(|line| line.starts_with("X/1|a|a|5 R15 sections 起点, 步骤, 结果")),
            "{strict:#?}"
        );
        assert_eq!(strict.last().unwrap(), "faults: 5");
        // The explanation does not count under R13: 12 sentences of 思路 and 解读 fit a 3-sentence part.
        let limits = Limits {
            max_sentences: 3,
            max_explain_sentences: 40,
            max_explain_chars: 2000,
            ..Limits::default()
        };
        let r13 = run_json(&cards, "", limits, false, "");
        let part = Regex::new(r"^X/1\|a\|a\|[1-4] R13 ").unwrap();
        assert!(!r13.iter().any(|line| part.is_match(line)), "{r13:#?}");
    }

    #[test]
    fn the_position_line_of_a_chain_is_not_the_writers_text() {
        let body = "**思路**\n第 2 张，共 3 张：这一行很长很长很长很长很长很长很长很长很长很长很长很长很长很长很长很长很长很长。\n本张求出剩下的现金。\n\n**起点**\n- 依赖设定 [[X/1|a|a]]。\n\n**步骤**\n1. 代入 $P=B$。\n\n**结果**\n$$R=c$$\n\n**解读**\n现金决定参与。";
        let lines = run_json(
            &json!([{ "address": "X/1|a|a|1", "facet": "推导", "title": "推导：现金", "body": body }]),
            "",
            Limits::default(),
            true,
            "",
        );
        assert_eq!(lines, ["faults: 0"]);
    }

    /// The Card layer rules read the entry's category, Links, kinds, and tree
    /// (the expected lines are card-style-check.ps1's own output on these entries).
    #[test]
    fn the_layer_rules_read_category_links_and_kinds() {
        let cards = json!([
            { "address": "5/1", "title": "税", "body": "引出 [[5/2]]。参见 [[5/3]]。\n\n依赖 [[5/3]] 的结论，税负落在买方。", "category": "idea", "parent": "", "links": ["5/2", "5/3"], "kinds": ["5/3"], "book": "Hart1995", "tree": "5" },
            { "address": "5/2", "title": "1999 年的亚洲金融危机", "body": "时间：1999 年。\n地点：亚洲。", "category": "event", "links": [], "inbound": ["5/1"], "parent": "5/1", "inbound_kinded": 0 },
            { "address": "5/3", "title": "1999 年亚洲的金融危机", "body": "- **时间**：1999 年。\n- 地点：亚洲。\n参与者：银行。\n经过：挤兑。\n来源：[[Hart1995/2]]", "category": "event", "links": ["Hart1995/2"], "kinds": [], "inbound": [], "inbound_kinded": 1, "book": "Hart1995" },
            { "address": "5/4", "title": "商品税的负担落在买方身上", "body": "商品税的负担落在买方身上，因为需求缺乏弹性。来源：[[Hart1995]]。", "category": "idea", "parent": "5/3", "links": ["Hart1995"], "kinds": [], "tree": "5" },
            { "address": "5/5", "title": "商品税的负担落在买方身上了", "body": "同上。", "category": "idea", "parent": "5/4", "links": [], "kinds": [], "tree": "5" },
        ]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        assert_eq!(
            lines,
            [
                "5/1 I1 (advisory) the title states no claim: 税",
                "5/1 I2 (19 characters < 40) merge candidate",
                "5/1 I3 no source line: no Link to [[Hart1995]] or one of its anchors",
                "5/1 L2 (2 characters < 8) a Link with no reason: 引出 [[5/2]]。",
                "5/1 L2 (2 characters < 8) a Link with no reason: 参见 [[5/3]]。",
                "5/1 L3 (advisory) [[5/3]] is linked in 2 paragraphs",
                "5/2 E2 missing labels: 参与者、经过、来源",
                "5/2 E4 no Idea Card links it with a kind word",
                "5/3 I5 no other Card has an authored Link here",
                "5/4 I2 (26 characters < 40) merge candidate",
                "5/4 I4 no kinded Link to a Card other than the parent",
                "5/5 I2 (3 characters < 40) merge candidate",
                "5/5 I4 no kinded Link to a Card other than the parent",
                "5/5 I6 (advisory) a title very similar to [[5/4]]",
                "5/3 E5 (advisory) the same event as [[5/2]]?",
                "faults: 15",
            ]
        );
    }

    /// S16-AC03 (#158, design 10.2): a Term use is no reasoning Link. A Card whose Links reach a
    /// Term head only by Term uses reports no L1 unkinded Link, no L2 missing reason, and no L3
    /// repeat, while an ordinary unkinded Link in the same body still reports exactly itself.
    #[test]
    fn term_uses_are_no_reasoning_links_in_the_l_rules() {
        let l_lines = |cards: Value| -> Vec<String> {
            run_json(&cards, "", Limits::default(), false, "")
                .into_iter()
                .filter(|line| {
                    line.split(' ')
                        .nth(1)
                        .is_some_and(|rule| matches!(rule, "L1" | "L2" | "L3"))
                })
                .collect()
        };
        // Two short paragraphs, each using the same Term head: no unkinded Link, no missing
        // reason, and no repeated-Link fault, because none of them is a reasoning Link.
        let term_only = json!([{
            "address": "6/1", "title": "均衡决定价格水平",
            "body": "见 [[均衡@2/1|a]]。\n\n又见 [[均衡@2/1|a]]。",
            "category": "idea", "parent": "6/0", "tree": "6",
            "links": ["2/1|a"], "kinds": []
        }]);
        assert_eq!(l_lines(term_only), Vec::<String>::new());
        // The same body with one ordinary unkinded Link: only that Link is read as a reference.
        let mixed = json!([{
            "address": "6/2", "title": "均衡与两种解释",
            "body": "均衡 [[6/9]] 与 [[均衡@2/1|a]] 在这张卡里并列地给出两种解释。",
            "category": "idea", "parent": "6/0", "tree": "6",
            "links": ["6/9", "2/1|a"], "kinds": []
        }]);
        assert_eq!(
            l_lines(mixed),
            ["6/2 L1 (1 Links read as refers) [[6/9]]"]
        );
    }

    /// S16-AC03 (#158): a Term use gives no Idea reasoning credit. A Card whose only kinded Link
    /// is a Term use carries no kinded Link at all (its `kinds` never holds `term`), so I4 still
    /// reports it, and the Term use is named apart in `term_links`.
    #[test]
    fn a_term_use_gives_no_kinded_link_credit() {
        let cards = json!([{
            "address": "6/3", "title": "均衡决定价格水平",
            "body": "均衡 [[均衡@2/1|a]] 决定价格水平，因为供给与需求在这里相等，价格不再变动。",
            "category": "idea", "parent": "6/0", "tree": "6",
            "links": ["2/1|a"], "kinds": [], "term_links": ["2/1|a"]
        }]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        assert_eq!(of_rule(&lines, "I4"), ["6/3 I4 no kinded Link to a Card other than the parent"]);
        assert!(of_rule(&lines, "L1").is_empty(), "{lines:#?}");
        assert!(of_rule(&lines, "L2").is_empty(), "{lines:#?}");
    }

    /// S16-AC03/S16-AC06 (#158): a source citation beside a Term use still counts as the source
    /// line (I3), and the Term use is still no reasoned Link (L1, L2, L3).
    #[test]
    fn a_source_citation_beside_a_term_use_keeps_its_own_meaning() {
        let cards = json!([{
            "address": "6/4", "title": "均衡的定义与它的出处",
            "body": "见 [[均衡@2/1|a]]。\n\n来源：[[Hart1995/2]] 第 3 节。",
            "category": "idea", "parent": "6/0", "tree": "6", "book": "Hart1995",
            "links": ["2/1|a", "Hart1995/2"], "kinds": []
        }]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        for rule in ["I3", "L1", "L2", "L3"] {
            assert!(
                of_rule(&lines, rule).is_empty(),
                "{rule} must not read the Term use or lose the citation: {lines:#?}"
            );
        }
    }

    /// F1 to F4: the labels, the source line, one result per Card, and a kinded Link in.
    #[test]
    fn the_finding_rules_read_labels_the_source_line_and_one_result() {
        let good = "测量：企业是否因为缺少保险市场而推迟投资。\n数据：48 家企业的访谈，2024 年，美国。\n方法：半结构化访谈。\n结果：31 家企业推迟了投资。\n局限：样本不是随机样本。\n来源：[[Smith2024/3]] §3.2\n\n这一结果印证 [[5/1]] 的判断：缺少市场会压低投资。";
        let cards = json!([
            { "address": "5/2", "title": "31 家受访企业因为缺少保险市场推迟了投资", "body": good, "category": "finding", "links": ["Smith2024/3", "5/1"], "kinds": ["5/1"], "inbound": ["5/1"], "parent": "5/0", "inbound_kinded": 1, "book": "Smith2024" },
            { "address": "5/3", "title": "受访企业推迟了投资", "body": "- **测量**：投资时点。\n- 数据：48 家企业。\n- 结果：\n  - 31 家推迟投资。\n  - 12 家取消投资。\n- 局限：自报原因。\n来源：见第三章", "category": "finding", "links": [], "kinds": [], "inbound": ["5/1"], "parent": "5/0", "inbound_kinded": 0, "book": "Smith2024" },
            { "address": "5/4", "title": "两项结果写在一张卡里", "body": "测量：投资。\n数据：企业。\n方法：访谈。\n结果：推迟。\n结果：取消。\n局限：小样本。\n来源：", "category": "finding" },
        ]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        let finding = lines
            .iter()
            .filter(|line| {
                line.split(' ')
                    .nth(1)
                    .is_some_and(|rule| rule.starts_with('F'))
            })
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            finding,
            [
                "5/3 F1 missing labels: 方法",
                "5/3 F2 the source line links neither [[Smith2024]] nor one of its anchors",
                "5/3 F3 (2 results under 结果) one result per Card: split it",
                "5/3 F4 no Idea Card links it with a kind word",
                "5/4 F2 the source line names no source",
                "5/4 F3 (2 结果 labels) one result per Card: split it",
            ]
        );
        // A Finding Card takes the shared layer rules too, but not I3: F2 checks its source line.
        assert!(
            !lines.iter().any(|line| line.starts_with("5/2 ")),
            "{lines:#?}"
        );
        assert!(
            !lines.iter().any(|line| line.contains(" I3 ")),
            "{lines:#?}"
        );
        let (kept, count) = filter_rules(&lines, DEFAULT_RULES);
        assert!(kept.iter().any(|line| line.starts_with("5/3 F1 ")));
        assert!(
            !kept.iter().any(|line| line.contains(" F4 ")),
            "F4 is not a default rule"
        );
        assert!(count >= 5);
    }

    /// R6 reads a rejected word inside another kept term as that term: 税率差 is rejected for 幅度, and
    /// 税率差异化 and 税率档数 are terms of their own (ebrill2001modern: 66 of its 100 R6 faults).
    #[test]
    fn a_rejected_word_inside_another_kept_term_is_that_term() {
        let body = "术语表

| 术语 | 不用 | 意思 | 首次定义 |
| --- | --- | --- | --- |
| 幅度 | 税率差、税率差距 | 最高与最低税率之差 | §4.2 |
| 税率差异化 | 差别税率 | 不同商品适用不同税率 | §7 |
| 税率差额 | — | 两个税率相减 | §7 |
";
        let check = Check::new(Limits::default(), false, "").with_terms(Some(body));
        let r6 = |sentence: &str| {
            let mut faults = Faults::default();
            check.sentence(&mut faults, "K/1", false, sentence, false, false);
            faults
                .diagnostics
                .iter()
                .map(Diagnostic::line)
                .filter(|line| line.contains(" R6 "))
                .collect::<Vec<_>>()
        };
        assert!(r6("税率差异化提高了征收成本。").is_empty(), "a kept term with a rejected word in it");
        assert!(r6("两档税率的税率差额是五个百分点。").is_empty(), "a kept term without rejected words of its own");
        assert_eq!(r6("税率差扩大以后，税率差异化的成本上升。").len(), 1, "the rejected word on its own is still a fault");
        assert_eq!(r6("差别税率提高了征收成本。").len(), 1);
    }

    /// R0270/R0271 and S12-AC01/AC07 (S12, #154): every language and size rule reads the visible
    /// word a Term link shows, never its address. Adding or moving only the markup changes no
    /// fault, while changing the visible label is a real prose change.
    #[test]
    fn term_links_read_as_visible_prose_in_the_language_and_size_rules() {
        let table = "术语表：\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |\n";
        let check = Check::new(Limits::default(), false, "").with_terms(Some(table));
        let rules = |sentence: &str| {
            let mut faults = Faults::default();
            check.sentence(&mut faults, "K/1", false, sentence, false, false);
            faults
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.rule.clone())
                .collect::<Vec<_>>()
        };

        // A 41-character visible sentence with a Term link is still an R2 fault: dropping the
        // macro used to hide its five visible characters and pass.
        let prefix = "这句用于检查可见句子长度的示例".repeat(2) + "检查句子长度";
        let visible = format!("{prefix}边际替代率");
        assert_eq!(measure(&visible).size, 41, "{visible}");
        let with_link = format!("{prefix}[[边际替代率@2/1|a]]。");
        assert_eq!(rules(&with_link), ["R2"], "the projected label is measured");
        assert_eq!(rules(&format!("{visible}。")), ["R2"], "the plain sentence reads the same");

        // The address is never prose: a visible sentence under the limit keeps no fault even with
        // a target whose address alone would push it over.
        let at_limit = format!("{prefix}均衡");
        assert_eq!(measure(&at_limit).size, 38);
        assert!(
            rules(&format!("{prefix}[[均衡@Lit2026/1|a]]。")).is_empty(),
            "a 38-character visible sentence with a 12-character address keeps no R2 fault"
        );

        // R6 checks the visible term, not the address: the rejected word inside the label is the
        // same fault it is in plain prose.
        assert_eq!(rules("价格 [[平衡@2/1|a]] 不变。"), ["R6"], "the label is prose");
        assert_eq!(rules("价格 平衡 不变。"), ["R6"], "the same word without markup");
        assert!(rules("价格 [[均衡@2/1|a]] 不变。").is_empty());
        assert!(rules("价格 均衡 不变。").is_empty());
        // Moving only the markup around the same words invents no fault (S12-AC07).
        assert!(rules("价格 均衡 [[均衡@2/1|a]] 不变。").is_empty());
    }

    /// R20 and R21: a local label that stands in for its content, and a deictic
    /// with no named Link in its sentence. Both are advice, not blocking faults.
    #[test]
    fn local_references_and_referents_are_advice() {
        let cards = json!([
            { "address": "T/1|a", "title": "式 (12) 有该项", "body": "这一张只给了编号。" },
            { "address": "T/1|b", "title": "加成项来自需求弹性",
              "body": "式 (12) 的加成项来自需求弹性。\n\n命题 3.G.3 说明均衡唯一。\n\n来源：[[book]] §11.D" },
            { "address": "T/1|c", "title": "第一福利定理成立", "body": "第一福利定理在这里成立。\n\n§11.D 有该项。" },
            { "address": "T/1|d", "title": "代码里的编号", "body": "```\n式 (12) 有该项\n```\n\n> 式 (12) 有该项\n\n`式 (12)` 有该项。" },
            { "address": "T/1|e", "title": "该卡说明了均衡", "body": "那里给出了答案。" },
            { "address": "T/1|f", "title": "均衡的存在性", "body": "该卡 [[T/1|a]] 说明了均衡。\n\n先看 [[T/1|a]] 的方法。该卡给出了结果。" },
            { "address": "T/1|g", "title": "平均分成三份", "body": "第 6 章给出了结论。\n\n本章。" },
            { "address": "T/1|h", "category": "anchor", "title": "第 3 组", "body": "§3.1 稀缺与配置\n\n§3.2 市场均衡" },
            // A book-local model number in the title, and formulas whose labels are no prose.
            { "address": "T/1|i", "title": "模型 1 的设定", "body": "模型 1 的设定说明企业如何选择产量。" },
            { "address": "T/1|j", "title": "加成项来自需求弹性", "body": "由 $式 (12)$ 可得。\n\n$$\n式 (12) 有该项\n$$" },
            // A Literature root's body is bibliographic/navigational, so a chapter label in it is no fault.
            { "address": "book", "category": "root", "title": "企业家理论", "body": "第 6 章" },
        ]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        assert_eq!(
            of_rule(&lines, "R20"),
            [
                "T/1|a R20 (advisory) local reference `式 (12)` names no content: 式 (12) 有该项",
                "T/1|c R20 (advisory) local reference `§11.D` names no content: §11.D 有该项。",
                "T/1|g R20 (advisory) local reference `本章` names no content: 本章。",
                "T/1|i R20 (advisory) local reference `模型 1` names no content: 模型 1 的设定",
            ]
        );
        assert_eq!(
            of_rule(&lines, "R21"),
            [
                "T/1|e R21 (advisory) unnamed referent `该卡` with no named Link in its sentence: 该卡说明了均衡",
                "T/1|e R21 (advisory) unnamed referent `那里` with no named Link in its sentence: 那里给出了答案。",
                "T/1|f R21 (advisory) unnamed referent `该卡` with no named Link in its sentence: 该卡给出了结果。",
            ]
        );
        // Advice: a default self-check reports the lines but counts no fault for them.
        let (kept, faults) = filter_rules(&lines, DEFAULT_RULES);
        assert!(kept.iter().any(|line| line.contains(" R20 (advisory) ")));
        assert_eq!(faults, 0);
        // D138's mode difference: --json counts every line it printed, advisory included.
        let printed = lines.len() - 1;
        assert_eq!(lines[printed], format!("faults: {printed}"));
        assert!(of_rule(&lines, "R20").len() >= 1 && of_rule(&lines, "R21").len() >= 1);
    }

    /// I7: a Topic Idea Card whose `rests-on` Link enters its own source's
    /// Literature tree. A source Link, `illustrates`, another book, and a Card
    /// in that Literature tree are no fault; missing source evidence is unknown.
    #[test]
    fn own_source_dependencies_are_reported() {
        let idea = |address: &str, tree: &str, body: &str, target: &str, kind: &str| {
            json!({
                "address": address, "category": "idea", "tree": tree, "parent": format!("{tree}/1"),
                "title": format!("{} 的前提", address), "body": body,
                "links": [target], "kinds": [target], "relations": { target: kind },
            })
        };
        let mut own = idea("12/3/1|a", "12/3", "依赖 [[book/4]] 的定义：所有者能拒绝合作。", "book/4", "rests-on");
        own["sources"] = json!(["book"]);
        let mut illustrates = idea("12/3/1|b", "12/3", "例示 [[book/5]] 的两期模型。", "book/5", "illustrates");
        illustrates["sources"] = json!(["book"]);
        let mut other = idea("12/3/1|c", "12/3", "依赖 [[other/2]] 的前提：投资不可转用。", "other/2", "rests-on");
        other["sources"] = json!(["book"]);
        let mut in_book = idea("book/4|a", "book", "依赖 [[book/4]] 的定义：控制权随状态转移。", "book/4", "rests-on");
        in_book["sources"] = json!(["book"]);
        let unknown = idea("12/3/1|d", "12/3", "依赖 [[book/6]] 的结论：事后分配由地位决定。", "book/6", "rests-on");
        let mut from_body = idea("12/3/1|e", "12/3", "依赖 [[book/7]] 的设定。\n\n来源：[[book]] §3.2", "book/7", "rests-on");
        from_body["links"] = json!(["book/7", "book"]);
        // D106: several named sources are each checked, and the matched premise is named.
        let mut explicit_two = idea("12/3/1|f", "12/3", "依赖 [[other/2]] 的前提：投资不可转用。", "other/2", "rests-on");
        explicit_two["sources"] = json!(["book", "other"]);
        let mut body_two = idea("12/3/1|g", "12/3", "依赖 [[book/8]] 的设定。\n\n来源：[[book]] §3.2\n来源：[[other]] §2.1", "book/8", "rests-on");
        body_two["links"] = json!(["book/8", "book", "other"]);
        // D105: `corresponds` and `contrasts` express a real relation, not a premise.
        let mut corresponds = idea("12/3/1|h", "12/3", "对应 [[book/9]] 的均衡。", "book/9", "corresponds");
        corresponds["sources"] = json!(["book"]);
        let mut contrasts = idea("12/3/1|i", "12/3", "对照 [[book/10]] 的设定。", "book/10", "contrasts");
        contrasts["sources"] = json!(["book"]);
        let cards = json!([own, illustrates, other, in_book, unknown, from_body, explicit_two, body_two, corresponds, contrasts]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        assert_eq!(
            of_rule(&lines, "I7"),
            [
                "12/3/1|a I7 `rests-on` [[book/4]] enters its own source `book`",
                "12/3/1|d I7 (advisory) source evidence unknown, so [[book/6]] may be an own-source premise",
                "12/3/1|e I7 `rests-on` [[book/7]] enters its own source `book`",
                "12/3/1|f I7 `rests-on` [[other/2]] enters its own source `other`",
                "12/3/1|g I7 `rests-on` [[book/8]] enters its own source `book`",
            ]
        );
    }

    /// I6 compares a checked Idea title with its own Workspace (D129, D135): another
    /// Workspace stays separate, a context Card is never the reported subject, and an
    /// unordered pair is reported once.
    #[test]
    fn similar_titles_read_the_workspace_context() {
        let cards = json!([
            { "address": "12/1|a", "category": "idea", "workspace": "agent", "tree": "12",
              "title": "剩余控制权决定契约空白处", "body": "所有者能拒绝合作，所以契约的空白处由所有者决定。" }
        ]);
        let context = title_context(
            &json!([
                { "address": "12/1|b", "title": "剩余控制权决定契约空白处", "workspace": "agent", "tree": "12" },
                { "address": "41/1|b", "title": "剩余控制权决定契约的空白处", "workspace": "agent", "tree": "41" },
                { "address": "h/1", "title": "剩余控制权决定契约空白处", "workspace": "human", "tree": "h" },
                { "address": "12/1|c", "title": "所有权的配置影响投资激励", "workspace": "agent", "tree": "12" }
            ]),
            true,
        );
        let lines = run_with_context(&cards, context, Limits::default());
        assert_eq!(
            of_rule(&lines, "I6"),
            [
                "12/1|a I6 (advisory) a title very similar to [[12/1|b]]",
                "12/1|a I6 (advisory) a title very similar to [[41/1|b]]",
            ]
        );
        // Two checked Cards with one title: the later one reports the pair once.
        let cards = json!([
            { "address": "12/1|a", "category": "idea", "workspace": "agent", "title": "剩余控制权决定契约空白处" },
            { "address": "12/1|d", "category": "idea", "workspace": "agent", "title": "剩余控制权决定契约空白处" }
        ]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        assert_eq!(of_rule(&lines, "I6"), ["12/1|d I6 (advisory) a title very similar to [[12/1|a]]"]);
    }

    /// An exact copy is a pair even when the two spellings differ only in case or in
    /// punctuation the Dice function already ignores: the candidate index folds case, so
    /// no full scan of the context is needed to find one (D137).
    #[test]
    fn an_exact_title_copy_is_found_without_scanning_the_context() {
        let cards = json!([
            { "address": "30/1|a", "category": "idea", "workspace": "agent",
              "title": "Default Risk Drives Out Good Sellers" },
            { "address": "30/1|b", "category": "idea", "workspace": "agent", "title": "公开" }
        ]);
        let context = title_context(
            &json!([
                { "address": "30/1|c", "title": "default risk drives out good sellers", "workspace": "agent" },
                { "address": "30/1|d", "title": "Default Risk Drives Out Good Sellers!", "workspace": "agent" },
                { "address": "30/1|e", "title": "公开", "workspace": "agent" },
                { "address": "h/1", "title": "default risk drives out good sellers", "workspace": "human" }
            ]),
            true,
        );
        let lines = run_with_context(&cards, context, Limits::default());
        assert_eq!(
            of_rule(&lines, "I6"),
            [
                // The case-only copy: the same title, and the other Workspace stays out.
                "30/1|a I6 (advisory) a title very similar to [[30/1|c]]",
                // One punctuation mark apart: one pair, reported once, on the checked Card.
                "30/1|a I6 (advisory) a title very similar to [[30/1|d]]",
                // A title of one character has no bigram; an exact copy is still a pair.
                "30/1|b I6 (advisory) a title very similar to [[30/1|e]]",
            ]
        );
    }

    /// D135: the context is one Workspace's title projection. An entry that names no
    /// Workspace cannot keep the two apart, so the run marks I6 unknown instead of
    /// silently comparing across a Workspace boundary.
    #[test]
    fn an_entry_without_a_workspace_marks_i6_unknown() {
        let cards = json!([
            { "address": "12/1|a", "category": "idea", "title": "剩余控制权决定契约空白处" }
        ]);
        let context = title_context(
            &json!([
                { "address": "12/1|b", "title": "剩余控制权决定契约空白处", "workspace": "agent" },
                { "address": "h/1", "title": "剩余控制权决定契约空白处", "workspace": "human" }
            ]),
            true,
        );
        let lines = run_with_context(&cards, context, Limits::default());
        assert_eq!(
            of_rule(&lines, "I6"),
            ["12/1|a I6 (advisory) Workspace scope unknown: the entry names no Workspace, so nothing is compared"]
        );
    }

    /// D136: a reader that supplies an incomplete Workspace title context makes
    /// I6 unknown. A tree-only read never claims a Workspace-wide check, and a
    /// run with no context keeps the documented run-scoped comparison.
    #[test]
    fn an_incomplete_title_context_marks_i6_unknown() {
        let card = json!({ "address": "12/1|a", "category": "idea", "workspace": "agent", "tree": "12",
            "title": "剩余控制权决定契约空白处", "body": "所有者能拒绝合作，所以契约的空白处由所有者决定。" });
        let row = json!([{ "address": "41/1|b", "title": "剩余控制权决定契约的空白处", "workspace": "agent", "tree": "41" }]);
        let incomplete = title_context(&row, false);
        let lines = run_with_context(&json!([card.clone()]), incomplete, Limits::default());
        assert_eq!(
            of_rule(&lines, "I6"),
            [
                "12/1|a I6 (advisory) Workspace title context incomplete: 1 known title compared, others unknown",
                "12/1|a I6 (advisory) a title very similar to [[41/1|b]]",
            ]
        );
        // The same rows with the completeness the reader asserts: no unknown line.
        let complete = title_context(&row, true);
        let lines = run_with_context(&json!([card.clone()]), complete, Limits::default());
        assert_eq!(of_rule(&lines, "I6"), ["12/1|a I6 (advisory) a title very similar to [[41/1|b]]"]);
        // No context at all: the documented comparison of the entries the run holds.
        let cards = json!([card]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        assert!(of_rule(&lines, "I6").is_empty());
    }


    #[test]
    fn lengths_count_utf16_units_and_a_clip_ends_with_an_ellipsis() {
        assert_eq!(length(&utf16_form("a😀b")), 4);
        assert_eq!(from_utf16_form(&utf16_form("a😀b")), "a😀b");
        let mut faults = Faults::default();
        let long = format!("{}😀tail", "x".repeat(88));
        faults.add("A/1", "R2", "sentence_length", &utf16_form(&long), &utf16_form(&long));
        assert_eq!(
            faults.diagnostics[0].line(),
            format!("A/1 R2 {}\u{FFFD}…", "x".repeat(88))
        );
    }

    /// acemoglu2007equilibrium: the Bridge apply writes `跨书关联：<sentence>`, and the judge's precheck had
    /// measured the sentence without its label, so 16 applied sentences of 36 to 40 characters failed R2.
    #[test]
    fn the_bridge_label_is_not_part_of_the_sentence_length() {
        let sentence = format!("对照 [[41/1|b]]：{}。", "甲".repeat(35));
        let card = |body: String| json!({ "cards": [{ "address": "12/1|a", "category": "idea", "workspace": "agent",
            "tree": "12", "title": "剩余控制权决定契约空白处", "body": body }] });
        let bridged = run_json(&card(format!("所有者决定空白处。\n\n跨书关联：{sentence}"))["cards"], "", Limits::default(), false, "");
        assert!(of_rule(&bridged, "R2").is_empty(), "{bridged:#?}");
        let unlabelled = run_json(&card(format!("所有者决定空白处。\n\n跨书关联：{sentence}{sentence}"))["cards"], "", Limits::default(), false, "");
        assert_eq!(of_rule(&unlabelled, "R2").len(), 0, "two sentences are measured one by one: {unlabelled:#?}");
        let long = run_json(&card(format!("所有者决定空白处。\n\n跨书关联：对照 [[41/1|b]]：{}。", "甲".repeat(41)))["cards"], "", Limits::default(), false, "");
        assert_eq!(of_rule(&long, "R2").len(), 1, "a sentence over the limit still fails without its label");
    }

    /// S03-AC05: one effective policy snapshot decides a whole run, and each
    /// entry is decided by the Workspace it names. Without a policy the checker
    /// keeps its pre-S03 severity, which is why the first run has no advisory
    /// line.
    #[test]
    fn a_policy_decides_each_entry_by_its_own_workspace() {
        let sentence = "甲".repeat(41);
        let cards = json!([
            { "address": "11/1", "workspace": "human", "title": "短标题", "body": sentence },
            { "address": "12/1", "workspace": "agent", "title": "短标题", "body": sentence },
        ]);
        let cards = cards.as_array().unwrap().clone();
        let check = Check::new(Limits::default(), false, "");
        assert_eq!(
            check.run(&cards),
            [
                format!("11/1 R2 (41 characters > 40) {sentence}"),
                format!("12/1 R2 (41 characters > 40) {sentence}"),
            ]
        );
        // The design defaults: the person's Card gets advice, the agent's blocks.
        let lines = check
            .with_policies(crate::policy::PolicySet::new())
            .run(&cards);
        assert_eq!(
            lines,
            [
                format!("11/1 R2 (advisory) (41 characters > 40) {sentence}"),
                format!("12/1 R2 (41 characters > 40) {sentence}"),
            ]
        );
        // Silencing the agent rule leaves the human line untouched: one snapshot,
        // two Workspaces, no cross-talk.
        let mut policies = crate::policy::PolicySet::new();
        policies
            .change(
                crate::policy::Workspace::Agent,
                "R2",
                crate::policy::Change::Set(crate::policy::Setting::Off),
            )
            .unwrap();
        let lines = Check::new(Limits::default(), false, "")
            .with_policies(policies)
            .run(&cards);
        assert_eq!(
            lines,
            [format!("11/1 R2 (advisory) (41 characters > 40) {sentence}")]
        );
    }

    /// S03-AC03 (R0123): the policy chooses severities, not applicability. An
    /// agent policy makes R13 blocking, but a concept model keeps the exemption
    /// the rule itself grants.
    #[test]
    fn a_policy_does_not_bypass_the_concept_model_exemption() {
        let body = (1..=13)
            .map(|n| format!("第{n}句说明一个条件。"))
            .collect::<Vec<_>>()
            .join("");
        let entry = json!({
            "address": "9/2", "workspace": "agent", "title": "设定",
            "body": body, "category": "part", "facet": "设定", "level": "concept",
        });
        let check = Check::new(Limits::default(), false, "")
            .with_policies(crate::policy::PolicySet::new());
        let lines = check.run(std::slice::from_ref(&entry));
        assert!(
            of_rule(&lines, "R13").is_empty(),
            "a concept model has no part-size rule: {lines:#?}"
        );
        let mut formal = entry;
        formal["level"] = json!("formal");
        let lines = check.run(&[formal]);
        assert_eq!(
            of_rule(&lines, "R13").len(),
            1,
            "the same part at the formal level meets the blocking rule: {lines:#?}"
        );
    }

    // --- the Term content rules (S09, #152) ---------------------------------------------------

    fn term_rows() -> Vec<String> {
        vec!["均衡".to_string(), "弹性".to_string()]
    }

    fn term_row() -> crate::text::TermRow {
        crate::text::TermRow {
            term: "均衡".to_string(),
            avoid: vec!["平衡".to_string()],
            meaning: "双方都不改变策略".to_string(),
            defined: "§1.1".to_string(),
        }
    }

    fn term_draft() -> Value {
        json!({
            "term": "均衡",
            "original": "equilibrium",
            "category": "math",
            "first_defined": "§1.1",
            "explanation": [
                "均衡是双方都不改变选择的状态。",
                "本书用它说明市场的稳定点。",
                "均衡不等于最优。"
            ],
            "book_definition": { "text": ["§1.1 把均衡定义为一个不动点。"], "section": "§1.1", "quote": "" },
            "hypotheses": [ { "id": "H1", "text": "策略集是紧凸集。" } ],
            "definition": {
                "formula": "x^* \\in \\arg\\max_{x} u(x) \\tag{T1}",
                "reading": ["$x^*$ 是没有人愿意偏离的策略。"]
            },
            "symbols": [
                { "symbol": "$x^*$", "meaning": "均衡策略", "type": "紧凸集", "auxiliary": false }
            ],
            "equivalent_forms": [ { "formula": "u(x^*)\\ge u(x)", "reason": "由 (T1) 直接得到。" } ],
            "derivations": [ {
                "claim": "均衡是纳什均衡",
                "steps": [
                    { "text": "起点是定义 (T1)。", "basis": "定义 (T1)", "formula": "x^*=x" },
                    { "text": "两边比较收益。", "basis": "H1", "formula": "u(x^*)\\ge u(x)" }
                ],
                "result": "均衡是纳什均衡。"
            } ],
            "limits": [ { "case": "无", "text": "本定义没有需要说明的极限情形。", "formula": "" } ],
            "example": ["取 $u(x)=-x^2$。", "唯一均衡是 $x^*=0$。"],
            "intuition": ["没有人愿意单方面偏离。", "均衡是相互一致的选择。"],
            "distinctions": [ { "term": "弹性", "text": "弹性度量反应程度。" } ],
            "misreadings": ["均衡不是最优。"],
            "role_in_book": ["本书第一章用均衡定义市场结果。"]
        })
    }

    fn render_math() -> crate::term::Rendered {
        let rows = term_rows();
        let draft = crate::term::parse_draft(&term_draft(), &rows).expect("a valid draft");
        crate::term::render(
            &draft,
            &term_row(),
            "Test2026",
            "Test2026/1|a|a",
            &Limits::default(),
        )
        .expect("renders")
    }

    fn term_entry(title: &str, body: &str) -> Value {
        json!({
            "address": "Test2026/1|a|a",
            "type": "term",
            "term": "均衡",
            "term_role": "card",
            "title": title,
            "body": body,
            "term_rows": term_rows(),
        })
    }

    fn term_lines(cards: &[Value]) -> Vec<String> {
        Check::new(Limits::default(), false, "")
            .run(cards)
            .into_iter()
            .filter(|line| {
                [" T1 ", " T2 ", " T3 ", " T4 ", " T5 ", " T6 ", " T7 ", " T9 ", " T10 "]
                    .iter()
                    .any(|rule| line.contains(rule))
            })
            .collect()
    }

    #[test]
    fn a_rendered_math_head_passes_the_term_rules_and_a_manual_edit_gets_the_same_fault() {
        let rendered = render_math();
        let entry = term_entry(&rendered.head.title, &rendered.head.body);
        let lines = term_lines(std::slice::from_ref(&entry));
        assert!(
            lines.iter().all(|line| line.contains("T4")),
            "a rendered head passes T1/T2/T3/T5/T6/T7/T9/T10: {lines:#?}"
        );
        assert!(
            lines.iter().all(|line| line.contains("(advisory)")),
            "the only remaining line is the approximate T4 advice: {lines:#?}"
        );
        // A manual edit that drops a step's basis is the same evaluator: the same fault, whether
        // the entry is a rendered draft or a stored Card.
        let body = rendered.head.body.replace("依据：定义 (T1)", "依据：");
        let manual = term_entry(&rendered.head.title, &body);
        let draft_lines = term_lines(std::slice::from_ref(&manual));
        assert_eq!(
            of_rule(&draft_lines, "T5").len(),
            1,
            "{draft_lines:#?}"
        );
        assert!(
            draft_lines.iter().any(|line| line.contains("step 1 names no `依据：` basis")),
            "{draft_lines:#?}"
        );
    }

    #[test]
    fn the_term_template_reports_unknown_and_misordered_content_and_a_title_mismatch() {
        let body = "术语：均衡\n不用：平衡\n类别：数学概念\n首次定义：§1.1\n\n## 释义\n\n说明。\n\n## 神秘\n\n内容。\n\n## 在本书中的作用\n\n作用。\n\n## 区别\n\n- 与 弹性：不同。\n\n来源：[[Test2026]] §1.1\n";
        let entry = term_entry("另一个词", body);
        let lines = term_lines(std::slice::from_ref(&entry));
        assert!(of_rule(&lines, "T1").iter().any(|line| line.contains("is no Term section: ## 神秘")), "{lines:#?}");
        assert!(of_rule(&lines, "T1").iter().any(|line| line.contains("out of template order")), "{lines:#?}");
        assert!(of_rule(&lines, "T2").iter().any(|line| line.contains("is not the exact term")), "{lines:#?}");
        // The `术语：` line disagreeing with the stored name is its own T2 fault.
        let body = body.replace("术语：均衡", "术语：平衡");
        let lines = term_lines(&[term_entry("均衡", &body)]);
        assert!(of_rule(&lines, "T2").iter().any(|line| line.contains("differs from the stored term")), "{lines:#?}");
    }

    #[test]
    fn a_concept_term_refuses_a_formal_section_but_allows_inline_formulas() {
        let concept = "术语：均衡\n不用：平衡\n类别：概念\n首次定义：§1.1\n\n## 释义\n\n均衡是 $x^*=x$ 的状态。\n均衡是双方都不改变选择的状态。\n均衡不等于最优。\n\n## 区别\n\n- 与 弹性：不同。\n\n## 在本书中的作用\n\n作用。\n\n来源：[[Test2026]] §1.1\n";
        let lines = term_lines(&[term_entry("均衡", concept)]);
        assert!(of_rule(&lines, "T3").is_empty(), "inline math is legitimate: {lines:#?}");
        let formal = concept.replace("## 区别", "## 推导：均衡\n\n1. 起点。依据：定义。\n\n结果：均衡。\n\n## 区别");
        let lines = term_lines(&[term_entry("均衡", &formal)]);
        assert!(
            of_rule(&lines, "T3").iter().any(|line| line.contains("carries no formal `## 推导`")),
            "{lines:#?}"
        );
    }

    #[test]
    fn a_math_head_without_hypotheses_symbols_or_derivation_reports_t3() {
        let body = "术语：均衡\n不用：平衡\n类别：数学概念\n首次定义：§1.1\n\n## 释义\n\n说明。\n均衡是稳定的状态。\n均衡不等于最优。\n\n## 数学定义\n\n$$x=1 \\tag{T1}$$\n\n读法。\n\n## 区别\n\n- 与 弹性：不同。\n\n来源：[[Test2026]] §1.1\n";
        let lines = term_lines(&[term_entry("均衡", body)]);
        let t3 = of_rule(&lines, "T3");
        assert!(t3.iter().any(|line| line.contains("labelled hypotheses")), "{lines:#?}");
        assert!(t3.iter().any(|line| line.contains("`符号表` table")), "{lines:#?}");
        assert!(t3.iter().any(|line| line.contains("`## 推导")), "{lines:#?}");
    }

    #[test]
    fn derivation_steps_report_numbering_basis_result_and_the_bound() {
        let mut steps = String::new();
        for number in [1, 3] {
            steps.push_str(&format!("{number}. 第 {number} 步做一次变换。依据：代数。\n\n"));
        }
        let body = format!("术语：均衡\n不用：平衡\n类别：数学概念\n首次定义：§1.1\n\n## 释义\n\n说明。\n均衡是稳定的状态。\n均衡不等于最优。\n\n## 推导：均衡是纳什均衡\n\n{steps}来源：[[Test2026]] §1.1\n");
        let lines = term_lines(&[term_entry("均衡", &body)]);
        assert!(of_rule(&lines, "T5").iter().any(|line| line.contains("are numbered")), "{lines:#?}");
        assert!(of_rule(&lines, "T5").iter().any(|line| line.contains("ends without a `结果：`")), "{lines:#?}");
        // More than 12 steps in one Card is the per-Card bound.
        let mut long = String::new();
        for number in 1..=13 {
            long.push_str(&format!("{number}. 第 {number} 步。依据：代数。\n\n"));
        }
        let body = format!("术语：均衡\n不用：平衡\n类别：数学概念\n首次定义：§1.1\n\n## 释义\n\n说明。\n均衡是稳定的状态。\n均衡不等于最优。\n\n## 推导：均衡是纳什均衡\n\n{long}结果：均衡。\n\n来源：[[Test2026]] §1.1\n");
        let lines = term_lines(&[term_entry("均衡", &body)]);
        assert!(
            of_rule(&lines, "T5").iter().any(|line| line.contains("13 derivation steps > 12")),
            "{lines:#?}"
        );
    }

    #[test]
    fn skipped_step_phrases_report_t6_and_a_named_step_is_allowed() {
        let allowed = "术语：均衡\n不用：平衡\n类别：概念\n首次定义：§1.1\n\n## 释义\n\n由步骤 2，同理可得结论。\n均衡是双方都不改变选择的状态。\n均衡不等于最优。\n\n## 区别\n\n- 与 弹性：不同。\n\n来源：[[Test2026]] §1.1\n";
        let lines = term_lines(&[term_entry("均衡", allowed)]);
        assert!(of_rule(&lines, "T6").is_empty(), "a named earlier step is allowed: {lines:#?}");
        let skipped = allowed.replace(
            "由步骤 2，同理可得结论。",
            "显然均衡成立。",
        );
        let lines = term_lines(&[term_entry("均衡", &skipped)]);
        assert!(
            of_rule(&lines, "T6").iter().any(|line| line.contains("`显然` without the named step")),
            "{lines:#?}"
        );
    }

    #[test]
    fn the_auto_link_opt_out_lines_are_validated_by_t1() {
        let base = "术语：均衡\n不用：平衡\n类别：概念\n首次定义：§1.1\n\n## 释义\n\n均衡是双方都不改变选择的状态。\n均衡不等于最优。\n均衡是稳定的状态。\n\n## 区别\n\n- 与 弹性：不同。\n\n来源：[[Test2026]] §1.1\n";
        let with_off = base.replace(
            "首次定义：§1.1\n",
            "首次定义：§1.1\n自动链接：否\n自动链接理由：这是过于一般的词。\n",
        );
        let lines = term_lines(&[term_entry("均衡", &with_off)]);
        assert!(of_rule(&lines, "T1").is_empty(), "a complete opt-out passes T1: {lines:#?}");
        let without_reason = base.replace("首次定义：§1.1\n", "首次定义：§1.1\n自动链接：否\n");
        let lines = term_lines(&[term_entry("均衡", &without_reason)]);
        assert!(of_rule(&lines, "T1").iter().any(|line| line.contains("needs its")), "{lines:#?}");
        let orphan = base.replace("首次定义：§1.1\n", "首次定义：§1.1\n自动链接理由：太一般。\n");
        let lines = term_lines(&[term_entry("均衡", &orphan)]);
        assert!(of_rule(&lines, "T1").iter().any(|line| line.contains("decides nothing")), "{lines:#?}");
        let enabled = base.replace("首次定义：§1.1\n", "首次定义：§1.1\n自动链接：是\n");
        let lines = term_lines(&[term_entry("均衡", &enabled)]);
        assert!(of_rule(&lines, "T1").iter().any(|line| line.contains("is no opt-out")), "{lines:#?}");
    }

    #[test]
    fn a_formula_symbol_outside_the_table_is_advice_only() {
        let rendered = render_math();
        let body = rendered.head.body.replace(
            "两边比较收益。依据：H1",
            "两边比较收益。依据：H1",
        );
        let body = body.replace("u(x^*)\\ge u(x)$$", "\\zeta(x^*)\\ge u(x)$$");
        let lines = term_lines(&[term_entry(&rendered.head.title, &body)]);
        let t4 = of_rule(&lines, "T4");
        assert!(t4.iter().any(|line| line.contains("\\zeta")), "{lines:#?}");
        // The archive policy keeps T4 advisory in both Workspaces.
        let policies = crate::policy::PolicySet::new();
        let check = Check::new(Limits::default(), false, "").with_policies(policies);
        let report = check.report(&[term_entry(&rendered.head.title, &body)]);
        assert!(
            report
                .iter()
                .filter(|diagnostic| diagnostic.rule == "T4")
                .all(|diagnostic| diagnostic.severity == Severity::Advise),
            "{report:#?}"
        );
    }

    #[test]
    fn a_term_part_uses_the_continuation_template_and_inherits_the_definition() {
        let part = "类别：数学概念\n续：[[Test2026/1|a|a]] 本卡沿用该卡的符号表。\n\n## 推导：均衡是纳什均衡\n\n1. 结果逐项比较。依据：步骤 1。\n\n   $$x=1$$\n\n结果：均衡是纳什均衡。\n\n来源：[[Test2026]] §1.1\n";
        let mut entry = term_entry("推导（续 2/2）：均衡是纳什均衡", part);
        entry["term_role"] = json!("part");
        let lines = term_lines(std::slice::from_ref(&entry));
        assert!(of_rule(&lines, "T1").is_empty(), "{lines:#?}");
        assert!(of_rule(&lines, "T2").is_empty(), "{lines:#?}");
        assert!(of_rule(&lines, "T3").is_empty(), "{lines:#?}");
        // A part without the continuation format and the head reference is refused.
        let mut wrong = entry.clone();
        wrong["title"] = json!("均衡");
        let lines = term_lines(&[wrong]);
        assert!(of_rule(&lines, "T2").iter().any(|line| line.contains("is no continuation title")), "{lines:#?}");
        let mut missing = term_entry("推导（续 2/2）：均衡是纳什均衡", &part.replace("续：[[Test2026/1|a|a]] 本卡沿用该卡的符号表。\n", ""));
        missing["term_role"] = json!("part");
        let lines = term_lines(&[missing]);
        assert!(of_rule(&lines, "T1").iter().any(|line| line.contains("missing the `续：` line")), "{lines:#?}");
    }

    #[test]
    fn an_open_formula_is_t10_for_a_term_card_and_r22_elsewhere() {
        let body = "术语：均衡\n不用：平衡\n类别：概念\n首次定义：§1.1\n\n## 释义\n\n价格是 $5 美元。\n均衡是稳定的状态。\n均衡不等于最优。\n\n## 区别\n\n- 与 弹性：不同。\n\n来源：[[Test2026]] §1.1\n";
        let lines = term_lines(&[term_entry("均衡", body)]);
        assert!(of_rule(&lines, "T10").len() == 1, "{lines:#?}");
        assert!(of_rule(&lines, "R22").is_empty(), "one predicate, one label: {lines:#?}");
        let plain = json!({ "address": "5/1", "title": "税", "body": "价格是 $5 美元。" });
        let lines = Check::new(Limits::default(), false, "").run(&[plain]);
        assert_eq!(of_rule(&lines, "R22").len(), 1, "{lines:#?}");
    }

    #[test]
    fn a_term_list_is_not_checked_against_the_content_template() {
        let list = json!({
            "address": "Test2026/1|a",
            "type": "term",
            "term": serde_json::Value::Null,
            "title": "术语表：测试",
            "body": "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |\n",
        });
        let lines = term_lines(std::slice::from_ref(&list));
        assert!(lines.is_empty(), "a Term list is audited by T8, not T1: {lines:#?}");
    }

    #[test]
    fn a_term_use_on_an_ordinary_card_reports_label_advice() {
        // T7 is a Card check for a Term use on any Card, not only a Term Card: the bounded target
        // facts travel with the entry, so an Idea or Model Card gets the same advice.
        let entry = json!({
            "address": "Test2026/2",
            "title": "均衡模型",
            "type": "model",
            "body": "说明 [[平衡@Test2026/1|a|a]] 的用法。",
            "term_targets": { "Test2026/1|a|a": "均衡" },
        });
        let report = Check::new(Limits::default(), false, "").report(std::slice::from_ref(&entry));
        let t7 = report.iter().filter(|diagnostic| diagnostic.rule == "T7").collect::<Vec<_>>();
        assert_eq!(t7.len(), 1, "{report:#?}");
        assert_eq!(t7[0].fault, "label_mismatch");
        assert_eq!(t7[0].severity, Severity::Advise);
        // The matching label raises nothing.
        let entry = json!({
            "address": "Test2026/2",
            "title": "均衡模型",
            "type": "model",
            "body": "说明 [[均衡@Test2026/1|a|a]] 的用法。",
            "term_targets": { "Test2026/1|a|a": "均衡" },
        });
        let report = Check::new(Limits::default(), false, "").report(std::slice::from_ref(&entry));
        assert!(report.iter().all(|diagnostic| diagnostic.rule != "T7"), "{report:#?}");
    }

    /// S09-AC02/R0281: the digest's source-fidelity convention is not rewritten. A writer that
    /// supplies content the book omits labels it `（补：书中略去）` or `（补：书中未写）`; the
    /// renderer keeps the label verbatim and the checker reports no Term fault for it.
    #[test]
    fn source_fidelity_supplement_labels_survive_the_renderer_and_the_checker() {
        let mut value = term_draft();
        value["example"] = json!(["书中略去这一步的代数。（补：书中略去）"]);
        value["derivations"][0]["steps"][0]["text"] = json!("由书中补写这一步。（补：书中未写）");
        let draft = crate::term::parse_draft(&value, &term_rows()).expect("a valid draft");
        let rendered = crate::term::render(
            &draft,
            &term_row(),
            "Test2026",
            "Test2026/1|a|a",
            &Limits::default(),
        )
        .expect("renders");
        assert!(rendered.head.body.contains("（补：书中略去）"), "{}", rendered.head.body);
        assert!(rendered.head.body.contains("（补：书中未写）"), "{}", rendered.head.body);
        let lines = term_lines(&[term_entry(&rendered.head.title, &rendered.head.body)]);
        assert!(
            lines.iter().all(|line| line.contains("T4")),
            "a source-fidelity label raises no Term fault: {lines:#?}"
        );
    }

    /// S29 (#171, R0561): a Term use is authored markup, not prose. One body with and without its
    /// Term macros gives the same diagnostics, so linking a Card adds no prose fault.
    #[test]
    fn term_markup_adds_no_prose_fault() {
        let plain = "这一段说明背景并给出充分理由。均衡是稳定的状态。这一段说明结果如何出现。它让读者看到条件。生产函数决定产出。甲；乙。";
        let linked = "这一段说明背景并给出充分理由。[[均衡@K/10]]是稳定的状态。这一段说明结果如何出现。它让读者看到条件。[[生产函数@K/11]]决定产出。甲；乙。";
        assert_ne!(plain, linked);
        let report = |body: &str| {
            let cards = json!([{
                "address": "K/2",
                "title": "均衡是稳定的状态",
                "body": body,
                "type": "idea",
                "category": "idea",
            }]);
            Check::new(Limits::default(), false, "").report(cards.as_array().expect("cards"))
        };
        let before = report(plain);
        let after = report(linked);
        assert_eq!(
            before, after,
            "Term markup changed the diagnostics: {before:#?} vs {after:#?}"
        );
        assert!(
            before.iter().any(|diagnostic| diagnostic.rule == "R3"),
            "the fixture still exercises a real prose rule: {before:#?}"
        );
    }
    #[test]
    fn term_prose_over_a_set_limit_reports_t9() {
        let long = "甲".repeat(1700);
        let body = format!("术语：均衡\n不用：平衡\n类别：概念\n首次定义：§1.1\n\n## 释义\n\n{long}。\n\n## 区别\n\n- 与 弹性：不同。\n\n来源：[[Test2026]] §1.1\n");
        let limits = Limits { max_term_prose: 1600, ..Limits::default() };
        let lines: Vec<String> = Check::new(limits, false, "").run(&[term_entry("均衡", &body)]);
        assert!(of_rule(&lines, "T9").iter().any(|line| line.contains("prose units > 1600")), "{lines:#?}");
    }

    #[test]
    fn a_long_term_has_no_prose_limit_by_default() {
        // 2026-10-26, the user's request: some books have very long terms, so a Term Card's length is not
        // restricted; `max_term_prose` 0 (the default) means no bound.
        let long = "甲".repeat(9000);
        let body = format!("术语：均衡\n不用：平衡\n类别：概念\n首次定义：§1.1\n\n## 释义\n\n{long}。\n\n## 区别\n\n- 与 弹性：不同。\n\n来源：[[Test2026]] §1.1\n");
        let lines = term_lines(&[term_entry("均衡", &body)]);
        assert!(of_rule(&lines, "T9").is_empty(), "{lines:#?}");
    }

    // --- the Literature-note category (issue #184) ---------------------------------------------

    /// One card of a Literature tree (`K`) and the same authored text in a Topic tree (`40`), so a
    /// test can read a rule's level for a Literature note beside the unchanged Idea level.
    fn literature_pair(title: &str, body: &str) -> (Value, Value) {
        let note = json!({
            "address": "K/1|a", "category": "idea", "tree": "K", "book": "K", "workspace": "agent",
            "title": title, "body": body, "links": [], "kinds": [], "parent": "K/1"
        });
        let idea = json!({
            "address": "40/1|a", "category": "idea", "tree": "40", "workspace": "agent",
            "title": title, "body": body, "links": [], "kinds": [], "parent": "40/1"
        });
        (note, idea)
    }

    /// The rules of one report, in order, as `(address, rule, advisory)`.
    fn levels(lines: &[String]) -> Vec<(String, String, bool)> {
        lines
            .iter()
            .filter_map(|line| {
                let mut parts = line.splitn(3, ' ');
                let address = parts.next()?;
                let rule = parts.next()?;
                if !rule.chars().next().is_some_and(|first| "RIELFT".contains(first)) {
                    return None;
                }
                Some((
                    address.to_string(),
                    rule.to_string(),
                    line.contains(" (advisory) "),
                ))
            })
            .collect()
    }

    fn level_of(lines: &[String], address: &str, rule: &str) -> Option<bool> {
        levels(lines)
            .into_iter()
            .find(|(at, id, _)| at == address && id == rule)
            .map(|(_, _, advisory)| advisory)
    }

    /// The classification itself: a Regular Card of type `idea` in a Literature tree, including an
    /// anchor, and no other Card. A Topic Card, a Finding, a Model part and a Term Card keep their
    /// categories.
    #[test]
    fn only_an_idea_in_a_literature_tree_is_a_literature_note() {
        let note = |extra: Value| {
            let mut entry = json!({ "category": "idea" });
            entry
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            is_literature_note(&entry)
        };
        assert!(note(json!({ "tree": "K" })), "a Citation-key tree is a book");
        assert!(note(json!({ "tree": "K", "category": "anchor" })), "an anchor is a note");
        assert!(
            note(json!({ "book": "K" })),
            "a draft that names its book and no tree is a note"
        );
        assert!(
            note(json!({ "tree": "K/3" })),
            "a Card read by its anchor address is a note"
        );
        assert!(!note(json!({ "tree": "40" })), "a Topic tree is no book");
        assert!(!note(json!({})), "no tree and no book names no book");
        assert!(!note(json!({ "tree": "40", "book": "K" })), "the tree decides when both are named");
        assert!(!is_literature_note(&json!({ "category": "finding", "tree": "K" })));
        assert!(!is_literature_note(&json!({ "category": "part", "facet": "设定", "tree": "K" })));
        assert!(!is_literature_note(&json!({ "category": "term", "tree": "K" })));
        assert!(!is_literature_note(&json!({ "category": "root", "tree": "K" })));
        assert!(!is_literature_note(&json!({ "tree": "K" })), "no category, no layer rule");
    }

    /// I1: a note has no claim-title rule, while the same title in a Topic tree keeps it.
    #[test]
    fn a_literature_note_has_no_claim_title() {
        let (note, idea) = literature_pair("税", "税是一种强制征收，由国家对个人征收。");
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        assert_eq!(
            of_rule(&lines, "I1"),
            ["40/1|a I1 (advisory) the title states no claim: 税"],
            "{lines:#?}"
        );
        assert_eq!(level_of(&lines, "K/1|a", "I1"), None);
    }

    /// R12: a note's title advises at 60 characters; the Idea title still blocks at 40.
    #[test]
    fn a_literature_note_title_advises_at_sixty_characters() {
        let title = "甲".repeat(45);
        let (note, idea) = literature_pair(&title, "所有者决定空白处，所以控制权归所有者。");
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        assert_eq!(
            of_rule(&lines, "R12"),
            [format!("40/1|a R12 (45 characters > 40) {title}")],
            "a 45-character title passes a note and blocks an Idea Card: {lines:#?}"
        );
        let title = "甲".repeat(61);
        let (note, idea) = literature_pair(&title, "所有者决定空白处，所以控制权归所有者。");
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        let r12 = of_rule(&lines, "R12");
        assert_eq!(r12.len(), 2, "{lines:#?}");
        assert!(r12[0].starts_with("K/1|a R12 (advisory) (61 characters > 60) "), "{lines:#?}");
        assert!(r12[1].starts_with("40/1|a R12 (61 characters > 40) "), "{lines:#?}");
        assert_eq!(level_of(&lines, "K/1|a", "R12"), Some(true));
    }

    /// I2: a note advises only below 80 characters and has no upper bound; an Idea Card keeps the
    /// merge candidate at 40 and the split candidate above 300.
    #[test]
    fn a_literature_note_advises_only_a_thin_body() {
        // About fifty characters: thin for a note, a normal size for an Idea Card.
        let middle =
            "税是一种强制征收，由国家对个人征收，并且用于公共支出、社会保险与基础建设；它不直接换取具体的服务，因此纳税与受益并不一一对应。"
                .to_string();
        let measured = measure(&middle).size;
        assert!((41..80).contains(&measured), "{measured}");
        let (note, idea) = literature_pair("强制征收的来源", &middle);
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        assert_eq!(
            of_rule(&lines, "I2"),
            [format!("K/1|a I2 (advisory) ({measured} characters < 80) merge candidate")],
            "{lines:#?}"
        );
        // Four hundred characters: no bound on a note, a split candidate for an Idea Card.
        let long = "税是一种强制征收，由国家对个人征收，并且用于公共支出。".repeat(16);
        let (note, idea) = literature_pair("强制征收的来源与用途", &long);
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        let i2 = of_rule(&lines, "I2");
        assert_eq!(i2.len(), 1, "only the Idea Card has a split candidate: {lines:#?}");
        assert!(i2[0].starts_with("40/1|a I2 ("), "{lines:#?}");
        assert!(i2[0].contains("characters > 300) split candidate"), "{lines:#?}");
    }

    /// R2, R3, R4, R5 and R7: the sentence rules of the Idea standard do not reach a note, and the
    /// same body in a Topic tree still fails all five.
    #[test]
    fn the_idea_sentence_rules_do_not_reach_a_literature_note() {
        let body = format!(
            "{}。这一条使用了分号；它在同一句里。这一句被改写成了被动语态。作者对这一组数据进行了处理。这一种的甲的那个的形式已经出现。",
            "甲".repeat(41)
        );
        let (note, idea) = literature_pair("制度与投资的关系", &body);
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        for rule in ["R2", "R3", "R4", "R5", "R7"] {
            assert_eq!(level_of(&lines, "K/1|a", rule), None, "the note has no {rule}: {lines:#?}");
            assert_eq!(
                level_of(&lines, "40/1|a", rule),
                Some(false),
                "the Idea Card still fails {rule}: {lines:#?}"
            );
        }
    }

    /// R9: a hedge is advice on a note, a blocking fault on an Idea Card.
    #[test]
    fn a_hedge_in_a_literature_note_is_advice() {
        let body = "值得注意的是，这条结论在样本之外也成立，因为机制并不依赖样本。";
        let (note, idea) = literature_pair("结论在样本之外成立", body);
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        assert_eq!(level_of(&lines, "K/1|a", "R9"), Some(true), "{lines:#?}");
        assert_eq!(level_of(&lines, "40/1|a", "R9"), Some(false), "{lines:#?}");
    }

    /// I6: a note is not checked for a near-duplicate title, while the Topic Idea Card beside it
    /// still reports the pair against the same Workspace title context.
    #[test]
    fn a_literature_note_has_no_duplicate_title_row() {
        let title = "剩余控制权决定契约空白处";
        let body = "所有者能拒绝合作，所以契约的空白处由所有者决定。";
        let (note, idea) = literature_pair(title, body);
        let context = title_context(
            &json!([
                { "address": "41/1|b", "title": title, "workspace": "agent", "tree": "41" }
            ]),
            true,
        );
        let note_lines = run_with_context(&json!([note]), context.clone(), Limits::default());
        assert!(of_rule(&note_lines, "I6").is_empty(), "the note is not compared: {note_lines:#?}");
        let idea_lines = run_with_context(&json!([idea]), context, Limits::default());
        assert_eq!(
            of_rule(&idea_lines, "I6"),
            ["40/1|a I6 (advisory) a title very similar to [[41/1|b]]"],
            "the Idea Card keeps the rule: {idea_lines:#?}"
        );
    }

    /// I7: a note resting on another book's Literature tree is not an own-source dependency; the
    /// Topic Idea Card with the same evidence still reports it.
    #[test]
    fn a_literature_note_has_no_own_source_row() {
        let (mut note, mut idea) = literature_pair(
            "制度决定长期增长",
            "制度把剩余控制权分配给所有者，所以投资激励与所有权配置一致。",
        );
        for card in [&mut note, &mut idea] {
            card["relations"] = json!({ "Other/2": "rests-on" });
            card["sources"] = json!(["Other"]);
        }
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        assert_eq!(
            of_rule(&lines, "I7"),
            ["40/1|a I7 `rests-on` [[Other/2]] enters its own source `Other`"],
            "{lines:#?}"
        );
        assert_eq!(level_of(&lines, "K/1|a", "I7"), None);
    }

    /// I4: a note's missing kinded Link is advice; the Idea Card's is a blocking fault.
    #[test]
    fn a_literature_note_needs_no_kinded_link() {
        let (note, idea) = literature_pair(
            "制度把控制权分配给所有者",
            "所有权把控制权交给承担成本的一方，所以投资激励与所有权配置一致。",
        );
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        assert_eq!(level_of(&lines, "K/1|a", "I4"), Some(true), "{lines:#?}");
        assert_eq!(level_of(&lines, "40/1|a", "I4"), Some(false), "{lines:#?}");
    }

    /// L1: an unkinded Link is advice on a note and a blocking fault on an Idea Card.
    #[test]
    fn a_literature_note_link_needs_no_relation_kind() {
        let body = "这里是另一个机制 [[41/2]]，它在别处出现并且解释得很清楚。";
        let (mut note, mut idea) = literature_pair("两种机制各自解释一部分结果", body);
        for card in [&mut note, &mut idea] {
            card["links"] = json!(["41/2"]);
        }
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        assert_eq!(
            of_rule(&lines, "L1"),
            [
                "K/1|a L1 (advisory) (1 Links read as refers) [[41/2]]",
                "40/1|a L1 (1 Links read as refers) [[41/2]]",
            ],
            "{lines:#?}"
        );
    }

    /// L2: a short Link reason is advice on a note and a blocking fault on an Idea Card.
    #[test]
    fn a_literature_note_link_reason_is_advice() {
        let body = "依赖 [[41/2]] 的结论。";
        let (mut note, mut idea) = literature_pair("两种机制各自解释一部分结果", body);
        for card in [&mut note, &mut idea] {
            card["links"] = json!(["41/2"]);
            card["kinds"] = json!(["41/2"]);
        }
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        assert_eq!(
            of_rule(&lines, "L2"),
            [
                "K/1|a L2 (advisory) (5 characters < 8) a Link with no reason: 依赖 [[41/2]] 的结论。",
                "40/1|a L2 (5 characters < 8) a Link with no reason: 依赖 [[41/2]] 的结论。",
            ],
            "{lines:#?}"
        );
    }

    /// L3, R20 and R21 stay advice on a note, as they are on an Idea Card.
    #[test]
    fn the_advisory_rows_stay_advice_on_a_literature_note() {
        let body = "这是第一个机制，[[41/2]] 在这里出现并且解释得很清楚。\n\n这是第二个机制，[[41/2]] 也在别处出现并且同样清楚。";
        let (mut note, mut idea) = literature_pair("两种机制各自解释一部分结果", body);
        for card in [&mut note, &mut idea] {
            card["links"] = json!(["41/2"]);
            card["kinds"] = json!(["41/2"]);
        }
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        assert_eq!(level_of(&lines, "K/1|a", "L3"), Some(true), "{lines:#?}");
        assert_eq!(level_of(&lines, "40/1|a", "L3"), Some(true), "{lines:#?}");

        let body = "该卡说明了结果。";
        let (note, idea) = literature_pair("这条结论说明了结果", body);
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        assert_eq!(level_of(&lines, "K/1|a", "R21"), Some(true), "{lines:#?}");
        assert_eq!(level_of(&lines, "40/1|a", "R21"), Some(true), "{lines:#?}");

        let body = "式 (12) 有该项。";
        let (note, idea) = literature_pair("这条结论说明了结果", body);
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        assert_eq!(level_of(&lines, "K/1|a", "R20"), Some(true), "{lines:#?}");
        assert_eq!(level_of(&lines, "40/1|a", "R20"), Some(true), "{lines:#?}");
    }

    /// I3: the source line still blocks on a note, as it does on an Idea Card.
    #[test]
    fn a_literature_note_still_needs_its_source_line() {
        let (note, idea) = literature_pair(
            "制度把控制权分配给所有者",
            "所有权把控制权交给承担成本的一方，所以投资激励与所有权配置一致。",
        );
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        assert_eq!(
            of_rule(&lines, "I3"),
            ["K/1|a I3 no source line: no Link to [[K]] or one of its anchors"],
            "{lines:#?}"
        );
        assert_eq!(level_of(&lines, "K/1|a", "I3"), Some(false), "{lines:#?}");
    }

    /// R22: a note with an unclosed formula still blocks, as an Idea Card does.
    #[test]
    fn a_literature_note_still_fails_an_open_formula() {
        let body = "成立条件是 $A_I<A_O，所以均衡稳定。\n\n来源：[[K]] §4";
        let (mut note, mut idea) = literature_pair("均衡稳定当且仅当门槛比较", body);
        for card in [&mut note, &mut idea] {
            card["links"] = json!(["K"]);
        }
        let lines = run_json(&json!([note, idea]), "", Limits::default(), false, "");
        assert_eq!(level_of(&lines, "K/1|a", "R22"), Some(false), "{lines:#?}");
        assert_eq!(level_of(&lines, "40/1|a", "R22"), Some(false), "{lines:#?}");
    }

    /// The acceptance case: a 1,500-character connected note — 60-character sentences, semicolons
    /// and passives — passes with no blocking fault, while the same body as a Topic Idea Card
    /// still blocks. The rows' advice is not promoted by the agent Workspace policy.
    #[test]
    fn a_long_connected_literature_note_passes_with_no_blocking_fault() {
        let subjects = [
            "制度", "产权", "契约", "投资", "谈判", "激励", "剩余", "配置", "组织", "边界", "企业",
            "市场", "价格", "成本", "收益", "风险", "信息", "监督", "协调", "分工", "规模", "技术",
            "资本", "劳动", "产出", "结构", "监管",
        ];
        let mut body = String::new();
        for (index, subject) in subjects.iter().enumerate() {
            // Some sentences join two clauses with a semicolon, and some state the passive.
            let join = if index % 2 == 0 { "；" } else { "，" };
            let passive = if index % 3 == 0 { "被" } else { "" };
            body.push_str(&format!(
                "{subject}方面的证据表明，所有权把控制权交给承担成本的一方{join}因为这一方事后{passive}要求补偿，所以投资激励与所有权配置一致。"
            ));
        }
        body.push_str("这一段的机制承接 [[K/9]] 的推导，说明成本由谁承担。\n\n来源：[[K]] §2");
        let note = json!({
            "address": "K/1|a", "category": "idea", "tree": "K", "book": "K", "workspace": "agent",
            "title": "制度把剩余控制权分配给所有者并决定长期投资",
            "body": body, "links": ["K", "K/9"], "kinds": ["K/9"], "parent": "K/1"
        });
        let report = Check::new(Limits::default(), false, "")
            .with_policies(crate::policy::PolicySet::new())
            .report(std::slice::from_ref(&note));
        let lines = report.iter().map(Diagnostic::line).collect::<Vec<_>>();
        assert_eq!(CheckReport::of(report).blockers, 0, "{lines:#?}");
        assert!(lines.iter().all(|line| line.contains(" (advisory) ")), "{lines:#?}");
        // The prose is not a short claim: it is about 1,500 measured characters.
        let prose = body.split("\n\n").next().unwrap();
        let measured = measure(prose).size;
        assert!((1400..1600).contains(&measured), "the note's prose is {measured} characters");
        // One of its sentences is over the Idea-card limit, so the same body still blocks there.
        let sentence = "制度方面的证据表明，所有权把控制权交给承担成本的一方；因为这一方事后要求补偿，所以投资激励与所有权配置一致。";
        assert!(measure(sentence).size > 40 && measure(sentence).size <= 60, "{}", measure(sentence).size);
        let mut topic = note.clone();
        topic["address"] = json!("45/1|a");
        topic["tree"] = json!("45");
        topic["parent"] = json!("45/1");
        let blockers = CheckReport::of(Check::new(Limits::default(), false, "").report(&[topic])).blockers;
        assert!(blockers > 20, "the Topic Idea Card keeps the Idea rules: {blockers}");
    }
}
