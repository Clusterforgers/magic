//! Storm (CR 702.40): "When you cast this spell, copy it for each spell cast before it this turn. You may choose new
//! targets for the copies."
//!
//! The keyword adds a trigger that works from the stack. The number of spells cast before the storm spell, by every
//! player, is the amount of the cast (`Amount::Triggering`), counted when the spell is cast so a spell cast in response
//! does not change it. The trigger puts the copies on the stack when it resolves (`Effect::CopySpell`); a copy is not
//! cast, so it has no storm trigger of its own.

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;

use super::KeywordRules;

/// The rules of storm.
#[derive(Debug)]
pub(super) struct Storm;

impl KeywordRules for Storm {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        vec![Ability::Triggered {
            when: TriggerPattern::SpellCast {
                what: Selector::this(),
                by: Ref::You,
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Stack,
            targets: Vec::new(),
            effects: vec![Effect::CopySpell {
                what: Ref::This,
                count: Amount::Triggering,
                new_targets: true,
                except: Vec::new(),
            }],
        }]
    }
}
