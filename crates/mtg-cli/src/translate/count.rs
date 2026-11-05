//! The grammar of Forge count expressions (`Count$ValidGraveyard,Exile Instant.YouOwn/Plus.1`, `Sacrificed$Valid Human`):
//! split into head, body and steps without losing a word, then lowered to an `Amount`. The readers of `amount` stay in
//! front; this reads what they leave as a gap and names every word of the string that stops it.

use serde_json::{Value, json};

use super::amount::{amount, apply_steps, targeted_amount};
use super::ctx::{Ctx, Tr, Unmapped, unmapped};
use super::valid::{defined, selector_any, selector_in};

/// A count expression: `head$body` and the `/step`s after it.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Expr<'a> {
    pub head: &'a str,
    pub body: Body<'a>,
    pub steps: Vec<Step<'a>>,
}

/// What follows the head: the objects of a valid-string, or one word with its argument.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Body<'a> {
    /// `Valid<zones> <filter>$<reduce>`: the zones are the comma list stuck to `Valid`, the filter and reduce may be missing.
    Valid {
        zones: Vec<&'a str>,
        filter: Option<&'a str>,
        reduce: Option<&'a str>,
    },
    /// `MostCardName Land.YouCtrl`, `CardPower`, `CardCounters.ALL`: the first word and the rest.
    Word { word: &'a str, arg: Option<&'a str> },
}

/// A `/Name` or `/Name.arg` after the body.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Step<'a> {
    pub name: &'a str,
    pub arg: Option<&'a str>,
}

/// Splits an expression; `None` when it has no `head$` of letters.
pub(super) fn parse(text: &str) -> Option<Expr<'_>> {
    let (head, rest) = text.split_once('$')?;
    if head.is_empty() || !head.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let mut pieces = rest.split('/');
    let body = body(pieces.next()?);
    let steps = pieces
        .map(|piece| match piece.split_once('.') {
            Some((name, arg)) => Step {
                name,
                arg: Some(arg),
            },
            None => Step {
                name: piece,
                arg: None,
            },
        })
        .collect();
    Some(Expr { head, body, steps })
}

fn body(text: &str) -> Body<'_> {
    if let Some(after) = text.strip_prefix("Valid") {
        let end = after.find([' ', '$']).unwrap_or(after.len());
        let (zones, rest) = after.split_at(end);
        if zones.chars().all(|c| c.is_ascii_alphabetic() || c == ',') {
            let zones = zones.split(',').filter(|zone| !zone.is_empty()).collect();
            let (filter, reduce) = match rest.strip_prefix(' ') {
                Some(more) => match more.split_once('$') {
                    Some((filter, reduce)) => (Some(filter), Some(reduce)),
                    None => (Some(more), None),
                },
                None => (None, rest.strip_prefix('$')),
            };
            return Body::Valid {
                zones,
                filter,
                reduce,
            };
        }
    }
    match text.split_once(' ') {
        Some((word, arg)) => Body::Word {
            word,
            arg: Some(arg),
        },
        None => Body::Word {
            word: text,
            arg: None,
        },
    }
}

#[cfg(test)]
impl Expr<'_> {
    /// The text the expression was split from.
    pub(super) fn render(&self) -> String {
        let mut text = format!("{}$", self.head);
        match &self.body {
            Body::Valid {
                zones,
                filter,
                reduce,
            } => {
                text.push_str("Valid");
                text.push_str(&zones.join(","));
                if let Some(filter) = filter {
                    text.push(' ');
                    text.push_str(filter);
                }
                if let Some(reduce) = reduce {
                    text.push('$');
                    text.push_str(reduce);
                }
            }
            Body::Word { word, arg } => {
                text.push_str(word);
                if let Some(arg) = arg {
                    text.push(' ');
                    text.push_str(arg);
                }
            }
        }
        for step in &self.steps {
            text.push('/');
            text.push_str(&step.render());
        }
        text
    }
}

impl Step<'_> {
    fn render(&self) -> String {
        match self.arg {
            Some(arg) => format!("{}.{arg}", self.name),
            None => self.name.to_owned(),
        }
    }
}

/// The gaps of one expression: every word the grammar does not know, not only the first.
#[derive(Default)]
struct Gaps(Vec<String>);

impl Gaps {
    fn note(&mut self, gap: &Unmapped) {
        for name in gap.0.split("; ") {
            if !self.0.iter().any(|seen| seen == name) {
                self.0.push(name.to_owned());
            }
        }
    }

    /// The value, or the gap with every name noted so far.
    fn finish(mut self, value: Option<Value>) -> Tr<Value> {
        match value {
            Some(value) if self.0.is_empty() => Ok(value),
            _ => {
                if self.0.is_empty() {
                    self.0.push("amount:count".to_owned());
                }
                Err(Unmapped(self.0.join("; ")))
            }
        }
    }

    fn take(&mut self, found: Tr<Value>) -> Option<Value> {
        match found {
            Ok(value) => Some(value),
            Err(gap) => {
                self.note(&gap);
                None
            }
        }
    }
}

/// The amount of an expression whose head the grammar owns (`Count$` of a valid-string or the heads that name an object),
/// `None` for any other so the gap of the readers stands.
pub(super) fn amount_of(ctx: &Ctx, text: &str) -> Option<Tr<Value>> {
    let expr = parse(text)?;
    let mut gaps = Gaps::default();
    let base = if expr.head == "Count" {
        if !owns_count(&expr.body) {
            return None;
        }
        counted(ctx, &expr.body, &mut gaps)
    } else if expr.head == "SVar" {
        // Another part, then the steps of this one (`SVar$X/Times.Y`).
        let Body::Word { word, arg: None } = &expr.body else {
            return None;
        };
        let part = amount(ctx, word);
        gaps.take(part)
    } else {
        let reference = reference_head(expr.head)?;
        if ctx.in_cost.get() && matches!(expr.head, "Targeted" | "ParentTargeted") {
            return Some(Err(Unmapped(format!("amount:{}$-in-a-cost", expr.head))));
        }
        referred(ctx, expr.head, reference, &expr.body, &mut gaps)
    };
    let value = steps(ctx, base, &expr.steps, &mut gaps);
    Some(gaps.finish(value))
}

/// Whether the grammar owns the body of a `Count$`: a valid-string or one of the words below. Any other word is a game
/// fact whose gap the readers name.
fn owns_count(body: &Body<'_>) -> bool {
    match body {
        Body::Valid { .. } => true,
        Body::Word { word, arg } => {
            matches!(*word, "MostProminentCreatureType" | "MostCardName") && arg.is_some()
                || word.starts_with("CardCounters.") && arg.is_none()
        }
    }
}

/// `Count$` of objects the grammar owns.
fn counted(ctx: &Ctx, body: &Body<'_>, gaps: &mut Gaps) -> Option<Value> {
    match body {
        Body::Valid {
            zones,
            filter,
            reduce,
        } => {
            let Some(filter) = filter else {
                gaps.note(&Unmapped("amount:Count$Valid-without-a-filter".to_owned()));
                return None;
            };
            let of = zone_selector(ctx, zones, filter);
            let of = gaps.take(of.map(Value::from))?;
            let word = format!("Valid{}", zones.join(","));
            let of = of.as_array().cloned().unwrap_or_default();
            let found = super::amount_more::reduced(&word, &of, reduce.unwrap_or(""));
            gaps.take(found)
        }
        Body::Word {
            word: "MostProminentCreatureType",
            arg: Some(filter),
        } => most_common(ctx, filter, "CreatureType", gaps),
        Body::Word {
            word: "MostCardName",
            arg: Some(filter),
        } => most_common(ctx, filter, "Name", gaps),
        Body::Word { word, .. } => {
            let kind = word.trim_start_matches("CardCounters.");
            let this = defined(ctx, "Self");
            let this = gaps.take(this)?;
            let counters = targeted_amount(
                "Count",
                &format!("CardCounters.{}", kind.to_uppercase()),
                &this,
            );
            gaps.take(counters)
        }
    }
}

fn most_common(ctx: &Ctx, filter: &str, stat: &str, gaps: &mut Gaps) -> Option<Value> {
    let of = selector_in(ctx, filter, None);
    let of = gaps.take(of.map(Value::from))?;
    Some(json!({"MostCommon": {"of": of, "stat": stat}}))
}

/// The selector of a zone list: the zones outside the battlefield share one `In`; a list with the battlefield or the
/// command zone is one alternative per zone, because control counts on the battlefield and ownership elsewhere.
pub(super) fn zone_selector(ctx: &Ctx, zones: &[&str], filter: &str) -> Tr<Vec<Value>> {
    match zones {
        [] | ["Battlefield"] => selector_in(ctx, filter, None),
        [only] => selector_in(ctx, filter, Some(only)),
        many if many
            .iter()
            .all(|zone| matches!(*zone, "Graveyard" | "Hand" | "Exile")) =>
        {
            selector_in(ctx, filter, Some(&many.join(",")))
        }
        many => {
            let mut gaps = Gaps::default();
            let mut alternatives = Vec::new();
            for zone in many {
                let preds = match *zone {
                    "Battlefield" => selector_in(ctx, filter, None).and_then(|mut preds| {
                        // Oracle says "you control" on the battlefield; the script's ownership there disagrees, except
                        // for a commander, which Oracle names by owner.
                        let owned = preds.iter().any(|pred| pred.get("OwnedBy").is_some());
                        if owned && !preds.iter().any(|pred| pred == "Commander") {
                            return unmapped("zone:YouOwn-on-the-battlefield");
                        }
                        preds.push(json!({"In": "Battlefield"}));
                        Ok(preds)
                    }),
                    other => selector_in(ctx, filter, Some(other)),
                };
                if let Some(preds) = gaps.take(preds.map(Value::from)) {
                    alternatives.push(all_of(preds.as_array().cloned().unwrap_or_default()));
                }
            }
            if !gaps.0.is_empty() {
                return Err(Unmapped(gaps.0.join("; ")));
            }
            Ok(vec![json!({ "AnyOf": alternatives })])
        }
    }
}

/// All of the predicates as one: not any of their negations.
fn all_of(mut preds: Vec<Value>) -> Value {
    if preds.len() == 1 {
        return preds.remove(0);
    }
    let negated: Vec<Value> = preds.iter().map(|pred| json!({"Not": pred})).collect();
    json!({"Not": {"AnyOf": negated}})
}

/// The `defined` word of a head that names objects the effect or the event holds, `None` for any other head.
fn reference_head(head: &str) -> Option<&'static str> {
    Some(match head {
        "Targeted" => "Targeted",
        "ParentTargeted" => "ParentTarget",
        "Sacrificed" => "Sacrificed",
        "Enchanted" => "Enchanted",
        "Equipped" => "Equipped",
        "TriggerRemembered" => "DelayedTriggerRemembered",
        "TriggeredCard" => "TriggeredCard",
        "TriggeredCardLKICopy" => "TriggeredCardLKICopy",
        "TriggeredNewCardLKICopy" => "TriggeredNewCardLKICopy",
        "TriggeredAttacker" => "TriggeredAttacker",
        "TriggeredAttackerLKICopy" => "TriggeredAttackerLKICopy",
        "TriggeredBlocker" => "TriggeredBlocker",
        "TriggeredBlockerLKICopy" => "TriggeredBlockerLKICopy",
        "TriggeredSource" => "TriggeredSource",
        "TriggeredSourceLKICopy" => "TriggeredSourceLKICopy",
        _ => return None,
    })
}

/// An expression about the objects a reference head names: how many satisfy a filter, a number of them, or a stat.
fn referred(ctx: &Ctx, head: &str, word: &str, body: &Body<'_>, gaps: &mut Gaps) -> Option<Value> {
    let reference = gaps.take(defined(ctx, word));
    match body {
        Body::Valid {
            zones,
            filter: Some(filter),
            reduce: None,
        } if zones.is_empty() => {
            let filter = gaps.take(selector_any(ctx, filter).map(Value::from));
            match (reference, filter) {
                (Some(of), Some(filter)) => Some(json!({"CountOf": {"of": of, "filter": filter}})),
                _ => None,
            }
        }
        Body::Valid { .. } => {
            gaps.note(&Unmapped(format!("amount:{head}$Valid-reduced")));
            None
        }
        // How many objects the head names: the cards paid or remembered, whatever they are.
        Body::Word {
            word: "Amount",
            arg: None,
        } if matches!(head, "Sacrificed" | "TriggerRemembered") => {
            reference.map(|of| json!({"CountOf": {"of": of, "filter": []}}))
        }
        Body::Word { word, arg: None } => {
            let stat = reference.map(|of| targeted_amount(head, word, &of));
            stat.and_then(|stat| gaps.take(stat))
        }
        Body::Word {
            word,
            arg: Some(arg),
        } => {
            gaps.note(&Unmapped(format!("amount:{head}${word} {arg}")));
            None
        }
    }
}

/// The steps after the base: the readers' own for those they know, the product of two amounts for `Times.<part>`.
fn steps(ctx: &Ctx, base: Option<Value>, steps: &[Step<'_>], gaps: &mut Gaps) -> Option<Value> {
    let mut value = base;
    for step in steps {
        let next = match (step.name, step.arg, value.as_ref()) {
            // A factor named by another part: the product of the two amounts.
            ("Times", Some(part), current) if part.parse::<i32>().is_err() => {
                let factor = amount(ctx, part);
                let factor = gaps.take(factor);
                current
                    .zip(factor)
                    .map(|(current, factor)| json!({"Product": [current, factor]}))
            }
            (_, _, Some(current)) => {
                let one = apply_steps(ctx, current.clone(), &step.render());
                gaps.take(one)
            }
            // The base is already a gap: the step is still read so its own unknown word is named.
            (_, _, None) => {
                let probe = apply_steps(ctx, json!(0), &step.render());
                gaps.take(probe);
                None
            }
        };
        value = next;
    }
    value
}

#[cfg(test)]
mod tests {
    //! Goal: every count expression of the Forge scripts at hand splits and renders back to the same string; the test is skipped when no checkout is there.
    use std::fs;
    use std::path::Path;

    use super::parse;

    fn scripts(dir: &Path, found: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scripts(&path, found);
            } else if let Ok(text) = fs::read_to_string(&path) {
                found.push(text);
            }
        }
    }

    /// The value of every `SVar:Name:Value` line that is an expression (`Head$...`).
    fn expressions(text: &str) -> Vec<String> {
        text.lines()
            .filter_map(|line| line.strip_prefix("SVar:"))
            .filter_map(|line| line.split_once(':').map(|(_, value)| value))
            .filter(|value| {
                let head = value.split('$').next().unwrap_or_default();
                value.contains('$')
                    && !head.is_empty()
                    && head.chars().all(|c| c.is_ascii_alphanumeric())
                    && !value.contains('|')
            })
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn every_count_expression_of_the_scripts_renders_as_written() {
        let Some(home) = std::env::var_os("HOME") else {
            return;
        };
        let dir = Path::new(&home).join(".cache/mtg-cli/forge/forge-gui/res/cardsfolder");
        let mut texts = Vec::new();
        scripts(&dir, &mut texts);
        let mut total = 0;
        let mut failures = Vec::new();
        for text in &texts {
            for value in expressions(text) {
                total += 1;
                let rendered = parse(&value).map(|expr| expr.render());
                if rendered.as_deref() != Some(value.as_str()) {
                    failures.push(value);
                }
            }
        }
        failures.sort();
        failures.dedup();
        eprintln!(
            "round trip: {total} expressions, {} distinct failures",
            failures.len()
        );
        for failure in failures.iter().take(40) {
            eprintln!("  {failure}");
        }
        assert!(
            failures.is_empty(),
            "{} expressions do not round trip",
            failures.len()
        );
    }
}
