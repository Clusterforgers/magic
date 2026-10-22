//! Backup (CR 702.165): "When this creature enters, put N +1/+1 counters on target creature. If that's another
//! creature, it gains the abilities listed after the backup ability until end of turn."
//!
//! The compiler lists those abilities in the keyword; a keyword among them is granted as a keyword, any other as an
//! ability of the target.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Duration, Modification};
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, Condition, CounterKind};
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of backup.
#[derive(Debug)]
pub(super) struct Backup;

/// What the target gains for each ability listed after the keyword.
fn grants(abilities: &[Ability]) -> Vec<Modification> {
    abilities
        .iter()
        .map(|ability| match ability {
            Ability::Keyword(keyword) => Modification::AddKeyword(keyword.clone()),
            other => Modification::GrantAbility(Arc::new(other.clone())),
        })
        .collect()
}

impl KeywordRules for Backup {
    /// The target may be the creature itself, which only gets the counters; another creature gets the abilities too.
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Backup { count, abilities } = keyword else {
            return Vec::new();
        };
        let mut effects = vec![Effect::PutCounters {
            on: Ref::Target(0),
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(i32::try_from(*count).unwrap_or(i32::MAX)),
        }];
        let mods = grants(abilities);
        if !mods.is_empty() {
            effects.push(Effect::If {
                condition: Condition::Matches {
                    subject: Ref::Target(0),
                    selector: Selector(vec![Pred::Other]),
                },
                then: vec![Effect::Continuous {
                    affected: Ref::Target(0),
                    mods,
                    duration: Duration::UntilEndOfTurn,
                }],
                otherwise: Vec::new(),
            });
        }
        vec![Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector::this(),
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: vec![TargetSpec {
                what: Targetable::Object(Selector(vec![Pred::Type(CardType::Creature)])),
                bounds: Bounds::default(),
                divide: None,
            }],
            effects,
        }]
    }
}
