//! Oracle wordings of rules the renderers write another way (fix26): "you may A. If you do, B" for a choice of several
//! steps (`Performed` asks whether the choice was accepted, so both are one rule); "an additional +1/+1 counter" put
//! by a trigger on +1/+1 counters; "another" for the second card of a sequence whose first left the zone; "a commander"
//! for the casts of your commanders; "two red cards from your hand" for an alternative cost the spell on the stack
//! cannot pay itself; the name of a token before its keywords; "the power of" a target that doubles; "its owner" for the
//! player who controls a card in a graveyard; and "your combat step" for the beginning of combat on your turn.

use super::super::variants;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Condition;

/// Adds the oracle wordings of the ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if let Some(split) = performed_split(ability) {
        texts.extend(variants(&split));
    }
    let found: Vec<String> = texts
        .iter()
        .flat_map(|text| {
            [
                additional(text),
                another(text),
                text.contains("you've cast your commander from").then(|| {
                    text.replace(
                        "you've cast your commander from",
                        "you've cast a commander from",
                    )
                }),
                alternative_hand(text),
                named_first(text),
                power_of(text),
                graveyard_owner(text),
                text.contains("At the beginning of combat on your turn")
                    .then(|| {
                        text.replace(
                            "At the beginning of combat on your turn",
                            "At the beginning of your combat step",
                        )
                    }),
            ]
        })
        .flatten()
        .filter(|text| !texts.contains(text))
        .collect();
    texts.extend(found);
}

/// The ability with "you may A and B" as "you may A. If you do, B": the first step alone in the choice and the rest
/// under `Performed`.
fn performed_split(ability: &Ability) -> Option<Ability> {
    let [
        Effect::May {
            who: Ref::You,
            effects,
        },
    ] = ability.effects()
    else {
        return None;
    };
    let [first, rest @ ..] = effects.as_slice() else {
        return None;
    };
    if rest.is_empty() {
        return None;
    }
    let split = vec![
        Effect::May {
            who: Ref::You,
            effects: vec![first.clone()],
        },
        Effect::If {
            condition: Condition::Performed,
            then: rest.to_vec(),
            otherwise: Vec::new(),
        },
    ];
    let mut changed = ability.clone();
    match &mut changed {
        Ability::Spell { effects, .. }
        | Ability::Activated { effects, .. }
        | Ability::Triggered { effects, .. } => *effects = split,
        _ => return None,
    }
    Some(changed)
}

/// "Whenever one or more +1/+1 counters are put on ~, put a +1/+1 counter on it" as "put an additional +1/+1 counter".
fn additional(text: &str) -> Option<String> {
    let (_, after) = text.split_once("one or more ")?;
    let (kind, _) = after.split_once(" counters are put on ")?;
    let more = format!(", put an additional {kind} counter on it");
    [" it", " ~"].iter().find_map(|end| {
        let one = format!(", put a {kind} counter on{end}");
        text.contains(&one).then(|| text.replacen(&one, &more, 1))
    })
}

/// The second "a ... card" of a sequence ("..., then return a creature card", ". Then that player discards a card at
/// random") as "another ... card" when the first step named the same card.
fn another(text: &str) -> Option<String> {
    let at = text.find(", then ").or_else(|| text.find(". Then "))?;
    let (head, tail) = text.split_at(at);
    let start = tail.find(" a ")? + 1;
    let words: Vec<&str> = tail[start..].split(' ').take(4).collect();
    let end = words.iter().position(|word| *word == "card")?;
    let phrase = words[..=end].join(" ");
    if !head.contains(&format!("{phrase} ")) && !head.ends_with(&phrase) {
        return None;
    }
    let other = format!("another{}", &phrase[1..]);
    Some(format!(
        "{head}{}{other}{}",
        &tail[..start],
        &tail[start + phrase.len()..]
    ))
}

/// "exile two other red cards from your hand rather than pay ~'s mana cost" without "other": the spell is on the stack
/// as its cost is paid, so no card it exiles from a hand is the spell itself.
fn alternative_hand(text: &str) -> Option<String> {
    let at = text.find(" cards from your hand rather than pay ")?;
    let start = text[..at].rfind("exile ")?;
    let segment = &text[start..at];
    let without = segment.replacen(" other ", " ", 1);
    (without != segment).then(|| format!("{}{without}{}", &text[..start], &text[at..]))
}

/// "a ... token with flying and crew 3 named Zeppelin." as "a ... token named Zeppelin with flying and crew 3.".
fn named_first(text: &str) -> Option<String> {
    let with = text.find(" token with ")? + " token".len();
    let named = with + text[with..].find(" named ")?;
    let end = named + text[named..].find('.')?;
    let name = &text[named..end];
    if name.contains(" with ") {
        return None;
    }
    Some(format!(
        "{}{name}{}{}",
        &text[..with],
        &text[with..named],
        &text[end..]
    ))
}

/// "double target creature you control's power" as "double the power of target creature you control".
fn power_of(text: &str) -> Option<String> {
    let start = text.find("double target ")? + "double ".len();
    let end = start + text[start..].find("'s power")?;
    Some(format!(
        "{}the power of {}{}",
        &text[..start],
        &text[start..end],
        &text[end + "'s power".len()..]
    ))
}

/// "... card from a graveyard. Its controller creates" as "Its owner creates": a card in a graveyard is controlled by its
/// owner.
fn graveyard_owner(text: &str) -> Option<String> {
    const CONTROLLER: &str = " card from a graveyard. Its controller ";
    text.contains(CONTROLLER)
        .then(|| text.replace(CONTROLLER, " card from a graveyard. Its owner "))
}
