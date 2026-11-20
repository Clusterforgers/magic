//! Goal: "Always pay for me" pays whenever plain mana pays, at tables of two to four seats: monocolored hybrid (The Reaper, King No More from a land of any colour, two Glades and two Swamps), hybrid and Phyrexian symbols with mana, a source's plain ability; it asks only when the rest needs a Treasure or life, and the question says why.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common;

use common::{Client, game_id, roomy};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::mana::ManaSymbol;
use mtg_engine::PresetPermanent;
use mtg_engine::decision::PriorityAction;
use mtg_engine::mana::AskedWhy;
use mtg_engine::{Answer, DecisionKind, GameSetup};
use mtg_protocol::{AutoChoices, ClientMessage, FullControlMode, ServerMessage};
use mtg_server::{Dwell, GameConfig, GameHandle, MemoryStore, Pace, spawn_game_with};
use mtg_testkit::cards::{creature_costing, forest, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

/// No clock and no dwell: only the settings decide who is asked.
const OFF: Pace = Pace {
    auto_pass: true,
    response: None,
    blocks: None,
    think: None,
    active: None,
    extension: None,
    relaxed: None,
    grace: std::time::Duration::ZERO,
    dwell: Dwell::OFF,
};

/// An artifact or land whose mana ability adds one mana of any colour, sacrificing it when `sacrificed` (a Treasure).
fn any_colour(name: &str, sacrificed: bool) -> Arc<CardDefinition> {
    let cost = if sacrificed {
        r#"["Tap","SacrificeSelf"]"#
    } else {
        r#"["Tap"]"#
    };
    let rule = format!(
        r#"{{"Activated":{{"cost":{cost},"effects":[{{"AddMana":{{"mana":{{"AnyColor":1}}}}}}]}}}}"#
    );
    let kind = if sacrificed {
        CardType::Artifact
    } else {
        CardType::Land
    };
    Arc::new(
        CardDefinition::new(name)
            .with_type(kind)
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
    )
}

/// The permanent called `name`.
fn permanent(name: &str) -> PresetPermanent {
    ready(match name {
        "Forest" => forest(),
        "Mountain" => mountain(),
        "Swamp" => swamp(),
        "Treasure" => any_colour("Treasure", true),
        "Prism" => any_colour("Prism", false),
        other => card(other).unwrap(),
    })
}

/// The spell called `name`: The Reaper from the corpus, or a 1/1 costing the symbols the name lists.
fn spell(name: &str) -> Arc<CardDefinition> {
    let symbols: &[ManaSymbol] = match name {
        "Hybrid" => &[ManaSymbol::Hybrid(Color::Red, Color::Green)],
        "Phyrexian" => &[ManaSymbol::Phyrexian(Color::Green)],
        "Black Phyrexian" => &[ManaSymbol::Phyrexian(Color::Black)],
        "Red" => &[ManaSymbol::Colored(Color::Red)],
        "X" => &[ManaSymbol::Generic(3), ManaSymbol::Colored(Color::Green)],
        other => return card(other).unwrap(),
    };
    creature_costing(name, symbols, 1, 1)
}

/// `seats` seats; seat 0 holds `spell` and controls `lands`.
fn table(seats: usize, held: &str, lands: &[&str]) -> GameSetup {
    Scenario::new(seats)
        .hand(0, [spell(held)])
        .battlefield(0, lands.iter().map(|name| permanent(name)))
        .setup()
        .unwrap()
}

async fn send(handle: &GameHandle, from: &Client, message: ClientMessage) {
    from.say(handle, message).await;
    handle.status().await.unwrap();
}

/// Every seat joins and is ready, seat 0 with "Always pay for me" on; seat 0 casts its spell and gets what follows.
async fn cast(setup: GameSetup, seats: usize) -> Vec<ServerMessage> {
    let config = GameConfig {
        pace: OFF,
        ..roomy()
    };
    let handle = spawn_game_with(game_id(8), setup, MemoryStore::new(), config).unwrap();
    let mut clients = Vec::new();
    for n in 0..seats {
        clients.push(Client::join(&handle, n).await);
    }
    let choices = AutoChoices {
        auto_pay: true,
        ..AutoChoices::default()
    };
    send(&handle, &clients[0], ClientMessage::AutoChoices { choices }).await;
    let mode = FullControlMode::Off;
    send(&handle, &clients[0], ClientMessage::FullControl { mode }).await;
    for client in &clients {
        send(&handle, client, ClientMessage::Ready).await;
    }
    let (id, decision) = clients[0].decision().await;
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("seat 0 holds priority, got {decision:?}");
    };
    let answer = Answer::Priority(PriorityAction::CastSpell(options.castable[0]));
    send(
        &handle,
        &clients[0],
        ClientMessage::Answer {
            decision: id,
            answer,
        },
    )
    .await;
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    clients[0].drain()
}

/// Why each payment question in `messages` was asked; empty when none was.
fn asked(messages: &[ServerMessage]) -> Vec<Option<AskedWhy>> {
    messages
        .iter()
        .filter_map(|message| match message {
            ServerMessage::Decision { decision, .. } => match &decision.kind {
                DecisionKind::PayMana(options) => Some(options.asked.as_deref().cloned()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

const REAPER: &str = "The Reaper, King No More";
const REAPER_LANDS: &[&str] = &["Prism", "Cinder Glade", "Cinder Glade", "Swamp", "Swamp"];

#[rstest]
#[case::the_reaper(REAPER, REAPER_LANDS, None)]
#[case::hybrid("Hybrid", &["Forest"], None)]
#[case::phyrexian_with_mana("Phyrexian", &["Forest"], None)]
#[case::a_treasure_beside_unused_lands("Red", &["Forest", "Mountain", "Treasure"], None)]
#[case::x_paid("X", &["Forest", "Mountain", "Mountain", "Swamp"], None)]
#[case::only_a_treasure("Red", &["Forest", "Treasure"], Some(AskedWhy::GivesUp(Vec::new())))]
#[case::only_life("Black Phyrexian", &["Forest"], Some(AskedWhy::Life(ManaSymbol::Phyrexian(Color::Black))))]
#[tokio::test(start_paused = true)]
async fn always_pay_for_me_asks_only_when_plain_mana_cannot_pay(
    #[values(2, 3, 4)] seats: usize,
    #[case] held: &str,
    #[case] lands: &[&str],
    #[case] expected: Option<AskedWhy>,
) {
    let sent = cast(table(seats, held, lands), seats).await;
    let got: Vec<_> = asked(&sent)
        .into_iter()
        .map(|why| match why {
            // The Treasure's object id depends on the table; only the kind is checked.
            Some(AskedWhy::GivesUp(_)) => Some(AskedWhy::GivesUp(Vec::new())),
            other => other,
        })
        .collect();
    let wanted: Vec<_> = expected.into_iter().map(Some).collect();
    assert_eq!(got, wanted, "{sent:?}");
}
