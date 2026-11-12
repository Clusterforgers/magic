//! Wordings of pack r30-3c: a static, rule or replacement under a condition written as "As long as C, S." from the
//! wordings of its twin without the condition ("As long as ~ is equipped, it must be blocked if able.", "As long as ~ is
//! tapped, other permanents enter tapped."); a card that has a keyword and "can't be blocked" in one sentence; Walker
//! tokens named for their reminder text; "Whenever ~ and at least two Zombies attack".

use super::as_long_as;
use super::defining;
use super::facts::quantity_texts;
use super::render::capitalize;
use super::selectors::{Form, phrase_text};
use super::subtypes::plural_subtype;
use crate::ability::Ability;
use crate::modify::{Blocking, CombatQuirk, GameRule, Modification};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use crate::value::{Amount, Cmp, Condition, StatKind};

/// The ability without its condition, and the condition; `None` for an ability without one or of another kind.
fn unconditioned(ability: &Ability) -> Option<(Ability, &Condition)> {
    let mut twin = ability.clone();
    let (Ability::Static {
        condition: Some(held),
        ..
    }
    | Ability::Rule {
        condition: Some(held),
        ..
    }
    | Ability::Replacement {
        condition: Some(held),
        ..
    }) = ability
    else {
        return None;
    };
    match &mut twin {
        Ability::Static { condition, .. }
        | Ability::Rule { condition, .. }
        | Ability::Replacement { condition, .. } => *condition = None,
        _ => return None,
    }
    Some((twin, held))
}

/// Puts the condition on a static, rule or replacement.
fn set_condition(ability: &mut Ability, held: Condition) {
    if let Ability::Static { condition, .. }
    | Ability::Rule { condition, .. }
    | Ability::Replacement { condition, .. } = ability
    {
        *condition = Some(held);
    }
}

/// Wordings of shapes no other writer words: "Equipped creature must be blocked if able.", "Prevent all damage that
/// would be dealt to ~ by spells that target it."
fn plain(ability: &Ability) -> Vec<String> {
    match ability {
        Ability::Rule {
            rule:
                GameRule::Combat {
                    who: Ref::Attached,
                    quirk: CombatQuirk::MustBeBlocked(Blocking::AtLeastOne),
                },
            condition: None,
            from: ZoneKind::Battlefield,
        } => ["Equipped", "Enchanted"]
            .iter()
            .map(|word| format!("{word} creature must be blocked if able."))
            .collect(),
        Ability::Replacement {
            when:
                ReplaceWhen::Damage {
                    to: Some(Ref::This),
                    source,
                    combat: None,
                },
            by: ReplaceBy::Prevent,
            condition: None,
            from: ZoneKind::Battlefield,
        } if source.0 == [Pred::Spell, Pred::Targeting(Selector::this())] => {
            vec!["Prevent all damage that would be dealt to ~ by spells that target it.".to_owned()]
        }
        _ => Vec::new(),
    }
}

/// "As long as equipped creature is attacking, it has first strike and must be blocked if able.": two abilities under one
/// condition that names their subject, each twin without the condition written with that subject.
pub(super) fn r303c_pair_texts(
    first: &Ability,
    second: &Ability,
    written: fn(&Ability) -> Vec<String>,
) -> Vec<String> {
    let (Some((one, held)), Some((two, other))) = (unconditioned(first), unconditioned(second))
    else {
        return Vec::new();
    };
    if held != other {
        return Vec::new();
    }
    let mut conditions = as_long_as::texts(held);
    conditions.extend(card_conditions(held));
    let mut out = Vec::new();
    for a in written(&one) {
        for b in written(&two) {
            let (a, b) = (a.trim_end_matches('.'), b.trim_end_matches('.'));
            let Some(subject) = ["Equipped creature ", "Enchanted creature ", "~ "]
                .into_iter()
                .find(|subject| a.starts_with(subject) && b.starts_with(subject))
            else {
                continue;
            };
            let (a, b) = (&a[subject.len()..], &b[subject.len()..]);
            let named = subject.trim_end().to_lowercase();
            for condition in &conditions {
                if condition.to_lowercase().starts_with(&named) {
                    let text = format!("As long as {condition}, it {a} and {b}.");
                    if !out.contains(&text) {
                        out.push(text);
                    }
                }
            }
        }
    }
    out
}

/// The sentence of the twin after the condition: "~ must be blocked" as "it must be blocked" when the condition names
/// the card, otherwise with a lower-case first letter; `None` for a twin that opens with a condition of its own.
fn after_condition(text: &str, names_card: bool) -> Option<String> {
    let text = text.trim_end_matches('.');
    if text.starts_with("As long as") || text.starts_with("During") || text.starts_with("If ") {
        return None;
    }
    if let Some(rest) = text.strip_prefix("~ ") {
        return Some(if names_card {
            format!("it {rest}")
        } else {
            format!("~ {rest}")
        });
    }
    let mut chars = text.chars();
    let first = chars.next()?;
    Some(format!("{}{}", first.to_lowercase(), chars.as_str()))
}

/// "~ has hexproof and can't be blocked": keywords of the card, then the other changes of the card, each part written by
/// its twin with one kind of change.
fn joined_statics(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ability::Static {
        affected: Ref::This,
        mods,
        condition: None,
        defining: false,
        from,
    } = ability
    else {
        return Vec::new();
    };
    let (keywords, rest): (Vec<Modification>, Vec<Modification>) = mods
        .iter()
        .cloned()
        .partition(|m| matches!(m, Modification::AddKeyword(_)));
    if keywords.is_empty() || rest.is_empty() {
        return Vec::new();
    }
    let part = |mods: Vec<Modification>| {
        written(&Ability::Static {
            affected: Ref::This,
            mods,
            condition: None,
            defining: false,
            from: *from,
        })
    };
    let mut names = Vec::new();
    for keyword in &keywords {
        let Modification::AddKeyword(keyword) = keyword else {
            return Vec::new();
        };
        let Ok(serde_json::Value::String(name)) = serde_json::to_value(keyword) else {
            return Vec::new();
        };
        names.push(spaced(&name));
    }
    let listed = match names.as_slice() {
        [one] => one.clone(),
        [one, two] => format!("{one} and {two}"),
        [init @ .., last] => format!("{}, and {last}", init.join(", ")),
        [] => return Vec::new(),
    };
    part(rest)
        .into_iter()
        .filter_map(|second| {
            let rest = second.trim_end_matches('.').strip_prefix("~ ")?.to_owned();
            (!rest.starts_with("has ")).then(|| format!("~ has {listed} and {rest}"))
        })
        .collect()
}

/// A keyword without parameters in words: `FirstStrike` as "first strike".
fn spaced(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_uppercase() && !out.is_empty() {
            out.push(' ');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// The named tokens whose reminder text gives their characteristics: the wording with the characteristics and the name,
/// and the short wording with the name alone.
const NAMED_TOKENS: &[(&str, &str)] = &[("2/2 black Zombie creature", "Walker")];

/// "create two Walker tokens" for "create two 2/2 black Zombie creature tokens named Walker".
fn named_tokens(texts: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (kind, name) in NAMED_TOKENS {
        let many = format!("{kind} tokens named {name}");
        let one = format!("a {kind} token named {name}");
        for text in texts {
            if text.contains(&many) {
                out.push(text.replace(&many, &format!("{name} tokens")));
            } else if text.contains(&one) {
                out.push(text.replace(&one, &format!("a {name} token")));
            }
        }
    }
    out
}

/// "Whenever ~ and at least two Zombies attack": the twin counting other attacking creatures, with the creature type in
/// place of "other creatures".
fn attacking_kind(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Vec<String> {
    let Ability::Triggered {
        when: TriggerPattern::Attacks { what },
        intervening_if: Some(Condition::AtTrigger(held)),
        ..
    } = ability
    else {
        return Vec::new();
    };
    if *what != Selector::this() {
        return Vec::new();
    }
    let Condition::Compare {
        left: Amount::Count(counted),
        ..
    } = held.as_ref()
    else {
        return Vec::new();
    };
    let [Pred::Subtype(kind), Pred::Attacking, Pred::Other] = counted.0.as_slice() else {
        return Vec::new();
    };
    let mut twin = ability.clone();
    if let Ability::Triggered {
        intervening_if: Some(Condition::AtTrigger(held)),
        ..
    } = &mut twin
        && let Condition::Compare {
            left: Amount::Count(counted),
            ..
        } = held.as_mut()
    {
        *counted = Selector(vec![
            Pred::Type(mtg_core::card::CardType::Creature),
            Pred::Attacking,
            Pred::Other,
        ]);
    }
    let kinds = plural_subtype(&kind.0);
    written(&twin)
        .into_iter()
        .filter(|text| text.contains(" other creatures attack"))
        .map(|text| text.replacen(" other creatures attack", &format!(" {kinds} attack"), 1))
        .collect()
}

/// Conditions on the card that `as_long_as` does not write: "~ is equipped", "~ hasn't dealt damage yet".
fn card_conditions(held: &Condition) -> Vec<String> {
    let on_card = |condition: &Condition| match condition {
        Condition::Matches {
            subject: Ref::This,
            selector,
        } => Some(selector.0.clone()),
        _ => None,
    };
    if let Some(preds) = on_card(held) {
        return match preds.as_slice() {
            [Pred::HasAttached(attached)]
                if attached.0
                    == [Pred::Subtype(mtg_core::card::Subtype(
                        "Equipment".to_owned(),
                    ))] =>
            {
                vec!["~ is equipped".to_owned()]
            }
            _ => Vec::new(),
        };
    }
    if let Condition::Compare {
        left,
        op: Cmp::Ge,
        right: Amount::Fixed(number),
    } = held
        && let Ok(number) = u32::try_from(*number)
    {
        return compared(left, number);
    }
    if let Condition::Not(inner) = held
        && let Some(preds) = on_card(inner)
    {
        return match preds.as_slice() {
            [Pred::DealtDamageYet] => vec!["~ hasn't dealt damage yet".to_owned()],
            [Pred::DealtCombatDamageYet] => vec!["~ hasn't dealt combat damage yet".to_owned()],
            _ => Vec::new(),
        };
    }
    Vec::new()
}

/// "there are three or more different kinds of counters among creatures you control", "an artifact creature you control
/// is attacking": an amount at least the number.
fn compared(left: &Amount, number: u32) -> Vec<String> {
    match left {
        Amount::Distinct {
            of,
            stat: StatKind::CounterKinds,
        } => {
            let Some(among) = defining::counted_text(of) else {
                return Vec::new();
            };
            quantity_texts(number)
                .into_iter()
                .map(|n| format!("there are {n} or more different kinds of counters among {among}"))
                .collect()
        }
        Amount::Count(counted) if number == 1 => {
            let [kinds @ .., Pred::ControlledBy(Ref::You), Pred::Attacking] = counted.0.as_slice()
            else {
                return Vec::new();
            };
            let Some(kind) = phrase_text(&Selector(kinds.to_vec()), Form::Target) else {
                return Vec::new();
            };
            vec![format!(
                "{} {kind} you control is attacking",
                article(&kind)
            )]
        }
        _ => Vec::new(),
    }
}

/// "a" or "an" before the word.
fn article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    }
}

/// Adds "As long as C, S" for an ability under a condition C whose twin without it is written S, each wording once.
pub(super) fn r303c_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    let mut found = plain(ability);
    found.extend(joined_statics(ability, written));
    found.extend(attacking_kind(ability, written));
    found.extend(named_tokens(texts));
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
    let Some((twin, held)) = unconditioned(ability) else {
        return;
    };
    let extra = card_conditions(held);
    if !extra.is_empty() {
        let mut stand_in = ability.clone();
        set_condition(&mut stand_in, Condition::YourTurn);
        for said in written(&stand_in) {
            if !said.contains("it's your turn") {
                continue;
            }
            for condition in &extra {
                let text = said.replace("it's your turn", condition);
                if !texts.contains(&text) {
                    texts.push(text);
                }
            }
        }
    }
    let mut conditions = as_long_as::texts(held);
    conditions.extend(extra);
    if conditions.is_empty() {
        return;
    }
    for said in written(&twin) {
        for condition in &conditions {
            let names_card = condition.starts_with("~ ");
            let Some(rest) = after_condition(&said, names_card) else {
                continue;
            };
            let text = capitalize(&format!("as long as {condition}, {rest}"));
            if !texts.contains(&text) {
                texts.push(text);
            }
        }
    }
}
