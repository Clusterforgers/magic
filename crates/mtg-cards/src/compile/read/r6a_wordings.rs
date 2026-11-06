//! Other Oracle wordings of the same triggered rule: each row rewrites one phrase the writers give into the phrase a
//! printed card uses for the same meaning ("a creature with flying you control" is "a creature you control with
//! flying", "When" and "Whenever" are one trigger word (CR 603.1), an imperative is done by "you", "A, then B" is "A and
//! B", cycling discards the card). A row never drops a word that carries meaning.

use crate::ability::Ability;
use crate::trigger::TriggerPattern;

/// The most wordings the rows add to one ability: the rows chain, and a long text could otherwise multiply them.
const LIMIT: usize = 96;

/// The verbs an effect says without "you": an instruction with no subject is done by the ability's controller.
const IMPERATIVE: &[&str] = &[
    "create",
    "draw",
    "scry",
    "surveil",
    "put",
    "proliferate",
    "investigate",
    "mill",
    "discard",
    "sacrifice",
    "exile",
    "return",
    "untap",
    "tap",
    "add",
    "destroy",
    "look",
    "reveal",
    "search",
    "shuffle",
    "amass",
    "populate",
    "venture",
    "conjure",
    "seek",
    "remove",
    "goad",
    "attach",
];

/// The phrases that say who controls a permanent after its other qualities.
const CONTROLLERS: &[&str] = &[
    " you control",
    " you don't control",
    " an opponent controls",
    " your opponents control",
    " defending player controls",
];

/// The words that open a quality of a permanent that may stand before or after who controls it.
const QUALITIES: &[&str] = &[" with ", " without ", " of the chosen type", " that's "];

/// The card types and colors whose order in "A or B" and "A and B" says nothing.
const SWAPPED: &[&str] = &[
    "artifact",
    "creature",
    "enchantment",
    "land",
    "planeswalker",
    "battle",
    "white",
    "blue",
    "black",
    "red",
    "green",
    "flying",
    "reach",
    "menace",
    "trample",
    "haste",
    "vigilance",
    "deathtouch",
    "lifelink",
];

/// Exact phrase rows: `(written, printed)`.
const ROWS: &[(&str, &str)] = &[
    // Cycling discards the card (CR 702.29a): "cycle or discard" triggers once, as a discard.
    ("Whenever you discard ", "Whenever you cycle or discard "),
    // Only a spell's controller casts it.
    (" spell you control each turn", " spell each turn"),
    (", then if you do, ", ". If you do, "),
    (
        ", then this ability triggers only once each turn",
        ". This ability triggers only once each turn",
    ),
    (
        ", then do this only once each turn",
        ". Do this only once each turn",
    ),
    (
        "each other creature you control",
        "each creature you control other than ~",
    ),
    (
        "each other attacking creature",
        "each attacking creature other than ~",
    ),
    (
        "each other blocking creature",
        "each blocking creature other than ~",
    ),
    (
        "a modified nontoken creature",
        "a nontoken modified creature",
    ),
    (
        " counters from each creature",
        " counters from all creatures",
    ),
    (" to her owner's ", " to its owner's "),
];

/// Adds the printed wordings of the rows to the wordings of an ability, chained, up to [`LIMIT`] new ones.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { when, .. } = ability else {
        return;
    };
    let json = serde_json::to_string(ability).unwrap_or_default();
    let mut fresh: Vec<String> = texts.clone();
    let mut added = 0;
    while !fresh.is_empty() && added < LIMIT {
        let mut next: Vec<String> = Vec::new();
        for text in &fresh {
            for found in rewrites(text, when, &json) {
                if added < LIMIT && !texts.contains(&found) && !next.contains(&found) {
                    next.push(found);
                    added += 1;
                }
            }
        }
        texts.extend(next.iter().cloned());
        fresh = next;
    }
}

/// Every one-step rewrite of a text.
fn rewrites(text: &str, when: &TriggerPattern, json: &str) -> Vec<String> {
    let mut out: Vec<String> = ROWS
        .iter()
        .filter(|(written, _)| text.contains(written))
        .map(|(written, printed)| text.replacen(written, printed, 1))
        .collect();
    // "each player's end step" is every end step, unless a later "that player" needs the player.
    if let Some(rest) = text.strip_prefix("At the beginning of each player's end step,")
        && ![" that player", " they ", " their "]
            .iter()
            .any(|word| rest.contains(word))
    {
        out.push(format!("At the beginning of the end step,{rest}"));
    }
    if let Some(rest) = text.strip_prefix("Whenever ")
        && once(rest)
    {
        out.push(format!("When {rest}"));
    }
    out.extend(controller_first(text));
    out.extend(controller_last(text));
    out.extend(card_not_token(text));
    out.extend(implied_you(text));
    out.extend(then_and(text));
    out.extend(sacrificed_yours(text));
    out.extend(attached_names(text));
    out.extend(enters_and_whenever(text));
    out.extend(swapped_pairs(text));
    out.extend(super::r6a_effects::rewrites(text, when, json));
    out.extend(super::r6a_more::rewrites(text));
    out
}

/// The ends of the card in an effect: a trigger that ends the card (or is about the card) is printed with "When".
const ENDS: [&str; 9] = [
    "sacrifice ~",
    "destroy ~",
    "exile ~",
    "return ~",
    "flip ~",
    "transform ~",
    "sacrifice it",
    "destroy it",
    "return it",
];

/// Whether "When" says what "Whenever" says for the rest of a trigger: CR 603.1 gives both one meaning, and the cards
/// print "When" for a trigger about the card or one that ends it.
fn once(rest: &str) -> bool {
    rest.starts_with("~ ")
        || rest.starts_with("enchanted ")
        || ENDS.iter().any(|end| rest.contains(end))
}

/// "a creature with flying you control" as "a creature you control with flying".
fn controller_first(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for controller in CONTROLLERS {
        for (end, _) in text.match_indices(controller) {
            let before = &text[..end];
            let Some(start) = QUALITIES
                .iter()
                .filter_map(|quality| before.rfind(quality))
                .max()
            else {
                continue;
            };
            let quality = &text[start..end];
            if quality.contains(',') || quality.contains('.') || quality.contains(" control") {
                continue;
            }
            out.push(format!(
                "{}{controller}{quality}{}",
                &text[..start],
                &text[end + controller.len()..]
            ));
        }
    }
    out
}

/// "a creature you control with flying" as "a creature with flying you control".
fn controller_last(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for controller in CONTROLLERS {
        for (start, _) in text.match_indices(controller) {
            let after = &text[start + controller.len()..];
            let Some(quality) = QUALITIES.iter().find(|quality| after.starts_with(*quality)) else {
                continue;
            };
            let end = [
                after.find(", "),
                after.find(". "),
                after.rfind('.'),
                after.find(" to "),
            ]
            .into_iter()
            .flatten()
            .filter(|end| *end > quality.len())
            .min()
            .unwrap_or(after.len());
            let moved = &after[..end];
            if moved.contains(" you") || moved.contains(" control") {
                continue;
            }
            out.push(format!(
                "{}{moved}{controller}{}",
                &text[..start],
                &after[end..]
            ));
        }
    }
    out
}

/// "a nontoken land card" as "a land card": a card is never a token.
fn card_not_token(text: &str) -> Vec<String> {
    text.match_indices(" nontoken ")
        .filter(|(at, _)| {
            let after = &text[at + " nontoken ".len()..];
            after
                .split(' ')
                .nth(1)
                .is_some_and(|word| word.starts_with("card"))
        })
        .map(|(at, _)| format!("{} {}", &text[..at], &text[at + " nontoken ".len()..]))
        .collect()
}

/// Whether the words open with "you" or an imperative verb: the doer is the ability's controller.
fn done_by_you(words: &str) -> bool {
    let words = words.strip_prefix("then ").unwrap_or(words);
    words.starts_with("you ")
        || IMPERATIVE.iter().any(|verb| {
            words
                .strip_prefix(verb)
                .is_some_and(|rest| rest.starts_with(' '))
        })
}

/// ", you draw a card" as ", draw a card", and "you gain 1 life and you scry 1" as "you gain 1 life and scry 1".
fn implied_you(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for verb in IMPERATIVE {
        let said = format!(", you {verb} ");
        if text.contains(&said) {
            out.push(text.replacen(&said, &format!(", {verb} "), 1));
        }
        let joined = format!(" and you {verb} ");
        for (at, _) in text.match_indices(&joined) {
            let before = &text[..at];
            let opens = before
                .rfind(", ")
                .map(|comma| comma + 2)
                .max(before.rfind(". ").map(|stop| stop + 2))
                .unwrap_or(0);
            if done_by_you(&before[opens..]) {
                out.push(format!(
                    "{before} and {verb} {}",
                    &text[at + joined.len()..]
                ));
            }
        }
    }
    out
}

/// "A, then B" as "A and B" when nothing before it in the sentence is optional or repeated; "~ gets +1/+0 until end of
/// turn, then ~ can't be blocked" also says the card once.
fn then_and(text: &str) -> Vec<String> {
    let Some(at) = text.find(", then ") else {
        return Vec::new();
    };
    let (before, after) = (&text[..at], &text[at + ", then ".len()..]);
    let sentence = &before[before.rfind(". ").map_or(0, |stop| stop + 2)..];
    if ["may ", "unless", "for each", "If ", "if ", "up to"]
        .iter()
        .any(|word| sentence.contains(word))
        || after.contains(", then ")
        || after.starts_with("if ")
    {
        return Vec::new();
    }
    let mut out = vec![format!("{before} and {after}")];
    let clause = &before[before.rfind(", ").map_or(0, |comma| comma + 2)..];
    if let Some(rest) = after.strip_prefix("~ ")
        && clause.starts_with("~ ")
    {
        out.push(format!("{before} and {rest}"));
    }
    out
}

/// "you sacrifice a token you control" and "a player sacrifices a token you control" as "you sacrifice a token": only a
/// permanent's controller sacrifices it (CR 701.17a).
fn sacrificed_yours(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for said in ["Whenever you sacrifice ", "Whenever a player sacrifices "] {
        let Some(rest) = text.strip_prefix(said) else {
            continue;
        };
        let Some(end) = rest.find(',') else {
            continue;
        };
        if let Some(bare) = rest[..end].strip_suffix(" you control") {
            out.push(format!("Whenever you sacrifice {bare}{}", &rest[end..]));
        }
    }
    out
}

/// The words before "enchanted creature" that make it a kind of creature rather than the one the card is attached to.
const KINDS: &[&str] = &[
    "each",
    "an",
    "a",
    "target",
    "another",
    "other",
    "all",
    "more",
    "no",
    "tapped",
    "untapped",
    "attacking",
    "blocking",
];

/// The names of the permanent the card is attached to: an Aura names it by what it enchants, an Equipment "equipped
/// creature"; the writers give the first two.
const ATTACHED: [&str; 5] = [
    "enchanted creature",
    "equipped creature",
    "enchanted permanent",
    "enchanted land",
    "enchanted artifact",
];

/// The permanent the card is attached to under its other names, and "a player taps it for mana" as "it is tapped for
/// mana".
fn attached_names(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for written in &ATTACHED[..2] {
        let attached = text.match_indices(written).all(|(at, _)| {
            let previous = text[..at]
                .split_whitespace()
                .next_back()
                .unwrap_or_default();
            let next = text[at + written.len()..].chars().next();
            !KINDS.contains(&previous) && next.is_none_or(|c| !c.is_alphanumeric())
        });
        if !text.contains(written) || !attached {
            continue;
        }
        out.extend(
            ATTACHED
                .iter()
                .filter(|name| *name != written)
                .map(|name| text.replace(written, name)),
        );
        let tapped = format!("Whenever a player taps {written} for mana,");
        if let Some(rest) = text.strip_prefix(&tapped) {
            out.push(format!("Whenever {written} is tapped for mana,{rest}"));
        }
    }
    out
}

/// "When ~ enters or an Army you control attacks" as "When ~ enters and whenever an Army you control attacks".
fn enters_and_whenever(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for head in ["When ~ enters or ", "Whenever ~ enters or "] {
        if let Some(rest) = text.strip_prefix(head)
            && !rest.starts_with("attacks")
            && !rest.starts_with("dies")
        {
            out.push(format!("When ~ enters and whenever {rest}"));
        }
    }
    if let Some(rest) = text.strip_prefix("Whenever you cast ")
        && let Some(at) = rest.find(" spell or ")
        && rest[..at].split_whitespace().count() <= 2
    {
        out.push(format!(
            "Whenever you cast {} spell and whenever {}",
            &rest[..at],
            &rest[at + " spell or ".len()..]
        ));
    }
    out
}

/// "creature or artifact" as "artifact or creature", "nonland, noncreature" as "noncreature, nonland" and "white and
/// green" as "green and white": the order of a list of qualities says nothing.
fn swapped_pairs(text: &str) -> Vec<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out = Vec::new();
    for at in 0..words.len().saturating_sub(2) {
        let (first, joiner, second) = (words[at], words[at + 1], words[at + 2]);
        let bare = second.trim_end_matches([',', '.']);
        let tail = &second[bare.len()..];
        let pair =
            matches!(joiner, "or" | "and") && SWAPPED.contains(&first) && SWAPPED.contains(&bare);
        let nons = first.starts_with("non")
            && first.ends_with(',')
            && joiner.starts_with("non")
            && joiner.len() > 3
            && joiner.chars().all(|c| c.is_alphanumeric() || c == '-');
        let swapped = if pair {
            Some(format!("{bare} {joiner} {first}{tail}"))
        } else if nons {
            let one = first.trim_end_matches(',');
            Some(format!("{joiner}, {one} {second}"))
        } else {
            None
        };
        if let Some(swapped) = swapped {
            let mut parts: Vec<String> = words[..at].iter().map(|w| (*w).to_owned()).collect();
            parts.push(swapped);
            parts.extend(words[at + 3..].iter().map(|w| (*w).to_owned()));
            out.push(parts.join(" "));
        }
    }
    out
}
