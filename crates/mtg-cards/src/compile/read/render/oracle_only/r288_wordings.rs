//! Wordings of pack r288: a chapter that distributes counters ("II — Distribute seven +1/+1 counters among any number
//! of target creatures you control."), a player's life total set to half their
//! starting life total ("up to one target player's life total becomes half their starting life total, rounded down"),
//! a delayed trigger with targets of its own ("III — Whenever an Elf you control attacks this turn, target creature an
//! opponent controls gets -1/-1 until end of turn.")
//! the final chapter ability of a Saga that triggers or resolves ("Whenever the final chapter ability of a Saga you
//! control resolves, ...")
//! and a draw of X cards counted from counters with a loss of X life if you do ("you may draw X cards, where X is the
//! number of counters on ~. If you do, you lose X life.").

use super::super::super::selectors::{Form, phrase_text};
use super::super::super::triggers::article;
use super::super::capitalize;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Ref, TargetSpec, Targetable};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Cmp, Condition, StatKind};

/// Adds the wordings of the shapes this family writes.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for text in chapter_twin(ability)
        .into_iter()
        .chain(life_becomes(ability))
        .chain(draw_counted_then_lose(ability))
        .chain(greater_than_this(ability))
        .chain(spell_causes_gain(ability))
        .chain(final_chapter(ability))
        .chain(tap_and_goad(ability))
        .chain(attacks_another_opponent(ability))
        .chain(lowest_returned(ability))
    {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
    creature_or_player(ability, texts);
}

/// "target creature or player": a target that is a player or a creature, worded as the twin that takes any target
/// says it with these words in place of "any target".
fn creature_or_player(ability: &Ability, texts: &mut Vec<String>) {
    let creature = Targetable::Object(crate::refs::Selector(vec![crate::refs::Pred::Type(
        mtg_core::card::CardType::Creature,
    )]));
    let is_either = |spec: &TargetSpec| matches!(&spec.what, Targetable::Either(kinds) if *kinds == [Targetable::Player, creature.clone()]);
    let mut twin = ability.clone();
    let (Ability::Triggered { targets, .. }
    | Ability::Spell { targets, .. }
    | Ability::Activated { targets, .. }) = &mut twin
    else {
        return;
    };
    let [spec] = targets.as_mut_slice() else {
        return;
    };
    if !is_either(spec) {
        return;
    }
    spec.what = Targetable::AnyTarget;
    for text in crate::compile::written_variants(&twin) {
        if text.matches("any target").count() != 1 {
            continue;
        }
        let said = text.replace("any target", "target creature or player");
        if !texts.contains(&said) {
            texts.push(said);
        }
    }
}

/// The heads of a triggered ability ("Whenever ~ deals combat damage to a player,"), from the wordings of its twin that
/// draws a card and has no targets.
pub(super) fn r288_heads(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when,
        intervening_if,
        limit,
        from,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let twin = Ability::Triggered {
        when: when.clone(),
        intervening_if: intervening_if.clone(),
        limit: *limit,
        from: *from,
        targets: Vec::new(),
        effects: vec![Effect::Draw {
            who: Ref::You,
            count: Amount::Fixed(1),
            bind: None,
        }],
    };
    let mut heads = Vec::new();
    for text in crate::compile::written_variants(&twin) {
        if let Some(head) = text.strip_suffix(" draw a card.")
            && head.ends_with(',')
            && !heads.iter().any(|known| known == head)
        {
            heads.push(head.to_owned());
        }
    }
    heads
}

/// A chapter that distributes counters or sets up a delayed trigger with targets: "II — Distribute seven +1/+1 counters among ...".
/// Other chapters keep the wordings of the renderers.
fn chapter_twin(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when: TriggerPattern::Chapter { number },
        intervening_if: None,
        limit: None,
        targets,
        effects,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let Some(numeral) = crate::compile::classify::numeral(*number) else {
        return Vec::new();
    };
    let mut texts: Vec<String> = distributed(targets, effects)
        .map(|said| format!("{numeral} \u{2014} {}.", capitalize(&said)))
        .into_iter()
        .collect();
    for said in delayed_this_turn(effects) {
        let said = format!("{numeral} \u{2014} {said}");
        if !texts.contains(&said) {
            texts.push(said);
        }
    }
    texts
}

/// "Whenever another creature you control enters, if that creature has greater power or toughness than this creature,
/// ...": the evolve test as the intervening "if" of another trigger.
fn greater_than_this(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when,
        intervening_if: Some(Condition::Any(tests)),
        limit,
        from,
        targets,
        effects,
    } = ability
    else {
        return Vec::new();
    };
    let greater = |stat: StatKind| Condition::Compare {
        left: Amount::Stat {
            of: Ref::Triggering,
            stat,
        },
        op: Cmp::Gt,
        right: Amount::Stat {
            of: Ref::This,
            stat,
        },
    };
    if *tests != [greater(StatKind::Power), greater(StatKind::Toughness)] {
        return Vec::new();
    }
    let plain = Ability::Triggered {
        when: when.clone(),
        intervening_if: None,
        limit: *limit,
        from: *from,
        targets: targets.clone(),
        effects: effects.clone(),
    };
    crate::compile::written_variants(&plain)
        .into_iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once(", ")?;
            Some([
                format!("{head}, if that creature has greater power or toughness than this creature, {rest}"),
                format!(
                    "{head}, if its power is greater than this creature's power or its toughness is greater than this creature's toughness, {rest}"
                ),
            ])
        })
        .flatten()
        .collect()
}

/// "Whenever a white instant or sorcery spell causes you to gain life, ...": a life gain caused by a spell of a kind,
/// from the wordings of the plain "Whenever you gain life, ...".
fn spell_causes_gain(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when:
            TriggerPattern::Caused {
                inner,
                by: None,
                source,
            },
        intervening_if,
        limit,
        from,
        targets,
        effects,
    } = ability
    else {
        return Vec::new();
    };
    if !matches!(**inner, TriggerPattern::LifeGained { who: Ref::You }) || source.is_empty() {
        return Vec::new();
    }
    let Some(kind) = phrase_text(source, Form::Subject) else {
        return Vec::new();
    };
    let spell = if kind.ends_with(" spell") {
        kind
    } else {
        format!("{kind} spell")
    };
    let plain = Ability::Triggered {
        when: (**inner).clone(),
        intervening_if: intervening_if.clone(),
        limit: *limit,
        from: *from,
        targets: targets.clone(),
        effects: effects.clone(),
    };
    crate::compile::written_variants(&plain)
        .into_iter()
        .filter_map(|text| {
            let rest = text.strip_prefix("Whenever you gain life, ")?;
            Some(format!(
                "Whenever {} {spell} causes you to gain life, {rest}",
                article(&spell)
            ))
        })
        .collect()
}

/// "Whenever the final chapter ability of a Saga you control resolves, ...": the head of the event, then the steps as
/// the twin entering trigger words them, or a drain of the Saga's mana value ("each opponent loses X life and you gain X
/// life, where X is that Saga's mana value.").
fn final_chapter(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when: TriggerPattern::FinalChapter { what, resolves },
        intervening_if: None,
        limit,
        from,
        targets,
        effects,
    } = ability
    else {
        return Vec::new();
    };
    let Some(saga) = phrase_text(what, Form::Subject) else {
        return Vec::new();
    };
    let head = format!(
        "Whenever the final chapter ability of {} {saga} {},",
        article(&saga),
        if *resolves { "resolves" } else { "triggers" }
    );
    let mana_value = Amount::Stat {
        of: Ref::Triggering,
        stat: StatKind::ManaValue,
    };
    if targets.is_empty()
        && *effects
            == [
                Effect::LoseLife {
                    who: Ref::Opponents,
                    amount: mana_value.clone(),
                },
                Effect::GainLife {
                    who: Ref::You,
                    amount: mana_value,
                },
            ]
    {
        return vec![format!(
            "{head} each opponent loses X life and you gain X life, where X is that Saga's mana value."
        )];
    }
    let twin = Ability::Triggered {
        when: TriggerPattern::ZoneChange {
            what: crate::refs::Selector::this(),
            from: None,
            to: Some(crate::refs::ZoneKind::Battlefield),
        },
        intervening_if: None,
        limit: *limit,
        from: *from,
        targets: targets.clone(),
        effects: effects.clone(),
    };
    crate::compile::written_variants(&twin)
        .into_iter()
        .filter_map(|text| {
            let rest = text.strip_prefix("When ~ enters, ")?;
            Some(format!("{head} {rest}"))
        })
        .collect()
}

/// "Whenever you attack a player, tap target creature that player controls and goad it.": the head of the event, then the
/// tap and goad of the one target, a creature the attacked player controls.
fn tap_and_goad(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        targets, effects, ..
    } = ability
    else {
        return Vec::new();
    };
    let [
        Effect::Tap {
            what: Ref::Target(0),
            bind: None,
        },
        Effect::Goad {
            what: Ref::Target(0),
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    let creature = crate::refs::Selector(vec![
        crate::refs::Pred::Type(mtg_core::card::CardType::Creature),
        crate::refs::Pred::ControlledBy(Ref::Against),
    ]);
    let [spec] = targets.as_slice() else {
        return Vec::new();
    };
    if spec.what != Targetable::Object(creature) || !spec.bounds.is_one() || spec.divide.is_some() {
        return Vec::new();
    }
    r288_heads(ability)
        .into_iter()
        .map(|head| format!("{head} tap target creature that player controls and goad it."))
        .collect()
}

/// "Whenever an opponent attacks another one of your opponents, you and the attacking player each draw a card and lose 1
/// life.".
fn attacks_another_opponent(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when:
            TriggerPattern::PlayerAttacks {
                by: Ref::Opponents,
                at: Ref::Opponents,
            },
        intervening_if: None,
        limit: None,
        targets,
        effects,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let one = Amount::Fixed(1);
    let expected = [
        Effect::Draw {
            who: Ref::You,
            count: one.clone(),
            bind: None,
        },
        Effect::Draw {
            who: Ref::Triggering,
            count: one.clone(),
            bind: None,
        },
        Effect::LoseLife {
            who: Ref::You,
            amount: one.clone(),
        },
        Effect::LoseLife {
            who: Ref::Triggering,
            amount: one,
        },
    ];
    if !targets.is_empty() || *effects != expected {
        return Vec::new();
    }
    vec![
        "Whenever an opponent attacks another one of your opponents, you and the attacking player each draw a card and lose 1 life."
            .to_owned(),
    ]
}

/// "Visit — Return a creature you don't control with the lowest toughness among creatures you don't control to its
/// owner's hand.": one creature of a group with the least of a stat in it, chosen and returned to its owner's hand.
fn lowest_returned(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        when: TriggerPattern::Visit {},
        intervening_if: None,
        limit: None,
        targets,
        effects,
        ..
    } = ability
    else {
        return Vec::new();
    };
    let [
        Effect::Choose {
            who: Ref::You,
            from: Ref::Each(chosen),
            count,
            bind,
            rest_as: None,
        },
        Effect::Move {
            what: Ref::Bound(moved),
            to: crate::effect::Destination::Hand,
            ..
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    if !targets.is_empty() || bind != moved || !count.is_one() {
        return Vec::new();
    }
    let Some((crate::refs::Pred::LeastAmong { stat, group }, rest)) = chosen.0.split_last() else {
        return Vec::new();
    };
    if rest != group.0.as_slice() {
        return Vec::new();
    }
    let stat = match stat {
        StatKind::Toughness => "toughness",
        StatKind::Power => "power",
        _ => return Vec::new(),
    };
    let (Some(one), Some(all)) = (phrase_text(group, Form::Subject), phrase_text(group, Form::All))
    else {
        return Vec::new();
    };
    vec![format!(
        "Visit \u{2014} Return {} {one} with the lowest {stat} among {all} to its owner's hand.",
        article(&one)
    )]
}

/// "distribute seven +1/+1 counters among any number of target creatures you control".
fn distributed(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let [
        Effect::PutCounters {
            on: Ref::Target(0),
            counter,
            count: Amount::Divided,
        },
    ] = effects
    else {
        return None;
    };
    let [spec] = targets else { return None };
    let total = super::w4e_counters::w4e_counter_phrase(spec.divide.as_ref()?, counter)?;
    let among = super::super::divided::among_text(spec)?;
    Some(format!("distribute {total} among {among}"))
}

/// "target player", "up to one target player", "target opponent".
fn player_spec(spec: &TargetSpec) -> Option<String> {
    let noun = match spec.what {
        Targetable::Player => "player",
        Targetable::Opponent => "opponent",
        _ => return None,
    };
    if spec.divide.is_some() {
        return None;
    }
    if spec.bounds.is_one() {
        return Some(format!("target {noun}"));
    }
    (spec.bounds.min == Amount::Fixed(0) && spec.bounds.max == Some(Amount::Fixed(1)))
        .then(|| format!("up to one target {noun}"))
}

/// "up to one target player's life total becomes half their starting life total, rounded down".
fn life_becomes(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        targets, effects, ..
    } = ability
    else {
        return Vec::new();
    };
    let [Effect::SetLife { who, amount }] = effects.as_slice() else {
        return Vec::new();
    };
    let (whose, their) = match who {
        Ref::You if targets.is_empty() => ("your".to_owned(), "your"),
        Ref::Target(0) => match targets.as_slice() {
            [spec] => match player_spec(spec) {
                Some(said) => (format!("{said}'s"), "their"),
                None => return Vec::new(),
            },
            _ => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    let value = match amount {
        Amount::Half { of, up } if **of == Amount::StartingLife(who.clone()) => format!(
            "half {their} starting life total, rounded {}",
            if *up { "up" } else { "down" }
        ),
        _ => return Vec::new(),
    };
    r288_heads(ability)
        .into_iter()
        .map(|head| format!("{head} {whose} life total becomes {value}."))
        .collect()
}

/// "you may draw X cards, where X is the number of counters on ~. If you do, you lose X life.".
fn draw_counted_then_lose(ability: &Ability) -> Vec<String> {
    let Ability::Triggered {
        targets, effects, ..
    } = ability
    else {
        return Vec::new();
    };
    let [
        Effect::May {
            who: Ref::You,
            effects,
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    let [
        Effect::Draw {
            who: Ref::You,
            count,
            bind: None,
        },
        Effect::LoseLife {
            who: Ref::You,
            amount,
        },
    ] = effects.as_slice()
    else {
        return Vec::new();
    };
    if count != amount || !targets.is_empty() {
        return Vec::new();
    }
    let counted = match count {
        Amount::AnyCounters(Ref::This) => "the number of counters on ~",
        _ => return Vec::new(),
    };
    r288_heads(ability)
        .into_iter()
        .map(|head| {
            format!(
                "{head} you may draw X cards, where X is {counted}. If you do, you lose X life."
            )
        })
        .collect()
}

/// "Whenever you cast a spell this turn, put a +1/+1 counter on target creature you control.": a delayed trigger that
/// lasts the turn and chooses targets of its own, from the wordings of the triggered ability it stands for.
fn delayed_this_turn(effects: &[Effect]) -> Vec<String> {
    let [
        Effect::DelayedTrigger {
            when,
            once: false,
            this_turn: false,
            until: None,
            targets,
            effects,
        },
    ] = effects
    else {
        return Vec::new();
    };
    if targets.is_empty() {
        return Vec::new();
    }
    let twin = Ability::Triggered {
        when: when.clone(),
        intervening_if: None,
        limit: None,
        from: crate::refs::ZoneKind::Battlefield,
        targets: targets.clone(),
        effects: effects.clone(),
    };
    crate::compile::written_variants(&twin)
        .into_iter()
        .filter_map(|text| {
            let (head, rest) = text.split_once(", ")?;
            head.starts_with("Whenever ")
                .then(|| format!("{head} this turn, {rest}"))
        })
        .collect()
}
