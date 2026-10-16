//! Parameters in the form the PowerShell scripts took them: `-Name value`,
//! `-Name:value`, and `-Switch` (or `-Switch:$true` / `-Switch:$false`).
//! Names ignore case. A value that does not follow a parameter name is
//! positional. An unknown name is an error, as in PowerShell.

use anyhow::{Context, Result, bail};

pub struct Args {
    values: Vec<(String, String)>,
    switches: Vec<(String, bool)>,
    pub positional: Vec<String>,
}

impl Args {
    /// `params` take a value; `switches` take none.
    pub fn parse(args: &[String], params: &[&str], switches: &[&str]) -> Result<Self> {
        let find = |name: &str, list: &[&str]| list.iter().find(|p| p.eq_ignore_ascii_case(name)).map(|p| p.to_string());
        let mut out = Self { values: Vec::new(), switches: Vec::new(), positional: Vec::new() };
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            let named = arg.len() > 1 && arg.starts_with('-') && arg[1..].chars().next().is_some_and(|c| c.is_ascii_alphabetic());
            if !named {
                out.positional.push(arg.clone());
                i += 1;
                continue;
            }
            let (name, inline) = match arg[1..].split_once(':') {
                Some((name, value)) => (name, Some(value.to_string())),
                None => (&arg[1..], None),
            };
            if let Some(param) = find(name, params) {
                let value = match inline {
                    Some(value) => value,
                    None => {
                        i += 1;
                        args.get(i).cloned().with_context(|| format!("-{param} needs a value"))?
                    }
                };
                out.values.retain(|(p, _)| p != &param);
                out.values.push((param, value));
            } else if let Some(switch) = find(name, switches) {
                let on = match inline.as_deref().map(str::to_ascii_lowercase).as_deref() {
                    None | Some("$true") | Some("true") | Some("1") => true,
                    Some("$false") | Some("false") | Some("0") => false,
                    Some(other) => bail!("-{switch} takes no value ({other})"),
                };
                out.switches.retain(|(s, _)| s != &switch);
                out.switches.push((switch, on));
            } else {
                bail!("unknown parameter -{name}");
            }
            i += 1;
        }
        Ok(out)
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.values.iter().find(|(p, _)| p.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// The value, or "" when it was not given.
    pub fn str(&self, name: &str) -> String {
        self.get(name).unwrap_or_default().to_string()
    }

    pub fn str_or(&self, name: &str, default: &str) -> String {
        self.get(name).unwrap_or(default).to_string()
    }

    pub fn required(&self, name: &str) -> Result<String> {
        match self.get(name) {
            Some(value) => Ok(value.to_string()),
            None => bail!("-{name} is required"),
        }
    }

    /// One of `allowed` (case ignored), returned as written in `allowed`.
    pub fn choice(&self, name: &str, allowed: &[&str]) -> Result<String> {
        let value = self.required(name)?;
        match allowed.iter().find(|a| a.eq_ignore_ascii_case(&value)) {
            Some(a) => Ok(a.to_string()),
            None => bail!("-{name} must be one of {}, not `{value}`", allowed.join(", ")),
        }
    }

    pub fn int_or(&self, name: &str, default: i64) -> Result<i64> {
        match self.get(name) {
            None => Ok(default),
            // PowerShell rounds a number with a fraction to the nearest whole number (to even).
            Some(v) => match v.trim().parse::<i64>() {
                Ok(n) => Ok(n),
                Err(_) => v.trim().parse::<f64>().map(|f| f.round_ties_even() as i64).with_context(|| format!("-{name}: `{v}` is not a number")),
            },
        }
    }

    pub fn float_or(&self, name: &str, default: f64) -> Result<f64> {
        match self.get(name) {
            None => Ok(default),
            Some(v) => v.trim().parse::<f64>().with_context(|| format!("-{name}: `{v}` is not a number")),
        }
    }

    pub fn flag(&self, name: &str) -> bool {
        self.switches.iter().any(|(s, on)| s.eq_ignore_ascii_case(name) && *on)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(list: &[&str]) -> Result<Args> {
        let args: Vec<String> = list.iter().map(|s| s.to_string()).collect();
        Args::parse(&args, &["Step", "Work", "PerJob", "Sample"], &["Apply", "Compact"])
    }

    #[test]
    fn names_ignore_case_and_take_values() {
        let a = parse(&["-step", "plan", "-Work:D:\\w", "-APPLY", "-PerJob", "3", "pos"]).unwrap();
        assert_eq!(a.str("Step"), "plan");
        assert_eq!(a.str("Work"), "D:\\w");
        assert!(a.flag("Apply"));
        assert!(!a.flag("Compact"));
        assert_eq!(a.int_or("PerJob", 8).unwrap(), 3);
        assert_eq!(a.float_or("Sample", 0.0).unwrap(), 0.0);
        assert_eq!(a.positional, vec!["pos"]);
    }

    #[test]
    fn unknown_names_and_missing_values_fail() {
        assert!(parse(&["-Nope", "1"]).is_err());
        assert!(parse(&["-Step"]).is_err());
        assert!(parse(&["-Apply:$false"]).is_ok_and(|a| !a.flag("Apply")));
        // A negative number is a value, not a name.
        assert!(parse(&["-PerJob", "-1"]).is_ok_and(|a| a.int_or("PerJob", 0).unwrap() == -1));
    }
}
