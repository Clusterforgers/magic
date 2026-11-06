//! Costs: Forge's `Cost$` words (`2 R T Sac<1/Creature>`) as the DSL's cost items, and loyalty costs.

use serde_json::{Value, json};

use super::amount::{amount, counter_kind};
use super::ctx::{Ctx, Tr, unmapped};
use super::valid::selector_any;

/// The parts of a cost: words split at spaces outside angle brackets.
pub fn parts(cost: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (at, c) in cost.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            c if c.is_whitespace() && depth == 0 => {
                if start < at {
                    parts.push(&cost[start..at]);
                }
                start = at + c.len_utf8();
            }
            _ => {}
        }
    }
    if start < cost.len() {
        parts.push(&cost[start..]);
    }
    parts
}

/// Whether a cost word is a mana symbol (`2`, `R`, `W/U`, `X`).
pub fn is_mana(word: &str) -> bool {
    !word.is_empty()
        && (word.chars().all(|c| c.is_ascii_digit())
            || hybrid(word).is_some()
            || (word.len() <= 3
                && word
                    .split('/')
                    .all(|side| side.len() == 1 && "WUBRGCXSP".contains(side) || side == "2")))
}

/// The symbol of a Forge hybrid or Phyrexian word: `WB` is `{W/B}` and `GP` is `{G/P}`.
fn hybrid(word: &str) -> Option<String> {
    let mut chars = word.chars();
    let (first, second) = (chars.next()?, chars.next()?);
    let color = |c: char| "WUBRG".contains(c);
    if chars.next().is_some() || !color(first) {
        return None;
    }
    (second == 'P' || (color(second) && second != first)).then(|| format!("{{{first}/{second}}}"))
}

/// The mana symbol a cost word stands for, with its braces.
pub fn symbol(word: &str) -> String {
    hybrid(word).unwrap_or_else(|| format!("{{{word}}}"))
}

/// The loyalty cost a word stands for (`AddCounter<2/LOYALTY>`).
fn loyalty_word(word: &str) -> Option<Value> {
    let (head, rest) = word.split_once('<')?;
    let inner = rest.strip_suffix('>')?;
    let (number, kind) = inner.split_once('/')?;
    if !kind.eq_ignore_ascii_case("loyalty") {
        return None;
    }
    match (head, number) {
        ("AddCounter" | "SubCounter", "0") => Some(json!("Zero")),
        ("AddCounter", n) => n.parse::<u32>().ok().map(|n| json!({"Plus": n})),
        ("SubCounter", "X") => Some(json!("MinusX")),
        ("SubCounter", n) => n.parse::<u32>().ok().map(|n| json!({"Minus": n})),
        _ => None,
    }
}

/// The loyalty cost of an ability whose whole cost is one loyalty word.
pub fn loyalty(cost: &str) -> Option<Value> {
    match parts(cost).as_slice() {
        [word] => loyalty_word(word),
        _ => None,
    }
}

/// The cost items a cost stands for, as the DSL's `Cost`: a string for mana alone.
pub fn cost(ctx: &Ctx, text: &str) -> Tr<Value> {
    ctx.in_cost.set(true);
    let found = cost_items(ctx, text);
    ctx.in_cost.set(false);
    found
}

/// The cost of an activated ability: `cost`, where a waterbend part is also read.
pub fn activated(ctx: &Ctx, text: &str) -> Tr<Value> {
    ctx.waterbend_ok.set(true);
    let found = cost(ctx, text);
    ctx.waterbend_ok.set(false);
    found
}

fn cost_items(ctx: &Ctx, text: &str) -> Tr<Value> {
    let mut mana = String::new();
    let mut items = Vec::new();
    for word in parts(text) {
        if is_mana(word) {
            // An {X} that the card ties to another value (the mana value of a target) is not the player's free choice.
            if word.contains('X') && ctx.svar("X").is_some_and(|value| !value.contains("xPaid")) {
                return unmapped("cost:X-tied-to-a-value");
            }
            mana.push_str(&symbol(word));
        } else {
            items.push(item(ctx, word)?);
        }
    }
    if items.is_empty() {
        return Ok(json!(mana));
    }
    if !mana.is_empty() {
        items.insert(0, json!({ "Mana": mana }));
    }
    Ok(Value::Array(items))
}

/// One non-mana cost word.
fn item(ctx: &Ctx, word: &str) -> Tr<Value> {
    match word {
        "T" => return Ok(json!("Tap")),
        "Q" => return Ok(json!("Untap")),
        _ => {}
    }
    if let Some(found) = super::cost_more::bare(word) {
        return Ok(found);
    }
    let Some((head, rest)) = word.split_once('<') else {
        return unmapped(format!("cost:{word}"));
    };
    let Some(inner) = rest.strip_suffix('>') else {
        return unmapped(format!("cost:{head}"));
    };
    let all: Vec<&str> = inner.split('/').collect();
    if let Some(found) = super::cost_more::item(ctx, head, &all) {
        return found;
    }
    let mut fields = inner.split('/');
    let number = fields.next().unwrap_or_default();
    let what = fields.next().unwrap_or_default();
    let on_self = matches!(what, "CARDNAME" | "NICKNAME" | "Card.Self");
    match head {
        "Sac" if on_self => Ok(json!("SacrificeSelf")),
        "Sac" => Ok(
            json!({"Sacrifice": {"filter": selector_any(ctx, &other_if_worded(ctx, "sacrifice", number, what)?)?, "count": amount(ctx, number)?}}),
        ),
        "PayLife" => Ok(json!({"PayLife": amount(ctx, number)?})),
        "PayEnergy" => Ok(json!({"PayEnergy": amount(ctx, number)?})),
        "Mill" => Ok(json!({"Mill": amount(ctx, number)?})),
        "Discard" if on_self => Ok(json!("DiscardSelf")),
        "Discard" => discard(ctx, number, what),
        "Exile" | "ExileFromGrave" if on_self => Ok(json!("ExileSelf")),
        "ExileFromGrave" => zone_cost(ctx, "ExileFromGraveyard", number, what),
        // "Exile it" in a death trigger: the card the permanent became, in whichever graveyard it went to.
        "ExileAnyGrave" if number == "1" && what == "Card.TriggeredNewCard" => {
            let card = super::triggered::remembered_by_event(
                ctx.trigger.as_ref(),
                what.split('.').nth(1).unwrap_or_default(),
            )?;
            Ok(json!({"ExileFromGraveyard": {"filter": [{"Is": card}], "any_player": true}}))
        }
        "ExileFromHand" => zone_cost(ctx, "ExileFromHand", number, what),
        "ExileCtrlOrGrave" => zone_cost(ctx, "Craft", number, what),
        "Return" if on_self => Ok(json!({"ReturnToHand": {"filter": ["This"]}})),
        "Return" => zone_cost(ctx, "ReturnToHand", number, what),
        "Reveal" => zone_cost(ctx, "RevealFromHand", number, what),
        "tapXType" => zone_cost(
            ctx,
            "TapPermanents",
            number,
            &other_if_worded(ctx, "tap", number, what)?,
        ),
        "SubCounter" => remove_counters(ctx, number, what),
        _ => unmapped(format!("cost:{head}")),
    }
}

/// The filter of a cost the script wrote without `Other` where the card says "tap another untapped creature": the paid
/// permanent is never the source. The wording must name the type and the count the cost pays, since a card with several
/// abilities words each of them; when the card words the same count both ways the cost cannot be told apart and is a gap.
fn other_if_worded(ctx: &Ctx, verb: &str, number: &str, what: &str) -> Tr<String> {
    if what.contains("Other") || what.contains("Self") {
        return Ok(what.to_string());
    }
    let stems: Vec<String> = what
        .split(';')
        .map(|alt| {
            let kind = alt.split('.').next().unwrap_or_default().to_lowercase();
            kind.chars().take(4).collect()
        })
        .collect();
    let names = |noun: &str| {
        stems
            .iter()
            .any(|stem| matches!(stem.as_str(), "card" | "perm") || noun.starts_with(stem))
    };
    let text = ctx.oracle.to_lowercase().replace("\\n", " ");
    let words: Vec<&str> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let (mut with_other, mut without) = (false, false);
    for (at, _) in words.iter().enumerate().filter(|(_, w)| **w == verb) {
        let rest = &words[at + 1..];
        let Some(head) = rest.iter().position(|next| *next != "untapped") else {
            continue;
        };
        let (count, other, nouns) = match rest[head] {
            "another" => (Some("1"), true, &rest[head + 1..]),
            "other" => (None, true, &rest[head + 1..]),
            first => {
                let count = match first {
                    "a" | "an" | "one" => "1",
                    "two" => "2",
                    "three" => "3",
                    "four" => "4",
                    "five" => "5",
                    "x" => "X",
                    _ => continue,
                };
                let mut tail = rest[head + 1..].iter().skip_while(|w| **w == "untapped");
                let other = tail.next() == Some(&"other");
                (Some(count), other, &rest[head + 1..])
            }
        };
        let counted = count.is_none_or(|count| count == number);
        let close = nouns.iter().take(4).any(|noun| names(noun));
        if counted && close {
            if other {
                with_other = true;
            } else {
                without = true;
            }
        }
    }
    if !with_other {
        return Ok(what.to_string());
    }
    if without {
        return unmapped("cost:Other-ambiguous");
    }
    Ok(what
        .split(';')
        .map(|alt| match alt {
            alt if alt.contains('.') => format!("{alt}+Other"),
            alt => format!("{alt}.Other"),
        })
        .collect::<Vec<_>>()
        .join(";"))
}

fn discard(ctx: &Ctx, number: &str, what: &str) -> Tr<Value> {
    if what == "Random" {
        return super::cost_more::random(number);
    }
    // "Hand" discards the whole hand whatever the number says: the count is the hand size.
    if what == "Hand" {
        return Ok(json!({"Discard": {"count": {"Count": [{"In": "Hand"}, {"OwnedBy": "You"}]}}}));
    }
    let count = amount(ctx, number)?;
    if matches!(what, "Card" | "") {
        return Ok(json!({"Discard": {"count": count}}));
    }
    Ok(json!({"Discard": {"count": count, "filter": selector_any(ctx, what)?}}))
}

fn zone_cost(ctx: &Ctx, variant: &str, number: &str, what: &str) -> Tr<Value> {
    Ok(json!({ variant: {"filter": selector_any(ctx, what)?, "count": amount(ctx, number)?} }))
}

fn remove_counters(ctx: &Ctx, number: &str, what: &str) -> Tr<Value> {
    let Some(kind) = counter_kind(what) else {
        return unmapped("cost:SubCounter-kind");
    };
    Ok(json!({"RemoveCounters": {"counter": kind, "count": amount(ctx, number)?}}))
}
