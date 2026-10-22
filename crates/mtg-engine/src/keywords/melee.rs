//! Melee (CR 702.121): "Whenever this creature attacks, it gets +1/+1 until end of turn for each opponent you attacked
//! with a creature this combat."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Duration, Modification};
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;

use super::KeywordRules;

/// The rules of melee.
#[derive(Debug)]
pub(super) struct Melee;

impl KeywordRules for Melee {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        let opponents_attacked = Amount::Players(Ref::Attacked);
        vec![Ability::Triggered {
            when: TriggerPattern::Attacks {
                what: Selector::this(),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Continuous {
                affected: Ref::This,
                mods: vec![Modification::ModifyPT {
                    power: opponents_attacked.clone(),
                    toughness: opponents_attacked,
                }],
                duration: Duration::UntilEndOfTurn,
            }],
        }]
    }
}
