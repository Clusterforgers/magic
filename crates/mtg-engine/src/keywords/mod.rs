//! Keyword abilities: one rules value per keyword, each in its own file (`docs/cards-dsl.md`, Keywords).
//!
//! A keyword task replaces its stub, adds its `Caps` entry and answers the hooks of [`KeywordRules`] the
//! engine asks about: blocking, attacking, damage, targeting, timing and destruction. The engine reads the
//! keywords an object has through [`keywords_of`] and the helpers below it, never from the rules of the card.

mod affinity;
pub(crate) mod aftermath;
mod backup;
mod battle_cry;
mod bestow;
mod blitz;
mod buyback;
mod cascade;
mod changeling;
mod choose_a_background;
mod companion;
mod convoke;
mod crew;
mod cumulative_upkeep;
mod cycling;
mod dash;
mod daybound;
mod deathtouch;
mod defender;
mod delve;
mod devoid;
mod disguise;
mod doctors_companion;
mod double_strike;
mod dredge;
mod echo;
mod embalm;
mod enchant;
mod encore;
mod entwine;
mod equip;
mod escape;
mod eternalize;
mod evoke;
mod fading;
mod fear;
mod first_strike;
mod flash;
mod flashback;
mod flying;
pub(crate) mod foretell;
mod fuse;
mod haste;
mod hexproof;
mod improvise;
mod indestructible;
mod infect;
mod intimidate;
mod jump_start;
mod kicker;
mod landwalk;
mod level_up;
mod lifelink;
mod living_weapon;
mod madness;
mod menace;
mod morph;
mod multikicker;
mod mutate;
mod myriad;
mod nightbound;
mod ninjutsu;
mod outlast;
mod overload;
mod partner;
mod partner_group;
mod partner_with;
mod persist;
mod protection;
#[cfg(test)]
mod provided;
mod prowess;
mod ravenous;
mod reach;
mod rebound;
mod reconfigure;
mod retrace;
mod saddle;
mod shadow;
mod shroud;
mod skulk;
mod split_second;
mod squad;
mod storm;
pub(crate) mod suspend;
mod toxic;
mod trample;
mod typecycling;
mod undaunted;
mod undying;
mod unearth;
mod vanishing;
mod vigilance;
mod ward;
mod wither;

use std::fmt::Debug;

use mtg_cards::ability::Ability;
use mtg_cards::cost::Cost;
use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::ability_reader::keyword_abilities;
use crate::characteristics::controller_of_object;
use crate::combat::DamageForm;
use crate::object::{ObjectKind, ObjectRef};
use crate::snapshot::Snapshot;
use crate::state::GameState;
use crate::zones::Zone;

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
    /// Damage it deals to a creature is dealt as -1/-1 counters (wither, CR 702.80).
    pub wither: bool,
    /// Damage it deals to a creature is dealt as -1/-1 counters and to a player as poison counters (infect, CR 702.90).
    pub infect: bool,
    /// The poison counters a player dealt combat damage by it also gets (toxic, cumulative, CR 702.164).
    pub toxic: u32,
}

impl DamageRules {
    /// Both sets of rules at once: a creature with two keywords gets what each gives.
    #[must_use]
    pub const fn and(self, other: DamageRules) -> DamageRules {
        DamageRules {
            first_strike: self.first_strike || other.first_strike,
            double_strike: self.double_strike || other.double_strike,
            deathtouch: self.deathtouch || other.deathtouch,
            trample: self.trample || other.trample,
            lifelink: self.lifelink || other.lifelink,
            wither: self.wither || other.wither,
            infect: self.infect || other.infect,
            toxic: self.toxic.saturating_add(other.toxic),
        }
    }

    /// What the damage it deals does to what it hurts: infect before wither.
    pub const fn form(self) -> DamageForm {
        if self.infect {
            DamageForm::Infect
        } else if self.wither {
            DamageForm::Wither
        } else {
            DamageForm::Normal
        }
    }

    /// Whether the holder deals damage in the first-strike step.
    pub const fn strikes_first(self) -> bool {
        self.first_strike || self.double_strike
    }

    /// Whether the holder deals damage in the regular step: always, but for first strike alone (CR 510.4).
    pub const fn strikes_regularly(self) -> bool {
        self.double_strike || !self.first_strike
    }
}

/// What has keywords that protect it: a permanent or a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holder {
    /// A permanent.
    Object(ObjectId),
    /// A player.
    Player(PlayerId),
}

/// What damage, an attachment, a blocker, a spell or an ability comes from, as protection reads it: the object, and the
/// snapshots that say how it was if it has gone (last-known information, CR 608.2h).
#[derive(Debug, Clone, Copy)]
pub struct Source<'a> {
    /// The object.
    pub object: ObjectRef,
    /// How objects that have changed zones were.
    pub snapshots: &'a [&'a Snapshot],
}

impl Source<'static> {
    /// The object `id` as it is now.
    pub(crate) fn live(state: &GameState, id: ObjectId) -> Option<Source<'static>> {
        state.object(id).map(|object| Source {
            object: object.reference(),
            snapshots: &[],
        })
    }
}

/// What paying an optional cost does, which says what reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalKind {
    /// Kicker or multikicker: read as `Condition::Kicked` and `Amount::Kicks`.
    Kicker,
    /// Buyback: the spell returns to its owner's hand as it resolves.
    Buyback,
    /// Entwine: every mode is chosen.
    Entwine,
    /// "As an additional cost to cast this spell, you may ...": read as `Condition::AdditionalCostPaid`.
    Additional,
}

/// A cost a keyword lets the caster pay in addition: kicker, multikicker, buyback and entwine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdditionalCost {
    /// The keyword as the card prints it, before its cost: `Kicker`.
    pub name: &'static str,
    /// What paying it once costs.
    pub cost: Cost,
    /// Whether it may be paid any number of times (multikicker) and not only once.
    pub repeatable: bool,
    /// What paying it does.
    pub kind: OptionalKind,
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

    /// Whether `blocker`, which has the keyword, may not block `attacker` (shadow: only creatures with shadow).
    fn restricts_blocked(
        &self,
        _state: &GameState,
        _keyword: &Keyword,
        _attacker: ObjectId,
        _blocker: ObjectId,
    ) -> bool {
        false
    }

    /// Whether a holder of the keyword may block a creature whose `evasion` keyword restricts blocking.
    fn answers_evasion(&self, _keyword: &Keyword, _evasion: &Keyword) -> bool {
        false
    }

    /// The fewest creatures that may block the holder together, when any block it.
    fn min_blockers(&self, _keyword: &Keyword) -> usize {
        1
    }

    /// Whether the holder cannot be declared as an attacker.
    fn forbids_attacking(&self, _keyword: &Keyword) -> bool {
        false
    }

    /// Whether summoning sickness does not hold the holder back from attacking and from `{T}` abilities.
    fn ignores_summoning_sickness(&self, _keyword: &Keyword) -> bool {
        false
    }

    /// Whether declaring the holder as an attacker leaves it untapped.
    fn attacks_untapped(&self, _keyword: &Keyword) -> bool {
        false
    }

    /// Whether the holder is not destroyed by lethal damage, by deathtouch or by an effect that says destroy.
    fn is_indestructible(&self, _keyword: &Keyword) -> bool {
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

    /// Whether `holder`, a player who has the keyword, may not be targeted by a spell or ability that `controller` controls.
    fn restricts_player_targeting(
        &self,
        _keyword: &Keyword,
        _holder: PlayerId,
        _controller: PlayerId,
    ) -> bool {
        false
    }

    /// Whether the keyword protects `holder` from `source` (CR 702.16): damage from it is prevented, it can't enchant or
    /// equip the holder, can't block it and can't target it.
    fn protects_from(
        &self,
        _state: &GameState,
        _keyword: &Keyword,
        _holder: Holder,
        _source: Source<'_>,
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

    /// The cost the keyword lets the caster pay in addition as the spell is announced (CR 601.2b, 702.33).
    fn additional_cost(&self, _keyword: &Keyword) -> Option<AdditionalCost> {
        None
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
        Keyword::Shroud => &shroud::Shroud,
        Keyword::Protection(_) => &protection::Protection,
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
        Keyword::LivingWeapon => &living_weapon::LivingWeapon,
        Keyword::Enchant(_) => &enchant::Enchant,
        Keyword::EnchantPlayer { .. } => &enchant::EnchantPlayer,
        Keyword::Cycling(_) => &cycling::Cycling,
        Keyword::Kicker(_) => &kicker::Kicker,
        Keyword::Buyback(_) => &buyback::Buyback,
        Keyword::Entwine(_) => &entwine::Entwine,
        Keyword::Multikicker(_) => &multikicker::Multikicker,
        Keyword::Flashback(_) => &flashback::Flashback,
        Keyword::Madness(_) => &madness::Madness,
        Keyword::Dredge(_) => &dredge::Dredge,
        Keyword::Morph(_) => &morph::Morph,
        Keyword::Disguise(_) => &disguise::Disguise,
        Keyword::Suspend { .. } => &suspend::Suspend,
        Keyword::Evoke(_) => &evoke::Evoke,
        Keyword::Overload(_) => &overload::Overload,
        Keyword::PartnerWith(_) => &partner_with::PartnerWith,
        Keyword::Changeling => &changeling::Changeling,
        Keyword::Devoid => &devoid::Devoid,
        Keyword::Infect => &infect::Infect,
        Keyword::Myriad => &myriad::Myriad,
        Keyword::Crew(_) => &crew::Crew,
        Keyword::Mutate(_) => &mutate::Mutate,
        Keyword::Encore(_) => &encore::Encore,
        Keyword::Squad(_) => &squad::Squad,
        Keyword::Foretell(_) => &foretell::Foretell,
        Keyword::Daybound => &daybound::Daybound,
        Keyword::Nightbound => &nightbound::Nightbound,
        Keyword::Wither => &wither::Wither,
        Keyword::Toxic(_) => &toxic::Toxic,
        Keyword::Fear => &fear::Fear,
        Keyword::Intimidate => &intimidate::Intimidate,
        Keyword::Landwalk(_) => &landwalk::Landwalk,
        Keyword::Skulk => &skulk::Skulk,
        Keyword::Shadow => &shadow::Shadow,
        Keyword::SplitSecond => &split_second::SplitSecond,
        Keyword::Undaunted => &undaunted::Undaunted,
        Keyword::PartnerGroup(_) => &partner_group::PartnerGroup,
        Keyword::DoctorsCompanion => &doctors_companion::DoctorsCompanion,
        Keyword::ChooseABackground => &choose_a_background::ChooseABackground,
        Keyword::Unearth(_) => &unearth::Unearth,
        Keyword::Escape(_) => &escape::Escape,
        Keyword::Rebound => &rebound::Rebound,
        Keyword::Aftermath => &aftermath::Aftermath,
        Keyword::Dash(_) => &dash::Dash,
        Keyword::Bestow(_) => &bestow::Bestow,
        Keyword::Blitz(_) => &blitz::Blitz,
        Keyword::Ninjutsu(_) => &ninjutsu::Ninjutsu,
        Keyword::Echo(_) => &echo::Echo,
        Keyword::CumulativeUpkeep(_) => &cumulative_upkeep::CumulativeUpkeep,
        Keyword::Undying => &undying::Undying,
        Keyword::Persist => &persist::Persist,
        Keyword::Vanishing(_) => &vanishing::Vanishing,
        Keyword::Fading(_) => &fading::Fading,
        Keyword::Backup { .. } => &backup::Backup,
        Keyword::LevelUp(_) => &level_up::LevelUp,
        Keyword::Outlast(_) => &outlast::Outlast,
        Keyword::Saddle(_) => &saddle::Saddle,
        Keyword::Saddled => &saddle::Saddled,
        Keyword::Ravenous => &ravenous::Ravenous,
        Keyword::JumpStart => &jump_start::JumpStart,
        Keyword::Retrace => &retrace::Retrace,
        Keyword::Embalm(_) => &embalm::Embalm,
        Keyword::Eternalize(_) => &eternalize::Eternalize,
        Keyword::Improvise => &improvise::Improvise,
        Keyword::Affinity(_) => &affinity::Affinity,
        Keyword::Typecycling { .. } => &typecycling::Typecycling,
    }
}

/// The keywords object `id` has now, in the order they are listed: the one reader of an object's keywords.
///
/// The keyword abilities of the object come first, as `ability_reader` lists them. Layer 6 joins here
/// (`docs/cards-dsl.md`, Triggers, replacement and layers): the continuous effects that add a keyword append to the
/// list in timestamp order, and a keyword the object already has is not listed twice, so nothing else in the engine
/// learns where a keyword came from. Effects apply to permanents only. An ability on the stack has no keywords of
/// its own.
pub fn keywords_of(state: &GameState, id: ObjectId) -> Vec<&Keyword> {
    let Some(object) = state.object(id) else {
        return Vec::new();
    };
    if object.kind() == ObjectKind::Ability {
        return Vec::new();
    }
    if object.zone() == Zone::Battlefield
        && let Some(held) = state.layers().get(id)
    {
        return held.keywords.iter().collect();
    }
    keyword_abilities(state, id)
}

/// Whether object `id` has `keyword` now.
pub fn has_keyword(state: &GameState, id: ObjectId, keyword: &Keyword) -> bool {
    keywords_of(state, id).contains(&keyword)
}

/// What the keywords of `id` do to the combat damage it deals.
pub(crate) fn damage_rules(state: &GameState, id: ObjectId) -> DamageRules {
    damage_rules_of(keywords_of(state, id))
}

/// What `keywords` do to the damage their holder deals: those of an object now, or those its snapshot holds.
pub(crate) fn damage_rules_of<'a>(keywords: impl IntoIterator<Item = &'a Keyword>) -> DamageRules {
    keywords
        .into_iter()
        .fold(DamageRules::default(), |all, keyword| {
            all.and(rules(keyword).modifies_damage(keyword))
        })
}

/// Whether a keyword stops `id` from being declared as an attacker.
pub(crate) fn forbids_attacking(state: &GameState, id: ObjectId) -> bool {
    keywords_of(state, id)
        .into_iter()
        .any(|keyword| rules(keyword).forbids_attacking(keyword))
}

/// Whether summoning sickness holds the permanent back from attacking and from `{T}` abilities now.
///
/// It has not been under its controller's control since their turn began and no keyword (haste) excuses it.
pub fn summoning_sick(state: &GameState, id: ObjectId) -> bool {
    let Some(object) = state.object(id) else {
        return false;
    };
    object
        .permanent()
        .is_some_and(|permanent| permanent.sick_for(controller_of_object(state, object)))
        && !keywords_of(state, id)
            .into_iter()
            .any(|keyword| rules(keyword).ignores_summoning_sickness(keyword))
}

/// Whether declaring `id` as an attacker leaves it untapped.
pub(crate) fn attacks_untapped(state: &GameState, id: ObjectId) -> bool {
    keywords_of(state, id)
        .into_iter()
        .any(|keyword| rules(keyword).attacks_untapped(keyword))
}

/// The fewest creatures that may block `id` together, when any do: 1 unless a keyword (menace) asks for more.
pub(crate) fn min_blockers(state: &GameState, id: ObjectId) -> usize {
    keywords_of(state, id)
        .into_iter()
        .map(|keyword| rules(keyword).min_blockers(keyword))
        .max()
        .unwrap_or(1)
        .max(1)
}

/// Whether a keyword keeps `id` from being destroyed by lethal damage, deathtouch or an effect that says destroy.
///
/// Zero toughness is not destruction (CR 704.5f): the state-based action for it ignores this. The
/// `Destroy` primitive asks it before it moves a permanent.
pub fn is_indestructible(state: &GameState, id: ObjectId) -> bool {
    keywords_of(state, id)
        .into_iter()
        .any(|keyword| rules(keyword).is_indestructible(keyword))
}

/// The keywords player `id` has now: the one reader of a player's keywords.
///
/// Today these are the ones their setup gives them; the continuous effects of layer 6 that name a player join
/// here, as they join [`keywords_of`].
pub fn player_keywords(state: &GameState, id: PlayerId) -> Vec<&Keyword> {
    state
        .player(id)
        .map_or_else(Vec::new, |player| player.keywords().iter().collect())
}

/// Whether a keyword of `holder` stops `controller`'s `source` from targeting it.
pub(crate) fn protects(
    state: &GameState,
    holder: ObjectId,
    source: ObjectId,
    controller: PlayerId,
) -> bool {
    keywords_of(state, holder).into_iter().any(|keyword| {
        rules(keyword).restricts_targeting(state, keyword, holder, source, controller)
    })
}

/// Whether a keyword of the player `holder` stops `controller`'s spell or ability from targeting them.
pub(crate) fn protects_player(state: &GameState, holder: PlayerId, controller: PlayerId) -> bool {
    player_keywords(state, holder)
        .into_iter()
        .any(|keyword| rules(keyword).restricts_player_targeting(keyword, holder, controller))
}

/// Whether a keyword of `holder` protects it from `source` (CR 702.16): the one reader of protection for damage,
/// attachments, blocking and targeting.
pub(crate) fn protected_from(state: &GameState, holder: Holder, source: Source<'_>) -> bool {
    let held: Vec<&Keyword> = match holder {
        Holder::Object(id) => keywords_of(state, id),
        Holder::Player(id) => player_keywords(state, id),
    };
    held.into_iter()
        .any(|keyword| rules(keyword).protects_from(state, keyword, holder, source))
}

/// Whether a keyword of the attacker stops `blocker` from blocking it: menace-like restrictions, and protection from
/// the blocker.
pub(crate) fn stops_block(state: &GameState, attacker: ObjectId, blocker: ObjectId) -> bool {
    keywords_of(state, attacker)
        .into_iter()
        .any(|keyword| rules(keyword).restricts_blocking(state, keyword, attacker, blocker))
        || keywords_of(state, blocker)
            .into_iter()
            .any(|keyword| rules(keyword).restricts_blocked(state, keyword, attacker, blocker))
        || Source::live(state, blocker)
            .is_some_and(|source| protected_from(state, Holder::Object(attacker), source))
}

/// Whether a keyword lets the card be cast whenever an instant could be.
pub(crate) fn grants_instant_timing(state: &GameState, id: ObjectId) -> bool {
    keywords_of(state, id)
        .into_iter()
        .any(|keyword| rules(keyword).grants_timing(keyword))
}

#[cfg(test)]
mod tests {
    //! Goal: every keyword the vocabulary declares has rules, the capabilities name exactly the keywords whose rules the engine runs, each keyword answers the hooks its row says and no others, and one function reads what an object has and one what a player has.
    use std::sync::Arc;

    use super::*;
    use crate::caps::caps;
    use crate::effects::Modification;
    use crate::fixtures::{creature, state};
    use crate::zones::Zone;
    use mtg_cards::CardDefinition;
    use mtg_cards::cost::Cost;
    use mtg_cards::refs::Selector;
    use mtg_cards::validate::used_primitives;
    use mtg_core::card::CardType;
    use rstest::rstest;

    /// One value of every keyword, parameters included.
    #[allow(clippy::too_many_lines)]
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
            Keyword::Shroud,
            Keyword::Protection(Selector::default()),
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
            Keyword::LivingWeapon,
            Keyword::Enchant(Selector::default()),
            Keyword::EnchantPlayer { opponent: false },
            Keyword::Cycling(cost()),
            Keyword::Kicker(cost()),
            Keyword::Multikicker(cost()),
            Keyword::Buyback(cost()),
            Keyword::Entwine(cost()),
            Keyword::Flashback(cost()),
            Keyword::Madness(cost()),
            Keyword::Dredge(3),
            Keyword::Morph(cost()),
            Keyword::Disguise(cost()),
            Keyword::Suspend {
                time: 3,
                cost: cost(),
            },
            Keyword::Evoke(cost()),
            Keyword::Overload(cost()),
            Keyword::PartnerWith("Pir, Imaginative Rascal".into()),
            Keyword::Changeling,
            Keyword::Devoid,
            Keyword::Infect,
            Keyword::Myriad,
            Keyword::Crew(3),
            Keyword::Mutate(cost()),
            Keyword::Encore(cost()),
            Keyword::Squad(cost()),
            Keyword::Foretell(cost()),
            Keyword::Daybound,
            Keyword::Nightbound,
            Keyword::Wither,
            Keyword::Toxic(1),
            Keyword::Fear,
            Keyword::Intimidate,
            Keyword::Landwalk(Selector::default()),
            Keyword::Skulk,
            Keyword::Shadow,
            Keyword::SplitSecond,
            Keyword::Undaunted,
            Keyword::PartnerGroup("Friends forever".into()),
            Keyword::DoctorsCompanion,
            Keyword::ChooseABackground,
            Keyword::Unearth(cost()),
            Keyword::Escape(cost()),
            Keyword::Rebound,
            Keyword::Aftermath,
            Keyword::Dash(cost()),
            Keyword::Bestow(cost()),
            Keyword::Blitz(cost()),
            Keyword::Ninjutsu(cost()),
            Keyword::Echo(cost()),
            Keyword::CumulativeUpkeep(cost()),
            Keyword::Undying,
            Keyword::Persist,
            Keyword::LevelUp(cost()),
            Keyword::Outlast(cost()),
            Keyword::Saddle(2),
            Keyword::Saddled,
            Keyword::Ravenous,
            Keyword::JumpStart,
            Keyword::Retrace,
            Keyword::Embalm(cost()),
            Keyword::Eternalize(cost()),
            Keyword::Improvise,
            Keyword::Affinity(Selector::default()),
            Keyword::Typecycling {
                find: Selector::default(),
                cost: cost(),
            },
            Keyword::Vanishing(3),
            Keyword::Fading(3),
            Keyword::Backup {
                count: 1,
                abilities: vec![],
            },
        ]
    }

    /// The keywords whose rules the engine runs, named as the capabilities name them.
    const RUN: &[&str] = &[
        "Keyword.Flash",
        "Keyword.Flying",
        "Keyword.Reach",
        "Keyword.FirstStrike",
        "Keyword.DoubleStrike",
        "Keyword.Deathtouch",
        "Keyword.Trample",
        "Keyword.Lifelink",
        "Keyword.Vigilance",
        "Keyword.Haste",
        "Keyword.Menace",
        "Keyword.Defender",
        "Keyword.Indestructible",
        "Keyword.Hexproof",
        "Keyword.Shroud",
        "Keyword.Protection",
        "Keyword.Prowess",
        "Keyword.BattleCry",
        "Keyword.Ward",
        "Keyword.Enchant",
        "Keyword.EnchantPlayer",
        "Keyword.Equip",
        "Keyword.Reconfigure",
        "Keyword.LivingWeapon",
        "Keyword.Kicker",
        "Keyword.Multikicker",
        "Keyword.Buyback",
        "Keyword.Entwine",
        "Keyword.Flashback",
        "Keyword.Evoke",
        "Keyword.Overload",
        "Keyword.Cycling",
        "Keyword.Cascade",
        "Keyword.Storm",
        "Keyword.Convoke",
        "Keyword.Delve",
        "Keyword.Madness",
        "Keyword.Dredge",
        "Keyword.Suspend",
        "Keyword.Foretell",
        "Keyword.Daybound",
        "Keyword.Nightbound",
        "Keyword.Fuse",
        "Keyword.Crew",
        "Keyword.Changeling",
        "Keyword.Devoid",
        "Keyword.Infect",
        "Keyword.Wither",
        "Keyword.Toxic",
        "Keyword.Fear",
        "Keyword.Intimidate",
        "Keyword.Landwalk",
        "Keyword.Skulk",
        "Keyword.Shadow",
        "Keyword.Partner",
        "Keyword.PartnerWith",
        "Keyword.PartnerGroup",
        "Keyword.DoctorsCompanion",
        "Keyword.ChooseABackground",
        "Keyword.Unearth",
        "Keyword.Escape",
        "Keyword.Rebound",
        "Keyword.Aftermath",
        "Keyword.Dash",
        "Keyword.Bestow",
        "Keyword.Blitz",
        "Keyword.Ninjutsu",
        "Keyword.Echo",
        "Keyword.CumulativeUpkeep",
        "Keyword.Undying",
        "Keyword.Persist",
        "Keyword.Vanishing",
        "Keyword.Fading",
        "Keyword.Backup",
        "Keyword.LevelUp",
        "Keyword.Outlast",
        "Keyword.Saddle",
        "Keyword.Saddled",
        "Keyword.Myriad",
        "Keyword.Squad",
        "Keyword.Ravenous",
        "Keyword.JumpStart",
        "Keyword.Retrace",
        "Keyword.Embalm",
        "Keyword.Eternalize",
        "Keyword.Improvise",
        "Keyword.Affinity",
        "Keyword.Typecycling",
        "Keyword.Encore",
    ];

    /// What a keyword answers to each hook that does not need a game.
    // One flag per hook, named after it, so a row of the table reads as the hooks it sets.
    #[allow(clippy::struct_excessive_bools)]
    #[derive(Debug, PartialEq, Eq)]
    struct Hooks {
        answers_flying: bool,
        min_blockers: usize,
        forbids_attacking: bool,
        ignores_sickness: bool,
        attacks_untapped: bool,
        indestructible: bool,
        shields_player: bool,
        damage: DamageRules,
        timing: bool,
        abilities: usize,
    }

    impl Hooks {
        /// A keyword that changes nothing.
        fn none() -> Hooks {
            Hooks {
                answers_flying: false,
                min_blockers: 1,
                forbids_attacking: false,
                ignores_sickness: false,
                attacks_untapped: false,
                indestructible: false,
                shields_player: false,
                damage: DamageRules::default(),
                timing: false,
                abilities: 0,
            }
        }

        fn of(keyword: &Keyword) -> Hooks {
            let rules = rules(keyword);
            Hooks {
                answers_flying: rules.answers_evasion(keyword, &Keyword::Flying),
                min_blockers: rules.min_blockers(keyword),
                forbids_attacking: rules.forbids_attacking(keyword),
                ignores_sickness: rules.ignores_summoning_sickness(keyword),
                attacks_untapped: rules.attacks_untapped(keyword),
                indestructible: rules.is_indestructible(keyword),
                shields_player: rules.restricts_player_targeting(keyword, PlayerId(0), PlayerId(1)),
                damage: rules.modifies_damage(keyword),
                timing: rules.grants_timing(keyword),
                abilities: rules.adds_abilities(keyword).len(),
            }
        }
    }

    fn damage(f: impl FnOnce(&mut DamageRules)) -> DamageRules {
        let mut rules = DamageRules::default();
        f(&mut rules);
        rules
    }

    #[test]
    fn the_list_has_every_keyword_the_vocabulary_names() {
        let names: Vec<&str> = every_keyword().iter().map(Keyword::name).collect();
        assert_eq!(names, Keyword::NAMES);
    }

    #[rstest]
    #[case::flash(Keyword::Flash, Hooks { timing: true, ..Hooks::none() })]
    #[case::flying(Keyword::Flying, Hooks { answers_flying: true, ..Hooks::none() })]
    #[case::reach(Keyword::Reach, Hooks { answers_flying: true, ..Hooks::none() })]
    #[case::menace(Keyword::Menace, Hooks { min_blockers: 2, ..Hooks::none() })]
    #[case::defender(Keyword::Defender, Hooks { forbids_attacking: true, ..Hooks::none() })]
    #[case::haste(Keyword::Haste, Hooks { ignores_sickness: true, ..Hooks::none() })]
    #[case::vigilance(Keyword::Vigilance, Hooks { attacks_untapped: true, ..Hooks::none() })]
    #[case::indestructible(Keyword::Indestructible, Hooks { indestructible: true, ..Hooks::none() })]
    #[case::hexproof(Keyword::Hexproof, Hooks { shields_player: true, ..Hooks::none() })]
    #[case::shroud(Keyword::Shroud, Hooks { shields_player: true, ..Hooks::none() })]
    #[case::protection(Keyword::Protection(Selector::default()), Hooks::none())]
    #[case::first_strike(Keyword::FirstStrike, Hooks { damage: damage(|d| d.first_strike = true), ..Hooks::none() })]
    #[case::double_strike(Keyword::DoubleStrike, Hooks { damage: damage(|d| d.double_strike = true), ..Hooks::none() })]
    #[case::deathtouch(Keyword::Deathtouch, Hooks { damage: damage(|d| d.deathtouch = true), ..Hooks::none() })]
    #[case::trample(Keyword::Trample, Hooks { damage: damage(|d| d.trample = true), ..Hooks::none() })]
    #[case::lifelink(Keyword::Lifelink, Hooks { damage: damage(|d| d.lifelink = true), ..Hooks::none() })]
    #[case::prowess(Keyword::Prowess, Hooks { abilities: 1, ..Hooks::none() })]
    #[case::battle_cry(Keyword::BattleCry, Hooks { abilities: 1, ..Hooks::none() })]
    #[case::ward(Keyword::Ward(Cost::mana("{2}".parse().unwrap())), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::enchant(Keyword::Enchant(Selector::default()), Hooks::none())]
    #[case::enchant_player(Keyword::EnchantPlayer { opponent: true }, Hooks::none())]
    #[case::equip(Keyword::Equip(Cost::mana("{1}".parse().unwrap())), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::reconfigure(Keyword::Reconfigure(Cost::mana("{2}".parse().unwrap())), Hooks { abilities: 3, ..Hooks::none() })]
    #[case::living_weapon(Keyword::LivingWeapon, Hooks { abilities: 1, ..Hooks::none() })]
    #[case::crew(Keyword::Crew(3), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::saddle(Keyword::Saddle(2), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::saddled(Keyword::Saddled, Hooks::none())]
    #[case::level_up(Keyword::LevelUp(Cost::mana("{1}".parse().unwrap())), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::outlast(Keyword::Outlast(Cost::mana("{1}".parse().unwrap())), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::evoke(Keyword::Evoke(Cost::mana("{2}".parse().unwrap())), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::flashback(Keyword::Flashback(Cost::mana("{2}".parse().unwrap())), Hooks::none())]
    #[case::overload(Keyword::Overload(Cost::mana("{2}".parse().unwrap())), Hooks::none())]
    #[case::cascade(Keyword::Cascade, Hooks { abilities: 1, ..Hooks::none() })]
    #[case::storm(Keyword::Storm, Hooks { abilities: 1, ..Hooks::none() })]
    #[case::madness(Keyword::Madness(Cost::mana("{R}".parse().unwrap())), Hooks { abilities: 2, ..Hooks::none() })]
    #[case::dredge(Keyword::Dredge(3), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::changeling(Keyword::Changeling, Hooks { abilities: 1, ..Hooks::none() })]
    #[case::devoid(Keyword::Devoid, Hooks { abilities: 1, ..Hooks::none() })]
    #[case::partner(Keyword::Partner, Hooks::none())]
    #[case::partner_with(Keyword::PartnerWith("Pir, Imaginative Rascal".into()), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::partner_group(Keyword::PartnerGroup("Friends forever".into()), Hooks::none())]
    #[case::doctors_companion(Keyword::DoctorsCompanion, Hooks::none())]
    #[case::choose_a_background(Keyword::ChooseABackground, Hooks::none())]
    #[case::unearth(Keyword::Unearth(Cost::mana("{B}".parse().unwrap())), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::escape(Keyword::Escape(Cost::mana("{B}".parse().unwrap())), Hooks::none())]
    #[case::rebound(Keyword::Rebound, Hooks::none())]
    #[case::aftermath(Keyword::Aftermath, Hooks::none())]
    #[case::bestow(Keyword::Bestow(Cost::mana("{3}{G}".parse().unwrap())), Hooks::none())]
    #[case::dash(Keyword::Dash(Cost::mana("{1}{R}".parse().unwrap())), Hooks { abilities: 2, ..Hooks::none() })]
    #[case::blitz(Keyword::Blitz(Cost::mana("{1}{R}".parse().unwrap())), Hooks { abilities: 3, ..Hooks::none() })]
    #[case::encore(Keyword::Encore(Cost::mana("{2}{W}".parse().unwrap())), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::ninjutsu(Keyword::Ninjutsu(Cost::mana("{1}{U}".parse().unwrap())), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::echo(Keyword::Echo(Cost::mana("{2}".parse().unwrap())), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::cumulative_upkeep(Keyword::CumulativeUpkeep(Cost::mana("{1}".parse().unwrap())), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::undying(Keyword::Undying, Hooks { abilities: 1, ..Hooks::none() })]
    #[case::persist(Keyword::Persist, Hooks { abilities: 1, ..Hooks::none() })]
    #[case::vanishing(Keyword::Vanishing(3), Hooks { abilities: 3, ..Hooks::none() })]
    #[case::fading(Keyword::Fading(3), Hooks { abilities: 2, ..Hooks::none() })]
    #[case::backup(Keyword::Backup { count: 1, abilities: vec![] }, Hooks { abilities: 1, ..Hooks::none() })]
    #[case::myriad(Keyword::Myriad, Hooks { abilities: 1, ..Hooks::none() })]
    #[case::squad(Keyword::Squad(Cost::mana("{2}".parse().unwrap())), Hooks { abilities: 1, ..Hooks::none() })]
    #[case::ravenous(Keyword::Ravenous, Hooks { abilities: 2, ..Hooks::none() })]
    #[case::suspend(Keyword::Suspend { time: 2, cost: Cost::mana("{R}".parse().unwrap()) }, Hooks { abilities: 3, ..Hooks::none() })]
    fn a_keyword_answers_the_hooks_of_its_row(#[case] keyword: Keyword, #[case] row: Hooks) {
        assert_eq!(Hooks::of(&keyword), row);
    }

    #[test]
    fn only_four_keywords_and_squad_add_a_cost_and_only_multikicker_and_squad_repeat() {
        let given: Vec<(&str, bool, OptionalKind)> = every_keyword()
            .iter()
            .filter_map(|keyword| {
                let extra = rules(keyword).additional_cost(keyword)?;
                Some((extra.name, extra.repeatable, extra.kind))
            })
            .collect();
        assert_eq!(
            given,
            [
                ("Kicker", false, OptionalKind::Kicker),
                ("Multikicker", true, OptionalKind::Kicker),
                ("Buyback", false, OptionalKind::Buyback),
                ("Entwine", false, OptionalKind::Entwine),
                ("Squad", true, OptionalKind::Kicker),
            ]
        );
    }

    #[test]
    fn every_other_keyword_changes_nothing() {
        for keyword in every_keyword() {
            if RUN.contains(&format!("Keyword.{}", name_of_variant(&keyword)).as_str()) {
                continue;
            }
            assert_eq!(Hooks::of(&keyword), Hooks::none(), "{keyword:?}");
        }
    }

    /// The variant name a capability uses (`FirstStrike`), which is the debug text up to its parameters.
    fn name_of_variant(keyword: &Keyword) -> String {
        let text = format!("{keyword:?}");
        text.split(['(', ' ', '{']).next().unwrap_or("").to_owned()
    }

    #[test]
    fn the_capabilities_name_exactly_the_keywords_whose_rules_run() {
        let caps = caps();
        let claimed: Vec<String> = every_keyword()
            .into_iter()
            .flat_map(|keyword| used_primitives(&Ability::Keyword(keyword)))
            .filter(|name| name.starts_with("Keyword.") && caps.runs(name))
            .collect();
        let mut want: Vec<String> = RUN.iter().map(|name| (*name).to_owned()).collect();
        want.sort();
        let mut claimed = claimed;
        claimed.sort();
        assert_eq!(claimed, want);
    }

    #[test]
    fn the_run_list_names_a_keyword_variant_once_each() {
        let variants: Vec<String> = every_keyword().iter().map(name_of_variant).collect();
        let mut seen = Vec::new();
        for name in RUN.iter().copied() {
            let variant = name.strip_prefix("Keyword.").unwrap_or(name);
            assert!(variants.iter().any(|known| known == variant), "{name}");
            assert!(!seen.contains(&name), "{name} is listed twice");
            seen.push(name);
        }
    }

    fn holder(keywords: &[Keyword]) -> Arc<CardDefinition> {
        let mut card = CardDefinition::new("Holder")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2);
        for keyword in keywords {
            card = card.with_rules(Ability::Keyword(keyword.clone()));
        }
        Arc::new(card)
    }

    #[test]
    fn one_function_lists_what_a_card_prints_in_the_order_of_its_rules() {
        let game = state(
            vec![vec![], vec![]],
            vec![
                vec![
                    holder(&[Keyword::Haste, Keyword::Flying, Keyword::Trample]),
                    creature("Plain", 1, 1),
                ],
                vec![],
            ],
        );
        let [held, plain] = [0, 1].map(|at| game.zone(Zone::Battlefield)[at]);
        assert_eq!(
            keywords_of(&game, held),
            [&Keyword::Haste, &Keyword::Flying, &Keyword::Trample]
        );
        assert!(has_keyword(&game, held, &Keyword::Flying));
        assert!(!has_keyword(&game, held, &Keyword::Reach));
        assert_eq!(keywords_of(&game, plain).len(), 0);
        assert_eq!(keywords_of(&game, ObjectId(u64::MAX)).len(), 0);
    }

    #[test]
    fn one_function_lists_the_keywords_a_player_has() {
        let mut setup = crate::fixtures::setup(vec![vec![], vec![]], vec![vec![], vec![]]);
        setup.players[1].keywords = vec![Keyword::Hexproof];
        let game = GameState::from_setup(&setup).unwrap();
        assert_eq!(player_keywords(&game, PlayerId(1)), [&Keyword::Hexproof]);
        assert_eq!(player_keywords(&game, PlayerId(0)).len(), 0);
        assert_eq!(player_keywords(&game, PlayerId(9)).len(), 0);
        assert!(protects_player(&game, PlayerId(1), PlayerId(0)));
        assert!(!protects_player(&game, PlayerId(1), PlayerId(1)));
        assert!(!protects_player(&game, PlayerId(0), PlayerId(1)));
    }

    #[test]
    fn the_helpers_combine_the_keywords_of_one_object() {
        let game = state(
            vec![vec![], vec![]],
            vec![
                vec![
                    holder(&[Keyword::FirstStrike, Keyword::Lifelink, Keyword::Menace]),
                    holder(&[Keyword::Menace, Keyword::Defender, Keyword::Indestructible]),
                    creature("Plain", 1, 1),
                ],
                vec![],
            ],
        );
        let [mixed, wall, plain] = [0, 1, 2].map(|at| game.zone(Zone::Battlefield)[at]);
        assert_eq!(
            damage_rules(&game, mixed),
            damage(|d| {
                d.first_strike = true;
                d.lifelink = true;
            })
        );
        assert_eq!(damage_rules(&game, plain), DamageRules::default());
        assert_eq!(
            (min_blockers(&game, mixed), min_blockers(&game, plain)),
            (2, 1)
        );
        assert!(forbids_attacking(&game, wall) && !forbids_attacking(&game, mixed));
        assert!(is_indestructible(&game, wall) && !is_indestructible(&game, mixed));
        assert!(!attacks_untapped(&game, wall));
    }

    #[rstest]
    #[case::first_strike_alone(damage(|d| d.first_strike = true), true, false)]
    #[case::double_strike(damage(|d| d.double_strike = true), true, true)]
    #[case::both(damage(|d| { d.first_strike = true; d.double_strike = true; }), true, true)]
    #[case::neither(DamageRules::default(), false, true)]
    #[case::deathtouch_only(damage(|d| d.deathtouch = true), false, true)]
    fn a_creature_strikes_in_the_steps_its_keywords_allow(
        #[case] rules: DamageRules,
        #[case] first: bool,
        #[case] regular: bool,
    ) {
        assert_eq!(
            (rules.strikes_first(), rules.strikes_regularly()),
            (first, regular)
        );
    }

    fn grant(game: &mut GameState, id: ObjectId, keyword: Keyword, timestamp: u64) {
        use crate::effects::{ContinuousEffect, Duration, Selector};
        use crate::events::Event;
        use crate::fold::apply_event;
        use mtg_core::ids::EffectId;
        let effect = ContinuousEffect::new(
            EffectId(timestamp),
            Selector::objects(game, [id]),
            Modification::AddKeyword(keyword),
            Duration::UntilEndOfTurn,
            timestamp,
        );
        apply_event(game, &Event::EffectCreated { effect }).unwrap();
    }

    fn board() -> (GameState, ObjectId, ObjectId) {
        let game = state(
            vec![vec![creature("Idle", 1, 1)], vec![]],
            vec![
                vec![holder(&[Keyword::Haste]), creature("Plain", 1, 1)],
                vec![],
            ],
        );
        let [held, plain] = [0, 1].map(|at| game.zone(Zone::Battlefield)[at]);
        (game, held, plain)
    }

    #[test]
    fn granted_keywords_follow_the_printed_ones_in_timestamp_order_and_are_not_listed_twice() {
        let (mut game, held, plain) = board();
        // Stored out of order on purpose: the order is the timestamps', not the order of creation events.
        grant(&mut game, held, Keyword::Trample, 9);
        grant(&mut game, held, Keyword::Flying, 4);
        grant(&mut game, held, Keyword::Haste, 6);
        grant(&mut game, plain, Keyword::Flying, 5);
        assert_eq!(
            keywords_of(&game, held),
            [&Keyword::Haste, &Keyword::Flying, &Keyword::Trample],
            "printed first, then granted by timestamp; haste is not listed again"
        );
        assert_eq!(keywords_of(&game, plain), [&Keyword::Flying]);
        assert!(has_keyword(&game, plain, &Keyword::Flying));
        assert!(!has_keyword(&game, plain, &Keyword::Trample));
    }

    #[test]
    fn a_granted_keyword_reaches_every_reader_the_printed_one_does() {
        let (mut game, _, plain) = board();
        for (keyword, timestamp) in [
            (Keyword::FirstStrike, 1),
            (Keyword::Defender, 2),
            (Keyword::Indestructible, 3),
            (Keyword::Menace, 4),
            (Keyword::Vigilance, 5),
        ] {
            grant(&mut game, plain, keyword, timestamp);
        }
        assert!(damage_rules(&game, plain).first_strike);
        assert!(forbids_attacking(&game, plain));
        assert!(is_indestructible(&game, plain));
        assert_eq!(min_blockers(&game, plain), 2);
        assert!(attacks_untapped(&game, plain));
    }

    #[test]
    fn granted_haste_excuses_summoning_sickness() {
        let (mut game, _, plain) = board();
        if let Some(permanent) = game
            .objects
            .get_mut(&plain)
            .and_then(|o| o.permanent.as_mut())
        {
            permanent.summoning_sick = true;
        }
        assert!(summoning_sick(&game, plain));
        grant(&mut game, plain, Keyword::Haste, 1);
        assert!(!summoning_sick(&game, plain));
    }

    #[test]
    fn a_granted_keyword_belongs_to_the_object_named_while_the_effect_lasts() {
        let (mut game, held, plain) = board();
        let idle = game.zone(Zone::Hand(PlayerId(0)))[0];
        grant(&mut game, plain, Keyword::Flying, 1);
        assert_eq!(
            keywords_of(&game, held),
            [&Keyword::Haste],
            "another permanent"
        );
        // Effects touch permanents only; the keyword is not on a card in a hand.
        grant(&mut game, idle, Keyword::Flying, 2);
        assert_eq!(keywords_of(&game, idle).len(), 0);
        // A new object does not keep what the old one was given (CR 400.7).
        game.objects.get_mut(&plain).unwrap().counter += 1;
        assert_eq!(keywords_of(&game, plain).len(), 0);
    }
}
