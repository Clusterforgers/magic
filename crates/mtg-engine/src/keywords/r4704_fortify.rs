//! Fortify (CR 702.67): "[Cost]: Attach this Fortification to target land you control. Activate only as a sorcery."

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
    //! Goal: Fortify adds one activated ability that costs its fortify cost, may be activated only as a sorcery and attaches its source to the one land its controller controls that it targets, and no other keyword adds it.
    use mtg_cards::cost::Cost;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("{3}")]
    #[case("{1}{W}")]
    fn fortify_attaches_to_a_land_you_control_at_sorcery_speed(#[case] cost: &str) {
        let keyword = Keyword::Fortify(Cost::mana(cost.parse().unwrap()));
        let added = Fortify.adds_abilities(&keyword);
        let [
            Ability::Activated {
                cost: paid,
                timing: Timing::Sorcery,
                targets,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one sorcery-speed activated ability");
        };
        assert_eq!(*paid, Cost::mana(cost.parse().unwrap()));
        assert_eq!(
            targets[0].what,
            Targetable::Object(Selector(vec![
                Pred::Type(CardType::Land),
                Pred::ControlledBy(Ref::You)
            ]))
        );
        assert_eq!(
            effects.as_slice(),
            [Effect::Attach {
                what: Ref::This,
                to: Ref::Target(0),
                bind: None
            }]
        );
        assert!(Fortify.adds_abilities(&Keyword::Flying).is_empty());
    }
}
