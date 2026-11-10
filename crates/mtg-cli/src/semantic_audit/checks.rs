//! The structural checks of `semantic-audit`: one table row per check, each a cue in the clause's Oracle text and the
//! shape the rules that stand for the clause must then hold.

use std::collections::BTreeSet;

use serde_json::Value;

/// What the rules of one clause hold, read from their JSON: every key and string, every number, the target bounds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    /// Every object key and string value (variant names and words).
    pub tokens: BTreeSet<String>,
    /// Every number.
    pub numbers: BTreeSet<i64>,
    /// The `max` of every target spec's bounds; `None` for "any number".
    pub maxes: Vec<Option<i64>>,
    /// Whether a target spec may choose nothing (`min` 0, "up to").
    pub min_zero: bool,
    /// Whether the rules choose targets.
    pub targets: bool,
}

impl Facts {
    /// Whether a token equals one of the names.
    fn any(&self, names: &[&str]) -> bool {
        names.iter().any(|name| self.tokens.contains(*name))
    }

    /// Whether a token contains one of the parts.
    fn any_part(&self, parts: &[&str]) -> bool {
        self.tokens
            .iter()
            .any(|token| parts.iter().any(|part| token.contains(part)))
    }
}

/// Keys of bookkeeping, not rules.
const SKIP: [&str; 2] = ["span", "oracle_fingerprint"];

/// The facts of the abilities that stand for a clause.
pub fn facts(rules: &[&Value]) -> Facts {
    let mut facts = Facts::default();
    for rule in rules {
        gather(rule, &mut facts);
    }
    facts
}

fn gather(value: &Value, facts: &mut Facts) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                if SKIP.contains(&key.as_str()) {
                    continue;
                }
                facts.tokens.insert(key.clone());
                if key == "targets" && inner.as_array().is_some_and(|a| !a.is_empty()) {
                    facts.targets = true;
                    for spec in inner.as_array().into_iter().flatten() {
                        let bounds = spec.get("bounds");
                        let max =
                            bounds.map_or(Some(1), |b| b.get("max").map_or(Some(1), Value::as_i64));
                        facts.maxes.push(max);
                        if bounds.and_then(|b| b.get("min")).and_then(Value::as_i64) == Some(0) {
                            facts.min_zero = true;
                        }
                    }
                }
                if key == "Target" {
                    facts.targets = true;
                }
                gather(inner, facts);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| gather(item, facts)),
        Value::String(text) => {
            facts.tokens.insert(text.clone());
        }
        Value::Number(n) => {
            if let Some(n) = n.as_i64() {
                facts.numbers.insert(n);
            }
        }
        Value::Bool(_) | Value::Null => {}
    }
}

/// The clause as the checks read it: lower case, reminder text removed, the card's names as `~`, one space between words.
pub fn clause_text(printed: &str, names: &[String]) -> String {
    let mut text = String::new();
    let mut depth = 0u32;
    for c in printed.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => text.push(c),
            _ => {}
        }
    }
    let mut text = text.to_lowercase().replace('\u{2019}', "'");
    let mut names: Vec<String> = names
        .iter()
        .map(|n| n.to_lowercase())
        .filter(|n| !n.is_empty())
        .collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    for name in names {
        text = text.replace(&name, "~");
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The words of a text: runs of letters, digits, `'`, `+`, `-` and `~`.
fn words(text: &str) -> Vec<&str> {
    text.split(|c: char| !(c.is_alphanumeric() || matches!(c, '\'' | '~' | '+' | '-' | '\u{2212}')))
        .filter(|w| !w.is_empty())
        .collect()
}

/// Whether the text holds the phrase as whole words.
fn has(text: &str, phrase: &str) -> bool {
    let w = words(text);
    let p = words(phrase);
    !p.is_empty() && w.windows(p.len()).any(|win| win == p.as_slice())
}

/// The text with mana symbols (`{...}`) removed.
fn without_symbols(text: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in text.chars() {
        match c {
            '{' => inside = true,
            '}' => inside = false,
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    out
}

/// Number words the `number` check reads; "one" and "a" are left out (one or more, one of).
const NUMBER_WORDS: [(&str, i64); 9] = [
    ("two", 2),
    ("three", 3),
    ("four", 4),
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
    ("nine", 9),
    ("ten", 10),
];

/// The value of a number word or numeral of at least 2.
fn number(word: &str) -> Option<i64> {
    NUMBER_WORDS
        .iter()
        .find(|(w, _)| *w == word)
        .map(|(_, n)| *n)
        .or_else(|| word.parse::<i64>().ok().filter(|n| *n >= 2))
}

/// Tokens that stand for a condition of any shape.
const CONDITIONAL: [&str; 9] = [
    "if",
    "condition",
    "If",
    "unless",
    "otherwise",
    "Unless",
    "RepeatWhile",
    "Pay",
    "paid",
];

/// Trigger filters that fold an intervening "if" into the event ("if you cast it", "if it was kicked").
const FILTERED: [&str; 15] = [
    "WasCast",
    "Kicked",
    "Test",
    "Compare",
    "CastWith",
    "Matches",
    "Not",
    "HasCounter",
    "HasAnyCounter",
    "from",
    "Tapped",
    "Untapped",
    "DamagedBy",
    "Attacking",
    "Blocking",
];

/// Amount and effect forms that count (anything but a fixed number).
const COUNTED: [&str; 45] = [
    "Count",
    "Stat",
    "Life",
    "Counters",
    "AnyCounters",
    "Sum",
    "Times",
    "Half",
    "Divided",
    "EvenShare",
    "TargetCount",
    "Devotion",
    "Greatest",
    "Total",
    "Distinct",
    "CardTypes",
    "Players",
    "LifeLostThisWay",
    "DamageDealtThisWay",
    "Destroyed",
    "Exiled",
    "Kicks",
    "Devoured",
    "Bound",
    "Tallied",
    "Largest",
    "HighestCounters",
    "Party",
    "Tally",
    "Remembered",
    "TriggeringTargets",
    "ManaSpent",
    "Each",
    "RepeatEach",
    "each",
    "per",
    "per_player",
    "each_target",
    "X",
    "CountOf",
    "ColorsSpent",
    "Triggering",
    "EachColorAmong",
    "AttackTax",
    "PerPlayer",
];

/// A finding of a check: a note, or `None` when the rules hold what the clause asks.
type Run = fn(&str, &Facts) -> Option<String>;

/// One structural check: its name, what it asks and how.
pub struct Check {
    /// The name in the findings TSV.
    pub name: &'static str,
    /// What a finding means, for the report.
    pub what: &'static str,
    /// The check.
    pub run: Run,
    /// The share of a hand-judged sample of findings that were real faults, in percent (100 when it found none).
    pub precision: u8,
}

/// The intervening "if" of a trigger ("When ~ enters, if ..., ...").
fn intervening_if(text: &str, f: &Facts) -> Option<String> {
    let trigger = ["when ", "whenever ", "at "]
        .iter()
        .any(|s| text.starts_with(s));
    let head = text.split(['.', ':']).next().unwrap_or_default();
    (trigger && head.contains(", if ") && !f.any(&CONDITIONAL) && !f.any(&FILTERED))
        .then(|| "trigger has an if, rules no condition".into())
}

/// "For each" without a counted amount.
fn for_each(text: &str, f: &Facts) -> Option<String> {
    (has(text, "for each") && !f.any(&COUNTED)).then(|| "for each, rules count nothing".into())
}

/// "May" without an optional step.
fn may(text: &str, f: &Facts) -> Option<String> {
    let permission = [
        "may cast",
        "may play",
        "may spend",
        "may activate",
        "may look",
        "may have",
        "may block",
        "may choose",
        "rather than pay",
    ]
    .iter()
    .any(|p| has(text, p));
    let asks = has(text, "may") && !permission;
    let optional = f.any_part(&[
        "May",
        "optional",
        "Optional",
        "Choose",
        "Pay",
        "Opening",
        "RevealOr",
        "AdditionalCost",
        "Exert",
    ]) || f.min_zero;
    (asks && !optional).then(|| "may, rules ask nothing".into())
}

/// "Target" without targets in the rules.
fn target(text: &str, f: &Facts) -> Option<String> {
    let w = words(text);
    let cued = w.iter().enumerate().any(|(i, word)| {
        *word == "target"
            && !matches!(
                i.checked_sub(1).map(|j| w[j]),
                Some(
                    "the"
                        | "a"
                        | "new"
                        | "its"
                        | "that"
                        | "single"
                        | "each"
                        | "could"
                        | "can"
                        | "for"
                )
            )
    });
    (cued && !f.targets).then(|| "target, rules choose none".into())
}

/// "Up to N target" or "N target" where the rules' target bounds differ.
fn target_count(text: &str, f: &Facts) -> Option<String> {
    let plain = without_symbols(text);
    let w = words(&plain);
    for (i, word) in w.iter().enumerate() {
        let Some(n) = number(word).filter(|_| word.chars().all(char::is_alphabetic)) else {
            continue;
        };
        let next = w.get(i + 1).copied();
        let targeted = next == Some("target")
            || (next.is_some() && w.get(i + 2) == Some(&"target") && next == Some("other"));
        let modal = i >= 1 && w[i - 1] == "choose";
        if !targeted || !f.targets || modal {
            continue;
        }
        let up_to = i >= 2 && w[i - 2] == "up" && w[i - 1] == "to";
        if !f.maxes.contains(&Some(n)) && !f.maxes.contains(&None) {
            return Some(format!("{n} targets, rules allow {:?}", f.maxes));
        }
        if up_to && !f.min_zero {
            return Some(format!("up to {n}, rules need {n}"));
        }
    }
    None
}

/// "Until end of turn" / "until your next turn" without the duration.
fn duration(text: &str, f: &Facts) -> Option<String> {
    if has(text, "until end of turn")
        && !f.any_part(&["EndOfTurn", "ThisTurn", "this_turn", "until_end_of_turn"])
        && !f.any(&["Turn"])
    {
        return Some("until end of turn, rules last otherwise".into());
    }
    (has(text, "until your next turn") && !f.any_part(&["NextTurn"]))
        .then(|| "until your next turn, rules last otherwise".into())
}

/// "Each opponent" with rules that never name the opponents.
fn each_opponent(text: &str, f: &Facts) -> Option<String> {
    let cued =
        has(text, "each opponent") || has(text, "each other player") || has(text, "your opponents");
    (cued && !f.any_part(&["Opponent", "EachPlayer", "Other"]))
        .then(|| "each opponent, rules name no opponent".into())
}

/// "Another" / "other <type>" without `Other`.
fn another(text: &str, f: &Facts) -> Option<String> {
    let w = words(text);
    let object = |word: &str| {
        matches!(
            word,
            "target"
                | "creature"
                | "creatures"
                | "permanent"
                | "permanents"
                | "artifact"
                | "artifacts"
                | "enchantment"
                | "enchantments"
                | "planeswalker"
                | "nonland"
                | "noncreature"
        )
    };
    // Only a selector that leaves the source out: "another target creature", "each other creature", "other creatures you
    // control". A union ("~ or another Equipment", "enchanted creature and other creatures"), a count ("had another land
    // enter") and a comparison ("as another creature") include or name the source otherwise.
    let cued = (1..w.len()).any(|i| {
        let before = i.checked_sub(2).map(|j| w[j]);
        let leaves_out = matches!(w[i - 1], "another" | "other")
            && object(w[i])
            && !matches!(before, Some("or" | "and" | "as" | "had" | "its" | "their"));
        leaves_out && !(w[i - 1] == "other" && matches!(before, Some("any" | "no")))
    });
    // "Not Is X" and "X or shares ..." leave out or name the object otherwise; one spec of several targets picks distinct ones.
    let excluded = (f.any(&["Not"]) || f.any(&["AnyOf"])) && f.any(&["Is"]);
    let excluded = excluded || f.maxes.iter().any(|m| *m != Some(1));
    (cued && !f.any_part(&["Other", "Another"]) && !f.any(&["rest_as"]) && !excluded)
        .then(|| "another, rules keep the source".into())
}

/// "Nontoken" without a token filter.
fn nontoken(text: &str, f: &Facts) -> Option<String> {
    (words(text).iter().any(|w| w.starts_with("nontoken")) && !f.any_part(&["Token"]))
        .then(|| "nontoken, rules filter no tokens".into())
}

/// "Non<type>" (nonland, noncreature, nonbasic, nonblack, ...) without a negation.
fn non_filter(text: &str, f: &Facts) -> Option<String> {
    let negated = words(text).into_iter().find(|w| {
        w.starts_with("non")
            && w.len() > 4
            && !w.starts_with("nontoken")
            && !matches!(*w, "none" | "nonetheless" | "noncombat")
    });
    let held = f.any(&["Not", "Non"]) || f.any_part(&["Non", "Not", "Except"]);
    negated
        .filter(|_| !held)
        .map(|w| format!("{w}, rules negate nothing"))
}

/// Numbers of the clause (2 and up, mana symbols aside) the rules lack.
fn numbers(text: &str, f: &Facts) -> Option<String> {
    let plain = without_symbols(text);
    let w = words(&plain);
    // Only amounts of a verb the rules always carry as a number: damage, life, cards drawn or milled, a scry, a pump.
    let counted = |i: usize| {
        let next = w.get(i + 1).copied();
        let before = i.checked_sub(1).map(|j| w[j]);
        matches!(next, Some("damage" | "life"))
            || matches!(
                before,
                Some("draw" | "draws" | "mill" | "mills" | "scry" | "surveil")
            )
            || (w[i].starts_with('+')
                && !w
                    .iter()
                    .skip(i + 1)
                    .take(2)
                    .any(|x| matches!(*x, "counter" | "counters")))
    };
    let missing: BTreeSet<i64> = (0..w.len())
        .filter(|&i| counted(i))
        .flat_map(|i| w[i].split('/'))
        .filter_map(|w| number(w.trim_start_matches(['+', '-', '\u{2212}'])))
        .filter(|n| !f.numbers.contains(n) && !f.numbers.contains(&-n))
        .collect();
    (!missing.is_empty()).then(|| {
        let list: Vec<String> = missing.iter().map(i64::to_string).collect();
        format!("{} missing", list.join(","))
    })
}

/// "Instead" without a replacement or an alternative.
fn instead(text: &str, f: &Facts) -> Option<String> {
    let held = f.any_part(&[
        "Instead",
        "Replace",
        "Shield",
        "Floor",
        "Replace",
        "Prevent",
        "otherwise",
        "If",
        "condition",
    ]);
    // A modal "choose both instead" and a destination ("instead of putting it into") need no replacement.
    let cued =
        has(text, "instead") && !has(text, "both instead") && !has(text, "instead of putting");
    (cued && !held).then(|| "instead, rules replace nothing".into())
}

/// The checks, in report order.
pub const CHECKS: &[Check] = &[
    Check {
        name: "intervening-if",
        what: "a trigger's intervening if with no condition",
        run: intervening_if,
        precision: 25,
    },
    Check {
        name: "for-each",
        what: "for each with no counted amount",
        run: for_each,
        precision: 0,
    },
    Check {
        name: "may",
        what: "may with no optional step",
        run: may,
        precision: 0,
    },
    Check {
        name: "target",
        what: "target with no targets",
        run: target,
        precision: 25,
    },
    Check {
        name: "target-count",
        what: "a target count the bounds differ from",
        run: target_count,
        precision: 0,
    },
    Check {
        name: "duration",
        what: "until end of turn / your next turn with no duration",
        run: duration,
        precision: 25,
    },
    Check {
        name: "each-opponent",
        what: "each opponent with no opponent in the rules",
        run: each_opponent,
        precision: 0,
    },
    Check {
        name: "another",
        what: "another / other with no Other",
        run: another,
        precision: 25,
    },
    Check {
        name: "nontoken",
        what: "nontoken with no token filter",
        run: nontoken,
        precision: 100,
    },
    Check {
        name: "non-filter",
        what: "a non- filter with no negation",
        run: non_filter,
        precision: 0,
    },
    Check {
        name: "number",
        what: "a number of the text missing from the rules",
        run: numbers,
        precision: 0,
    },
    Check {
        name: "instead",
        what: "instead with no replacement",
        run: instead,
        precision: 0,
    },
];

/// The name of the check for a clause sentence no ability stands for.
pub const UNCLAUSED: &str = "unclaused";

/// The findings of every check on one clause: (check, note).
pub fn run(text: &str, facts: &Facts) -> Vec<(&'static str, String)> {
    CHECKS
        .iter()
        .filter_map(|check| (check.run)(text, facts).map(|note| (check.name, note)))
        .collect()
}

/// The measured precision of a check by name; `unclaused` and unknown names count as 100.
pub fn precision(name: &str) -> u8 {
    CHECKS
        .iter()
        .find(|check| check.name == name)
        .map_or(100, |check| check.precision)
}
