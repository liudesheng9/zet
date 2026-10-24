//! The process-free rule core: the Card language rules (R2 to R22) and the Card
//! layer rules (I, E, F, L), evaluated from the entries and the bounded context
//! a caller materialized. It reads no file, starts no process, and imports no
//! archive or platform binding, so the standalone checker, zt-digest, and the
//! zt service evaluate one definition of Card legality.

use crate::text::{
    TitleContext, TitleRow, Terms, bigrams, book_place, culture, culture_cmp,
    culture_starts_with, entry_section, from_utf16_form, is_high_stand_in, is_low_stand_in, key,
    leads_to, length, literature_tree, measure, open_math, passive, plain, prop, prose_lines,
    ps_eq, ps_in, ps_int, ps_string, read_terms, similarity, split_sentences, strings,
    structure_lines, term_table, text, truthy, utf16_form, BOLD, CJK, CODE, DOCUMENT_LINE,
    HEADING, INLINE_MATH, LINK, LINK_TARGET, PLACEHOLDERS, REASON_STRIP, SOURCE_LINE, SPACE,
    SPACES, WORD, pattern,
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
#[derive(Clone, Debug)]
pub struct Limits {
    pub max_sentences: i64,
    pub max_chars: i64,
    pub max_steps: i64,
    pub max_explain_sentences: i64,
    pub max_explain_chars: i64,
    pub min_idea_chars: i64,
    pub max_idea_chars: i64,
    pub min_reason: i64,
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
        }
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
        }
    }

    /// The Workspace title context from the mode's bulk reader, or a bundle's context.
    pub fn with_context(mut self, context: TitleContext) -> Self {
        self.context = context;
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
            prose_chars.insert(key(&address), chars);
        }
        self.layer(cards, &prose_chars, &mut faults);
        faults.diagnostics
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
        corruption(&authored_body, &title, address, faults);
        if open_math(&authored_body) {
            faults.add(
                address,
                "R22",
                "open_formula",
                &format!("body:{authored_body}"),
                "the body leaves an inline formula open (an odd number of $; write a currency as 美元 or \\$)",
            );
        }
        let facet = text(prop(card, "facet"));
        // A root, an anchor's section index, a hub's navigation list, and a register part's term
        // rows are not claims; the standalone rules read the authored prose that states one
        // (D126's Literature prose excludes a locator, and an index line is a locator).
        let navigation = ps_in(
            &text(prop(card, "category")),
            &["anchor", "root", "hub", "register-part", "restatement-entry"],
        );
        let shared = ps_in(address, &self.shared);
        let derivation = is_derivation_facet(&facet);
        if !title.is_empty() {
            let measure = measure(&title);
            let limit = if measure.words { 25 } else { 40 };
            if measure.size > limit {
                faults.add(
                    address,
                    "R12",
                    "title_size",
                    &format!("{} {} > {limit}: {title}", measure.size, measure.unit()),
                    &format!("({} {} > {limit}) {title}", measure.size, measure.unit()),
                );
            }
            if SEMICOLON.is_match(&plain(&title)) {
                faults.add(address, "R12", "two_claims", &title, &format!("two claims: {title}"));
            }
            if open_math(&title) {
                faults.add(
                    address,
                    "R22",
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
                self.sentence(faults, address, shared, &sentence, is_step);
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

    /// R2 to R9 for one sentence.
    pub(crate) fn sentence(
        &self,
        faults: &mut Faults,
        address: &str,
        shared: bool,
        sentence: &str,
        step: bool,
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
            faults.add(
                address,
                "R2",
                "sentence_length",
                &format!("{} {} > {limit}: {sentence}", measure.size, measure.unit()),
                &format!("({} {} > {limit}) {sentence}", measure.size, measure.unit()),
            );
        }
        if SEMICOLON.is_match(&plain) {
            faults.add(address, "R3", "semicolon", sentence, sentence);
        }
        // `被收购方`, `被投资方`: a party, not a passive (and a 被 that names the party has its actor).
        if passive(&plain) {
            faults.add(address, "R4", "passive", sentence, sentence);
        }
        if LIGHT_VERB.is_match(&plain) {
            faults.add(address, "R5", "light_verb", sentence, sentence);
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
            faults.add(address, "R7", "noun_phrase", sentence, sentence);
        }
        if let Some(word) = HEDGES.iter().find(|word| plain.contains(*word)) {
            faults.add(
                address,
                "R9",
                "hedge",
                &format!("{word}|{sentence}"),
                &format!("`{word}`: {sentence}"),
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
            let title_plain = SPACE
                .replace_all(&PLACEHOLDERS.replace_all(&plain(&title), ""), "")
                .into_owned();
            if is(&["idea", "anchor"])
                && length(&title_plain) <= 6
                && !CLAIM_VERB.is_match(&title_plain)
            {
                faults.add(
                    &address,
                    "I1",
                    "no_claim_title",
                    &title,
                    &format!("the title states no claim: {title}"),
                );
            }
            if is(&["idea"]) {
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
                faults.add(
                    &address,
                    "I4",
                    "no_kinded_link",
                    "",
                    "no kinded Link to a Card other than the parent",
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
                let mut refers: Vec<String> = Vec::new();
                for link in &links {
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
                    faults.add(
                        &address,
                        "L1",
                        "unkinded_links",
                        &identity.join(","),
                        &format!("({} Links read as refers) {shown}", refers.len()),
                    );
                }
            }
            // L2 and L3 read the body: the sentence of each Link, and the paragraphs it appears in.
            let mut paragraphs: Vec<(String, BTreeSet<usize>)> = Vec::new();
            let mut paragraph = 0;
            for raw in body.split('\n') {
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
            for line in prose_lines(&body) {
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
                        faults.add(
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
                        );
                    }
                }
            }
            paragraphs.sort_by(|a, b| culture_cmp(&a.0, &b.0));
            for (target, seen) in &paragraphs {
                if seen.len() >= 2 {
                    faults.add(
                        &address,
                        "L3",
                        "repeated_paragraph_link",
                        target,
                        &format!("[[{target}]] is linked in {} paragraphs", seen.len()),
                    );
                }
            }
            // I7 reads the relationship evidence, so only a Card that carries it is in scope.
            if is(&["idea"]) {
                own_source_dependency(card, &address, faults);
            }
        }
        // I6: the checked Idea titles against their Workspace. It reads the context, not
        // only this run, so a tree-only read is never reported as a Workspace-wide check.
        self.similar_titles(&layer, faults);
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
fn own_source_dependency(card: &Value, address: &str, faults: &mut Faults) {
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
        );
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
    ) {
        self.diagnostics.push(Diagnostic {
            address: from_utf16_form(address),
            rule: rule.to_string(),
            fault: fault.to_string(),
            key: from_utf16_form(key),
            message: from_utf16_form(message),
            severity,
            legacy: false,
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
    use crate::policy::{DEFAULT_RULES, filter_rules};
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
            .with_shared(shared);
        let report = check.report(cards);
        // S02-AC01: every fault carries the structured contract, its severity and
        // scope come from the rule catalog, and the familiar lines are exactly
        // the formatted report, in the same order.
        for diagnostic in &report {
            assert!(!diagnostic.address.is_empty(), "{diagnostic:?}");
            assert!(!diagnostic.rule.is_empty(), "{diagnostic:?}");
            assert!(!diagnostic.fault.is_empty(), "{diagnostic:?}");
            assert!(!diagnostic.message.is_empty(), "{diagnostic:?}");
            assert!(!diagnostic.legacy);
            assert_eq!(diagnostic.severity, crate::policy::severity(&diagnostic.rule));
            assert_eq!(diagnostic.scope, crate::policy::scope(&diagnostic.rule));
        }
        let lines = faulted(report.iter().map(Diagnostic::line).collect());
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
            check.sentence(&mut faults, "K/1", false, sentence, false);
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
}
