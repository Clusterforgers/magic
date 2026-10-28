//! Dash (CR 702.109): "You may cast this card for its dash cost. If you do, it gains haste, and it's returned from the
//! battlefield to its owner's hand at the beginning of the next end step."
//!
//! The cost is an alternative cost chosen with the cast (`ways.rs`). The keyword adds a static ability that gives the
//! permanent haste while it was cast that way, and an enters trigger that binds the permanent and sets up the delayed
//! trigger that returns it. The bound object is the one that entered, so a permanent that left and came back is not
//! returned (CR 400.7).

use mtg_cards::ability::Ability;
use mtg_cards::effect::{Destination, Effect};
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Modification;
use mtg_cards::refs::{Bounds, Ref, Selector, ZoneKind};
use mtg_cards::trigger::{Step, TriggerPattern};
use mtg_cards::value::{CastWay, Condition};

use super::KeywordRules;

/// The name the permanent that was dashed in is bound to.
const DASHED: &str = "dashed";

/// The rules of dash.
#[derive(Debug)]
pub(super) struct Dash;

impl KeywordRules for Dash {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Dash(_)) {
            return Vec::new();
        }
        vec![
            haste_when_cast(CastWay::Dash),
            at_next_end_step(
                CastWay::Dash,
                DASHED,
                Effect::Move {
                    what: Ref::Bound(DASHED.to_owned()),
                    to: Destination::Hand,
                },
            ),
        ]
    }
}

/// "If its dash (or blitz) cost was paid, it has haste."
pub(super) fn haste_when_cast(way: CastWay) -> Ability {
    Ability::Static {
        affected: Ref::This,
        mods: vec![Modification::AddKeyword(Keyword::Haste)],
        condition: Some(Condition::CastWith(way)),
        defining: false,
        from: ZoneKind::Battlefield,
    }
}

/// "When it enters, if it was cast for its dash (or blitz) cost, `effect` happens at the beginning of the next end step."
/// The permanent that entered is bound as `name`, so one that left and came back is not touched (CR 400.7).
pub(super) fn at_next_end_step(way: CastWay, name: &str, effect: Effect) -> Ability {
    Ability::Triggered {
        when: TriggerPattern::ZoneChange {
            what: Selector::this(),
            from: None,
            to: Some(ZoneKind::Battlefield),
        },
        intervening_if: Some(Condition::CastWith(way)),
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![
            Effect::Choose {
                who: Ref::You,
                from: Ref::This,
                count: Bounds::default(),
                bind: name.to_owned(),
                rest_as: None,
            },
            Effect::DelayedTrigger {
                when: TriggerPattern::StepBegins {
                    step: Step::End,
                    whose: Ref::EachPlayer,
                },
                once: true,
                this_turn: false,
                effects: vec![effect],
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Dash adds a static ability of haste and an enters trigger that sets up the return to the hand at the next end step, both only for a permanent cast for its dash cost, and no other keyword adds them.
    use mtg_cards::cost::Cost;
    use rstest::rstest;

    use super::*;

    fn cost_of(mana: &str) -> Cost {
        Cost::mana(mana.parse().unwrap())
    }

    #[rstest]
    #[case::one_red("{R}")]
    #[case::generic_and_colored("{2}{R}")]
    #[case::two_colors("{3}{B}{R}")]
    fn dash_adds_haste_and_an_end_step_return_for_a_dashed_permanent(#[case] mana: &str) {
        let added = Dash.adds_abilities(&Keyword::Dash(cost_of(mana)));
        let [
            Ability::Static {
                mods, condition, ..
            },
            Ability::Triggered {
                intervening_if,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("a static and a trigger, got {added:?}");
        };
        assert_eq!(mods, &[Modification::AddKeyword(Keyword::Haste)]);
        assert_eq!(*condition, Some(Condition::CastWith(CastWay::Dash)));
        assert_eq!(*intervening_if, Some(Condition::CastWith(CastWay::Dash)));
        assert!(matches!(
            effects.as_slice(),
            [
                Effect::Choose { .. },
                Effect::DelayedTrigger {
                    when: TriggerPattern::StepBegins {
                        step: Step::End,
                        ..
                    },
                    once: true,
                    ..
                }
            ]
        ));
    }

    #[rstest]
    #[case::flashback(Keyword::Flashback(cost_of("{1}")))]
    #[case::evoke(Keyword::Evoke(cost_of("{1}")))]
    #[case::haste(Keyword::Haste)]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Dash.adds_abilities(&keyword), []);
    }
}
