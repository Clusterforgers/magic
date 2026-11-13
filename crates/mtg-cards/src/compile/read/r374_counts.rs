//! Counts of an object's own types and keyword values (`StatKind::R374*`) after "for each": "for each supertype, card
//! type, and subtype it has", "for each of its creature types, to a maximum of 10", "for each point of bushido it has";
//! "its total toxic value"; and the color pairs among permanents of two colors. Their readers and writers.

use super::defining;
use super::tokens::Reader;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Cmp, Condition, CounterKind, StatKind};

/// The words after "for each" of each count of the object's own, with "it" or "its" in them.
const EACH: [(&str, StatKind); 3] = [
    (
        "supertype, card type, and subtype it has",
        StatKind::R374Types,
    ),
    ("of its creature types", StatKind::R374CreatureTypes),
    ("point of bushido it has", StatKind::R374Bushido),
];

/// Whose types "it" names: each object a static ability changes (its subject is no single object), or the only target
/// of a spell.
fn it(r: &Reader<'_>) -> Option<Ref> {
    match super::t17b_attached::its(r) {
        None | Some(Ref::Each(_)) if r.static_grant => Some(Ref::Affected),
        None | Some(Ref::Target(0)) if r.targets.len() == 1 => Some(Ref::Target(0)),
        _ => None,
    }
}

/// What follows "for each" of a count of the object's own, with ", to a maximum of N" after it.
pub(super) fn r374_each(r: &mut Reader<'_>) -> Option<Amount> {
    r.attempt(|r| {
        let stat = if r
            .attempt(|r| {
                (r.word("supertype") && r.punct(',') && r.words("card type") && r.punct(','))
                    .then_some(())?;
                r.need("and subtype it has")
            })
            .is_some()
        {
            StatKind::R374Types
        } else {
            EACH.iter()
                .find(|(words, _)| r.words(words))
                .map(|(_, stat)| *stat)?
        };
        let amount = Amount::Stat { of: it(r)?, stat };
        let capped = r.attempt(|r| {
            r.punct(',').then_some(())?;
            r.need("to a maximum of")?;
            r.number()
        });
        Some(match capped {
            Some(most) => Amount::R374AtMost(Box::new(amount), most),
            None => amount,
        })
    })
}

/// What `r374_each` reads, written back.
pub(super) fn r374_each_text(amount: &Amount) -> Option<String> {
    let (inner, most) = match amount {
        Amount::R374AtMost(inner, most) => (inner.as_ref(), Some(*most)),
        other => (other, None),
    };
    let Amount::Stat {
        of: Ref::Affected | Ref::Target(0),
        stat,
    } = inner
    else {
        return None;
    };
    let (words, _) = EACH.iter().find(|(_, kind)| kind == stat)?;
    Some(match most {
        Some(most) => format!("{words}, to a maximum of {most}"),
        None => (*words).to_owned(),
    })
}

/// "its total toxic value" of the object an earlier sentence named.
pub(super) fn r374_toxic(r: &mut Reader<'_>) -> Option<Amount> {
    r.attempt(|r| {
        r.need("its total toxic value")?;
        Some(Amount::Stat {
            of: super::t17b_attached::its(r)?,
            stat: StatKind::R374Toxic,
        })
    })
}

/// What `r374_toxic` reads, written back.
pub(super) fn r374_toxic_text(amount: &Amount) -> Option<String> {
    matches!(
        amount,
        Amount::Stat {
            stat: StatKind::R374Toxic,
            ..
        }
    )
    .then(|| "its total toxic value".to_owned())
}

/// The objects of exactly two colors.
fn two_colors() -> Pred {
    Pred::Compare {
        stat: StatKind::Colors,
        op: Cmp::Eq,
        to: Amount::Fixed(2),
    }
}

/// "different color pairs among permanents you control that are exactly two colors", after "the number of".
pub(super) fn r374_color_pairs(r: &mut Reader<'_>) -> Option<Amount> {
    r.attempt(|r| {
        r.need("different color pairs among")?;
        let Selector(mut of) = defining::counted(r)?;
        r.need("that are exactly two colors")?;
        of.push(two_colors());
        Some(Amount::Distinct {
            of: Selector(of),
            stat: StatKind::R374ColorSet,
        })
    })
}

/// What `r374_color_pairs` reads, written back.
pub(super) fn r374_color_pairs_text(amount: &Amount) -> Option<String> {
    let Amount::Distinct {
        of,
        stat: StatKind::R374ColorSet,
    } = amount
    else {
        return None;
    };
    let mut rest = of.0.clone();
    let at = rest.iter().position(|pred| *pred == two_colors())?;
    rest.remove(at);
    Some(format!(
        "different color pairs among {} that are exactly two colors",
        defining::counted_text(&Selector(rest))?
    ))
}

/// The first letter of a sentence lowered, to join it into another.
fn lowered(sentence: &str) -> String {
    let mut chars = sentence.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}

/// "A, B, and C, where X is ...": three steps that read one X joined into one sentence, from the wording that says each
/// step and X apart ("A. B. C. X is ...").
fn three_steps(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter_map(|text| {
            let (body, said) = text.strip_suffix('.')?.rsplit_once(". X is ")?;
            let parts: Vec<&str> = body.split(". ").collect();
            let [first, second, third] = parts.as_slice() else {
                return None;
            };
            Some(format!(
                "{first}, {}, and {}, where X is {said}.",
                lowered(second),
                lowered(third)
            ))
        })
        .collect()
}

/// "choose a creature you control, then draw cards equal to its total toxic value": the choice and the draw it counts,
/// written from the twin that draws one card.
fn choose_then_draw(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
) -> Option<Vec<String>> {
    let Ability::Triggered {
        when,
        intervening_if,
        limit,
        from,
        targets,
        effects,
    } = ability
    else {
        return None;
    };
    let [
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(chosen),
            count,
            bind,
            rest_as: None,
            differ: None,
        },
        Effect::Draw {
            who: Ref::You,
            count: drawn,
            bind: None,
        },
    ] = effects.as_slice()
    else {
        return None;
    };
    count.is_one().then_some(())?;
    let Amount::Stat {
        of: Ref::Bound(name),
        stat: StatKind::R374Toxic,
    } = drawn
    else {
        return None;
    };
    (name == bind).then_some(())?;
    let noun = super::selectors::phrase_text(chosen, super::selectors::Form::Subject)?;
    let article = if noun.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    // "if an opponent has three or more poison counters" is written in place of a condition the writers word.
    let (condition, said) = match intervening_if {
        Some(Condition::Compare {
            left:
                Amount::HighestCounters {
                    who: Ref::Opponents,
                    counter,
                },
            op: Cmp::Ge,
            right,
        }) if *counter == CounterKind::Poison => {
            let many = super::amounts::count_word(right.fixed()?)?;
            let stand = Condition::Compare {
                left: Amount::Count(Selector(vec![
                    Pred::Type(mtg_core::card::CardType::Artifact),
                    Pred::ControlledBy(Ref::You),
                ])),
                op: Cmp::Ge,
                right: right.clone(),
            };
            (
                Some(stand),
                Some((
                    format!("you control {many} or more artifacts"),
                    format!("an opponent has {many} or more poison counters"),
                )),
            )
        }
        other => (other.clone(), None),
    };
    let twin = Ability::Triggered {
        when: when.clone(),
        intervening_if: condition,
        limit: *limit,
        from: *from,
        targets: targets.clone(),
        effects: vec![Effect::Draw {
            who: Ref::You,
            count: Amount::Fixed(1),
            bind: None,
        }],
    };
    let step = format!("choose {article} {noun}, then draw cards equal to its total toxic value");
    Some(
        written(&twin)
            .iter()
            .filter(|text| text.ends_with(", draw a card."))
            .map(|text| text.replace(", draw a card.", &format!(", {step}.")))
            .filter_map(|text| match &said {
                Some((stand, real)) => text.contains(stand).then(|| text.replace(stand, real)),
                None => Some(text),
            })
            .collect(),
    )
}

/// Adds the wordings of this file's abilities that the other writers do not say.
pub(super) fn r374_wordings(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
    texts: &mut Vec<String>,
) {
    // Each of the three steps reads the same number, which X names.
    let three = match ability {
        Ability::Triggered { effects, .. } | Ability::Spell { effects, .. } => {
            let numbers: Vec<Option<serde_json::Value>> = effects
                .iter()
                .map(|effect| {
                    let value = serde_json::to_value(effect).ok()?;
                    let body = value.as_object()?.values().next()?.clone();
                    body.get("amount").or_else(|| body.get("count")).cloned()
                })
                .collect();
            effects.len() == 3
                && numbers.iter().all(Option::is_some)
                && numbers.windows(2).all(|pair| pair[0] == pair[1])
        }
        _ => false,
    };
    let mut found = if three {
        three_steps(texts)
    } else {
        Vec::new()
    };
    if let Some(drawn) = choose_then_draw(ability, written) {
        found.extend(drawn);
    }
    if let Some(unless) = unless_it_has(ability, written) {
        found.extend(unless);
    }
    if let Some(championed) = super::r374_champion::r374_championed(ability, written) {
        found.extend(championed);
    }
    if let Some(dealt) = target_deals_damage(ability, written) {
        found.extend(dealt);
    }
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// "If it doesn't have rampage, that creature gains rampage 2 until end of turn.": a last step a spell does unless its
/// target has a keyword, written from the twin that does it always.
fn unless_it_has(ability: &Ability, written: fn(&Ability) -> Vec<String>) -> Option<Vec<String>> {
    let Ability::Spell { targets, effects } = ability else {
        return None;
    };
    let (
        Effect::If {
            condition,
            then,
            otherwise,
        },
        before,
    ) = effects.split_last()?
    else {
        return None;
    };
    otherwise.is_empty().then_some(())?;
    let Condition::Not(inner) = condition else {
        return None;
    };
    let Condition::Matches {
        subject: Ref::Target(0),
        selector,
    } = inner.as_ref()
    else {
        return None;
    };
    let [Pred::HasKeywordNamed(name)] = selector.0.as_slice() else {
        return None;
    };
    let mut steps = before.to_vec();
    steps.extend(then.iter().cloned());
    let twin = Ability::Spell {
        targets: targets.clone(),
        effects: steps,
    };
    let said = format!(
        ". If it doesn't have {}, that creature ",
        name.to_lowercase()
    );
    Some(
        written(&twin)
            .iter()
            .filter(|text| text.contains(". That creature "))
            .map(|text| text.replacen(". That creature ", &said, 1))
            .collect(),
    )
}

/// "Whenever target creature deals damage this turn, you gain that much life.": an activated ability whose one step sets
/// up a trigger on its target for the turn, written from the twin that taps the target and the trigger of the card itself.
fn target_deals_damage(
    ability: &Ability,
    written: fn(&Ability) -> Vec<String>,
) -> Option<Vec<String>> {
    let Ability::Activated { effects, .. } = ability else {
        return None;
    };
    let [
        Effect::DelayedTrigger {
            when: TriggerPattern::Batch(inner),
            once: false,
            this_turn: true,
            until: None,
            targets,
            effects: steps,
        },
    ] = effects.as_slice()
    else {
        return None;
    };
    targets.is_empty().then_some(())?;
    let TriggerPattern::DealsDamage { source, .. } = inner.as_ref() else {
        return None;
    };
    (source.0 == [Pred::Is(Ref::Target(0))]).then_some(())?;
    let mut own = inner.as_ref().clone();
    if let TriggerPattern::DealsDamage { source, .. } = &mut own {
        *source = Selector::this();
    }
    let trigger = Ability::Triggered {
        when: TriggerPattern::Batch(Box::new(own)),
        intervening_if: None,
        limit: None,
        from: crate::refs::ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: steps.clone(),
    };
    let tails: Vec<String> = written(&trigger)
        .iter()
        .filter_map(|text| {
            text.strip_prefix("Whenever ~ deals damage, ")
                .map(str::to_owned)
        })
        .collect();
    let mut twin = ability.clone();
    if let Ability::Activated { effects, .. } = &mut twin {
        *effects = vec![Effect::Tap {
            what: Ref::Target(0),
            bind: None,
        }];
    }
    let mut found = Vec::new();
    for text in written(&twin) {
        let Some((cost, rest)) = text.split_once(": Tap ") else {
            continue;
        };
        let Some(target) = rest.strip_suffix('.') else {
            continue;
        };
        for tail in &tails {
            found.push(format!(
                "{cost}: Whenever {target} deals damage this turn, {tail}"
            ));
        }
    }
    Some(found)
}
