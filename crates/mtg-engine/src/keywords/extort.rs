//! Extort (CR 702.101): "Whenever you cast a spell, you may pay {W/B}. If you do, each opponent loses 1 life and you
//! gain life equal to the total life lost this way."

use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::mana::ManaCostX;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;
use mtg_core::color::Color;
use mtg_core::mana::{ManaCost, ManaSymbol};

use super::KeywordRules;

/// The rules of extort.
#[derive(Debug)]
pub(super) struct Extort;

impl KeywordRules for Extort {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        let hybrid = ManaCost::from_symbols([ManaSymbol::Hybrid(Color::White, Color::Black)]);
        vec![Ability::Triggered {
            when: TriggerPattern::SpellCast {
                what: Selector::default(),
                by: Ref::You,
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Pay {
                who: Ref::You,
                cost: Cost::mana(ManaCostX::new(0, hybrid)),
                times: Amount::one(),
                paid: vec![
                    Effect::LoseLife {
                        who: Ref::Opponents,
                        amount: Amount::one(),
                    },
                    // Nothing prevents the loss, so the life lost is one for each opponent still in the game.
                    Effect::GainLife {
                        who: Ref::You,
                        amount: Amount::Players(Ref::Opponents),
                    },
                ],
                unpaid: Vec::new(),
            }],
        }]
    }
}
