//! Fortify (CR 702.67): "[Cost]: Attach this Fortification to target land you control. Fortify only as a sorcery."

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of fortify.
#[derive(Debug)]
pub(super) struct Fortify;

impl KeywordRules for Fortify {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Fortify(cost) = keyword else {
            return Vec::new();
        };
        vec![Ability::Activated {
            cost: cost.clone(),
            timing: Timing::Sorcery,
            condition: None,
            limit: None,
            by: mtg_cards::ability::Activators::Controller,
            from: ZoneKind::Battlefield,
            targets: vec![TargetSpec {
                what: Targetable::Object(Selector(vec![
                    Pred::Type(CardType::Land),
                    Pred::ControlledBy(Ref::You),
                ])),
                bounds: Bounds::default(),
                divide: None,
            }],
            effects: vec![Effect::Attach {
                what: Ref::This,
                to: Ref::Target(0),
                bind: None,
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Fortify adds one sorcery-speed activated ability for its cost that attaches its source to the land its
    //! controller controls that it targets, and Equip of the same cost targets a creature instead.
    #![allow(clippy::unwrap_used)]
    use super::Fortify;
    use crate::keywords::KeywordRules;
    use mtg_cards::ability::{Ability, Timing};
    use mtg_cards::cost::Cost;
    use mtg_cards::keyword::Keyword;
    use mtg_cards::refs::{Pred, Targetable};
    use mtg_core::card::CardType;
    use rstest::rstest;

    #[rstest]
    #[case::three("{3}")]
    #[case::colored("{1}{W}")]
    fn r5510_fortify_attaches_to_a_land_you_control_as_a_sorcery(#[case] mana: &str) {
        let cost = Cost::mana(mana.parse().unwrap());
        let added = Fortify.adds_abilities(&Keyword::Fortify(cost.clone()));
        let [
            Ability::Activated {
                cost: paid,
                timing,
                targets,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one activated ability: {added:?}");
        };
        assert_eq!((paid, *timing), (&cost, Timing::Sorcery));
        let Targetable::Object(selector) = &targets[0].what else {
            panic!("an object target");
        };
        assert!(selector.0.contains(&Pred::Type(CardType::Land)));
        assert_eq!(Fortify.adds_abilities(&Keyword::Equip(cost)), Vec::new());
    }
}
