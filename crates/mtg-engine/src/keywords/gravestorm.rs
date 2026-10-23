//! Gravestorm (CR 702.69): "When you cast this spell, copy it for each permanent put into a graveyard from the battlefield
//! this turn. You may choose new targets for the copies."
//!
//! The keyword adds a trigger that works from the stack, like storm's. The number of copies is the count the game keeps of
//! permanents put into graveyards this turn (`Amount::PermanentsGraved`), read as the trigger resolves. A copy is not
//! cast, so it has no gravestorm trigger of its own.

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;

use super::KeywordRules;

/// The rules of gravestorm.
#[derive(Debug)]
pub(super) struct Gravestorm;

impl KeywordRules for Gravestorm {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Gravestorm) {
            return Vec::new();
        }
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
                who: Ref::You,
                what: Ref::This,
                count: Amount::PermanentsGraved,
                new_targets: true,
                except: Vec::new(),
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: gravestorm adds one cast trigger from the stack that copies the spell once for each permanent put into a graveyard this turn and lets the caster choose new targets; no other keyword adds it.
    use super::*;

    #[test]
    fn the_trigger_copies_the_spell_for_each_permanent_in_a_graveyard() {
        let added = Gravestorm.adds_abilities(&Keyword::Gravestorm);
        let [
            Ability::Triggered {
                when,
                from,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one triggered ability: {added:?}");
        };
        assert!(matches!(when, TriggerPattern::SpellCast { .. }));
        assert_eq!(*from, ZoneKind::Stack);
        assert!(matches!(
            effects.as_slice(),
            [Effect::CopySpell {
                count: Amount::PermanentsGraved,
                new_targets: true,
                ..
            }]
        ));
    }

    #[test]
    fn gravestorm_adds_nothing_for_another_keyword() {
        assert_eq!(Gravestorm.adds_abilities(&Keyword::Storm), Vec::new());
    }
}
