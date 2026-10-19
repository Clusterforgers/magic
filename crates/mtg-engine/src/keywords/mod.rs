//! Keyword abilities: one rules value per keyword, each in its own file (`docs/cards-dsl.md`, Keywords).
//!
//! Every file is a stub that changes nothing. A keyword task replaces its stub, adds its `Caps` entry
//! and reads its hooks where the engine asks for them: blocking, targeting and timing call them today.

mod battle_cry;
mod cascade;
mod companion;
mod convoke;
mod cycling;
mod deathtouch;
mod defender;
mod delve;
mod disguise;
mod double_strike;
mod enchant;
mod equip;
mod evoke;
mod first_strike;
mod flash;
mod flashback;
mod flying;
mod fuse;
mod haste;
mod hexproof;
mod indestructible;
mod kicker;
mod lifelink;
mod madness;
mod menace;
mod morph;
mod multikicker;
mod overload;
mod partner;
mod partner_with;
mod prowess;
mod reach;
mod reconfigure;
mod storm;
mod suspend;
mod trample;
mod vigilance;
mod ward;

use std::fmt::Debug;

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::object::GameObject;
use crate::state::GameState;

/// How a keyword changes the combat damage its holder deals.
// Independent switches the keyword tasks set by name, not the states of one machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DamageRules {
    /// Deals damage in the first-strike step.
    pub first_strike: bool,
    /// Deals damage in the first-strike step and in the regular one.
    pub double_strike: bool,
    /// Any damage it deals to a creature is lethal.
    pub deathtouch: bool,
    /// Assigns the rest of its damage to the player or planeswalker it attacks.
    pub trample: bool,
    /// Its controller gains that much life.
    pub lifelink: bool,
}

/// What a keyword does to the rules machinery; every hook defaults to doing nothing.
pub trait KeywordRules: Debug + Sync {
    /// Whether `blocker` may not block `attacker`, which has the keyword.
    fn restricts_blocking(
        &self,
        _state: &GameState,
        _keyword: &Keyword,
        _attacker: ObjectId,
        _blocker: ObjectId,
    ) -> bool {
        false
    }

    /// Whether `holder`, which has the keyword, may not be targeted by a spell or ability of `source` that `controller` controls.
    fn restricts_targeting(
        &self,
        _state: &GameState,
        _keyword: &Keyword,
        _holder: ObjectId,
        _source: ObjectId,
        _controller: PlayerId,
    ) -> bool {
        false
    }

    /// How the keyword changes the combat damage its holder deals.
    fn modifies_damage(&self, _keyword: &Keyword) -> DamageRules {
        DamageRules::default()
    }

    /// Whether the keyword lets its holder be cast whenever its controller could cast an instant.
    fn grants_timing(&self, _keyword: &Keyword) -> bool {
        false
    }

    /// The abilities the keyword stands for, written in the DSL like any card.
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        Vec::new()
    }
}

/// The rules of `keyword`.
pub fn rules(keyword: &Keyword) -> &'static dyn KeywordRules {
    match keyword {
        Keyword::Flash => &flash::Flash,
        Keyword::Flying => &flying::Flying,
        Keyword::Reach => &reach::Reach,
        Keyword::FirstStrike => &first_strike::FirstStrike,
        Keyword::DoubleStrike => &double_strike::DoubleStrike,
        Keyword::Deathtouch => &deathtouch::Deathtouch,
        Keyword::Trample => &trample::Trample,
        Keyword::Lifelink => &lifelink::Lifelink,
        Keyword::Vigilance => &vigilance::Vigilance,
        Keyword::Haste => &haste::Haste,
        Keyword::Menace => &menace::Menace,
        Keyword::Defender => &defender::Defender,
        Keyword::Indestructible => &indestructible::Indestructible,
        Keyword::Hexproof => &hexproof::Hexproof,
        Keyword::Prowess => &prowess::Prowess,
        Keyword::Convoke => &convoke::Convoke,
        Keyword::Delve => &delve::Delve,
        Keyword::Cascade => &cascade::Cascade,
        Keyword::Storm => &storm::Storm,
        Keyword::Partner => &partner::Partner,
        Keyword::Companion => &companion::Companion,
        Keyword::BattleCry => &battle_cry::BattleCry,
        Keyword::Fuse => &fuse::Fuse,
        Keyword::Ward(_) => &ward::Ward,
        Keyword::Equip(_) => &equip::Equip,
        Keyword::Reconfigure(_) => &reconfigure::Reconfigure,
        Keyword::Enchant(_) => &enchant::Enchant,
        Keyword::Cycling(_) => &cycling::Cycling,
        Keyword::Kicker(_) => &kicker::Kicker,
        Keyword::Multikicker(_) => &multikicker::Multikicker,
        Keyword::Flashback(_) => &flashback::Flashback,
        Keyword::Madness(_) => &madness::Madness,
        Keyword::Morph(_) => &morph::Morph,
        Keyword::Disguise(_) => &disguise::Disguise,
        Keyword::Suspend { .. } => &suspend::Suspend,
        Keyword::Evoke(_) => &evoke::Evoke,
        Keyword::Overload(_) => &overload::Overload,
        Keyword::PartnerWith(_) => &partner_with::PartnerWith,
    }
}

/// The keywords an object has, from the rules of its card.
pub fn held_by(object: &GameObject) -> impl Iterator<Item = &Keyword> {
    object
        .card()
        .rules
        .iter()
        .filter_map(|ability| match &**ability {
            Ability::Keyword(keyword) => Some(keyword),
            _ => None,
        })
}

/// Whether a keyword of `holder` stops `controller`'s `source` from targeting it.
pub(crate) fn protects(
    state: &GameState,
    holder: ObjectId,
    source: ObjectId,
    controller: PlayerId,
) -> bool {
    state.object(holder).is_some_and(|object| {
        held_by(object).any(|keyword| {
            rules(keyword).restricts_targeting(state, keyword, holder, source, controller)
        })
    })
}

/// Whether a keyword of the attacker stops `blocker` from blocking it.
pub(crate) fn stops_block(state: &GameState, attacker: ObjectId, blocker: ObjectId) -> bool {
    state.object(attacker).is_some_and(|object| {
        held_by(object)
            .any(|keyword| rules(keyword).restricts_blocking(state, keyword, attacker, blocker))
    })
}

/// Whether a keyword lets the card be cast whenever an instant could be.
pub(crate) fn grants_instant_timing(object: &GameObject) -> bool {
    held_by(object).any(|keyword| rules(keyword).grants_timing(keyword))
}

#[cfg(test)]
mod tests {
    //! Goal: every keyword the vocabulary declares has rules, the stubs change nothing, and no keyword is claimed by the capabilities before its task.
    use std::sync::Arc;

    use super::*;
    use crate::caps::caps;
    use crate::fixtures::{creature, state};
    use crate::zones::Zone;
    use mtg_cards::CardDefinition;
    use mtg_cards::cost::Cost;
    use mtg_cards::refs::Selector;
    use mtg_cards::validate::used_primitives;
    use mtg_core::card::CardType;

    /// One value of every keyword, parameters included.
    fn every_keyword() -> Vec<Keyword> {
        let cost = || Cost::mana("{1}".parse().unwrap());
        vec![
            Keyword::Flash,
            Keyword::Flying,
            Keyword::Reach,
            Keyword::FirstStrike,
            Keyword::DoubleStrike,
            Keyword::Deathtouch,
            Keyword::Trample,
            Keyword::Lifelink,
            Keyword::Vigilance,
            Keyword::Haste,
            Keyword::Menace,
            Keyword::Defender,
            Keyword::Indestructible,
            Keyword::Hexproof,
            Keyword::Prowess,
            Keyword::Convoke,
            Keyword::Delve,
            Keyword::Cascade,
            Keyword::Storm,
            Keyword::Partner,
            Keyword::Companion,
            Keyword::BattleCry,
            Keyword::Fuse,
            Keyword::Ward(cost()),
            Keyword::Equip(cost()),
            Keyword::Reconfigure(cost()),
            Keyword::Enchant(Selector::default()),
            Keyword::Cycling(cost()),
            Keyword::Kicker(cost()),
            Keyword::Multikicker(cost()),
            Keyword::Flashback(cost()),
            Keyword::Madness(cost()),
            Keyword::Morph(cost()),
            Keyword::Disguise(cost()),
            Keyword::Suspend {
                time: 3,
                cost: cost(),
            },
            Keyword::Evoke(cost()),
            Keyword::Overload(cost()),
            Keyword::PartnerWith("Pir, Imaginative Rascal".into()),
        ]
    }

    #[test]
    fn the_list_has_every_keyword_the_vocabulary_names() {
        let names: Vec<&str> = every_keyword().iter().map(Keyword::name).collect();
        assert_eq!(names, Keyword::NAMES);
    }

    #[test]
    fn a_stub_changes_nothing() {
        for keyword in every_keyword() {
            let rules = rules(&keyword);
            assert_eq!(rules.modifies_damage(&keyword), DamageRules::default());
            assert!(!rules.grants_timing(&keyword), "{keyword:?}");
            assert!(rules.adds_abilities(&keyword).is_empty(), "{keyword:?}");
        }
    }

    #[test]
    fn a_keyword_stub_changes_nothing_where_the_engine_asks_about_blocking_targeting_and_timing() {
        for keyword in every_keyword() {
            let holder = Arc::new(
                CardDefinition::new("Holder")
                    .with_type(CardType::Creature)
                    .with_power_toughness(2, 2)
                    .with_rules(Ability::Keyword(keyword.clone())),
            );
            let game = state(
                vec![vec![], vec![]],
                vec![vec![holder, creature("Other", 1, 1)], vec![]],
            );
            let (held, other) = (
                game.zone(Zone::Battlefield)[0],
                game.zone(Zone::Battlefield)[1],
            );
            let object = game.object(held).unwrap();
            assert_eq!(held_by(object).collect::<Vec<_>>(), [&keyword]);
            assert!(!stops_block(&game, held, other), "{keyword:?}");
            assert!(!protects(&game, held, other, PlayerId(1)), "{keyword:?}");
            assert!(!grants_instant_timing(object), "{keyword:?}");
            // A creature without the keyword holds none.
            assert_eq!(held_by(game.object(other).unwrap()).count(), 0);
        }
    }

    #[test]
    fn the_capabilities_name_no_keyword_yet() {
        let caps = caps();
        for keyword in every_keyword() {
            let used = used_primitives(&Ability::Keyword(keyword));
            let claimed: Vec<_> = used
                .iter()
                .filter(|name| name.starts_with("Keyword.") && caps.runs(name))
                .collect();
            assert!(claimed.is_empty(), "{claimed:?} is claimed before its task");
        }
    }
}
