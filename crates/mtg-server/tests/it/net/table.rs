//! Goal: a ready-made two-seat game on a running server, with the means to prove that nothing a misbehaving client sent changed it.

use std::net::SocketAddr;

use mtg_core::ids::PlayerId;
use mtg_protocol::{ClientMessage, ServerMessage};
use mtg_server::{GameHandle, GameStatus, ServerConfig};
use serde_json::json;

use super::{TestServer, WsClient, bot_config, connect, create_game};

pub fn two_seats() -> serde_json::Value {
    json!({ "format": "normal", "seats": [
        { "name": "Ann", "deck": "mono-green" },
        { "name": "Bo", "deck": "mono-red" },
    ]})
}

/// The bot config with a fixed seed, so the same seat always moves first.
pub fn table_config() -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(1),
        ..bot_config()
    }
}

/// A server with one two-seat game, started (waiting for its first answer) or still in the lobby.
pub struct Table {
    pub server: TestServer,
    pub game: String,
    pub tokens: Vec<String>,
    pub handle: GameHandle,
    pub before: GameStatus,
    /// The seat the running game waits for.
    pub waiting: usize,
}

impl Table {
    pub async fn start(config: ServerConfig, started: bool) -> Table {
        let server = TestServer::start(config).await;
        let (game, tokens) = create_game(server.addr, two_seats()).await;
        let handle = server.registry.handle(game.parse().unwrap()).unwrap();
        if started {
            for seat in [0, 1] {
                let ready = ClientMessage::Ready;
                handle
                    .send(PlayerId::from_index(seat), ready)
                    .await
                    .unwrap();
            }
        }
        let before = handle.status().await.unwrap();
        assert_eq!(before.started, started);
        Table {
            waiting: before.waiting_on.map_or(0, PlayerId::index),
            server,
            game,
            tokens,
            handle,
            before,
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.server.addr
    }

    /// Opens a seat's socket and joins; of what a running game then sends, only `Welcome` is consumed.
    pub async fn player(&self, seat: usize) -> WsClient {
        let mut client = connect(self.addr(), &self.game, &self.tokens[seat]).await;
        let welcomed = client.welcome(&self.game).await;
        assert_eq!(welcomed.index(), seat);
        client
    }

    /// Reads the snapshot and decision a running game sends the seat it waits for.
    pub async fn expect_resync(&self, client: &mut WsClient) {
        assert!(matches!(
            client.message().await,
            ServerMessage::Snapshot { .. }
        ));
        assert!(matches!(
            client.message().await,
            ServerMessage::Decision { .. }
        ));
    }

    /// The game must be exactly as it was: nothing a refused client sent got through.
    pub async fn assert_untouched(&self) {
        let now = self.handle.status().await.unwrap();
        assert_eq!(now.events, self.before.events);
        assert_eq!(now.state_hash, self.before.state_hash);
        assert_eq!(now.waiting_on, self.before.waiting_on);
        assert_eq!(now.result, None, "somebody conceded");
    }

    pub async fn nobody_connected(&self) {
        let connected = self.handle.status().await.unwrap().connected;
        assert_eq!(connected, Vec::<PlayerId>::new());
    }
}
