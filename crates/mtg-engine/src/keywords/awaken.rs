//! Awaken (CR 702.113): "Awaken N—[cost]" is "You may pay [cost] rather than this spell's mana cost" and "If this spell's
//! awaken cost was paid, also put N +1/+1 counters on target land you control. That land becomes a 0/0 Elemental creature
//! with haste. It's still a land."
//!
//! The cost is an alternative cost chosen with the cast (`ways.rs`, `CastWay::Awaken`). The spell ability gains the land
//! as its last target, asked for only when the spell is cast that way (`Amount::CastWith` in the most of the
//! specification), and the effects that follow its own (`awakened`, applied by `resolve::spell_ability`).

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Duration, Modification};
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable};
use mtg_cards::value::{Amount, CastWay, Condition, CounterKind};
use mtg_core::card::{CardType, Subtype};

use super::KeywordRules;
use crate::ability_reader;
use crate::object::GameObject;

/// The rules of awaken.
#[derive(Debug)]
pub(super) struct Awaken;

impl KeywordRules for Awaken {}

/// The spell ability of `card` with the awaken part its keyword adds: the land target and, behind the effects of the spell,
/// the counters and the animation when the spell was cast for the awaken cost. Other rules come back as they are.
pub(crate) fn awakened(card: &GameObject, rules: Arc<Ability>) -> Arc<Ability> {
    let Some(count) = ability_reader::printed_rules(card.card()).find_map(|rule| match &**rule {
        Ability::Keyword(Keyword::Awaken { count, .. }) => Some(*count),
        _ => None,
    }) else {
        return rules;
    };
    let Ability::Spell { targets, effects } = &*rules else {
        return rules;
    };
    let land = Ref::Target(u8::try_from(targets.len()).unwrap_or(u8::MAX));
    let mut targets = targets.clone();
    targets.push(TargetSpec {
        what: Targetable::Object(Selector(vec![
            Pred::Type(CardType::Land),
            Pred::ControlledBy(Ref::You),
        ])),
        bounds: Bounds {
            min: Amount::one(),
            max: Some(Amount::CastWith(CastWay::Awaken)),
        },
        divide: None,
    });
    let mut effects = effects.clone();
    effects.push(Effect::If {
        condition: Condition::CastWith(CastWay::Awaken),
        then: vec![
            Effect::PutCounters {
                on: land.clone(),
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(i32::try_from(count).unwrap_or(i32::MAX)),
            },
            Effect::Continuous {
                affected: land,
                mods: vec![
                    Modification::AddType(CardType::Creature),
                    Modification::AddSubtype(Subtype("Elemental".to_owned())),
                    Modification::SetPT {
                        power: Amount::Fixed(0),
                        toughness: Amount::Fixed(0),
                    },
                    Modification::AddKeyword(Keyword::Haste),
                ],
                duration: Duration::Forever,
            },
        ],
        otherwise: Vec::new(),
    });
    Arc::new(Ability::Spell { targets, effects })
}
