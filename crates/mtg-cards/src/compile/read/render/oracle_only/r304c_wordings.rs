//! Wordings of the r30-4c pack: damage replaced by other steps, said with the source and recipient the replacement
//! names ("If a source you control would deal damage to an opponent, prevent that damage and each opponent mills that
//! many cards.", "If damage would be dealt to a player, that player exiles that many cards from the top of their
//! library instead.", "If a Zombie you control would deal combat damage to a player, instead that player mills that
//! many cards.").

use crate::ability::Ability;
use crate::compile::read::defining::value_texts;
use crate::compile::read::triggers::subject_text;
use crate::effect::{Destination, Effect, Place};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen, TriggerPattern};
use crate::value::{Amount, Cmp, Condition, CounterKind, StatKind};
use mtg_core::card::CardType;

/// Adds the wordings of the shapes this family writes.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for text in damage_replaced(ability)
        .into_iter()
        .chain(damage_changed(ability))
        .chain(counters_changed(ability))
        .chain(milled_exiled(ability))
        .chain(draw_replaced(ability))
        .chain(dies_exiled(ability))
        .chain(lich(ability))
        .chain(exiled_then(ability))
    {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}

/// The recipient of the damage as "would deal damage to" names it.
fn recipient(to: Option<&Ref>) -> Vec<&'static str> {
    match to {
        Some(Ref::This) => vec!["~"],
        Some(Ref::You) => vec!["you"],
        Some(Ref::Opponents) => vec!["an opponent"],
        Some(Ref::EachPlayer) => vec!["a player"],
        Some(Ref::Attached) => vec!["equipped creature", "enchanted creature"],
        Some(Ref::Each(what)) if what.0 == [Pred::Type(CardType::Creature)] => vec!["a creature"],
        Some(Ref::Union(parts)) => match parts.as_slice() {
            [Ref::Opponents, Ref::Each(what)] if what.0 == [Pred::ControlledBy(Ref::Opponents)] => {
                vec!["an opponent or a permanent an opponent controls"]
            }
            [Ref::Opponents, Ref::Each(what)]
                if what.0
                    == [Pred::AnyOf(vec![
                        Pred::Type(CardType::Creature),
                        Pred::Type(CardType::Battle),
                    ])] =>
            {
                vec!["a creature, battle, or opponent"]
            }
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// The source of the damage as the subject of "would deal": "a source", "a source you control", "~", "a Zombie you
/// control".
fn dealer(source: &Selector) -> Option<String> {
    match source.0.as_slice() {
        [] => Some("a source".to_owned()),
        [Pred::ControlledBy(Ref::You)] => Some("a source you control".to_owned()),
        [Pred::ControlledBy(Ref::Opponents)] => Some("a source an opponent controls".to_owned()),
        [Pred::Spell, Pred::AnyOf(kinds), Pred::ChosenColor]
            if *kinds == [Pred::Type(CardType::Instant), Pred::Type(CardType::Sorcery)] =>
        {
            Some("an instant or sorcery spell of the chosen color".to_owned())
        }
        [Pred::Not(kind), Pred::ControlledBy(Ref::You)]
            if **kind == Pred::Type(CardType::Creature) =>
        {
            Some("a noncreature source you control".to_owned())
        }
        _ => subject_text(source),
    }
}

/// The heads of the sentence: "a source you control would deal damage to an opponent", "combat damage would be dealt
/// to ~".
fn heads(to: Option<&Ref>, source: &Selector, combat: Option<bool>) -> Vec<String> {
    let kind = match combat {
        Some(true) => "combat ",
        Some(false) => "noncombat ",
        None => "",
    };
    let mut found = Vec::new();
    for to in recipient(to) {
        if let Some(by) = dealer(source) {
            found.push(format!("{by} would deal {kind}damage to {to}"));
        }
        if source.is_empty() {
            found.push(format!("{kind}damage would be dealt to {to}"));
        }
    }
    found
}

/// One step lowercase and without its period, as a spell's sentence says it, with the steps only replacements say.
fn step_texts(step: &Effect) -> Vec<String> {
    let mut found = Vec::new();
    if let Effect::ExileTop {
        who,
        count: Amount::Triggering,
        sealed: false,
        ..
    } = step
    {
        match who {
            Ref::You => found.push("exile that many cards from the top of your library".to_owned()),
            Ref::Triggering => found.push(
                "that player exiles that many cards from the top of their library".to_owned(),
            ),
            _ => {}
        }
    }
    if let Effect::Move {
        what: Ref::Triggering,
        to: Destination::Library {
            place: Place::Shuffled,
            ..
        },
        ..
    } = step
    {
        found.push("that creature's owner shuffles it into their library".to_owned());
    }
    let spell = Ability::Spell {
        targets: Vec::new(),
        effects: vec![step.clone()],
    };
    for text in crate::compile::read::render::variants(&spell) {
        let text = text.strip_suffix('.').unwrap_or(&text);
        let mut chars = text.chars();
        if let Some(first) = chars.next() {
            found.push(first.to_lowercase().chain(chars).collect());
        }
    }
    found
}

/// "If <head>, prevent that damage and <step>.", "If <head>, instead <step>." and "If <head>, <step> instead.", and the
/// prevention whose step draws for the source's controller.
fn damage_replaced(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when: ReplaceWhen::Damage { to, source, combat },
        by,
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    let heads = heads(to.as_ref(), source, *combat);
    let mut texts = Vec::new();
    match by {
        ReplaceBy::PreventAnd(steps) => {
            let [step] = steps.as_slice() else {
                return Vec::new();
            };
            if *step
                == (Effect::Draw {
                    who: Ref::ControllerOf(Box::new(Ref::Dealer)),
                    count: Amount::Triggering,
                    bind: None,
                })
            {
                for head in &heads {
                    texts.push(format!(
                        "If {head}, prevent that damage. The source's controller draws cards equal to the damage \
                         prevented this way."
                    ));
                }
                return texts;
            }
            for words in step_texts(step) {
                for head in &heads {
                    texts.push(format!("If {head}, prevent that damage and {words}."));
                }
            }
            texts.extend(per_prevented(step, to.as_ref(), &heads));
        }
        ReplaceBy::Instead(steps) => {
            let [step] = steps.as_slice() else {
                return Vec::new();
            };
            if let Effect::DealDamage {
                source: Some(Ref::Dealer),
                to: Ref::ControllerOf(of),
                amount: Amount::Triggering,
                unpreventable: false,
                ..
            } = step
                && **of == Ref::Dealer
            {
                for head in &heads {
                    texts.push(format!(
                        "If {head}, it deals that damage to its controller instead."
                    ));
                }
                return texts;
            }
            for words in step_texts(step) {
                for head in &heads {
                    texts.push(format!("If {head}, instead {words}."));
                    texts.push(format!("If {head}, {words} instead."));
                }
            }
        }
        _ => {}
    }
    texts
}

/// The words of a count, with the power of the card itself as "~'s power".
fn counts(amount: &Amount) -> Vec<String> {
    let mut found = value_texts(amount);
    if let Amount::Stat {
        of: Ref::This,
        stat: StatKind::Power,
    } = amount
    {
        found.push("~'s power".to_owned());
    }
    found
}

/// The number of a damage change: "2", or "X, where X is ~'s power" and "an amount of damage equal to <count>".
fn more_texts(by: &ReplaceBy) -> Vec<String> {
    match by {
        ReplaceBy::Add(more) if *more > 0 => vec![more.to_string()],
        ReplaceBy::AddAmount(amount) => counts(amount)
            .into_iter()
            .flat_map(|count| {
                [
                    format!("X, where X is {count}"),
                    format!("an amount of damage equal to {count}"),
                ]
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// "If <head>, it deals that much damage plus <N> instead.", "If <head>, instead it deals that much damage plus X,
/// where X is <count>.", "If <head>, prevent X of that damage, where X is <count>." and "If a source would deal 3 or
/// less damage to ~, prevent that damage."
fn damage_changed(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when: ReplaceWhen::Damage { to, source, combat },
        by,
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    let mut texts = Vec::new();
    match by {
        ReplaceBy::Add(_) | ReplaceBy::AddAmount(_) => {
            for head in heads(to.as_ref(), source, *combat) {
                for more in more_texts(by) {
                    texts.push(format!(
                        "If {head}, it deals that much damage plus {more} instead."
                    ));
                    texts.push(format!(
                        "If {head}, instead it deals that much damage plus {more}."
                    ));
                }
            }
        }
        ReplaceBy::PreventAmount(amount) => {
            for head in heads(to.as_ref(), source, *combat) {
                for count in counts(amount) {
                    texts.push(format!(
                        "If {head}, prevent X of that damage, where X is {count}."
                    ));
                }
            }
        }
        ReplaceBy::PreventAtMost(most) => {
            let kind = match combat {
                Some(true) => "combat ",
                Some(false) => "noncombat ",
                None => "",
            };
            if let Some(by) = dealer(source) {
                for to in recipient(to.as_ref()) {
                    texts.push(format!(
                        "If {by} would deal {most} or less {kind}damage to {to}, prevent that damage."
                    ));
                }
            }
        }
        _ => {}
    }
    texts
}

/// The printed name of a counter kind.
fn counter_name(kind: &CounterKind) -> Option<String> {
    match kind {
        CounterKind::Other(name) => Some(name.clone()),
        other => serde_json::to_value(other)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned)),
    }
}

/// "If you would put one or more counters on a creature or planeswalker you control or on yourself, put that many plus
/// one of each of those kinds of counters on that permanent or player instead.", "If one or more counters would be put
/// on a permanent your team controls, that many plus one of each of those kinds of counters are put on that permanent
/// instead." and "If one or more -1/-1 counters would be put on a creature you control, that many -1/-1 counters minus
/// one are put on it instead."
fn counters_changed(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when:
            ReplaceWhen::CountersPut {
                on: Some(on),
                counter,
                by: putter,
                effect: false,
            },
        by,
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    let mut texts = Vec::new();
    match (on, counter, putter, by) {
        (Ref::Union(parts), None, Some(Ref::You), ReplaceBy::Add(1)) => {
            if let [Ref::You, Ref::Each(what)] = parts.as_slice()
                && let Some(place) = subject_text(what)
            {
                texts.push(format!(
                    "If you would put one or more counters on {place} or on yourself, put that many plus one of each \
                     of those kinds of counters on that permanent or player instead."
                ));
            }
        }
        (Ref::Each(what), None, None, ReplaceBy::Add(1))
            if what.0
                == [
                    Pred::ControlledBy(Ref::You),
                    Pred::In(ZoneKind::Battlefield),
                ] =>
        {
            for owner in ["your team controls", "you control"] {
                texts.push(format!(
                    "If one or more counters would be put on a permanent {owner}, that many plus one of each of \
                     those kinds of counters are put on that permanent instead."
                ));
            }
        }
        (Ref::Each(what), Some(kind), None, ReplaceBy::Subtract(1)) => {
            if let (Some(place), Some(kind)) = (subject_text(what), counter_name(kind)) {
                texts.push(format!(
                    "If one or more {kind} counters would be put on {place}, that many {kind} counters minus one \
                     are put on it instead."
                ));
            }
        }
        _ => {}
    }
    texts
}

/// "Whenever a creature card is put into an opponent's graveyard from their library, exile that card and create a 2/2
/// black Zombie creature token.": a card milled, exiled again by the trigger, then the steps after it.
fn milled_exiled(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when:
            TriggerPattern::ZoneChange {
                what,
                from: Some(ZoneKind::Library),
                to: Some(ZoneKind::Graveyard),
            },
        intervening_if: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets,
        effects,
    } = ability
    else {
        return Vec::new();
    };
    let ([Pred::Type(kind), Pred::OwnedBy(Ref::Opponents)], true) =
        (what.0.as_slice(), targets.is_empty())
    else {
        return Vec::new();
    };
    let [
        Effect::Exile {
            what: Ref::Triggering,
            bind: None,
            duration: None,
        },
        rest @ ..,
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    let kind = format!("{kind:?}").to_lowercase();
    let head = format!(
        "Whenever a {kind} card is put into an opponent's graveyard from their library, exile that card"
    );
    if rest.is_empty() {
        return vec![format!("{head}.")];
    }
    let [step] = rest else {
        return Vec::new();
    };
    step_texts(step)
        .into_iter()
        .map(|words| format!("{head} and {words}."))
        .collect()
}

/// "If you would draw a card, you may skip that draw instead." and "If you would draw a card, <first step> instead.
/// <the other steps>": a draw replaced by the steps a spell would say.
fn draw_replaced(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when:
            ReplaceWhen::Draw {
                who: Ref::You,
                except_draw_step: 0,
            },
        by: ReplaceBy::Instead(steps),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    if let [
        Effect::May {
            who: Ref::You,
            effects,
        },
    ] = steps.as_slice()
        && effects.is_empty()
    {
        return vec!["If you would draw a card, you may skip that draw instead.".to_owned()];
    }
    let mut texts = revealed_instead(steps).into_iter().collect::<Vec<_>>();
    let spell = Ability::Spell {
        targets: Vec::new(),
        effects: steps.clone(),
    };
    for text in crate::compile::read::render::variants(&spell) {
        let mut chars = text.chars();
        let Some(first) = chars.next() else {
            continue;
        };
        let lower: String = first.to_lowercase().chain(chars).collect();
        if let Some((head, rest)) = lower.split_once(". ") {
            texts.push(format!("If you would draw a card, {head} instead. {rest}"));
            texts.push(format!("If you would draw a card, instead {head}. {rest}"));
        }
        if let Some(whole) = lower.strip_suffix('.') {
            texts.push(format!("If you would draw a card, {whole} instead."));
            texts.push(format!("If you would draw a card, instead {whole}."));
            // "put one into your hand and the rest into your graveyard" (Underrealm Lich).
            if whole.contains(" put one of them into ") {
                let short = whole.replace(" put one of them into ", " put one into ");
                texts.push(format!("If you would draw a card, instead {short}."));
            }
        }
    }
    texts
}

/// "reveal the top card of your library instead. If it's a creature card, put it into your graveyard. Otherwise, draw a
/// card." after "If you would draw a card, ".
fn revealed_instead(steps: &[Effect]) -> Option<String> {
    let [
        Effect::LookAtTop {
            who: Ref::You,
            count: Amount::Fixed(1),
            bind: top,
            ..
        },
        Effect::Reveal { what: shown, .. },
        Effect::If {
            condition: Condition::Matches { subject, selector },
            then,
            otherwise,
        },
    ] = steps
    else {
        return None;
    };
    let named = Ref::Bound(top.clone());
    let ([Pred::Type(kind)], true, true) =
        (selector.0.as_slice(), *shown == named, *subject == named)
    else {
        return None;
    };
    let (
        [
            Effect::Move {
                what,
                to: Destination::Graveyard,
                bind: None,
            },
        ],
        [draw],
    ) = (then.as_slice(), otherwise.as_slice())
    else {
        return None;
    };
    (*what == named
        && *draw
            == (Effect::Draw {
                who: Ref::You,
                count: Amount::Fixed(1),
                bind: None,
            }))
    .then(|| {
        let kind = format!("{kind:?}").to_lowercase();
        format!(
            "If you would draw a card, reveal the top card of your library instead. If it's a {kind} card, put it \
             into your graveyard. Otherwise, draw a card."
        )
    })
}

/// "If a creature dealt damage this turn by a source you controlled would die, exile it instead."
fn dies_exiled(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when:
            ReplaceWhen::ZoneChange {
                what,
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Graveyard),
            },
        by: ReplaceBy::MoveTo(ZoneKind::Exile),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    let [Pred::Type(kind), Pred::DamagedThisTurnBy(Ref::You)] = what.0.as_slice() else {
        return Vec::new();
    };
    let kind = format!("{kind:?}").to_lowercase();
    vec![format!(
        "If a {kind} dealt damage this turn by a source you controlled would die, exile it instead."
    )]
}

/// "If damage would be dealt to you, exile that many cards from your graveyard instead. If you can't, you lose the
/// game." and "If you would gain life, draw that many cards instead."
fn lich(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when,
        by: ReplaceBy::Instead(steps),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    let graveyard = Selector(vec![Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(Ref::You)]);
    match (when, steps.as_slice()) {
        (
            ReplaceWhen::LifeGain {
                who: Ref::You,
                caused_own: false,
            },
            [
                Effect::Draw {
                    who: Ref::You,
                    count: Amount::Triggering,
                    bind: None,
                },
            ],
        ) => vec!["If you would gain life, draw that many cards instead.".to_owned()],
        (
            ReplaceWhen::Damage {
                to: Some(Ref::You),
                source,
                combat: None,
            },
            [
                Effect::If {
                    condition:
                        Condition::Compare {
                            left,
                            op: Cmp::Ge,
                            right: Amount::Triggering,
                        },
                    then,
                    otherwise,
                },
            ],
        ) if source.is_empty()
            && *left == Amount::Count(graveyard.clone())
            && matches!(otherwise.as_slice(), [Effect::LoseGame { who: Ref::You }]) =>
        {
            let [
                Effect::Choose {
                    who: Ref::You,
                    from: Ref::Each(from),
                    count,
                    bind,
                    rest_as: None,
                    differ: None,
                },
                Effect::Exile {
                    what: Ref::Bound(exiled),
                    bind: None,
                    duration: None,
                },
            ] = then.as_slice()
            else {
                return Vec::new();
            };
            let exact = count.min == Amount::Triggering && count.max == Some(Amount::Triggering);
            if *from == graveyard && exact && bind == exiled {
                vec!["If damage would be dealt to you, exile that many cards from your graveyard instead. If you \
                      can't, you lose the game."
                    .to_owned()]
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// "If a nontoken creature an opponent controls would die, exile it instead. When you do, <steps>": a death replaced
/// by exile, with a reflexive trigger after it.
fn exiled_then(ability: &Ability) -> Vec<String> {
    let Ability::Replacement {
        when:
            ReplaceWhen::ZoneChange {
                what,
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Graveyard),
            },
        by: ReplaceBy::Instead(steps),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return Vec::new();
    };
    let [
        Effect::Exile {
            what: Ref::Triggering,
            bind: None,
            duration: None,
        },
        Effect::Reflexive {
            targets, effects, ..
        },
    ] = steps.as_slice()
    else {
        return Vec::new();
    };
    let Some(subject) = subject_text(what) else {
        return Vec::new();
    };
    if !targets.is_empty() {
        return Vec::new();
    }
    let spell = Ability::Spell {
        targets: Vec::new(),
        effects: effects.clone(),
    };
    crate::compile::read::render::variants(&spell)
        .into_iter()
        .filter_map(|text| {
            let mut chars = text.chars();
            let first = chars.next()?;
            let lower: String = first.to_lowercase().chain(chars).collect();
            Some(format!(
                "If {subject} would die, exile it instead. When you do, {lower}"
            ))
        })
        .collect()
}

/// The prevention's step said once for each 1 damage prevented, as its own sentence: "prevent that damage. Put a -1/-1
/// counter on this creature for each 1 damage prevented this way.", "prevent that damage. You gain life equal to the
/// damage prevented this way."
fn per_prevented(step: &Effect, to: Option<&Ref>, heads: &[String]) -> Vec<String> {
    let mut texts = Vec::new();
    if let Effect::GainLife {
        who: Ref::You,
        amount: Amount::Triggering,
    } = step
    {
        for head in heads {
            texts.push(format!(
                "If {head}, prevent that damage. You gain life equal to the damage prevented this way."
            ));
        }
        return texts;
    }
    let one = match step {
        Effect::PutCounters {
            on,
            counter,
            count: Amount::Triggering,
        } => Effect::PutCounters {
            // Damage to the card itself puts the counters on the card the event names: on ~.
            on: if *on == Ref::Triggering && to == Some(&Ref::This) {
                Ref::This
            } else {
                on.clone()
            },
            counter: counter.clone(),
            count: Amount::Fixed(1),
        },
        Effect::CreateToken {
            count: Amount::Triggering,
            ..
        } => {
            let mut one = step.clone();
            if let Effect::CreateToken { count, .. } = &mut one {
                *count = Amount::Fixed(1);
            }
            one
        }
        _ => return texts,
    };
    for words in step_texts(&one) {
        let mut chars = words.chars();
        let Some(first) = chars.next() else {
            continue;
        };
        let words: String = first.to_uppercase().chain(chars).collect();
        for head in heads {
            texts.push(format!(
                "If {head}, prevent that damage. {words} for each 1 damage prevented this way."
            ));
        }
    }
    texts
}
