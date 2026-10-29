//! Rules of static abilities about targets in graveyards, losing and winning, casting, combat damage by toughness and
//! counters that stay on a card as it changes zones (`docs/engine.md`, Rules of the game).

use std::collections::{BTreeMap, BTreeSet};

use mtg_cards::modify::GameRule;
use mtg_cards::refs::Ref;
use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::controller_of_object;
use crate::object::GameObject;
use crate::resolve::{ResolveError, Scope};
use crate::rules::Rule;
use crate::state::GameState;
use crate::zones::Zone;

/// The rules of this module in force.
#[derive(Debug, Default)]
pub(crate) struct W17bRules {
    graveyards: bool,
    cant_lose: BTreeSet<PlayerId>,
    winless: BTreeSet<PlayerId>,
    cant_cast: Vec<(PlayerId, Option<PlayerId>)>,
    toughness: BTreeSet<PlayerId>,
}

/// None of these rules in force.
pub(crate) const W17B_NONE: W17bRules = W17bRules {
    graveyards: false,
    cant_lose: BTreeSet::new(),
    winless: BTreeSet::new(),
    cant_cast: Vec::new(),
    toughness: BTreeSet::new(),
};

impl W17bRules {
    /// Puts one of these rules in force.
    pub(crate) fn w17b_put(&mut self, rule: &Rule) {
        match rule {
            Rule::GraveyardCardsUntargetable => self.graveyards = true,
            Rule::CantLoseGame { who, winless } => {
                self.cant_lose.extend(who);
                self.winless.extend(winless);
            }
            Rule::CantCastSpells { who, turn } => {
                self.cant_cast
                    .extend(who.iter().map(|player| (*player, *turn)));
            }
            Rule::ToughnessAssignsDamage(players) => self.toughness.extend(players),
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
            Rule::CantLoseGame { who, winless }
        }
        GameRule::CantCastSpells { who, your_turn } => Rule::CantCastSpells {
            who: scope.players(who)?,
            turn: your_turn.then_some(scope.controller),
        },
        GameRule::ToughnessAssignsDamage(who) => Rule::ToughnessAssignsDamage(scope.players(who)?),
        _ => return Err(ResolveError::Unsupported("Rule.CountersRemain")),
    })
}

/// Whether one of these rules names no player, so putting it in force changes nothing.
pub(crate) fn w17b_names_nothing(rule: &Rule) -> bool {
    match rule {
        Rule::CantLoseGame { who, winless } => who.is_empty() && winless.is_empty(),
        Rule::CantCastSpells { who, .. } | Rule::ToughnessAssignsDamage(who) => who.is_empty(),
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

/// Whether `player` can't win the game now: an effect that would have them win does nothing.
pub fn w17b_cant_win(state: &GameState, player: PlayerId) -> bool {
    state.rules().w17b.winless.contains(&player)
}

/// Whether `player` can't cast spells now: a rule forbids it on every turn or during the active player's turn.
pub fn w17b_cant_cast(state: &GameState, player: PlayerId) -> bool {
    let active = state.turn().active();
    state
        .rules()
        .w17b
        .cant_cast
        .iter()
        .any(|(who, turn)| *who == player && turn.is_none_or(|turn| turn == active))
}

/// Whether the creature `id` assigns combat damage equal to its toughness rather than its power.
pub(crate) fn w17b_by_toughness(state: &GameState, id: ObjectId) -> bool {
    let rules = &state.rules().w17b;
    !rules.toughness.is_empty()
        && state
            .object(id)
            .is_some_and(|o| rules.toughness.contains(&controller_of_object(state, o)))
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
pub(crate) fn w17b_keeps_counters(object: &GameObject, to: Zone) -> bool {
    !matches!(to, Zone::Hand(_) | Zone::Library(_))
        && crate::ability_reader::w17b_definition_keeps_counters(object.card())
}
