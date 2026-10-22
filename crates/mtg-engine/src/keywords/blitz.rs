//! Blitz (CR 702.152): "You may cast this card by paying [cost] rather than its mana cost. If you do, it gains haste and
//! 'When this permanent is put into a graveyard from the battlefield, draw a card.' Sacrifice it at the beginning of the
//! next end step."
//!
//! The cost is an alternative cost chosen with the cast (`ways.rs`). Like dash (`dash.rs`, whose helpers it shares) the
//! keyword adds haste and an enters trigger that binds the permanent and sets up the delayed trigger, here a sacrifice;
//! a third ability draws a card when the permanent dies. All three work only while the permanent was cast that way, so a
//! copy has none of them and a permanent that left and came back is not sacrificed (CR 400.7).

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, CastWay, Condition};

use super::KeywordRules;
use super::dash::{at_next_end_step, haste_when_cast};

/// The name the permanent that was blitzed in is bound to.
const BLITZED: &str = "blitzed";

/// The rules of blitz.
#[derive(Debug)]
pub(super) struct Blitz;

impl KeywordRules for Blitz {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Blitz(_)) {
            return Vec::new();
        }
        vec![
            haste_when_cast(CastWay::Blitz),
            draw_when_it_dies(),
            at_next_end_step(CastWay::Blitz, BLITZED, sacrifice_it()),
        ]
    }
}

/// "When this permanent is put into a graveyard from the battlefield, draw a card." The condition reads the permanent as
/// it was when it died.
fn draw_when_it_dies() -> Ability {
    Ability::Triggered {
        when: TriggerPattern::ZoneChange {
            what: Selector::this(),
            from: Some(ZoneKind::Battlefield),
            to: Some(ZoneKind::Graveyard),
        },
        intervening_if: Some(Condition::CastWith(CastWay::Blitz)),
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![Effect::Draw {
            who: Ref::You,
            count: Amount::one(),
        }],
    }
}

/// "Sacrifice it": the controller of the permanent that was blitzed in sacrifices it, if it is still there.
fn sacrifice_it() -> Effect {
    let blitzed = Ref::Bound(BLITZED.to_owned());
    Effect::Sacrifice {
        who: Ref::ControllerOf(Box::new(blitzed.clone())),
        filter: Selector(vec![Pred::Is(blitzed)]),
        count: Amount::one(),
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Blitz adds haste, a trigger that draws a card when the permanent dies and an enters trigger that sets up the sacrifice at the next end step, each only for a permanent cast for its blitz cost, and no other keyword adds them.
    use mtg_cards::cost::Cost;
    use mtg_cards::modify::Modification;
    use mtg_cards::trigger::Step;
    use rstest::rstest;

    use super::*;

    fn cost_of(mana: &str) -> Cost {
        Cost::mana(mana.parse().unwrap())
    }

    #[rstest]
    #[case::one_red("{R}")]
    #[case::generic_and_colored("{2}{R}")]
    #[case::two_colors("{3}{B}{R}")]
    fn blitz_adds_haste_a_draw_on_death_and_a_sacrifice_at_the_end_step(#[case] mana: &str) {
        let added = Blitz.adds_abilities(&Keyword::Blitz(cost_of(mana)));
        let [
            Ability::Static {
                mods, condition, ..
            },
            Ability::Triggered {
                when: TriggerPattern::ZoneChange { to: dies, .. },
                intervening_if: draws_if,
                effects: draws,
                ..
            },
            Ability::Triggered {
                intervening_if: enters_if,
                effects: enters,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("a static and two triggers, got {added:?}");
        };
        assert_eq!(mods, &[Modification::AddKeyword(Keyword::Haste)]);
        let blitzed = Some(Condition::CastWith(CastWay::Blitz));
        assert_eq!(*condition, blitzed);
        assert_eq!(*draws_if, blitzed);
        assert_eq!(*enters_if, blitzed);
        assert_eq!(*dies, Some(ZoneKind::Graveyard));
        assert!(matches!(draws.as_slice(), [Effect::Draw { .. }]));
        assert!(matches!(
            enters.as_slice(),
            [
                Effect::Choose { .. },
                Effect::DelayedTrigger {
                    when: TriggerPattern::StepBegins {
                        step: Step::End,
                        ..
                    },
                    once: true,
                    effects,
                }
            ] if matches!(effects.as_slice(), [Effect::Sacrifice { .. }])
        ));
    }

    #[rstest]
    #[case::dash(Keyword::Dash(cost_of("{1}")))]
    #[case::flashback(Keyword::Flashback(cost_of("{1}")))]
    #[case::haste(Keyword::Haste)]
    fn no_other_keyword_adds_them(#[case] keyword: Keyword) {
        assert_eq!(Blitz.adds_abilities(&keyword), []);
    }
}
