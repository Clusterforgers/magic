//! Cards that control a player or a spell (CR 722): a turn taken over, a turn taken over and given back twice, a tutor
//! that searches, a permanent that controls its opponents while they search, and a spell that takes another spell.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::{Destination, Effect, NewTargets};
use mtg_cards::modify::{Duration, GameRule, Modification};
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, Targetable, ZoneKind};
use mtg_core::card::CardType;

use crate::cards::{cost, one_target};

/// A sorcery for {0}: "You control target player during that player's next turn." With `extra_turn`, "After that
/// turn, that player takes an extra turn."
pub fn mindslave(name: &str, extra_turn: bool) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Player)],
                effects: vec![Effect::ControlPlayer {
                    who: Ref::Target(0),
                    extra_turn,
                }],
            }),
    )
}

/// A sorcery for {0}: "Search your library for a card, put it into your hand, then shuffle."
pub fn tutor(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![
                    Effect::Search {
                        who: Ref::You,
                        of: None,
                        zone: ZoneKind::Library,
                        also: Vec::new(),
                        filter: Selector(Vec::new()),
                        count: Bounds::default(),
                        reveal: false,
                        distinct: false,
                        bind: "found".into(),
                    },
                    Effect::Shuffle { who: Ref::You },
                    Effect::Move {
                        what: Ref::Bound("found".into()),
                        to: Destination::Hand,
                        bind: None,
                    },
                ],
            }),
    )
}

/// An artifact for {0}: "You control your opponents while they're searching their libraries."
pub fn search_watcher(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Rule {
                rule: GameRule::ControlSearches(Ref::Opponents),
                condition: None,
                from: ZoneKind::Battlefield,
            }),
    )
}

/// An instant for {0}: "Gain control of target spell. You may choose new targets for it."
pub fn spell_thief(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(Ability::Spell {
                targets: vec![one_target(Targetable::Object(Selector(vec![Pred::Spell])))],
                effects: vec![
                    Effect::Continuous {
                        affected: Ref::Target(0),
                        mods: vec![Modification::SetController(Ref::You)],
                        duration: Duration::Forever,
                    },
                    Effect::ChangeTargets {
                        what: Ref::Target(0),
                        to: NewTargets::Choose,
                        onto: None,
                    },
                ],
            }),
    )
}
