//! The effects in force on one player, as the table shows them on the player's pod: the rules that name the player
//! and the keywords the player has. Read from the rules in force, so each holds exactly while the game applies it.

use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

use crate::state::GameState;
use crate::w17b_statics as w;

/// One effect in force on a player.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum PlayerEffect {
    /// The player can't lose the game (CR 104.3).
    CantLose,
    /// The player doesn't lose the game for having 0 or less life (CR 704.5a).
    LifeSpared,
    /// The player can't win the game.
    CantWin,
    /// The player can't cast spells now.
    CantCastSpells,
    /// The player can't gain life.
    CantGainLife,
    /// The player's life total can't change (CR 119.7, 119.8).
    LifeCantChange,
    /// Damage dealt to the player causes no loss of life.
    NoLifeLossFromDamage,
    /// Damage can't take the player's life below this total.
    LifeFloor(i32),
    /// The player can't draw cards.
    CantDraw,
    /// The player skips their draw step.
    SkipsDrawStep,
    /// The player draws from the bottom of their library rather than the top.
    DrawsFromBottom,
    /// The player can't search libraries.
    CantSearch,
    /// The player skips each extra turn they would begin.
    SkipsExtraTurns,
    /// The player has no maximum hand size.
    NoMaxHandSize,
    /// The player's maximum hand size is this many cards.
    MaxHandSize(u32),
    /// The player may play this many lands more each turn.
    ExtraLandPlays(u32),
    /// The player plays with their hand revealed.
    HandRevealed,
    /// The player plays with the top card of their library revealed.
    TopRevealed,
    /// The player may look at the top card of their library any time.
    LooksAtTop,
    /// The player has this keyword (hexproof, protection, shroud), by its shown name.
    Keyword(String),
}

/// Every effect in force on `player`, in the order of [`PlayerEffect`], keywords last and each once.
#[must_use]
pub fn player_effects(state: &GameState, player: PlayerId) -> Vec<PlayerEffect> {
    let rules = state.rules();
    let fixed = rules.no_life_change.contains(&player);
    let flags = [
        (w::w17b_cant_lose(state, player), PlayerEffect::CantLose),
        (w::w17b_life_spared(state, player), PlayerEffect::LifeSpared),
        (w::w17b_cant_win(state, player), PlayerEffect::CantWin),
        (
            w::w17b_cant_cast(state, player),
            PlayerEffect::CantCastSpells,
        ),
        (
            !fixed && rules.no_life_gain.contains(&player),
            PlayerEffect::CantGainLife,
        ),
        (fixed, PlayerEffect::LifeCantChange),
        (
            !fixed && rules.no_damage_loss.contains(&player),
            PlayerEffect::NoLifeLossFromDamage,
        ),
    ];
    let mut out: Vec<PlayerEffect> = flags
        .into_iter()
        .filter_map(|(on, effect)| on.then_some(effect))
        .collect();
    if let Some(floor) = rules
        .life_floors
        .get(&player)
        .and_then(|floors| floors.iter().copied().max())
    {
        out.push(PlayerEffect::LifeFloor(floor));
    }
    let more = [
        (rules.no_draw.contains(&player), PlayerEffect::CantDraw),
        (
            rules.skip_draw.contains(&player),
            PlayerEffect::SkipsDrawStep,
        ),
        (
            rules.draw_bottom.contains(&player),
            PlayerEffect::DrawsFromBottom,
        ),
        (rules.no_search.contains(&player), PlayerEffect::CantSearch),
        (
            rules.skip_extra_turns.contains(&player),
            PlayerEffect::SkipsExtraTurns,
        ),
        (
            rules.no_hand_limit.contains(&player),
            PlayerEffect::NoMaxHandSize,
        ),
    ];
    out.extend(
        more.into_iter()
            .filter_map(|(on, effect)| on.then_some(effect)),
    );
    if let Some(max) = rules.max_hand.get(&player) {
        out.push(PlayerEffect::MaxHandSize(*max));
    }
    if let Some(lands) = rules.extra_lands.get(&player).filter(|lands| **lands > 0) {
        out.push(PlayerEffect::ExtraLandPlays(*lands));
    }
    let sight = [
        (
            rules.reveal_hand.contains(&player),
            PlayerEffect::HandRevealed,
        ),
        (
            rules.reveal_top.contains(&player),
            PlayerEffect::TopRevealed,
        ),
        (rules.look_top.contains(&player), PlayerEffect::LooksAtTop),
    ];
    out.extend(
        sight
            .into_iter()
            .filter_map(|(on, effect)| on.then_some(effect)),
    );
    let granted = rules
        .player_keywords
        .iter()
        .filter(|(who, _)| *who == player)
        .map(|(_, keyword)| keyword);
    for keyword in crate::keywords::player_keywords(state, player)
        .into_iter()
        .chain(granted)
    {
        let shown = PlayerEffect::Keyword(keyword.shown_name());
        if !out.contains(&shown) {
            out.push(shown);
        }
    }
    out
}
