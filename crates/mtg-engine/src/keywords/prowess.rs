//! Prowess (CR 702.108): "Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Duration, Modification};
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of prowess.
#[derive(Debug)]
pub(super) struct Prowess;

impl KeywordRules for Prowess {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        vec![Ability::Triggered {
            when: TriggerPattern::SpellCast {
                what: Selector(vec![Pred::Not(Box::new(Pred::Type(CardType::Creature)))]),
                by: Ref::You,
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Continuous {
                affected: Ref::This,
                mods: vec![Modification::ModifyPT {
                    power: Amount::Fixed(1),
                    toughness: Amount::Fixed(1),
                }],
                duration: Duration::UntilEndOfTurn,
            }],
        }]
    }
}
