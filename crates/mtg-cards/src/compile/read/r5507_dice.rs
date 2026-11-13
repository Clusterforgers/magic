//! r55-07 rolls of a number of dice worked out as they happen (`RollDice` with `count`), read and written whole:
//! "When ~ enters, roll X six-sided dice. For each odd result, create a 1/1 white Clown Robot artifact creature token.
//! For each even result, put a +1/+1 counter on ~.", "{1}, {T}: Roll X six-sided dice, where X is the number of
//! Bobbleheads you control. Create a tapped Treasure token for each even result. If you rolled 6 exactly seven times,
//! you win the game." and "As ~ enters, roll X d6. It enters with a number of +1/+1 counters on it equal to the total
//! of those results." The head of the ability and the steps after the roll are read and written through twins by the
//! other readers and writers.

use super::amounts::{count_word, number_word};
use super::counters::kind_text;
use super::tokens::same_text;
use crate::ability::Ability;
use crate::compile::ClauseKind;
use crate::effect::Effect;
use crate::refs::{Ref, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::{Amount, Cmp, Condition, CounterKind, DieResult};

/// The step the twins put in place of the roll and its steps.
const R5507_TWIN: &str = "draw a card.";

/// The effects of an ability with a head (a trigger or a cost) before them.
fn effects_of(ability: &mut Ability) -> Option<&mut Vec<Effect>> {
    match ability {
        Ability::Triggered { effects, .. } | Ability::Activated { effects, .. } => Some(effects),
        _ => None,
    }
}

/// The roll that starts the steps: its faces and its number of dice.
fn roll_of(effect: &Effect) -> Option<(u32, &Amount)> {
    match effect {
        Effect::RollDice {
            who: Ref::You,
            sides,
            dice: 1,
            choose: false,
            plus,
            results,
            count: Some(count),
        } if plus.is_zero() && results.is_empty() => Some((*sides, count)),
        _ => None,
    }
}

/// The number of an effect that its result may stand for, and the effect with that number set.
fn with_count(effect: &Effect, count: Amount) -> Option<(Amount, Effect)> {
    let mut twin = effect.clone();
    let (Effect::CreateToken { count: slot, .. } | Effect::PutCounters { count: slot, .. }) =
        &mut twin
    else {
        return None;
    };
    let old = std::mem::replace(slot, count);
    Some((old, twin))
}

/// The words of a number of results a step is done for.
const fn results_words(which: DieResult) -> Option<&'static str> {
    match which {
        DieResult::Odd => Some("odd result"),
        DieResult::Even => Some("even result"),
        _ => None,
    }
}

/// The sentence with its first letter in upper case.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The sentence with its first letter in lower case.
fn uncapitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}

/// The wordings of the roll: "roll X six-sided dice" (also "roll X d6"), with "where X is ..." for a count.
fn roll_texts(sides: u32, count: &Amount) -> Vec<String> {
    let mut dice = Vec::new();
    let faces = count_word(sides).map(|word| format!("{word}-sided dice"));
    match count {
        Amount::X => {
            dice.extend(faces.iter().map(|faces| format!("roll X {faces}")));
            dice.push(format!("roll X d{sides}"));
        }
        other => {
            let Some(words) = super::defining::counting_text(other) else {
                return Vec::new();
            };
            dice.extend(
                faces
                    .iter()
                    .map(|faces| format!("roll X {faces}, where X is {words}")),
            );
            dice.push(format!("roll X d{sides}, where X is {words}"));
        }
    }
    dice
}

/// The wordings of one step after the roll, as a sentence.
fn step_texts(effect: &Effect, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    if let Effect::If {
        condition:
            Condition::Compare {
                left: Amount::DieResult(DieResult::Times(result)),
                op: Cmp::Eq,
                right: Amount::Fixed(times),
            },
        then,
        otherwise,
    } = effect
        && otherwise.is_empty()
        && matches!(then.as_slice(), [Effect::WinGame { who: Ref::You }])
        && let Some(times) = u32::try_from(*times).ok().and_then(count_word)
    {
        return vec![format!(
            "If you rolled {result} exactly {times} times, you win the game."
        )];
    }
    let Some((Amount::DieResult(which), twin)) = with_count(effect, Amount::Fixed(1)) else {
        return Vec::new();
    };
    let Some(words) = results_words(which) else {
        return Vec::new();
    };
    let spell = Ability::Spell {
        targets: Vec::new(),
        effects: vec![twin],
    };
    let mut texts = Vec::new();
    for text in written(&spell) {
        let Some(body) = text.strip_suffix('.') else {
            continue;
        };
        texts.push(format!("For each {words}, {}.", uncapitalized(body)));
        texts.push(format!("{body} for each {words}."));
    }
    texts
}

/// The wordings of the head of a triggered or activated ability, up to its steps ("When ~ enters, ", "{1}, {T}: ").
fn head_texts(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let mut twin = ability.clone();
    let Some(effects) = effects_of(&mut twin) else {
        return Vec::new();
    };
    *effects = vec![Effect::Draw {
        who: Ref::You,
        count: Amount::one(),
        bind: None,
    }];
    written(&twin)
        .into_iter()
        .filter_map(|text| {
            let cut = text.len().checked_sub(R5507_TWIN.len())?;
            let (head, tail) = text.split_at(cut);
            tail.eq_ignore_ascii_case(R5507_TWIN)
                .then(|| head.to_owned())
        })
        .collect()
}

/// Every wording of a roll and its steps after a head.
fn whole_texts(
    head: &str,
    (sides, count): (u32, &Amount),
    steps: &[Effect],
    written: fn(&Ability) -> Vec<String>,
) -> Vec<String> {
    let first = |roll: &str| {
        if head.is_empty() || head.ends_with(": ") {
            capitalized(roll)
        } else {
            roll.to_owned()
        }
    };
    let mut texts: Vec<String> = roll_texts(sides, count)
        .iter()
        .map(|roll| format!("{head}{}.", first(roll)))
        .collect();
    for step in steps {
        let sentences = step_texts(step, written);
        if sentences.is_empty() {
            return Vec::new();
        }
        texts = texts
            .iter()
            .flat_map(|text| sentences.iter().map(move |step| format!("{text} {step}")))
            .collect();
    }
    texts
}

/// The counters a permanent enters with equal to the total of the dice ("As ~ enters, roll X d6. It enters with a
/// number of +1/+1 counters on it equal to the total of those results.").
fn entering_texts(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when: ReplaceWhen::Enters { what },
        by:
            ReplaceBy::AsEntersWith {
                effects,
                counter,
                count: Amount::DieResult(DieResult::Total),
            },
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    let ([Effect::RollDice { .. }], Some(kind)) = (effects.as_slice(), kind_text(counter)) else {
        return Vec::new();
    };
    if what.0 != [crate::refs::Pred::This] {
        return Vec::new();
    }
    let Some((sides, count)) = effects.first().and_then(roll_of) else {
        return Vec::new();
    };
    roll_texts(sides, count)
        .iter()
        .map(|roll| {
            format!(
                "As ~ enters, {roll}. It enters with a number of {kind} counters on it equal to the total of those results."
            )
        })
        .collect()
}

/// The wordings of an ability whose steps start with a roll of a number of dice worked out as it happens.
pub(super) fn r5507_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let mut found = entering_texts(ability);
    let mut copy = ability.clone();
    if let Some(effects) = effects_of(&mut copy)
        && let Some((roll, steps)) = effects.split_first()
        && let Some(roll) = roll_of(roll)
    {
        for head in head_texts(ability, written) {
            found.extend(whole_texts(&head, roll, steps, written));
        }
    }
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The roll a sentence starts with ("roll X six-sided dice", "roll X d6, where X is ..."): its faces, its number of
/// dice and the rest of the text after the roll's sentence; `read` reads the number of a "where X is" as a spell twin.
fn read_roll(
    text: &str,
    read: &mut dyn FnMut(&str, ClauseKind) -> Option<Ability>,
) -> Option<(u32, Amount, String)> {
    let rest = text
        .strip_prefix("roll X ")
        .or_else(|| text.strip_prefix("Roll X "))?;
    let (sides, rest) = if let Some(rest) = rest.strip_prefix('d') {
        let end = rest.find(|c: char| !c.is_ascii_digit())?;
        (rest[..end].parse::<u32>().ok()?, &rest[end..])
    } else {
        let (word, rest) = rest.split_once("-sided dice")?;
        (number_word(word)?, rest)
    };
    if let Some(rest) = rest.strip_prefix(". ") {
        return Some((sides, Amount::X, rest.to_owned()));
    }
    let rest = rest.strip_prefix(", where X is ")?;
    let (words, rest) = rest.split_once(". ")?;
    let twin = read(&format!("Draw cards equal to {words}."), ClauseKind::Spell)?;
    let Ability::Spell { effects, .. } = twin else {
        return None;
    };
    let [Effect::Draw { count, .. }] = effects.as_slice() else {
        return None;
    };
    Some((sides, count.clone(), rest.to_owned()))
}

/// The steps after the roll: "For each odd result, S.", "S for each even result." and "If you rolled 6 exactly seven
/// times, you win the game.", one sentence each.
fn read_steps(
    text: &str,
    read: &mut dyn FnMut(&str, ClauseKind) -> Option<Ability>,
) -> Option<Vec<Effect>> {
    let mut steps = Vec::new();
    for sentence in text.split_inclusive(". ").map(str::trim) {
        if let Some(rest) = sentence.strip_prefix("If you rolled ") {
            let (result, rest) = rest.split_once(" exactly ")?;
            let times = rest.strip_suffix(" times, you win the game.")?;
            steps.push(Effect::If {
                condition: Condition::Compare {
                    left: Amount::DieResult(DieResult::Times(result.parse().ok()?)),
                    op: Cmp::Eq,
                    right: Amount::Fixed(i32::try_from(number_word(times)?).ok()?),
                },
                then: vec![Effect::WinGame { who: Ref::You }],
                otherwise: Vec::new(),
            });
            continue;
        }
        let parity = |words: &str| match words {
            "odd result" => Some(DieResult::Odd),
            "even result" => Some(DieResult::Even),
            _ => None,
        };
        let (which, body) = if let Some(rest) = sentence.strip_prefix("For each ") {
            let (words, body) = rest.split_once(", ")?;
            (parity(words)?, capitalized(body))
        } else {
            let (body, words) = sentence.rsplit_once(" for each ")?;
            (parity(words.strip_suffix('.')?)?, format!("{body}."))
        };
        let Ability::Spell { effects, .. } = read(&body, ClauseKind::Spell)? else {
            return None;
        };
        let [effect] = effects.as_slice() else {
            return None;
        };
        let (old, effect) = with_count(effect, Amount::DieResult(which))?;
        old.is_one().then_some(())?;
        steps.push(effect);
    }
    Some(steps)
}

/// The ability of a roll of a number of dice the clause words, read whole and written back the same; `read` reads a
/// twin of the head or a step.
pub(super) fn r5507_read(
    text: &str,
    kind: ClauseKind,
    mut read: impl FnMut(&str, ClauseKind) -> Option<Ability>,
) -> Option<Ability> {
    if !text.contains("roll X ") && !text.contains("Roll X ") {
        return None;
    }
    let entering = ["As ~ enters, ", "As this creature enters, "]
        .iter()
        .find_map(|head| text.strip_prefix(head));
    let found = if let Some(rest) = entering {
        let (sides, count, rest) = read_roll(rest, &mut read)?;
        let tail = rest.strip_prefix("It enters with a number of ")?;
        let (kind, _) = tail.split_once(" counters on it equal to the total of those results")?;
        let counter: CounterKind =
            serde_json::from_value(serde_json::Value::String(kind.to_owned())).ok()?;
        Ability::Replacement {
            when: ReplaceWhen::Enters {
                what: crate::refs::Selector(vec![crate::refs::Pred::This]),
            },
            by: ReplaceBy::AsEntersWith {
                effects: vec![roll(sides, count)],
                counter,
                count: Amount::DieResult(DieResult::Total),
            },
            condition: None,
            from: ZoneKind::Battlefield,
        }
    } else {
        let at = text.find("roll X ").or_else(|| text.find("Roll X "))?;
        let (head, rest) = text.split_at(at);
        let mut twin = read(
            &format!(
                "{head}{}",
                if head.ends_with(": ") {
                    "Draw a card."
                } else {
                    R5507_TWIN
                }
            ),
            kind,
        )?;
        let (sides, count, rest) = read_roll(rest, &mut read)?;
        let steps = read_steps(&rest, &mut read)?;
        let effects = effects_of(&mut twin)?;
        *effects = std::iter::once(roll(sides, count)).chain(steps).collect();
        twin
    };
    let mut written = Vec::new();
    r5507_wordings(&found, super::render::written_variants, &mut written);
    written
        .iter()
        .any(|written| same_text(written, text))
        .then_some(found)
}

/// A roll of `count` dice of `sides` faces by you.
fn roll(sides: u32, count: Amount) -> Effect {
    Effect::RollDice {
        who: Ref::You,
        sides,
        dice: 1,
        choose: false,
        plus: Amount::zero(),
        results: Vec::new(),
        count: Some(count),
    }
}
