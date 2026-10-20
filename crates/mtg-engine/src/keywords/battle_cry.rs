//! Battle cry (CR 702.91): "Whenever this creature attacks, each other attacking creature gets +1/+0 until end of turn."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Duration, Modification};
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of battle cry.
#[derive(Debug)]
pub(super) struct BattleCry;

impl KeywordRules for BattleCry {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        vec![Ability::Triggered {
            when: TriggerPattern::Attacks {
                what: Selector::this(),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Continuous {
                affected: Ref::Each(Selector(vec![
                    Pred::Type(CardType::Creature),
                    Pred::Attacking,
                    Pred::Other,
                ])),
                mods: vec![Modification::ModifyPT {
                    power: Amount::Fixed(1),
                    toughness: Amount::Fixed(0),
                }],
                duration: Duration::UntilEndOfTurn,
            }],
        }]
    }
}
