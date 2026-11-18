//! Minimal robots.txt support (RFC 9309 semantics, wildcard and end-anchor extensions).

#[derive(Debug, Default, Clone)]
pub struct RobotsRules {
    rules: Vec<Rule>,
}

#[derive(Debug, Clone)]
struct Rule {
    allow: bool,
    pattern: String,
}

impl RobotsRules {
    pub fn allow_all() -> Self {
        Self::default()
    }

    /// Parse a robots.txt body for the given user agent string.
    ///
    /// Groups are selected by longest matching user-agent token, falling back to `*`.
    pub fn parse(text: &str, user_agent: &str) -> Self {
        let ua = user_agent.to_ascii_lowercase();
        let mut groups: Vec<(Vec<String>, Vec<Rule>)> = Vec::new();
        let mut current_agents: Vec<String> = Vec::new();
        let mut current_rules: Vec<Rule> = Vec::new();
        let mut in_rules = false;

        let flush = |agents: &mut Vec<String>,
                     rules: &mut Vec<Rule>,
                     groups: &mut Vec<(Vec<String>, Vec<Rule>)>| {
            if !agents.is_empty() {
                groups.push((std::mem::take(agents), std::mem::take(rules)));
            } else {
                rules.clear();
            }
        };

        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                "user-agent" => {
                    if in_rules {
                        flush(&mut current_agents, &mut current_rules, &mut groups);
                        in_rules = false;
                    }
                    current_agents.push(value.to_ascii_lowercase());
                }
                "allow" | "disallow" => {
                    in_rules = true;
                    if key == "disallow" && value.is_empty() {
                        continue;
                    }
                    current_rules.push(Rule {
                        allow: key == "allow",
                        pattern: value.to_string(),
                    });
                }
                _ => {}
            }
        }
        flush(&mut current_agents, &mut current_rules, &mut groups);

        let mut best_len = 0usize;
        let mut selected: Vec<Rule> = Vec::new();
        for (agents, rules) in groups {
            let mut match_len = 0usize;
            for agent in agents {
                if agent == "*" {
                    match_len = match_len.max(1);
                } else if ua.contains(&agent) {
                    match_len = match_len.max(agent.len());
                }
            }
            if match_len > best_len {
                best_len = match_len;
                selected = rules;
            }
        }
        Self { rules: selected }
    }

    /// True when the path may be fetched.
    pub fn allows(&self, path: &str) -> bool {
        let path = if path.is_empty() { "/" } else { path };
        let mut best: Option<(usize, bool)> = None;
        for rule in &self.rules {
            if !pattern_matches(&rule.pattern, path) {
                continue;
            }
            let len = rule.pattern.trim_end_matches('$').len();
            best = match best {
                Some((best_len, best_allow)) => {
                    if len > best_len || (len == best_len && rule.allow && !best_allow) {
                        Some((len, rule.allow))
                    } else {
                        Some((best_len, best_allow))
                    }
                }
                None => Some((len, rule.allow)),
            };
        }
        best.map(|(_, allow)| allow).unwrap_or(true)
    }
}

/// Glob matcher with `*` (any sequence) and a trailing `$` (end anchor).
fn pattern_matches(pattern: &str, path: &str) -> bool {
    let (body, anchored) = match pattern.strip_suffix('$') {
        Some(body) => (body, true),
        None => (pattern, false),
    };
    let parts: Vec<&str> = body.split('*').collect();
    if parts.len() == 1 {
        return if anchored {
            path == body
        } else {
            path.starts_with(body)
        };
    }
    let mut position = 0usize;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if index == 0 {
            if !path[position..].starts_with(part) {
                return false;
            }
            position += part.len();
        } else {
            match path[position..].find(part) {
                Some(found) => position += found + part.len(),
                None => return false,
            }
        }
    }
    if anchored {
        let last = parts.last().copied().unwrap_or("");
        path.ends_with(last)
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROBOTS: &str = r#"
User-agent: *
Disallow: /private/
Allow: /private/public

User-agent: Coqui
Disallow: /coqui-only
"#;

    #[test]
    fn respects_disallow_and_allow() {
        let wildcard = RobotsRules::parse(ROBOTS, "OtherBot");
        assert!(!wildcard.allows("/private/secret"));
        assert!(wildcard.allows("/private/public/page"));
        assert!(wildcard.allows("/blog"));

        let coqui = RobotsRules::parse(ROBOTS, "Coqui/0.1");
        assert!(!coqui.allows("/coqui-only/page"));
        assert!(coqui.allows("/private/secret"));
    }

    #[test]
    fn wildcard_and_dollar() {
        let rules = RobotsRules::parse(
            "User-agent: *\nDisallow: /*.pdf$\nDisallow: /tmp*\n",
            "Coqui/0.1",
        );
        assert!(!rules.allows("/files/report.pdf"));
        assert!(rules.allows("/files/report.pdf.html"));
        assert!(!rules.allows("/tmp123"));
        assert!(rules.allows("/blog"));
    }

    #[test]
    fn longer_allow_wins() {
        let rules = RobotsRules::parse(
            "User-agent: *\nDisallow: /private/\nAllow: /private/public/\n",
            "Coqui/0.1",
        );
        assert!(!rules.allows("/private/secret"));
        assert!(rules.allows("/private/public/page"));
    }
}
