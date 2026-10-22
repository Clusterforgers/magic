//! Fading (CR 702.32): "This permanent enters with N fade counters on it" and "At the beginning of your upkeep,
//! remove a fade counter from it. If you can't, sacrifice it."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::value::{Amount, Cmp, CounterKind};

use super::KeywordRules;
use super::vanishing::{counters_are, enters_with, upkeep};

/// The rules of fading.
#[derive(Debug)]
pub(super) struct Fading;

/// The kind of counter fading counts.
fn fade() -> CounterKind {
    CounterKind::Other("fade".to_owned())
}

impl KeywordRules for Fading {
    /// One trigger decides as it resolves: with a fade counter on the permanent one is removed, without it is
    /// sacrificed, so a counter put on in response saves it.
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Fading(count) = keyword else {
            return Vec::new();
        };
        let mut abilities: Vec<Ability> = enters_with(fade(), *count).into_iter().collect();
        abilities.push(Ability::Triggered {
            when: upkeep(),
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::If {
                condition: counters_are(&fade(), Cmp::Ge, 1),
                then: vec![Effect::RemoveCounters {
                    from: Ref::This,
                    counter: fade(),
                    count: Amount::one(),
                }],
                otherwise: vec![Effect::Sacrifice {
                    who: Ref::You,
                    filter: Selector::this(),
                    count: Amount::one(),
                }],
            }],
        });
        abilities
    }
}
