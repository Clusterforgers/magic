//! Synthetic cards with triggered abilities: the patterns of the card DSL built in code, and permanents that carry
//! them, for the tests of triggers.

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::{Ability, Limit, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use mtg_cards::trigger::{Step, TriggerPattern};
use mtg_cards::value::Condition;
use mtg_core::card::CardType;

use crate::cards::{cost, gain_life, one_target};

/// The events triggered abilities wait for.
pub mod when {
    use super::{Pred, Ref, Selector, Step, TriggerPattern, ZoneKind};
    use mtg_core::card::CardType;

    /// "When this enters the battlefield".
    pub fn this_enters() -> TriggerPattern {
        TriggerPattern::ZoneChange {
            what: Selector::this(),
            from: None,
            to: Some(ZoneKind::Battlefield),
        }
    }

    /// "When this dies".
    pub fn this_dies() -> TriggerPattern {
        TriggerPattern::ZoneChange {
            what: Selector::this(),
            from: Some(ZoneKind::Battlefield),
            to: Some(ZoneKind::Graveyard),
        }
    }

    /// "Whenever a creature you control dies".
    pub fn your_creature_dies() -> TriggerPattern {
        TriggerPattern::ZoneChange {
            what: Selector(vec![
                Pred::Type(CardType::Creature),
                Pred::ControlledBy(Ref::You),
            ]),
            from: Some(ZoneKind::Battlefield),
            to: Some(ZoneKind::Graveyard),
        }
    }

    /// "Whenever a creature dies".
    pub fn any_creature_dies() -> TriggerPattern {
        TriggerPattern::ZoneChange {
            what: Selector(vec![Pred::Type(CardType::Creature)]),
            from: Some(ZoneKind::Battlefield),
            to: Some(ZoneKind::Graveyard),
        }
    }

    /// "Whenever a land enters the battlefield under your control".
    pub fn your_land_enters() -> TriggerPattern {
        TriggerPattern::ZoneChange {
            what: Selector(vec![
                Pred::Type(CardType::Land),
                Pred::ControlledBy(Ref::You),
            ]),
            from: None,
            to: Some(ZoneKind::Battlefield),
        }
    }

    /// "Whenever a creature enters the battlefield".
    pub fn any_creature_enters() -> TriggerPattern {
        TriggerPattern::ZoneChange {
            what: Selector(vec![Pred::Type(CardType::Creature)]),
            from: None,
            to: Some(ZoneKind::Battlefield),
        }
    }

    /// "Whenever you draw a card".
    pub fn you_draw() -> TriggerPattern {
        TriggerPattern::ZoneChange {
            what: Selector(vec![Pred::OwnedBy(Ref::You)]),
            from: Some(ZoneKind::Library),
            to: Some(ZoneKind::Hand),
        }
    }

    /// "Whenever a player draws a card" (the card is the opponent's too).
    pub fn anyone_draws() -> TriggerPattern {
        TriggerPattern::ZoneChange {
            what: Selector::default(),
            from: Some(ZoneKind::Library),
            to: Some(ZoneKind::Hand),
        }
    }

    /// "Whenever you discard a card".
    pub fn you_discard() -> TriggerPattern {
        TriggerPattern::ZoneChange {
            what: Selector(vec![Pred::OwnedBy(Ref::You)]),
            from: Some(ZoneKind::Hand),
            to: Some(ZoneKind::Graveyard),
        }
    }

    /// "Whenever this attacks".
    pub fn this_attacks() -> TriggerPattern {
        TriggerPattern::Attacks {
            what: Selector::this(),
        }
    }

    /// "Whenever a creature attacks".
    pub fn any_creature_attacks() -> TriggerPattern {
        TriggerPattern::Attacks {
            what: Selector(vec![Pred::Type(CardType::Creature)]),
        }
    }

    /// "Whenever an opponent casts a spell".
    pub fn opponent_casts() -> TriggerPattern {
        TriggerPattern::SpellCast {
            what: Selector::default(),
            by: Ref::Opponents,
        }
    }

    /// "Whenever you cast a spell".
    pub fn you_cast() -> TriggerPattern {
        TriggerPattern::SpellCast {
            what: Selector::default(),
            by: Ref::You,
        }
    }

    /// "Whenever you sacrifice a permanent".
    pub fn you_sacrifice() -> TriggerPattern {
        TriggerPattern::Sacrificed {
            what: Selector::default(),
            by: Ref::You,
        }
    }

    /// "At the beginning of your upkeep".
    pub fn your_upkeep() -> TriggerPattern {
        TriggerPattern::StepBegins {
            step: Step::Upkeep,
            whose: Ref::You,
        }
    }

    /// "At the beginning of each upkeep".
    pub fn each_upkeep() -> TriggerPattern {
        TriggerPattern::StepBegins {
            step: Step::Upkeep,
            whose: Ref::EachPlayer,
        }
    }

    /// "At the beginning of the end step" (any player's).
    pub fn an_end_step() -> TriggerPattern {
        TriggerPattern::StepBegins {
            step: Step::End,
            whose: Ref::EachPlayer,
        }
    }

    /// "Whenever this becomes the target of a spell or ability".
    pub fn this_becomes_a_target() -> TriggerPattern {
        TriggerPattern::BecomesTarget {
            what: Selector::this(),
            by: Selector::default(),
        }
    }

    /// Either of two events: "when this enters the battlefield or attacks".
    pub fn either(first: TriggerPattern, second: TriggerPattern) -> TriggerPattern {
        TriggerPattern::AnyOf(vec![first, second])
    }
}

/// "When ...: effects", a triggered ability with no target, no condition and no limit.
pub fn trigger(when: TriggerPattern, effects: Vec<Effect>) -> Ability {
    Ability::Triggered {
        when,
        intervening_if: None,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects,
    }
}

/// `ability` with one target of the kind `what`.
#[must_use]
pub fn aimed_at(ability: Ability, what: Targetable) -> Ability {
    match ability {
        Ability::Triggered {
            when,
            intervening_if,
            limit,
            from,
            mut targets,
            effects,
        } => {
            targets.push(one_target(what));
            Ability::Triggered {
                when,
                intervening_if,
                limit,
                from,
                targets,
                effects,
            }
        }
        other => other,
    }
}

/// `ability` with an intervening "if" (CR 603.4).
#[must_use]
pub fn only_if(ability: Ability, condition: Condition) -> Ability {
    match ability {
        Ability::Triggered {
            when,
            limit,
            from,
            targets,
            effects,
            ..
        } => Ability::Triggered {
            when,
            intervening_if: Some(condition),
            limit,
            from,
            targets,
            effects,
        },
        other => other,
    }
}

/// `ability` limited to triggering once each turn.
#[must_use]
pub fn once_each_turn(ability: Ability) -> Ability {
    match ability {
        Ability::Triggered {
            when,
            intervening_if,
            from,
            targets,
            effects,
            ..
        } => Ability::Triggered {
            when,
            intervening_if,
            limit: Some(Limit::OncePerTurn),
            from,
            targets,
            effects,
        },
        Ability::Activated {
            cost,
            timing,
            from,
            targets,
            effects,
            ..
        } => Ability::Activated {
            cost,
            timing,
            limit: Some(Limit::OncePerTurn),
            from,
            targets,
            effects,
        },
        other => other,
    }
}

/// A 1/1 creature that costs nothing and carries these rules.
pub fn creature_with(name: &str, rules: Vec<Ability>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_mana_cost(cost(0, &[]))
        .with_power_toughness(1, 1);
    for rule in rules {
        card = card.with_rules(rule);
    }
    Arc::new(card)
}

/// A creature of the given size that costs nothing and carries these rules.
pub fn sized_creature_with(
    name: &str,
    power: i32,
    toughness: i32,
    rules: Vec<Ability>,
) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_mana_cost(cost(0, &[]))
        .with_power_toughness(power, toughness);
    for rule in rules {
        card = card.with_rules(rule);
    }
    Arc::new(card)
}

/// An artifact that costs nothing and carries these rules.
pub fn artifact_with(name: &str, rules: Vec<Ability>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Artifact)
        .with_mana_cost(cost(0, &[]));
    for rule in rules {
        card = card.with_rules(rule);
    }
    Arc::new(card)
}

/// A 1/1 creature for no mana with a single triggered ability.
pub fn watcher(name: &str, when: TriggerPattern, effects: Vec<Effect>) -> Arc<CardDefinition> {
    creature_with(name, vec![trigger(when, effects)])
}

/// `card` with these rules added.
pub fn with_rules(card: &Arc<CardDefinition>, rules: Vec<Ability>) -> Arc<CardDefinition> {
    let mut definition = (**card).clone();
    for rule in rules {
        definition = definition.with_rules(rule);
    }
    Arc::new(definition)
}

/// An artifact costing `generic` mana and carrying these rules.
pub fn artifact_costing(name: &str, generic: u32, rules: Vec<Ability>) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Artifact)
        .with_mana_cost(cost(generic, &[]));
    for rule in rules {
        card = card.with_rules(rule);
    }
    Arc::new(card)
}

/// "{T}: Set up a delayed triggered ability": at the next event `when` names (or, if it is not `once`, at each until
/// the end of the turn) it does `effects`.
pub fn delaying(when: TriggerPattern, once: bool, effects: Vec<Effect>) -> Ability {
    Ability::Activated {
        cost: Cost(vec![CostItem::Tap]),
        timing: Timing::Instant,
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![Effect::DelayedTrigger {
            when,
            once,
            effects,
        }],
    }
}

/// An artifact for no mana with "{T}: At the beginning of the next end step, you gain `life` life".
pub fn hourglass(name: &str, life: i32) -> Arc<CardDefinition> {
    artifact_with(
        name,
        vec![delaying(when::an_end_step(), true, vec![gain_life(life)])],
    )
}

/// An artifact for no mana with "{T}: Until end of turn, whenever a creature dies, you gain `life` life".
pub fn vigil(name: &str, life: i32) -> Arc<CardDefinition> {
    artifact_with(
        name,
        vec![delaying(
            when::any_creature_dies(),
            false,
            vec![gain_life(life)],
        )],
    )
}

#[cfg(test)]
mod tests {
    //! Goal: the helpers build the ability they are named for and leave every other kind of ability alone.
    use super::*;
    use crate::cards::draw;

    #[test]
    fn the_modifiers_change_one_part_of_a_triggered_ability_and_pass_others_by() {
        let base = trigger(when::this_enters(), vec![draw(1)]);
        let aimed = aimed_at(base.clone(), Targetable::Opponent);
        assert_eq!(aimed.targets().len(), 1);
        let conditional = only_if(base.clone(), Condition::YourTurn);
        assert!(matches!(
            conditional,
            Ability::Triggered {
                intervening_if: Some(Condition::YourTurn),
                ..
            }
        ));
        let limited = once_each_turn(base.clone());
        assert!(matches!(
            limited,
            Ability::Triggered {
                limit: Some(Limit::OncePerTurn),
                ..
            }
        ));
        let keyword = Ability::Keyword(mtg_cards::keyword::Keyword::Flash);
        assert_eq!(aimed_at(keyword.clone(), Targetable::Player), keyword);
        assert_eq!(only_if(keyword.clone(), Condition::YourTurn), keyword);
        assert_eq!(once_each_turn(keyword.clone()), keyword);
    }

    #[test]
    fn the_cards_carry_their_rules() {
        let card = watcher("Watcher", when::your_upkeep(), vec![draw(1)]);
        assert_eq!(card.rules.len(), 1);
        assert!(card.is_type(CardType::Creature));
        assert!(artifact_with("Relic", Vec::new()).is_type(CardType::Artifact));
        let big = sized_creature_with("Big", 4, 4, Vec::new());
        assert_eq!((big.power, big.toughness), (Some(4), Some(4)));
    }
}
