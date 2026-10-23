//! Unearth (CR 702.84): "[Cost]: Return this card from your graveyard to the battlefield. It gains haste. Exile it at the
//! beginning of the next end step or if it would leave the battlefield. Activate only as a sorcery."
//!
//! An activated ability that works from a graveyard. The card is bound as it leaves, so every later step reads the
//! permanent that arrived and nothing else: a card that left the graveyard in response does nothing (CR 400.7). The
//! permanent is given haste and a replacement effect that exiles it instead of letting it leave the battlefield, and
//! a delayed trigger exiles it at the next end step.

use std::sync::Arc;

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::Cost;
use mtg_cards::effect::{Destination, Effect};
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Duration, Modification};
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen, Step, TriggerPattern};
use mtg_cards::value::Condition;

use super::KeywordRules;

/// The name the card unearthed is bound to.
const UNEARTHED: &str = "unearthed";

/// The rules of unearth.
#[derive(Debug)]
pub(super) struct Unearth;

fn unearthed() -> Ref {
    Ref::Bound(UNEARTHED.to_owned())
}

impl KeywordRules for Unearth {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Unearth(cost) = keyword else {
            return Vec::new();
        };
        vec![activation(cost)]
    }
}

/// The ability the keyword adds for `cost`.
fn activation(cost: &Cost) -> Ability {
    Ability::Activated {
        cost: cost.clone(),
        timing: Timing::Sorcery,
        condition: None,
        limit: None,
        from: ZoneKind::Graveyard,
        targets: Vec::new(),
        effects: vec![
            Effect::Choose {
                who: Ref::You,
                from: Ref::This,
                count: Bounds::default(),
                bind: UNEARTHED.to_owned(),
                rest_as: None,
            },
            Effect::Move {
                what: unearthed(),
                to: Destination::Battlefield {
                    tapped: false,
                    under: None,
                    transformed: false,
                    counters: Vec::new(),
                },
            },
            // Only the permanent that arrived is in play: a card that was moved away in response stays where it is.
            Effect::If {
                condition: Condition::Matches {
                    subject: unearthed(),
                    selector: Selector(vec![Pred::In(ZoneKind::Battlefield)]),
                },
                then: vec![
                    Effect::Continuous {
                        affected: unearthed(),
                        mods: vec![
                            Modification::AddKeyword(Keyword::Haste),
                            Modification::GrantAbility(Arc::new(exile_instead_of_leaving())),
                        ],
                        duration: Duration::Forever,
                    },
                    Effect::DelayedTrigger {
                        when: TriggerPattern::StepBegins {
                            step: Step::End,
                            whose: Ref::EachPlayer,
                        },
                        once: true,
                        effects: vec![Effect::Exile {
                            what: unearthed(),
                            bind: None,
                        }],
                    },
                ],
                otherwise: Vec::new(),
            },
        ],
    }
}

/// "If it would leave the battlefield, exile it instead of putting it anywhere else."
fn exile_instead_of_leaving() -> Ability {
    Ability::Replacement {
        when: ReplaceWhen::ZoneChange {
            what: Selector::this(),
            from: Some(ZoneKind::Battlefield),
            to: None,
        },
        by: ReplaceBy::MoveTo(ZoneKind::Exile),
        condition: None,
        from: ZoneKind::Battlefield,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Unearth adds one activated ability that works from a graveyard at sorcery speed for the unearth cost, binds the card, returns it, and gives the permanent that arrived haste, a replacement that exiles it when it would leave and a delayed exile at the next end step, and no other keyword adds it.
    use mtg_cards::cost::CostItem;
    use mtg_cards::value::Amount;
    use rstest::rstest;

    use super::*;

    fn cost_of(mana: &str) -> Cost {
        Cost::mana(mana.parse().unwrap())
    }

    #[rstest]
    #[case::one_black(cost_of("{B}"))]
    #[case::generic_and_colored(cost_of("{2}{R}"))]
    #[case::three_colors(cost_of("{U}{B}{R}"))]
    #[case::life(Cost(vec![CostItem::PayLife(Amount::Fixed(2))]))]
    fn unearth_activates_from_a_graveyard_at_sorcery_speed_for_its_cost(#[case] cost: Cost) {
        let added = Unearth.adds_abilities(&Keyword::Unearth(cost.clone()));
        let [
            Ability::Activated {
                cost: paid,
                timing,
                from,
                targets,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one activated ability, got {added:?}");
        };
        assert_eq!(paid, &cost);
        assert_eq!(*timing, Timing::Sorcery);
        assert_eq!(*from, ZoneKind::Graveyard);
        assert_eq!(*targets, Vec::new());
        assert!(matches!(
            effects.as_slice(),
            [
                Effect::Choose { .. },
                Effect::Move { .. },
                Effect::If { .. }
            ]
        ));
    }

    #[test]
    fn the_permanent_gets_haste_a_leaving_replacement_and_an_end_step_exile() {
        let Ability::Activated { effects, .. } = activation(&cost_of("{B}")) else {
            panic!("activated");
        };
        let Some(Effect::If { then, .. }) = effects.last() else {
            panic!("an if");
        };
        let [
            Effect::Continuous { mods, duration, .. },
            Effect::DelayedTrigger { when, once, .. },
        ] = then.as_slice()
        else {
            panic!("a continuous effect and a delayed trigger, got {then:?}");
        };
        assert_eq!(*duration, Duration::Forever);
        assert!(mods.contains(&Modification::AddKeyword(Keyword::Haste)));
        assert!(mods.iter().any(|m| matches!(
            m,
            Modification::GrantAbility(granted) if matches!(
                **granted,
                Ability::Replacement { by: ReplaceBy::MoveTo(ZoneKind::Exile), .. }
            )
        )));
        assert!(*once);
        assert!(matches!(
            when,
            TriggerPattern::StepBegins {
                step: Step::End,
                whose: Ref::EachPlayer
            }
        ));
    }

    #[rstest]
    #[case::flashback(Keyword::Flashback(cost_of("{1}")))]
    #[case::cycling(Keyword::Cycling(cost_of("{1}")))]
    #[case::haste(Keyword::Haste)]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Unearth.adds_abilities(&keyword), []);
    }
}
