//! The Learn course's practice games: one per learner, ended when the learner leaves the lesson or starts another, and
//! soon after the learner's last connection closed (`docs/server.md`, Tutorial table).

use std::sync::Arc;

use mtg_core::ids::PlayerId;
use mtg_engine::GameSetup;

use super::{CreateError, Created, GameOptions, Registry, Removal};
use crate::game_id::GameId;
use crate::handle::GameHandle;
use crate::pace::Pace;
use crate::recipe::FormatKind;
use crate::user_id::UserId;

/// What the registry knows of a practice game: who it is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Practice {
    /// The learner's account; none for a guest, whose game is found by its seat token.
    user: Option<UserId>,
}

/// The reason a practice game's seats are told when it ends early.
const LEFT: &str = "the lesson was left";

impl Registry {
    /// Registers a practice game (`crate::tutorial`): a Commander game that is never recorded or listed, with every
    /// seat held, `bot` played by a bot and no response clock. The earlier practice games of `user` end, so an account
    /// has one at a time. Returns its tokens and its handle, for the bot that plays a seat.
    pub async fn register_tutorial(
        &self,
        setup: GameSetup,
        names: Vec<String>,
        bot: PlayerId,
        user: Option<UserId>,
    ) -> Result<(Created, GameHandle), CreateError> {
        let pace = Pace {
            response: None,
            blocks: None,
            think: None,
            active: None,
            extension: None,
            relaxed: None,
            ..self.shared.game.pace
        };
        let options = GameOptions {
            format: FormatKind::Commander,
            pace: Some(pace),
            ..GameOptions::default()
        };
        let (created, handle) = self.admit(setup, names, None, None, options, None).await?;
        if let Some(entry) = self.games().get_mut(&created.id) {
            entry.practice = Some(Practice { user });
            entry.bots = vec![bot];
        }
        if let Some(user) = user {
            let earlier: Vec<GameId> = self
                .games()
                .iter()
                .filter(|(id, entry)| {
                    **id != created.id
                        && entry
                            .practice
                            .is_some_and(|practice| practice.user == Some(user))
                })
                .map(|(id, _)| *id)
                .collect();
            for game in earlier {
                self.end_practice_game(game).await;
            }
        }
        Ok((created, handle))
    }

    /// Ends the practice game `game` for the holder of its seat `token` (the learner left it); false, with nothing
    /// changed, when the token opens no seat of it or it is no practice game.
    pub async fn end_practice(&self, game: GameId, token: &str) -> bool {
        if self.authenticate(game, token).is_none() {
            return false;
        }
        self.end_practice_game(game).await
    }

    /// The practice games running or finished in the registry.
    pub fn practice_games(&self) -> Vec<GameId> {
        self.games()
            .iter()
            .filter(|(_, entry)| entry.practice.is_some())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Ends practice game `game`: its seats are told and its actor stops (the bot with it), and it leaves the registry,
    /// so its sockets close. Nothing is recorded; false when it is no practice game.
    async fn end_practice_game(&self, game: GameId) -> bool {
        let live = self
            .games()
            .get(&game)
            .filter(|entry| entry.practice.is_some())
            .map(|entry| (entry.handle.clone(), Arc::clone(&entry.turn)));
        let Some((handle, turn)) = live else {
            return false;
        };
        let _turn = turn.lock().await;
        // A finished game answers false and an actor that is gone an error: either way the entry goes.
        let _ = handle.abort(Some(LEFT.to_owned())).await;
        let removed = self.games().remove(&game);
        if let Some(entry) = &removed {
            entry.alive.send_replace(Removal::Dropped);
            tracing::info!(%game, "practice game ended");
        }
        removed.is_some()
    }
}
