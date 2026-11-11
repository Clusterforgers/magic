//! How many turns each player has begun this game (CR 500), counted as each turn begins, extra turns included: "your
//! first, second, or third turn of the game" (`Amount::TurnsTaken`).

use std::collections::BTreeMap;

use mtg_core::ids::PlayerId;

use crate::state::GameState;

impl GameState {
    /// How many turns the player has begun this game, the current one included; 0 before their first turn.
    pub(crate) fn rmt8_turns_taken(&self, player: PlayerId) -> u32 {
        self.rmt8_turns.get(&player).copied().unwrap_or(0)
    }

    /// Counts a turn the player begins, and keeps every player's life total as it begins.
    pub(crate) fn rmt8_count_turn(&mut self, player: PlayerId) {
        let turns = self.rmt8_turns.entry(player).or_insert(0);
        *turns = turns.saturating_add(1);
        self.rmt8_lives = self
            .players
            .iter()
            .map(|seat| (seat.id(), seat.life()))
            .collect();
    }

    /// The active player's upkeep begins: the latest of their upkeeps becomes the one before it.
    pub(crate) fn rmt8_mark_upkeep(&mut self) {
        let turn = self.turn.number;
        let upkeeps = self.rmt8_upkeeps.entry(self.turn.active).or_insert([0, 0]);
        *upkeeps = [upkeeps[1], turn];
    }

    /// Whether a permanent that fought in the turns `fought` (attacked, blocked, became blocked) did one of the `asked`
    /// things since the upkeep of `player` before their latest began: in that upkeep's turn (combat follows it) or later.
    /// Before a second upkeep of theirs, anything this game counts.
    pub(crate) fn rmt8_fought_since(
        &self,
        player: PlayerId,
        fought: [u32; 3],
        asked: [bool; 3],
    ) -> bool {
        let since = self
            .rmt8_upkeeps
            .get(&player)
            .map_or(0, |upkeeps| upkeeps[0]);
        fought
            .iter()
            .zip(asked)
            .any(|(turn, asked)| asked && *turn > 0 && *turn >= since)
    }

    /// The player's life total as the current turn began; `None` for a player the game never had.
    pub(crate) fn rmt8_life_as_turn_began(&self, player: PlayerId) -> Option<i32> {
        self.rmt8_lives.get(&player).copied()
    }
}

/// The turns each seat has begun in a game set up at turn `turn` (0 before the first), the seats taking their turns in
/// order from `first`.
pub(crate) fn turns_before(turn: u32, first: PlayerId, seats: usize) -> BTreeMap<PlayerId, u32> {
    let mut turns = BTreeMap::new();
    let seats = seats.max(1);
    for number in 0..turn {
        let seat = (first.index() + usize::try_from(number).unwrap_or(0)) % seats;
        let taken = turns.entry(PlayerId::from_index(seat)).or_insert(0u32);
        *taken = taken.saturating_add(1);
    }
    turns
}

#[cfg(test)]
#[path = "rmt8_turns_tests.rs"]
mod tests;

/// A permanent is turned face down (CR 708.2a): it lies face down as a morph does, so it is turned face up for its
/// morph cost; one that is not on the battlefield or is face down already is an error.
pub(crate) fn turn_face_down(
    state: &mut GameState,
    id: mtg_core::ids::ObjectId,
) -> Result<(), crate::error::EngineError> {
    let object = state
        .objects
        .get_mut(&id)
        .filter(|object| object.zone == crate::zones::Zone::Battlefield)
        .ok_or_else(|| crate::fold::invalid(format!("{id:?} is not a permanent")))?;
    if object.face_down.is_some() {
        return Err(crate::fold::invalid(format!("{id:?} is face down already")));
    }
    object.face_down = Some(crate::object::FaceDown::Morph);
    Ok(())
}

/// A permanent becomes harnessed: it was not, and it is a permanent.
pub(crate) fn set_harnessed(
    state: &mut GameState,
    id: mtg_core::ids::ObjectId,
) -> Result<(), crate::error::EngineError> {
    let perm = state
        .objects
        .get_mut(&id)
        .and_then(|object| object.permanent.as_mut())
        .ok_or_else(|| crate::fold::invalid(format!("{id:?} is not a permanent")))?;
    if perm.designations.harnessed {
        return Err(crate::fold::invalid(format!("{id:?} is harnessed already")));
    }
    perm.designations.harnessed = true;
    Ok(())
}

/// The creature that evolves by the change (CR 702.100d): +1/+1 counters put on a creature as an evolve ability of its own
/// resolves, read from the ability that resolves during the batch.
pub(crate) fn evolved(
    pre: &GameState,
    post: &GameState,
    changed: &crate::counters::CountersChanged,
) -> Option<mtg_core::ids::ObjectId> {
    use mtg_cards::keyword::Keyword;
    let crate::choose::Target::Object(holder) = changed.on else {
        return None;
    };
    if changed.counter != mtg_cards::value::CounterKind::PlusOnePlusOne || changed.delta <= 0 {
        return None;
    }
    let resolving = post.resolving().or_else(|| pre.resolving())?;
    let ability = post
        .object(resolving.object)
        .or_else(|| pre.object(resolving.object))?
        .ability()?;
    let from_keyword = matches!(
        ability.origin(),
        crate::abilities::AbilitySource::PrintedKeyword(_)
            | crate::abilities::AbilitySource::GrantedKeyword(_)
    );
    let evolve = crate::keywords::rules(&Keyword::Evolve).adds_abilities(&Keyword::Evolve);
    (from_keyword
        && ability.source().id == holder.id
        && evolve.iter().any(|rules| rules == ability.rules()))
    .then_some(holder.id)
}

/// Marks the turn in which permanents attack, block and become blocked, as the declarations fold.
pub(crate) fn mark_fought(state: &mut GameState, event: &crate::events::Event) {
    let turn = state.turn.number;
    let mut mark = |id: mtg_core::ids::ObjectId, at: usize| {
        if let Some(fought) = state
            .objects
            .get_mut(&id)
            .and_then(|object| object.permanent.as_mut())
            .and_then(|held| held.designations.fought.get_mut(at))
        {
            *fought = turn;
        }
    };
    match event {
        crate::events::Event::AttackersDeclared { attackers, .. } => {
            for (attacker, _) in attackers {
                mark(*attacker, 0);
            }
        }
        crate::events::Event::BlockersDeclared { blocks, .. } => {
            for (blocker, attacker) in blocks {
                mark(*blocker, 1);
                mark(*attacker, 2);
            }
        }
        _ => {}
    }
}
