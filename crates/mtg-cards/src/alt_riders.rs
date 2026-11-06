//! The alternative-cost keywords written as an `AltCast` with the abilities that read how the card was cast (warp,
//! surge, web-slinging, impending): the shapes the translator and the tests write, and the keyword line they stand for.

use crate::ability::Ability;
use crate::cost::{Cost, CostItem};
use crate::effect::Effect;
use crate::mana::ManaCostX;
use crate::modify::{CastPermission, Duration, GameRule, Modification};
use crate::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen, Step, TriggerPattern};
use crate::value::{Amount, CastWay, Cmp, Condition, CounterKind, TurnTally};
use mtg_core::card::CardType;

/// The name the warped permanent is bound to.
const WARPED: &str = "warped";

/// The name the warped card is bound to once it lies in exile.
const EXILED: &str = "warped_card";

/// "You cast another spell this turn": the spells the caster cast, counted before the one being cast.
pub fn spell_cast_this_turn() -> Condition {
    Condition::Compare {
        left: Amount::Tallied {
            who: Ref::You,
            what: TurnTally::SpellsCast,
        },
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    }
}

/// The part of a web-slinging cost beyond its mana: return a tapped creature you control to its owner's hand.
pub fn web_slinging_return() -> CostItem {
    CostItem::ReturnToHand {
        filter: Selector(vec![
            Pred::Type(CardType::Creature),
            Pred::ControlledBy(Ref::You),
            Pred::Tapped,
        ]),
        count: Amount::one(),
    }
}

/// The part of a sneak cost beyond its mana: return an unblocked attacker you control to its owner's hand.
pub fn sneak_return() -> CostItem {
    CostItem::ReturnToHand {
        filter: Selector(vec![
            Pred::Attacking,
            Pred::Unblocked,
            Pred::Type(CardType::Creature),
            Pred::ControlledBy(Ref::You),
        ]),
        count: Amount::one(),
    }
}

/// Sneak: the permanent enters tapped; it joins the attack of the creature the cost returned as it resolves.
pub fn sneak_riders() -> Vec<Ability> {
    vec![Ability::Replacement {
        when: ReplaceWhen::Enters {
            what: Selector::this(),
        },
        by: ReplaceBy::Tapped,
        condition: Some(Condition::CastWith(CastWay::Sneak)),
        from: ZoneKind::Battlefield,
    }]
}

/// The alternative cost of the keyword with its mana and the conditions it carries: the `AltCast` a `Warp`, `Surge`,
/// `Web-slinging` or `Impending` line stands for.
pub fn alt_cast(way: CastWay, mana: ManaCostX) -> Ability {
    let mut items = vec![CostItem::Mana(mana)];
    if way == CastWay::WebSlinging {
        items.push(web_slinging_return());
    }
    if way == CastWay::Sneak {
        items.push(sneak_return());
    }
    Ability::AltCast {
        way,
        cost: Some(Cost(items)),
        from: ZoneKind::Hand,
        condition: match way {
            CastWay::Surge => Some(spell_cast_this_turn()),
            CastWay::Sneak => Some(Condition::Step(Step::DeclareBlockers)),
            _ => None,
        },
        flash: way == CastWay::Sneak,
    }
}

/// Warp: "At the beginning of the next end step, exile it. You may cast it from exile on a later turn."
pub fn warp_riders() -> Vec<Ability> {
    let exile_then_cast = vec![
        Effect::Exile {
            what: Ref::Bound(WARPED.to_owned()),
            bind: Some(EXILED.to_owned()),
            duration: None,
        },
        Effect::Rule {
            rule: GameRule::MayCast(CastPermission {
                who: Ref::You,
                what: Selector::default(),
                from: Some(ZoneKind::Exile),
                free: false,
                flash: false,
                only: Some(Ref::Bound(EXILED.to_owned())),
                play: false,
                once: false,
                additional: None,
                spend_as: crate::modify::SpendAs::Normal,
                when: None,
            }),
            duration: Duration::Forever,
        },
    ];
    vec![Ability::Triggered {
        when: TriggerPattern::ZoneChange {
            what: Selector::this(),
            from: None,
            to: Some(ZoneKind::Battlefield),
        },
        intervening_if: Some(Condition::CastWith(CastWay::Warp)),
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![
            Effect::Choose {
                who: Ref::You,
                from: Ref::This,
                count: Bounds::default(),
                bind: WARPED.to_owned(),
                rest_as: None,
            },
            Effect::DelayedTrigger {
                when: TriggerPattern::StepBegins {
                    step: Step::End,
                    whose: Ref::EachPlayer,
                },
                once: true,
                this_turn: false,
                effects: exile_then_cast,
            },
        ],
    }]
}

fn time() -> CounterKind {
    CounterKind::Other("time".to_owned())
}

/// Impending N: "enters with N time counters", "isn't a creature while it has a time counter".
pub fn impending_riders(count: u32) -> Vec<Ability> {
    vec![
        Ability::Replacement {
            when: ReplaceWhen::Enters {
                what: Selector::this(),
            },
            by: ReplaceBy::WithCounters {
                counter: time(),
                count: Amount::Fixed(i32::try_from(count).unwrap_or(i32::MAX)),
                tapped: false,
            },
            condition: Some(Condition::CastWith(CastWay::Impending)),
            from: ZoneKind::Battlefield,
        },
        Ability::Static {
            affected: Ref::This,
            mods: vec![Modification::RemoveType(CardType::Creature)],
            condition: Some(Condition::Compare {
                left: Amount::Counters {
                    on: Ref::This,
                    counter: time(),
                },
                op: Cmp::Gt,
                right: Amount::Fixed(0),
            }),
            defining: false,
            from: ZoneKind::Battlefield,
        },
    ]
}

/// The abilities after the `AltCast` of a way: its riders.
fn riders(way: CastWay, count: u32) -> Vec<Ability> {
    match way {
        CastWay::Warp => warp_riders(),
        CastWay::Impending => impending_riders(count),
        CastWay::Sneak => sneak_riders(),
        _ => Vec::new(),
    }
}

/// The keyword line the abilities from the first one on write, and how many of them it takes: the `AltCast` of a warp,
/// surge, web-slinging or impending cost followed by exactly its riders. `None` for any other abilities.
pub fn keyword_line(abilities: &[Ability]) -> Option<(String, usize)> {
    let Some(Ability::AltCast {
        way,
        cost: Some(cost),
        from: ZoneKind::Hand,
        condition,
        flash: _,
    }) = abilities.first()
    else {
        return None;
    };
    let name = match way {
        CastWay::Warp => "Warp",
        CastWay::Surge => "Surge",
        CastWay::WebSlinging => "Web-slinging",
        CastWay::Impending => "Impending",
        CastWay::Sneak => "Sneak",
        _ => return None,
    };
    let mana = cost.0.iter().find_map(|item| match item {
        CostItem::Mana(mana) => Some(mana),
        _ => None,
    })?;
    let count = if *way == CastWay::Impending {
        impending_count(abilities.get(1)?)?
    } else {
        0
    };
    if abilities.first() != Some(&alt_cast(*way, mana.clone()))
        || (!matches!(way, CastWay::Surge | CastWay::Sneak) && condition.is_some())
    {
        return None;
    }
    let after = riders(*way, count);
    if abilities.get(1..=after.len()) != Some(after.as_slice()) {
        return None;
    }
    let line = if *way == CastWay::Impending {
        format!("{name} {count}\u{2014}{mana}")
    } else {
        format!("{name} {mana}")
    };
    Some((line, 1 + after.len()))
}

/// The number of time counters an impending replacement puts on the permanent.
fn impending_count(ability: &Ability) -> Option<u32> {
    match ability {
        Ability::Replacement {
            by:
                ReplaceBy::WithCounters {
                    count: Amount::Fixed(count),
                    ..
                },
            ..
        } => u32::try_from(*count).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the keyword line of an `AltCast` with its riders is written only for exactly those abilities.
    use rstest::rstest;

    use super::*;

    fn with_riders(way: CastWay, mana: &str) -> Vec<Ability> {
        let mut all = vec![alt_cast(way, mana.parse().unwrap_or_default())];
        match way {
            CastWay::Warp => all.extend(warp_riders()),
            CastWay::Impending => all.extend(impending_riders(3)),
            CastWay::Sneak => all.extend(sneak_riders()),
            _ => {}
        }
        all
    }

    #[rstest]
    #[case::warp(CastWay::Warp, "{1}{R}", "Warp {1}{R}", 2)]
    #[case::surge(CastWay::Surge, "{3}{R}{R}", "Surge {3}{R}{R}", 1)]
    #[case::web(CastWay::WebSlinging, "{2}{W}", "Web-slinging {2}{W}", 1)]
    #[case::impending(CastWay::Impending, "{2}{U}{U}", "Impending 3\u{2014}{2}{U}{U}", 3)]
    #[case::sneak(CastWay::Sneak, "{W}{B}", "Sneak {W}{B}", 2)]
    fn a_keyword_line_is_written(
        #[case] way: CastWay,
        #[case] mana: &str,
        #[case] line: &str,
        #[case] taken: usize,
    ) {
        let all = with_riders(way, mana);
        assert_eq!(keyword_line(&all), Some((line.to_owned(), taken)));
    }

    #[test]
    fn missing_or_changed_riders_write_nothing() {
        let mut all = with_riders(CastWay::Warp, "{1}{R}");
        all.pop();
        assert_eq!(keyword_line(&all), None);
        let mut other = with_riders(CastWay::Impending, "{1}{G}");
        other.truncate(2);
        assert_eq!(keyword_line(&other), None);
    }
}
