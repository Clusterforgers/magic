//! Exalted (CR 702.83): "Whenever a creature you control attacks alone, that creature gets +1/+1 until end of turn."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Duration, Modification};
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of exalted.
#[derive(Debug)]
pub(super) struct Exalted;

impl KeywordRules for Exalted {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        vec![Ability::Triggered {
            when: TriggerPattern::AttacksAlone {
                what: Selector(vec![
                    Pred::Type(CardType::Creature),
                    Pred::ControlledBy(Ref::You),
                ]),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Continuous {
                affected: Ref::Triggering,
                mods: vec![Modification::ModifyPT {
                    power: Amount::Fixed(1),
                    toughness: Amount::Fixed(1),
                }],
                duration: Duration::UntilEndOfTurn,
            }],
        }]
    }
}
