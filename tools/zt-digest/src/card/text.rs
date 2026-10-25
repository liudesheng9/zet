//! What a rewrite must keep, and the Link kinds as ZT reads them (card-compiler.ps1:
//! Get-Keep, Test-Kinded, Get-LinkKinds).

use crate::ps::{lower_invariant, select_unique};
use regex::Regex;
use std::sync::LazyLock;

/// The words before a Link that ZT reads as a Relation kind (src/model.rs RELATION_KINDS).
pub const CUES: &[&str] = &[
    "依赖", "依据", "基于", "建立在", "取自", "沿用", "承接", "用到", "借用", "代入", "推导自", "由此推出", "按定义", "例示", "举例", "例子", "示例",
    "演示", "例证", "实例", "案例", "印证", "对照", "对比", "相比", "不同于", "区别", "相反", "逆命题", "放宽", "放松", "推广", "扩展", "拓展",
    "一般化", "对应", "类比", "同构", "相当于", "等价", "参照", "反驳", "批评", "质疑", "反例", "否定", "挑战", "特例", "特殊情形",
];
/// The English cues of the same table. ZT matches them in lower case, never inside a word.
pub const CUES_EN: &[&str] = &[
    "substitut", "rests on", "builds on", "depends on", "relies on", "derived from", "follows from", "uses", "cites", "by definition",
    "by theorem", "by lemma", "by proposition", "by corollary", "by axiom", "illustrat", "example of", "instance of", "for example", "e.g.",
    "contrast", "unlike", "compared with", "compare", "versus", "differs from", "relax", "extend", "generaliz", "correspond", "maps onto",
    "analog", "equivalent", "mirrors", "objection", "objects to", "critici", "refut", "counterexample", "challeng", "special case", "specializ",
];

pub static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]]+)\]\]").unwrap());
static FIGURE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!\[[^\]]*\]\(artifact:[0-9a-fA-F]+\)").unwrap());
static DISPLAY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$\$[\s\S]*?\$\$").unwrap());
static INLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$[^$\n]+\$").unwrap());
static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s").unwrap());

/// The Link targets of a text, trimmed, in order (with repeats).
pub fn link_targets(text: &str) -> Vec<String> {
    LINK.captures_iter(text).map(|c| c[1].trim().to_string()).collect()
}

/// `Get-Keep`: the Links, figure lines, and formulas (white space removed) of a text.
pub struct Keep {
    pub links: Vec<String>,
    pub figures: Vec<String>,
    pub formulas: Vec<String>,
}

pub fn keep(text: &str) -> Keep {
    let display: Vec<String> = DISPLAY.find_iter(text).map(|m| SPACE.replace_all(m.as_str(), "").into_owned()).collect();
    let without = DISPLAY.replace_all(text, " ");
    let inline: Vec<String> = INLINE.find_iter(&without).map(|m| SPACE.replace_all(m.as_str(), "").into_owned()).collect();
    Keep {
        links: select_unique(link_targets(text)),
        figures: select_unique(FIGURE.find_iter(text).map(|m| m.as_str().to_string())),
        formulas: select_unique(display.into_iter().chain(inline)),
    }
}

/// `Test-Kinded`: a kind word before the Link to `target`, in its sentence.
pub fn kinded(text: &str, target: &str) -> bool {
    let Some(at) = text.find(&format!("[[{target}")) else { return false };
    let chars: Vec<char> = text[..at].chars().collect();
    let at_char = chars.len();
    // LastIndexOfAny('。！？!?', max(0, at - 1)): searches back from that index, inclusive.
    let from = at_char.saturating_sub(1);
    let mut start = 0;
    if !text.is_empty() {
        let all: Vec<char> = text.chars().collect();
        for i in (0..=from.min(all.len().saturating_sub(1))).rev() {
            if "。！？!?".contains(all[i]) {
                start = i + 1;
                break;
            }
        }
    }
    let before: String = chars.get(start.min(at_char)..at_char).map(|c| c.iter().collect()).unwrap_or_default();
    CUES.iter().any(|cue| before.contains(cue))
}

/// `Get-LinkKinds`: the Link targets of a body that ZT reads a Relation kind for (src/model.rs
/// read_links): the cue that ends last in the words before the Link, in its sentence and after an
/// earlier Link of that sentence, decides the kind. A Link with no cue there takes the kind of
/// that earlier Link. 引出 is a plain reference.
pub fn link_kinds(body: &str) -> Vec<String> {
    let chars: Vec<char> = body.chars().collect();
    let lower: Vec<char> = lower_invariant(body).chars().collect();
    // byte offset -> char index
    let char_index = |byte: usize| body[..byte].chars().count();
    let mut kinds: Vec<String> = Vec::new();
    let mut prev_end: isize = -1;
    let mut prev_kinded = false;
    let cues: Vec<&str> = CUES.iter().chain(CUES_EN).copied().chain(["引出"]).collect();
    for m in LINK.captures_iter(body) {
        let whole = m.get(0).unwrap();
        let index = char_index(whole.start());
        let length = whole.as_str().chars().count();
        let mut start = 0usize;
        let mut i = index as isize - 1;
        while i >= 0 {
            let ch = chars[i as usize];
            let next_white = (i as usize + 1) < chars.len() && chars[i as usize + 1].is_whitespace();
            if "。！？!?；;".contains(ch) || ch == '\n' || (ch == '.' && next_white) {
                start = i as usize + 1;
                break;
            }
            i -= 1;
        }
        let shared = prev_end > start as isize;
        let from = if shared { prev_end as usize } else { start };
        let words: String = lower[from..index].iter().collect();
        let words_chars: Vec<char> = words.chars().collect();
        // The cue that ends last; of two that end at one place, the longer.
        let mut best: Option<(usize, &str)> = None;
        for cue in &cues {
            let word = cue.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
            let cue_chars: Vec<char> = cue.chars().collect();
            if cue_chars.len() > words_chars.len() {
                continue;
            }
            for at in 0..=words_chars.len() - cue_chars.len() {
                if words_chars[at..at + cue_chars.len()] != cue_chars[..] {
                    continue;
                }
                if word && at > 0 && words_chars[at - 1].is_ascii_alphanumeric() {
                    continue;
                }
                let end = at + cue_chars.len();
                let better = match best {
                    None => true,
                    Some((best_end, best_cue)) => end > best_end || (end == best_end && cue_chars.len() > best_cue.chars().count()),
                };
                if better {
                    best = Some((end, cue));
                }
            }
        }
        let is_kinded = match best {
            Some((_, cue)) => cue != "引出",
            None => shared && prev_kinded,
        };
        let target = m[1].trim().to_string();
        if is_kinded && !kinds.contains(&target) {
            kinds.push(target);
        }
        prev_end = (index + length) as isize;
        prev_kinded = is_kinded;
    }
    kinds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_finds_links_figures_formulas() {
        let k = keep("见 [[3/1]] 与 [[ 3/2 ]] 和 [[3/1]]。![图](artifact:ab12) $$ a + b $$ 以及 $x = 1$");
        assert_eq!(k.links, vec!["3/1", "3/2"]);
        assert_eq!(k.figures, vec!["![图](artifact:ab12)"]);
        assert_eq!(k.formulas, vec!["$$a+b$$", "$x=1$"]);
    }

    /// S27 (#166, AC02): a Term macro is kept as the raw authored macro. `keep` compares what a
    /// rewrite must preserve, so a relabelled or dropped Term use is a kept-Link difference.
    /// S26 (#169, design 14.5, US087): `Compiler::lost_links` then excuses a dropped Term macro
    /// whose visible label still stands, because the finalization's Term-link round restores it.
    #[test]
    fn a_term_macro_is_kept_as_the_raw_authored_macro() {
        let old = "均衡 [[均衡@Key/3|a]] 是本书的核心。";
        let kept = keep(old);
        assert_eq!(kept.links, vec!["均衡@Key/3|a"]);
        // The visible term written plainly is no Link at all: it changes nothing a rewrite keeps.
        assert!(keep("均衡是本书的核心。").links.is_empty());
        // Relabelling the same target, and dropping the use, each lose the kept macro.
        for rewritten in [
            "均衡 [[生产函数@Key/3|a]] 是本书的核心。",
            "均衡是本书的核心。",
        ] {
            let now = keep(rewritten);
            let lost: Vec<&String> = kept.links.iter().filter(|link| !now.links.contains(link)).collect();
            assert_eq!(lost, vec!["均衡@Key/3|a"], "{rewritten}");
        }
    }

    #[test]
    fn kind_words() {
        assert!(kinded("对照 [[0/1]]：那里讨论投资。", "0/1"));
        assert!(!kinded("[[0/1]] 讨论投资。", "0/1"));
        assert!(!kinded("依赖这个。见 [[0/1]]。", "0/1"));
        assert_eq!(link_kinds("依赖 [[a]] 与 [[b]]。见 [[c]]。引出 [[d]]，例示 [[e]]"), vec!["a", "b", "e"]);
        assert_eq!(link_kinds("This uses [[a]]; it causes [[b]]."), vec!["a"]);
        assert_eq!(link_kinds("misuses [[a]]"), Vec::<String>::new());
    }
}
