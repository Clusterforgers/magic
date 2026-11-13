//! Rules of static abilities about targets in graveyards, losing and winning, casting, combat damage by toughness and
//! counters that stay on a card as it changes zones (`docs/engine.md`, Rules of the game).

use std::collections::{BTreeMap, BTreeSet};

use mtg_cards::modify::GameRule;
use mtg_cards::refs::Ref;
use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::controller_of_object;
use crate::object::{GameObject, ObjectRef};
use crate::resolve::{ResolveError, Scope};
use crate::rules::Rule;
use crate::state::GameState;
use crate::zones::Zone;

/// The rules of this module in force.
#[derive(Debug, Default)]
pub(crate) struct W17bRules {
    graveyards: bool,
    cant_lose: BTreeSet<PlayerId>,
    life_spared: BTreeSet<PlayerId>,
    winless: BTreeSet<PlayerId>,
    /// Who can't cast spells, during whose turn (every turn when `None`), and the object whose rule it is.
    cant_cast: Vec<(PlayerId, Option<PlayerId>, Option<ObjectRef>)>,
    toughness: BTreeSet<PlayerId>,
    /// The players whose creatures' lethal damage is determined by their power.
    lethal_power: BTreeSet<PlayerId>,
    /// How much greater each player's devotion is.
    devotion: BTreeMap<PlayerId, u32>,
    /// The players dealt damage as though its source had infect.
    infect: BTreeSet<PlayerId>,
    /// The fewest ventures each limited player may make each turn.
    ventures: BTreeMap<PlayerId, u32>,
}

/// None of these rules in force.
pub(crate) const W17B_NONE: W17bRules = W17bRules {
    graveyards: false,
    cant_lose: BTreeSet::new(),
    life_spared: BTreeSet::new(),
    winless: BTreeSet::new(),
    cant_cast: Vec::new(),
    toughness: BTreeSet::new(),
    lethal_power: BTreeSet::new(),
    devotion: BTreeMap::new(),
    infect: BTreeSet::new(),
    ventures: BTreeMap::new(),
};

impl W17bRules {
    /// Puts one of these rules of `source` in force.
    pub(crate) fn w17b_put(&mut self, rule: &Rule, source: Option<ObjectRef>) {
        match rule {
            Rule::GraveyardCardsUntargetable => self.graveyards = true,
            Rule::CantLoseGame {
                who,
                winless,
                life_only,
            } => {
                if *life_only {
                    self.life_spared.extend(who);
                } else {
                    self.cant_lose.extend(who);
                }
                self.winless.extend(winless);
            }
            Rule::CantCastSpells { who, turn } => {
                self.cant_cast
                    .extend(who.iter().map(|player| (*player, *turn, source)));
            }
            Rule::ToughnessAssignsDamage(players) => self.toughness.extend(players),
            Rule::LethalByPower(players) => self.lethal_power.extend(players),
            Rule::DamageAsInfect(players) => self.infect.extend(players),
            Rule::VentureLimit { who, per_turn } => {
                for player in who {
                    let most = self.ventures.entry(*player).or_insert(*per_turn);
                    *most = (*most).min(*per_turn);
                }
            }
            Rule::DevotionIncreased { who, by } => {
                for player in who {
                    *self.devotion.entry(*player).or_default() += by;
                }
            }
            _ => {}
        }
    }
}

/// One of these rules as the engine plays it. Counters that remain are read from the card's definition as it moves
/// (`w17b_keeps_counters`), so no rule is ever in force for them.
pub(crate) fn w17b_lower(scope: &Scope<'_>, wanted: &GameRule) -> Result<Rule, ResolveError> {
    Ok(match wanted {
        GameRule::GraveyardCardsUntargetable => Rule::GraveyardCardsUntargetable,
        GameRule::CantLoseGame {
            who,
            opponents_cant_win,
            life_only,
        } => {
            let who = scope.players(who)?;
            let winless = if *opponents_cant_win {
                scope
                    .players(&Ref::EachPlayer)?
                    .into_iter()
                    .filter(|player| !who.contains(player))
                    .collect()
            } else {
                Vec::new()
            };
            Rule::CantLoseGame {
                who,
                winless,
                life_only: *life_only,
            }
        }
        GameRule::CantCastSpells { who, your_turn } => Rule::CantCastSpells {
            who: scope.players(who)?,
            turn: your_turn.then_some(scope.controller),
        },
        GameRule::ToughnessAssignsDamage(who) => Rule::ToughnessAssignsDamage(scope.players(who)?),
        GameRule::LethalByPower(who) => Rule::LethalByPower(scope.players(who)?),
        GameRule::DamageAsInfect(who) => Rule::DamageAsInfect(scope.players(who)?),
        GameRule::VentureLimit { who, per_turn } => Rule::VentureLimit {
            who: scope.players(who)?,
            per_turn: *per_turn,
        },
        GameRule::DevotionIncreased { who, by } => Rule::DevotionIncreased {
            who: scope.players(who)?,
            by: *by,
        },
        _ => return Err(ResolveError::Unsupported("Rule.CountersRemain")),
    })
}

/// Whether one of these rules names no player, so putting it in force changes nothing.
pub(crate) fn w17b_names_nothing(rule: &Rule) -> bool {
    match rule {
        Rule::CantLoseGame { who, winless, .. } => who.is_empty() && winless.is_empty(),
        Rule::CantCastSpells { who, .. }
        | Rule::ToughnessAssignsDamage(who)
        | Rule::LethalByPower(who)
        | Rule::DevotionIncreased { who, .. }
        | Rule::DamageAsInfect(who)
        | Rule::VentureLimit { who, .. } => who.is_empty(),
        _ => false,
    }
}

/// Whether cards in graveyards can't be targeted now.
pub(crate) fn w17b_graveyards_untargetable(state: &GameState) -> bool {
    state.rules().w17b.graveyards
}

/// Whether `object` is a card in a graveyard that can't be targeted now.
pub(crate) fn w17b_shielded_in_graveyard(state: &GameState, object: ObjectId) -> bool {
    w17b_graveyards_untargetable(state)
        && state
            .object(object)
            .is_some_and(|o| matches!(o.zone(), Zone::Graveyard(_)))
}

/// Whether `player` can't lose the game now (CR 104.3): a loss the rules or an effect would give them does not happen.
pub fn w17b_cant_lose(state: &GameState, player: PlayerId) -> bool {
    state.rules().w17b.cant_lose.contains(&player)
}

/// Whether `player` doesn't lose the game for having 0 or less life now (CR 704.5a); they still lose any other way.
pub fn w17b_life_spared(state: &GameState, player: PlayerId) -> bool {
    state.rules().w17b.life_spared.contains(&player)
}

/// Whether `player` can't win the game now: an effect that would have them win does nothing.
pub fn w17b_cant_win(state: &GameState, player: PlayerId) -> bool {
    state.rules().w17b.winless.contains(&player)
}

/// Whether `player` can't cast spells now: a rule forbids it on every turn or during the active player's turn.
pub fn w17b_cant_cast(state: &GameState, player: PlayerId) -> bool {
    w17b_cant_cast_by(state, player).is_some()
}

/// The first "can't cast spells" rule that holds for `player` now: `Some` with the object whose rule it is, if any.
#[allow(clippy::option_option)]
pub(crate) fn w17b_cant_cast_by(state: &GameState, player: PlayerId) -> Option<Option<ObjectRef>> {
    let active = state.turn().active();
    state
        .rules()
        .w17b
        .cant_cast
        .iter()
        .find(|(who, turn, _)| *who == player && turn.is_none_or(|turn| turn == active))
        .map(|(_, _, source)| *source)
}

/// Whether all damage dealt to `player` is dealt as though its source had infect.
pub(crate) fn r377_damage_as_infect(state: &GameState, player: PlayerId) -> bool {
    state.rules().w17b.infect.contains(&player)
}

/// Whether `player` has ventured into the dungeon as many times this turn as a rule allows.
pub(crate) fn r377_ventured_enough(state: &GameState, player: PlayerId) -> bool {
    state
        .rules()
        .w17b
        .ventures
        .get(&player)
        .is_some_and(|most| state.tallied(player, crate::state::Tally::R377Ventured) >= *most)
}

/// How much greater `player`'s devotion to each color and combination of colors is (CR 700.5).
pub(crate) fn r377_devotion_bonus(state: &GameState, player: PlayerId) -> u32 {
    state
        .rules()
        .w17b
        .devotion
        .get(&player)
        .copied()
        .unwrap_or(0)
}

/// Whether lethal damage to the creature `id` is determined by its power rather than its toughness (CR 704.5g).
pub(crate) fn r377_lethal_by_power(state: &GameState, id: ObjectId) -> bool {
    let rules = &state.rules().w17b;
    !rules.lethal_power.is_empty()
        && state
            .object(id)
            .is_some_and(|o| rules.lethal_power.contains(&controller_of_object(state, o)))
}

/// Whether the creature `id` assigns combat damage equal to its toughness rather than its power.
pub(crate) fn w17b_by_toughness(state: &GameState, id: ObjectId) -> bool {
    let rules = &state.rules().w17b;
    crate::combat_quirks::assigns_by_toughness(state, id)
        || (!rules.toughness.is_empty()
            && state
                .object(id)
                .is_some_and(|o| rules.toughness.contains(&controller_of_object(state, o))))
}

/// The counters that remain on `object` as it moves to `to`: those on the permanent or on the card, when it keeps them
/// (`w17b_keeps_counters`); none otherwise.
pub(crate) fn w17b_kept_counters(object: &GameObject, to: Zone) -> BTreeMap<CounterKind, u32> {
    if !w17b_keeps_counters(object, to) {
        return BTreeMap::new();
    }
    match &object.permanent {
        Some(permanent) => permanent.counters.clone(),
        None => object.card_counters.clone(),
    }
}

/// Puts the counters that remained back on `object` in its new zone: on the permanent it became, else on the card.
pub(crate) fn w17b_restore_counters(object: &mut GameObject, kept: BTreeMap<CounterKind, u32>) {
    if kept.is_empty() {
        return;
    }
    match &mut object.permanent {
        Some(permanent) => permanent.counters = kept,
        None => object.card_counters = kept,
    }
}

/// Whether counters remain on the card as it moves to `to` (CR 122.2): its definition prints the rule and `to` is
/// neither a hand nor a library.
pub fn w17b_keeps_counters(object: &GameObject, to: Zone) -> bool {
    !matches!(to, Zone::Hand(_) | Zone::Library(_))
        && crate::ability_reader::w17b_definition_keeps_counters(object.card())
}
