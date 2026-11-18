//! Teams (CR 808, CR 810): who plays with whom, read once here so every "opponent" and every "your turn" follows the
//! table's teams.

use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};

use crate::state::GameState;

impl GameState {
    /// The table's teams; `None` when every player plays for themself.
    pub fn teams(&self) -> Option<&Teams> {
        self.house_rules().teams.as_ref()
    }

    /// The team of `player`; at a table without teams every seat is a team of its own.
    pub fn team_of(&self, player: PlayerId) -> u32 {
        self.teams().map_or_else(
            || u32::try_from(player.index()).unwrap_or(u32::MAX),
            |teams| teams.team_of(player),
        )
    }

    /// Whether `a` and `b` play on one team; a player is their own teammate.
    pub fn are_teammates(&self, a: PlayerId, b: PlayerId) -> bool {
        a == b || self.team_of(a) == self.team_of(b)
    }

    /// Whether `other` is an opponent of `player`: a player of another team (CR 102.3, CR 808.2).
    pub fn is_opponent(&self, player: PlayerId, other: PlayerId) -> bool {
        !self.are_teammates(player, other)
    }

    /// The teammates of `player` still in the game, themself left out, in turn order from the active player.
    pub fn r4710_teammates_of(&self, player: PlayerId) -> Vec<PlayerId> {
        let mut order = self.turn_order_from(self.turn().active());
        order.retain(|other| *other != player && self.are_teammates(player, *other));
        order
    }

    /// The opponents of `player` still in the game, in turn order from the active player.
    pub fn opponents_of(&self, player: PlayerId) -> Vec<PlayerId> {
        let mut order = self.turn_order_from(self.turn().active());
        order.retain(|other| self.is_opponent(player, *other));
        order
    }

    /// Whether `viewer` sees the hand of `owner` by the table's team rule: a teammate seat while `team_hands` holds.
    /// A viewer that is no seat (a spectator) is on no team.
    pub fn sees_team_hand(&self, viewer: PlayerId, owner: PlayerId) -> bool {
        self.ruleset().team_hands()
            && self
                .teams()
                .is_some_and(|teams| viewer.index() < teams.seats.len())
            && self.are_teammates(viewer, owner)
    }

    /// Whether teams share their turns (Two-Headed Giant and shared team turns, CR 805).
    pub fn shares_turns(&self) -> bool {
        self.teams().is_some_and(Teams::shares_turns)
    }

    /// Whether teams share their life totals (Two-Headed Giant, CR 810.9).
    pub fn shares_life(&self) -> bool {
        self.teams().is_some_and(Teams::shares_life)
    }

    /// Whether it is `player`'s turn: they are the active player, or a teammate of the active player when teams share
    /// their turns (CR 805.3).
    pub fn is_active(&self, player: PlayerId) -> bool {
        let active = self.turn().active();
        player == active || (self.shares_turns() && self.are_teammates(player, active))
    }

    /// The players whose turn it is and who are still in the game, the active player first and then in turn order.
    pub fn active_players(&self) -> Vec<PlayerId> {
        let mut order = self.turn_order_from(self.turn().active());
        order.retain(|player| self.is_active(*player));
        order
    }

    /// The seat whose turn follows a turn of `player`: the next living seat in turn order, past the teammates of `player`
    /// when teams share their turns (CR 805.2).
    pub fn next_turn_seat(&self, player: PlayerId) -> Option<PlayerId> {
        let mut next = self.next_alive_after(player)?;
        if self.shares_turns() {
            for _ in 0..self.players().len() {
                if !self.are_teammates(player, next) {
                    break;
                }
                next = self.next_alive_after(next)?;
            }
        }
        Some(self.turn_lead(next))
    }

    /// The seat that takes a turn of `player`'s team: under shared team turns its first living seat, so the boss and
    /// not a minion is the active player of its side's turn; `player` in any other game.
    pub fn turn_lead(&self, player: PlayerId) -> PlayerId {
        if self
            .teams()
            .is_none_or(|teams| teams.mode != TeamMode::SharedTurns)
        {
            return player;
        }
        self.alive_players()
            .find(|seat| self.are_teammates(*seat, player))
            .unwrap_or(player)
    }

    /// The teammates of `player` who share their life total (Two-Headed Giant, CR 810.9), `player` excluded.
    pub fn life_sharers(&self, player: PlayerId) -> Vec<PlayerId> {
        if !self.shares_life() {
            return Vec::new();
        }
        self.players()
            .iter()
            .map(crate::player::Player::id)
            .filter(|other| *other != player && self.are_teammates(player, *other))
            .collect()
    }

    /// The team every player still in the game plays on, once only one team is left (CR 104.2a, CR 808.6); `None` at a
    /// table without teams.
    pub fn last_team(&self) -> Option<u32> {
        self.teams()?;
        let mut alive = self.alive_players().map(|player| self.team_of(player));
        let first = alive.next()?;
        alive.all(|team| team == first).then_some(first)
    }
}
