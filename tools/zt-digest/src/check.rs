//! zt-check in-process: `Invoke-ZtCheck` of zt-calls.ps1, with the same
//! parameters and defaults. It returns zt-check's lines (one per fault, then
//! `faults: N`) and fails when zt-check did not run.

use crate::zt::Zt;
use anyhow::{Result, bail};

pub struct Check {
    pub json: String,
    pub model: String,
    pub addresses: String,
    pub cards: String,
    pub log: String,
    pub book: String,
    pub rules: String,
    pub setting: String,
    pub level: String,
    pub require_explain: bool,
    pub skip_shared: bool,
    pub max_sentences: i64,
    pub max_chars: i64,
    pub max_steps: i64,
    pub max_explain_sentences: i64,
    pub max_explain_chars: i64,
    pub min_idea_chars: i64,
    pub max_idea_chars: i64,
    pub min_reason: i64,
}

impl Default for Check {
    fn default() -> Self {
        Self {
            json: String::new(),
            model: String::new(),
            addresses: String::new(),
            cards: String::new(),
            log: String::new(),
            book: String::new(),
            rules: String::new(),
            setting: String::new(),
            level: String::new(),
            require_explain: false,
            skip_shared: false,
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

impl Check {
    /// zt-check's command-line arguments, as Invoke-ZtCheck builds them.
    pub fn args(&self) -> Vec<String> {
        let mut a: Vec<String> = Vec::new();
        let mut push = |k: &str, v: &str| {
            a.push(k.to_string());
            a.push(v.to_string());
        };
        if !self.json.is_empty() {
            push("--json", &self.json);
        } else if !self.model.is_empty() {
            push("--model", &self.model);
        } else if !self.addresses.is_empty() {
            push("--addresses", &self.addresses);
        } else {
            push("--cards", &self.cards);
        }
        for (k, v) in [("--log", &self.log), ("--book", &self.book), ("--rules", &self.rules), ("--setting", &self.setting), ("--level", &self.level)] {
            if !v.is_empty() {
                push(k, v);
            }
        }
        if self.require_explain {
            a.push("--require-explain".into());
        }
        if self.skip_shared {
            a.push("--skip-shared".into());
        }
        for (k, v) in [
            ("--max-sentences", self.max_sentences),
            ("--max-chars", self.max_chars),
            ("--max-steps", self.max_steps),
            ("--max-explain-sentences", self.max_explain_sentences),
            ("--max-explain-chars", self.max_explain_chars),
            ("--min-idea-chars", self.min_idea_chars),
            ("--max-idea-chars", self.max_idea_chars),
            ("--min-reason", self.min_reason),
        ] {
            a.push(k.into());
            a.push(v.to_string());
        }
        a
    }

    /// Runs the check on `zt` (reads go to that `zt mcp`).
    pub fn run(&self, zt: &mut Zt) -> Result<Vec<String>> {
        let lines = match zt_check::check::run_lines(&self.args(), Some(zt)) {
            Ok(lines) => lines,
            Err(error) => bail!("zt-check did not run: {error:#}"),
        };
        if !lines.iter().any(|l| l.starts_with("faults: ") && l[8..].chars().all(|c| c.is_ascii_digit())) {
            bail!("zt-check did not run: {}", lines.join(" | "));
        }
        Ok(lines)
    }
}
