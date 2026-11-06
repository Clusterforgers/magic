//! "You may cast this as though it had flash. If you cast it any time a sorcery couldn't have been cast, the controller
//! of the permanent it becomes sacrifices it at the beginning of the next cleanup step." (Mirage's flash enchantments)
//!
//! The keyword grants instant timing. The spell remembers whether it was cast off sorcery timing
//! (`Condition::CastOffSorceryTiming`), and an enters trigger reading it binds the permanent that entered and sets up the
//! delayed trigger at the next cleanup step, so a permanent that left and came back is not sacrificed (CR 400.7).
//! A trigger during cleanup gives the players priority and cleanup begins again (CR 514.3a).

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::{Step, TriggerPattern};
use mtg_cards::value::{Amount, Condition};

use super::KeywordRules;

/// The name the permanent that entered is bound to.
const FLASHED: &str = "flashed in";

/// The rules of the flash sacrifice keyword.
#[derive(Debug)]
pub(super) struct FlashSacrifice;

impl KeywordRules for FlashSacrifice {
    fn grants_timing(&self, keyword: &Keyword) -> bool {
        matches!(keyword, Keyword::FlashSacrifice)
    }

    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::FlashSacrifice) {
            return Vec::new();
        }
        let flashed = Ref::Bound(FLASHED.to_owned());
        vec![Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector::this(),
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
            intervening_if: Some(Condition::CastOffSorceryTiming),
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![
                Effect::Choose {
                    who: Ref::You,
                    from: Ref::This,
                    count: Bounds::default(),
                    bind: FLASHED.to_owned(),
                    rest_as: None,
                },
                Effect::DelayedTrigger {
                    when: TriggerPattern::StepBegins {
                        step: Step::Cleanup,
                        whose: Ref::EachPlayer,
                    },
                    once: true,
                    this_turn: false,
                    effects: vec![Effect::Sacrifice {
                        who: Ref::ControllerOf(Box::new(flashed.clone())),
                        filter: Selector(vec![Pred::Is(flashed)]),
                        count: Amount::one(),
                        bind: None,
                    }],
                },
            ],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the flash sacrifice keyword grants instant timing and adds one enters trigger that, only for a spell cast off sorcery timing, sets up a sacrifice of the permanent that entered at the next cleanup step; other keywords get neither.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::flash_sacrifice(Keyword::FlashSacrifice, true)]
    #[case::flash(Keyword::Flash, false)]
    #[case::flying(Keyword::Flying, false)]
    fn only_the_keyword_grants_timing_and_the_cleanup_sacrifice(
        #[case] keyword: Keyword,
        #[case] has: bool,
    ) {
        assert_eq!(FlashSacrifice.grants_timing(&keyword), has);
        let added = FlashSacrifice.adds_abilities(&keyword);
        assert_eq!(added.len(), usize::from(has));
        if let [
            Ability::Triggered {
                intervening_if,
                effects,
                ..
            },
        ] = added.as_slice()
        {
            assert_eq!(*intervening_if, Some(Condition::CastOffSorceryTiming));
            assert!(matches!(
                &effects[1],
                Effect::DelayedTrigger {
                    when: TriggerPattern::StepBegins {
                        step: Step::Cleanup,
                        ..
                    },
                    once: true,
                    ..
                }
            ));
        }
    }
}
