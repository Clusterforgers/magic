//! Take-backs: a player's last cast, activation or land play undone with the table's consent, and a cast backed out of
//! together with the mana tapped for it. Both rebuild the game from the log without the events since the action began
//! (`docs/engine.md`, Take-backs); this module judges which events allow that.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::events::Event;
use crate::state::GameState;
use crate::zones::Zone;

/// What a take-back undoes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum TakeBackKind {
    /// A spell was cast.
    Cast,
    /// An ability was activated.
    Activation,
    /// A land was played.
    Land,
}

/// Why a player's last action cannot be taken back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Error)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum TakeBackRefusal {
    /// The table plays without take-backs (`HouseRules::take_backs`).
    #[error("this table plays without take-backs")]
    Off,
    /// The player has no cast, activation or land play that is the table's last action.
    #[error("there is nothing to take back")]
    Nothing,
    /// The game moved on since: something resolved, a step ended or the game ended.
    #[error("the game has moved on since")]
    MovedOn,
    /// Another player answered a question or acted since.
    #[error("another player has acted since")]
    OtherPlayerActed,
    /// A card was drawn since.
    #[error("a card was drawn since")]
    CardDrawn,
    /// A hidden card was revealed, looked at or moved out of a hidden zone since.
    #[error("a hidden card became known since")]
    CardRevealed,
    /// A library was searched since.
    #[error("a library was searched since")]
    LibrarySearched,
    /// A library was shuffled since.
    #[error("a library was shuffled since")]
    LibraryShuffled,
    /// Something was decided at random since (a coin, a die, a random pick).
    #[error("something random happened since")]
    Random,
}

/// What a take-back would undo, as the table is asked about it. It names the card, never its object: the card may be
/// back in a hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TakeBackOffer {
    /// Who would take it back.
    pub player: PlayerId,
    /// What they did.
    pub kind: TakeBackKind,
    /// The card cast or played, or the source of the ability.
    pub name: String,
    /// Where the action begins in the log: the events from here on are undone.
    pub at: u64,
}

/// The table's last action: where it begins in the log and the generator's position there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LastAction {
    pub(crate) offer: TakeBackOffer,
    /// Why it can no longer be taken back, once a later step ruled it out.
    pub(crate) blocked: Option<TakeBackRefusal>,
    pub(crate) from: usize,
    pub(crate) rng: u64,
}

/// A run of mana abilities a player activated at priority: where it begins in the log and the generator's position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ManaRun {
    pub(crate) player: PlayerId,
    pub(crate) from: usize,
    pub(crate) rng: u64,
}

/// One answered step, as the take-back records read it.
pub(crate) struct StepFacts<'a> {
    /// The player who acted.
    pub(crate) player: PlayerId,
    /// Whether the step answered a priority decision.
    pub(crate) priority: bool,
    /// Whether the answer passed priority.
    pub(crate) pass: bool,
    /// Where the step's events begin in the log.
    pub(crate) mark: usize,
    /// The generator's position before the step.
    pub(crate) rng: u64,
    /// The step's events.
    pub(crate) events: &'a [Event],
}

/// The cast, activation or land play of `player` among `events`.
fn action_of(events: &[Event], player: PlayerId) -> Option<(TakeBackKind, ObjectId)> {
    events.iter().find_map(|event| match event {
        Event::SpellCast {
            player: p, object, ..
        } if *p == player => Some((TakeBackKind::Cast, *object)),
        Event::AbilityActivated {
            player: p, source, ..
        } if *p == player => Some((TakeBackKind::Activation, source.id)),
        Event::LandPlayed {
            player: p, object, ..
        } if *p == player => Some((TakeBackKind::Land, *object)),
        _ => None,
    })
}

/// Whether `events` are mana abilities `player` activated and nothing else of note.
fn mana_only(events: &[Event], player: PlayerId) -> bool {
    events
        .iter()
        .any(|e| matches!(e, Event::ManaAbilityActivated { player: p, .. } if *p == player))
        && action_of(events, player).is_none()
}

/// The table's last action after `step`, given the one before it; `state` is the state after the step.
pub(crate) fn next_last(
    last: Option<LastAction>,
    state: &GameState,
    run: Option<ManaRun>,
    step: &StepFacts<'_>,
) -> Option<LastAction> {
    if let Some((kind, object)) = action_of(step.events, step.player) {
        let (from, rng) = run
            .filter(|run| run.player == step.player)
            .map_or((step.mark, step.rng), |run| (run.from, run.rng));
        return Some(LastAction {
            offer: TakeBackOffer {
                player: step.player,
                kind,
                name: state.name_of(object).unwrap_or_default(),
                at: u64::try_from(from).unwrap_or(u64::MAX),
            },
            blocked: None,
            from,
            rng,
        });
    }
    let keeps = step.events.is_empty()
        || step.pass
        || last.as_ref().is_some_and(|last| {
            step.player == last.offer.player
                && (!step.priority || mana_only(step.events, step.player))
        });
    if keeps {
        last
    } else {
        acted_aside(last, step.player)
    }
}

/// The table's last action after `player` acted on something else (a manual action, a concession, another answer).
pub(crate) fn acted_aside(last: Option<LastAction>, player: PlayerId) -> Option<LastAction> {
    let mut last = last?;
    if last.blocked.is_none() {
        last.blocked = Some(if player == last.offer.player {
            TakeBackRefusal::MovedOn
        } else {
            TakeBackRefusal::OtherPlayerActed
        });
    }
    Some(last)
}

/// The mana run after `step`, given the one before it.
pub(crate) fn next_run(run: Option<ManaRun>, step: &StepFacts<'_>) -> Option<ManaRun> {
    if step.events.is_empty() {
        return run;
    }
    if !(step.priority && mana_only(step.events, step.player)) {
        return None;
    }
    Some(
        run.filter(|run| run.player == step.player)
            .unwrap_or(ManaRun {
                player: step.player,
                from: step.mark,
                rng: step.rng,
            }),
    )
}

/// Why the events since an action keep it from being undone for `player`, if anything does; `rng` is the generator's
/// position where they begin and `state` the state after them.
pub(crate) fn refusal(
    events: &[Event],
    player: PlayerId,
    rng: u64,
    state: &GameState,
) -> Option<TakeBackRefusal> {
    let found = events.iter().find_map(|event| class(event, player));
    found.or_else(|| (state.rng_index() != rng).then_some(TakeBackRefusal::Random))
}

/// Whether a card leaving `from` becomes known to a player who could not see it.
fn hidden_from(from: Zone, player: PlayerId) -> bool {
    match from {
        Zone::Library(_) => true,
        Zone::Hand(owner) => owner != player,
        _ => false,
    }
}

/// The refusal one event since an action of `player` makes, if any. Every variant is named, so a new event needs a
/// decision here.
#[allow(clippy::too_many_lines)]
fn class(event: &Event, player: PlayerId) -> Option<TakeBackRefusal> {
    use TakeBackRefusal as R;
    match event {
        Event::CardDrawn { .. } | Event::DrewFromEmptyLibrary { .. } | Event::Cycled { .. } => {
            Some(R::CardDrawn)
        }
        Event::ZoneChanged { from, .. } if hidden_from(*from, player) => Some(R::CardRevealed),
        Event::Revealed(_)
        | Event::FirstDrawRevealed { .. }
        | Event::FaceDownLooked { .. }
        | Event::LibraryLooked(_)
        | Event::HandLooked(_)
        | Event::Scried { .. }
        | Event::Surveilled { .. }
        | Event::Explored { .. }
        | Event::Clashed { .. }
        | Event::Connived { .. }
        | Event::Discovered { .. }
        | Event::DreadManifested { .. }
        | Event::Learned { .. }
        | Event::TurnedFaceUp { .. }
        | Event::CompanionRevealed { .. }
        | Event::HandDealt { .. }
        | Event::CardsBottomed { .. }
        | Event::LibraryBottomed { .. }
        | Event::LibraryInserted { .. }
        | Event::ManualAction { .. } => Some(R::CardRevealed),
        Event::CardsFound(_) | Event::Sought { .. } => Some(R::LibrarySearched),
        Event::LibraryShuffled { .. }
        | Event::LibraryRefilled { .. }
        | Event::AttractionDeckShuffled { .. } => Some(R::LibraryShuffled),
        Event::DieRolled { .. }
        | Event::CoinFlipped { .. }
        | Event::ChosenAtRandom { .. }
        | Event::PlayersChosenAtRandom { .. }
        | Event::RolledToVisit { .. }
        | Event::StickerSheetsChosen { .. }
        | Event::ManualRolled { .. }
        | Event::FirstPlayerChosen { .. }
        | Event::MulliganTaken { .. } => Some(R::Random),
        Event::SpellResolved { .. }
        | Event::ResolutionBegan { .. }
        | Event::StepDone { .. }
        | Event::ResolutionEnded { .. }
        | Event::StepBegan { .. }
        | Event::TurnBegan { .. }
        | Event::GameEnded { .. }
        | Event::PlayerLost { .. }
        | Event::BecameSpirit { .. }
        | Event::SpiritRevived { .. }
        | Event::PhaseBegan { .. }
        | Event::SchemeSetInMotion { .. }
        | Event::GameRestarted { .. }
        | Event::MulliganPhaseBegan
        | Event::MulliganDeclared { .. }
        | Event::MulliganKept { .. }
        | Event::MulliganPhaseEnded
        | Event::OpeningCardsPlayed { .. }
        | Event::TookBack { .. } => Some(R::MovedOn),
        Event::ZoneChanged { .. }
        | Event::ExtraTurnAdded { .. }
        | Event::ExtraTurnSkipped { .. }
        | Event::SpellControlGained { .. }
        | Event::PlayerControlGained { .. }
        | Event::PhasesAdded { .. }
        | Event::StepAdded { .. }
        | Event::NothingToDo { .. }
        | Event::AnnouncementRewound { .. }
        | Event::PriorityGranted { .. }
        | Event::PriorityPassed { .. }
        | Event::LandPlayed { .. }
        | Event::SpellCast { .. }
        | Event::PermanentTapped { .. }
        | Event::PermanentUntapped { .. }
        | Event::PermanentPhasedOut { .. }
        | Event::PermanentPhasedIn { .. }
        | Event::ManaAdded { .. }
        | Event::ManaSpent { .. }
        | Event::ManaPoolEmptied { .. }
        | Event::ManaBurned { .. }
        | Event::LifePaid { .. }
        | Event::CardDiscarded { .. }
        | Event::AttackersDeclared { .. }
        | Event::BandFormed { .. }
        | Event::BlockersDeclared { .. }
        | Event::DamageAssigned { .. }
        | Event::DamageDealt { .. }
        | Event::CombatEnded
        | Event::DamageCleared
        | Event::RemovedFromCombat { .. }
        | Event::AttackersBlocked { .. }
        | Event::Regenerated { .. }
        | Event::DamageRemoved { .. }
        | Event::ShieldUsed { .. }
        | Event::EffectCreated { .. }
        | Event::EffectExpired { .. }
        | Event::RuleCreated { .. }
        | Event::RuleExpired { .. }
        | Event::CreatureDied { .. }
        | Event::ObjectLeftGame { .. }
        | Event::AbilityPutOnStack(_)
        | Event::AbilityActivated { .. }
        | Event::ManaAbilityActivated { .. }
        | Event::AbilityTriggered(_)
        | Event::TriggersOrdered { .. }
        | Event::TriggerRemoved { .. }
        | Event::DelayedTriggerCreated(_)
        | Event::DelayedTriggerExpired { .. }
        | Event::ChoiceMade { .. }
        | Event::AbilityLeftToTable(_)
        | Event::ByHandDue(_)
        | Event::LifeGained { .. }
        | Event::LifeLost { .. }
        | Event::CountersChanged(_)
        | Event::TokenCreated(_)
        | Event::EmblemCreated { .. }
        | Event::CardConjured(_)
        | Event::Entered { .. }
        | Event::Attached { .. }
        | Event::ChosenAsAttached { .. }
        | Event::AttachedToPlayer { .. }
        | Event::JoinedAttack { .. }
        | Event::AttackRedirected { .. }
        | Event::Unattached { .. }
        | Event::Paired { .. }
        | Event::Unpaired { .. }
        | Event::Transformed { .. }
        | Event::Flipped { .. }
        | Event::Prepared { .. }
        | Event::Unprepared { .. }
        | Event::Suspected { .. }
        | Event::Unsuspected { .. }
        | Event::Solved { .. }
        | Event::Harnessed { .. }
        | Event::BecameCopy(_)
        | Event::SpellCopied(_)
        | Event::CardCopied(_)
        | Event::CopyCeased { .. }
        | Event::AbilityCopied(_)
        | Event::TargetsChanged(_)
        | Event::Remembered { .. }
        | Event::CopyEnded { .. }
        | Event::MonarchChanged { .. }
        | Event::RingTempted { .. }
        | Event::RingBearerLost { .. }
        | Event::SpeedChanged { .. }
        | Event::CitysBlessingGained { .. }
        | Event::EnduringStoryGained { .. }
        | Event::Ventured { .. }
        | Event::DungeonCompleted { .. }
        | Event::InitiativeTaken { .. }
        | Event::EvidenceCollected { .. }
        | Event::Investigated { .. }
        | Event::GiftGiven { .. }
        | Event::UpkeepSettled { .. }
        | Event::CommanderColorChosen { .. }
        | Event::Foraged { .. }
        | Event::PrizeClaimed { .. }
        | Event::StickerPut { .. }
        | Event::Proliferated { .. }
        | Event::Amassed { .. }
        | Event::Devoured { .. }
        | Event::Monstrous { .. }
        | Event::Renowned { .. }
        | Event::Enlisted { .. }
        | Event::Exploited { .. }
        | Event::Fought { .. }
        | Event::Crewed { .. }
        | Event::ClassLevelGained { .. }
        | Event::TurnedFaceDown { .. }
        | Event::DoorUnlocked { .. }
        | Event::DoorLocked { .. }
        | Event::Mutated { .. }
        | Event::CardParted { .. }
        | Event::Adapted { .. }
        | Event::Bolstered { .. }
        | Event::Exerted { .. }
        | Event::ExertEnded { .. }
        | Event::DayNightChanged { .. }
        | Event::CommanderStayed { .. }
        | Event::DamagePrevented { .. }
        | Event::VotesFinished { .. }
        | Event::CompanionBrought { .. } => None,
    }
}

#[cfg(test)]
#[path = "take_back_tests.rs"]
mod tests;
