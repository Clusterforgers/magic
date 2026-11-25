//! Settles a run the moment its boss fight's game ends: the registry tells of each recorded game whose end is
//! written, and the run of a Delves fight is brought up to date and saved, so its sockets hear the result without
//! anyone reading the run. A read settles the same way, so whichever comes first folds the fight in once.

use tokio::sync::broadcast::error::RecvError;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::catch_up;
use crate::game_id::GameId;
use crate::server::AppState;

/// Starts the listener of a server with Delves; none without a database. It ends when the server stops.
pub(crate) fn spawn_settler(app: AppState) -> Option<JoinHandle<()>> {
    app.delves()?;
    let mut ended = app.registry().ended_games();
    let mut stop = app.stop();
    Some(tokio::spawn(async move {
        loop {
            let games = tokio::select! {
                () = stopped(&mut stop) => return,
                game = ended.recv() => match game {
                    Ok(game) => vec![game],
                    // Fell behind: any live fight may be among the games missed.
                    Err(RecvError::Lagged(_)) => app
                        .registry()
                        .delve_games()
                        .into_iter()
                        .map(|(game, _)| game)
                        .collect(),
                    Err(RecvError::Closed) => return,
                },
            };
            for game in games {
                settle(&app, game).await;
            }
        }
    }))
}

/// Brings the run `game` is a fight of up to date; any other game is left alone.
async fn settle(app: &AppState, game: GameId) {
    let Some(run) = app.registry().delve_of(game) else {
        return;
    };
    let (Some(delves), Ok(run)) = (app.delves(), Uuid::parse_str(&run)) else {
        return;
    };
    let _turn = delves.turn.lock().await;
    if let Err(error) = catch_up(app, delves, run).await {
        tracing::warn!(?error, delve = %run, %game, "an ended fight waits for the next read of its run");
    }
}

/// Resolves once the server is stopping (or gone, which comes to the same).
async fn stopped(stop: &mut watch::Receiver<bool>) {
    let _ = stop.wait_for(|stopping| *stopping).await;
}
