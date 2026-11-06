//! Oracle wordings of activated and static abilities the writers say another way, each derived from a wording the
//! writers give for the same rule: "you control" before a qualifier ("creatures you control with flying"), colors before
//! the noun ("target white or black creature"), "artifact creatures", an implied "you" after "and", "Exhaust —" for an
//! ability activated only once, "Creatures" for "All creatures", two effects of one subject joined by "and", a land that
//! becomes a creature "that's still a land", a token that "has" its quoted ability in a sentence of its own, and
//! "Enchanted permanent" for the attached permanent.

use crate::ability::{Ability, Limit};
use crate::refs::Ref;

/// The qualifiers a selector may say before or after "you control"; a qualifier ends where "you control" begins.
const QUALIFIERS: [&str; 4] = [
    " with ",
    " without ",
    " of the chosen type",
    " that entered the battlefield this turn",
];

/// The colors a selector names before its noun.
const COLORS: [&str; 5] = ["white", "blue", "black", "red", "green"];

/// The verbs that open an instruction to the ability's controller ("Draw a card and put ...").
const IMPERATIVES: [&str; 9] = [
    "Draw",
    "Exile",
    "Destroy",
    "Return",
    "Create",
    "Untap",
    "Tap",
    "Sacrifice",
    "You",
];

/// The verbs an implied "you" may lead after "and".
const AND_VERBS: [&str; 5] = ["put", "scry", "surveil", "create", "draw"];

/// Adds the Oracle wordings of each row to the wordings of the ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let mut found = Vec::new();
    for text in texts.iter() {
        found.extend(you_control_first(text));
        found.extend(colors_first(text));
        found.extend(artifact_creatures(text));
        found.extend(implied_you(text));
        found.extend(joined(text));
        found.extend(still_a_land(text));
        found.extend(token_has(text));
        found.extend(all_creatures(ability, text));
        found.extend(enchanted_permanent(ability, text));
        found.extend(exhaust(ability, text));
        found.extend(size_first(text));
        found.extend(enchanted_group(ability, text));
        found.extend(also(text));
        found.extend(bare_color(text));
        found.extend(artifact_or_creature(text));
        found.extend(keyword_pairs(text));
        found.extend(loses_first(text));
        found.extend(noun_is_condition(ability, text));
        found.extend(an_hour(text));
        found.extend(pair_swaps(text));
        found.extend(enchanted_object(ability, text));
    }
    if found.is_empty() {
        return;
    }
    let mut seen: std::collections::HashSet<String> = texts.iter().cloned().collect();
    for text in found {
        if seen.insert(text.clone()) {
            texts.push(text);
        }
    }
}

/// "creatures with flying you control" said as "creatures you control with flying", and "creatures your opponents control
/// of the chosen type" as "creatures of the chosen type your opponents control": a selector is a conjunction.
fn you_control_first(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for controller in CONTROLLERS {
        let Some(at) = text.find(controller) else {
            continue;
        };
        let (before, after) = (&text[..at], &text[at + controller.len()..]);
        // The qualifier before the controller.
        let start = QUALIFIERS
            .iter()
            .filter_map(|qualifier| {
                let from = before.rfind(qualifier)?;
                // "of the chosen type" and "that entered ..." end where the controller begins.
                (qualifier.starts_with(" with") || before.ends_with(qualifier)).then_some(from)
            })
            .min();
        if let Some(start) = start
            && plain_qualifier(&before[start..])
        {
            found.push(format!(
                "{}{controller}{}{after}",
                &before[..start],
                &before[start..]
            ));
        }
        // The qualifier after the controller, up to the verb or the end of the phrase.
        let Some(qualifier) = QUALIFIERS.iter().find(|qualifier| {
            after.starts_with(qualifier.trim_end())
                && after[qualifier.trim_end().len()..].starts_with([' ', '.', ','])
        }) else {
            continue;
        };
        let end = if qualifier.starts_with(" with") {
            QUALIFIED_ENDS
                .iter()
                .filter_map(|word| after.find(word))
                .min()
        } else {
            Some(qualifier.trim_end().len())
        };
        if let Some(end) = end
            && plain_qualifier(&after[..end])
        {
            found.push(format!(
                "{before}{}{controller}{}",
                &after[..end],
                &after[end..]
            ));
        }
    }
    found
}

/// The controllers a selector names last.
const CONTROLLERS: [&str; 2] = [" you control", " your opponents control"];

/// The words that end a "with" qualifier said after the controller: the verb of the sentence.
const QUALIFIED_ENDS: [&str; 7] = [
    " get ", " gets ", " have ", " has ", " can't ", " gain ", " gains ",
];

/// Whether a qualifier is one phrase: no quote, clause break or player of its own.
fn plain_qualifier(qualifier: &str) -> bool {
    const VERBS: [&str; 22] = [
        "can't", "can", "get", "gets", "gain", "gains", "has", "have", "deal", "deals", "attack",
        "attacks", "block", "blocks", "is", "are", "become", "becomes", "lose", "loses", "target",
        "each",
    ];
    !qualifier.contains(['"', ',', '.', ':'])
        && !qualifier.contains(" you")
        && !qualifier.is_empty()
        && !qualifier.split(' ').any(|word| VERBS.contains(&word))
}

/// "creature that's white or black" said as "white or black creature".
fn colors_first(text: &str) -> Option<String> {
    let at = text.find("creature that's ")?;
    let rest = &text[at + "creature that's ".len()..];
    let (first, tail) = rest.split_once(" or ")?;
    let second = tail.split([' ', '.', ',']).next()?;
    if !COLORS.contains(&first) || !COLORS.contains(&second) {
        return None;
    }
    let after = &tail[second.len()..];
    Some(format!(
        "{}{first} or {second} creature{after}",
        &text[..at]
    ))
}

/// "creature artifacts" said as "artifact creatures": both types, in the order the card says them.
fn artifact_creatures(text: &str) -> Option<String> {
    text.contains(" creature artifact").then(|| {
        text.replace(" creature artifacts", " artifact creatures")
            .replace(" creature artifact ", " artifact creature ")
    })
}

/// "Draw a card and you put a counter ..." said without the "you" the instruction already implies.
fn implied_you(text: &str) -> Option<String> {
    let at = text.find(" and you ")?;
    let verb = text[at + " and you ".len()..].split(' ').next()?;
    if !AND_VERBS.contains(&verb) {
        return None;
    }
    // "the sacrificed creature" names a sacrifice of the cost; without one the wording is not this rule's.
    if text.contains(" sacrificed ")
        && !text
            .split(": ")
            .next()
            .is_some_and(|cost| cost.contains("Sacrifice"))
    {
        return None;
    }
    let clause_start = text[..at].rfind(": ").map_or(0, |colon| colon + 2);
    let clause = &text[clause_start..at];
    let opener = clause.split(' ').next()?;
    if !IMPERATIVES.contains(&opener) || clause.contains(['.', ',']) {
        return None;
    }
    Some(text.replacen(" and you ", " and ", 1))
}

/// Two effects of one subject in one sentence: "~ gets +2/+0 until end of turn, then ~ deals 1 damage to you" said as
/// "~ gets +2/+0 until end of turn and deals 1 damage to you", and "~ gains flying until end of turn, then ~ loses trample
/// until end of turn" as "~ gains flying and loses trample until end of turn".
fn joined(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let Some((cost, effect)) = text.split_once(": ") else {
        return found;
    };
    let effect = effect.strip_suffix('.').unwrap_or(effect);
    for (subject, again) in [("~ ", "~ "), ("Target creature ", "it ")] {
        let Some(rest) = effect.strip_prefix(subject) else {
            continue;
        };
        let Some((first, second)) = rest.split_once(" until end of turn, then ") else {
            continue;
        };
        let Some(second) = second.strip_prefix(again) else {
            continue;
        };
        if [first, second]
            .iter()
            .any(|part| part.contains(['.', ',', ':']) || part.contains(" then "))
        {
            continue;
        }
        match second.strip_suffix(" until end of turn") {
            Some(second) if !second.contains(" until ") => {
                found.push(format!(
                    "{cost}: {subject}{first} and {second} until end of turn."
                ));
            }
            _ if !second.contains(" until end of turn") => {
                found.push(format!(
                    "{cost}: {subject}{first} until end of turn and {second}."
                ));
            }
            _ => {}
        }
    }
    found
}

/// "becomes a 2/2 creature. It's still a land." said as "becomes a 2/2 creature that's still a land."
fn still_a_land(text: &str) -> Option<String> {
    text.contains(" creature. It's still a land").then(|| {
        text.replacen(
            " creature. It's still a land",
            " creature that's still a land",
            1,
        )
    })
}

/// A token with a quoted ability said in a sentence of its own: "Create a white Avatar creature token. It has "..."."
fn token_has(text: &str) -> Option<String> {
    let at = text.find(" with \"")?;
    let token = text[..at].rfind(" token")?;
    // The token, or the token and its name: "token named Goldmeadow Harrier".
    let name = &text[token + " token".len()..at];
    if !(name.is_empty() || name.starts_with(" named ")) || name.contains(['.', '"', ',']) {
        return None;
    }
    let made = text[..token]
        .rfind("Create a")
        .map(|start| &text[start..token])?;
    if made.contains(['.', '"']) || !text.ends_with("\".") && !text.ends_with('"') {
        return None;
    }
    let quoted = &text[at + " with ".len()..];
    let quoted = quoted.strip_suffix('.').unwrap_or(quoted);
    // One quoted ability that runs to the end of the text.
    if quoted.matches('"').count() != 2 {
        return None;
    }
    Some(format!("{}. It has {quoted}", &text[..at]))
}

/// "All creatures can't block." said as "Creatures can't block.": a static ability of every creature.
fn all_creatures(ability: &Ability, text: &str) -> Option<String> {
    let Ability::Static {
        affected: Ref::Each(_),
        ..
    } = ability
    else {
        return None;
    };
    text.strip_prefix("All creatures ")
        .map(|rest| format!("Creatures {rest}"))
}

/// A static ability of the attached permanent: the noun after "Enchanted" repeats what the Aura enchants, which its
/// enchant keyword says.
fn enchanted_permanent(ability: &Ability, text: &str) -> Option<String> {
    let Ability::Static {
        affected: Ref::Attached,
        ..
    } = ability
    else {
        return None;
    };
    text.strip_prefix("Enchanted creature ")
        .map(|rest| format!("Enchanted permanent {rest}"))
}

/// An ability activated only once (CR 702.177, exhaust): "Exhaust — {cost}: effect."
fn exhaust(ability: &Ability, text: &str) -> Option<String> {
    let Ability::Activated {
        limit: Some(Limit::OncePerGame),
        ..
    } = ability
    else {
        return None;
    };
    let body = text.strip_suffix(" Activate only once.")?;
    if body.contains("Activate ") || !body.contains(": ") {
        return None;
    }
    Some(format!("Exhaust \u{2014} {body}"))
}

/// "target creature with power 1 with toughness 1" said as "target 1/1 creature".
fn size_first(text: &str) -> Option<String> {
    let at = text.find("creature with power ")?;
    let rest = &text[at + "creature with power ".len()..];
    let (power, rest) = rest.split_once(" with toughness ")?;
    let toughness = rest.split([' ', '.', ',']).next()?;
    if !digits(power) || !digits(toughness) {
        return None;
    }
    let after = &rest[toughness.len()..];
    Some(format!(
        "{}{power}/{toughness} creature{after}",
        &text[..at]
    ))
}

/// Whether the word is a number in digits.
fn digits(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_ascii_digit())
}

/// "Each enchanted creature you control gets +1/+1." said as "Creatures you control that are enchanted get +1/+1."
fn enchanted_group(ability: &Ability, text: &str) -> Option<String> {
    let Ability::Static {
        affected: Ref::Each(_),
        ..
    } = ability
    else {
        return None;
    };
    let rest = text.strip_prefix("Each enchanted creature you control gets ")?;
    (!rest.contains(" and ") && !rest.contains(". "))
        .then(|| format!("Creatures you control that are enchanted get {rest}"))
}

/// "also" before the verb of a later sentence ("White creatures you control also gain first strike"), a word without
/// meaning of its own.
fn also(text: &str) -> Vec<String> {
    const VERBS: [&str; 6] = ["gain ", "gains ", "get ", "gets ", "discards ", "draws "];
    let Some(first_end) = text.find(". ") else {
        return Vec::new();
    };
    VERBS
        .iter()
        .filter_map(|verb| {
            let at = first_end + text[first_end..].find(&format!(" {verb}"))? + 1;
            Some(format!("{}also {}", &text[..at], &text[at..]))
        })
        .collect()
}

/// "becomes a white until end of turn" said as "becomes white until end of turn": a color alone has no article.
fn bare_color(text: &str) -> Option<String> {
    COLORS.iter().find_map(|color| {
        let written = format!(" becomes a {color} until end of turn");
        text.contains(&written)
            .then(|| text.replacen(&written, &format!(" becomes {color} until end of turn"), 1))
    })
}

/// "a hour counter" said with the article of the vowel sound: "an hour counter".
fn an_hour(text: &str) -> Option<String> {
    text.contains(" a hour counter")
        .then(|| text.replace(" a hour counter", " an hour counter"))
}

/// Two parts of one list in the other order, the same list: "white and green" as "green and white", "Goblins and Orcs"
/// as "Orcs and Goblins", "gains lifelink and vigilance" as "gains vigilance and lifelink", "get +X/+X and gain flying"
/// as "gain flying and get +X/+X".
fn pair_swaps(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (at, _) in text.match_indices(" and ") {
        let before = &text[..at];
        let after = &text[at + " and ".len()..];
        let left_start = before.rfind(' ').map_or(0, |space| space + 1);
        let left = &before[left_start..];
        let right = after.split([' ', '.', ',']).next().unwrap_or_default();
        let rest = &after[right.len()..];
        let head = &before[..left_start];
        let colors = COLORS.contains(&left) && COLORS.contains(&right);
        // Two subtypes before "you control": "Other Orcs and Goblins you control".
        let capital = |word: &str| {
            word.len() > 1
                && word.starts_with(|c: char| c.is_ascii_uppercase())
                && word.ends_with('s')
                && word.chars().all(|c| c.is_ascii_alphabetic())
        };
        let subtypes = capital(left) && capital(right) && rest.starts_with(" you control");
        let keywords = head.ends_with("gains ") || head.ends_with("gain ");
        let plain = |word: &str| !word.is_empty() && word.chars().all(|c| c.is_ascii_lowercase());
        if colors
            || subtypes
            || (keywords
                && plain(left)
                && plain(right)
                && KEYWORDS.contains(&left)
                && KEYWORDS.contains(&right))
        {
            found.push(format!("{head}{right} and {left}{rest}"));
        }
    }
    // "get +X/+X and gain flying" as "gain flying and get +X/+X".
    if let Some(at) = text.find(" get +")
        && let Some(gain) = text[at..].find(" and gain ")
    {
        let stats = &text[at + " get ".len()..at + gain];
        let tail = &text[at + gain + " and gain ".len()..];
        let keyword = tail.split([' ', '.', ',']).next().unwrap_or_default();
        if !stats.contains(' ') && KEYWORDS.contains(&keyword) {
            found.push(format!(
                "{} gain {keyword} and get {stats}{}",
                &text[..at],
                &tail[keyword.len()..]
            ));
        }
    }
    found
}

/// The one-word keywords a pair of keywords is swapped in.
const KEYWORDS: [&str; 12] = [
    "flying",
    "vigilance",
    "lifelink",
    "trample",
    "haste",
    "reach",
    "deathtouch",
    "menace",
    "indestructible",
    "hexproof",
    "defender",
    "shroud",
];

/// "tap enchanted creature" said as "tap enchanted permanent" in an ability of an Aura: the noun repeats what the Aura
/// enchants.
fn enchanted_object(ability: &Ability, text: &str) -> Option<String> {
    if !matches!(
        ability,
        Ability::Triggered { .. } | Ability::Activated { .. }
    ) || !text.contains("ap enchanted creature")
    {
        return None;
    }
    let mut said = text.to_owned();
    for verb in ["tap", "untap", "Tap", "Untap"] {
        said = said.replace(
            &format!("{verb} enchanted creature"),
            &format!("{verb} enchanted permanent"),
        );
    }
    (said != text).then_some(said)
}

/// "creature or artifact" said as "artifact or creature": either type.
fn artifact_or_creature(text: &str) -> Option<String> {
    text.contains(" creature or artifact")
        .then(|| text.replacen(" creature or artifact", " artifact or creature", 1))
}

/// "has lifelink and has flying" said as "has flying and lifelink" or "has lifelink and flying": two keywords.
fn keyword_pairs(text: &str) -> Vec<String> {
    let Some(at) = text.find(" has ") else {
        return Vec::new();
    };
    let rest = &text[at + " has ".len()..];
    let Some((first, tail)) = rest.split_once(" and has ") else {
        return Vec::new();
    };
    let end = tail.find(['.', ',']).unwrap_or(tail.len());
    let second = &tail[..end];
    let plain = |word: &str| {
        !word.is_empty()
            && word.split(' ').count() <= 2
            && word.chars().all(|c| c.is_ascii_lowercase() || c == ' ')
    };
    if !plain(first) || !plain(second) {
        return Vec::new();
    }
    let (head, after) = (&text[..at], &tail[end..]);
    vec![
        format!("{head} has {first} and {second}{after}"),
        format!("{head} has {second} and {first}{after}"),
    ]
}

/// "has base power and toughness 1/1 and loses all abilities" said as "loses all abilities and has base power and
/// toughness 1/1": the layers apply both whatever order the sentence names them in.
fn loses_first(text: &str) -> Option<String> {
    const BASE: &str = " has base power and toughness ";
    let at = text.find(BASE)?;
    let rest = &text[at + BASE.len()..];
    let (size, tail) = rest.split_once(" and loses all abilities")?;
    let (power, toughness) = size.split_once('/')?;
    if !digits(power) || !digits(toughness) {
        return None;
    }
    Some(format!(
        "{} loses all abilities and has base power and toughness {size}{tail}",
        &text[..at]
    ))
}

/// "Enchanted land has ... as long as it is a land.": the noun of the subject says the condition already.
fn noun_is_condition(ability: &Ability, text: &str) -> Option<String> {
    let Ability::Static {
        affected: Ref::Attached,
        condition: Some(condition),
        ..
    } = ability
    else {
        return None;
    };
    let held = serde_json::to_value(condition).ok()?;
    let (noun, article) = [("land", "a"), ("creature", "a"), ("artifact", "an")]
        .into_iter()
        .find(|(noun, _)| {
            let kind = format!("{}{}", noun[..1].to_uppercase(), &noun[1..]);
            held == serde_json::json!({"Matches": {"selector": [{"Type": kind}], "subject": "Attached"}})
        })?;
    text.strip_prefix(&format!("Enchanted {noun} "))?;
    let body = text.strip_suffix(&format!(" as long as it is {article} {noun}."))?;
    Some(format!("{body}."))
}
