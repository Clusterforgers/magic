//! Goal: on Postgres a deck led by two commanders works end to end for every pair kind (Partner, Partner with, a partner group, Doctor's companion, Choose a Background): a Moxfield list, which marks no commander, has its first line and the partner on its second line read as the commanders (and a second line that cannot pair stays in the main deck), a list that marks them keeps both, the stored deck passes the check, and a game made from it starts with both commanders in the seat's command zone, each with its own tax.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;
use crate::pg_commander::{create_body, deck, join, led_by, line, snapshot, start, store};

use net::{PASSWORD, connect, lobby, register};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A pair of the corpus with the set and number of each printing, whether the importer makes
/// the second card a commander, and whether a deck of the pair and 98 Forests is legal.
const PAIRS: [(&str, &str, &str, &str, bool, bool); 6] = [
    (
        "Thrasios, Triton Hero",
        "C16 46",
        "Silas Renn, Seeker Adept",
        "C16 43",
        true,
        true,
    ),
    (
        "Bebop, Skull & Crossbones",
        "TMC 15",
        "Rocksteady, Mutant Marauder",
        "TMC 33",
        true,
        true,
    ),
    (
        "Michelangelo, the Heart",
        "TMC 5",
        "Leonardo, the Balance",
        "TMC 1",
        true,
        true,
    ),
    (
        "Graham O'Brien",
        "WHO 104",
        "The Thirteenth Doctor",
        "WHO 4",
        true,
        true,
    ),
    // A Background is a commander beside its chooser, but neither has green for the Forests.
    (
        "Burakos, Party Leader",
        "CLB 653",
        "Folk Hero",
        "MSC 132",
        true,
        false,
    ),
    // Partner beside Partner with: no pair, so the second line stays in the deck.
    (
        "Thrasios, Triton Hero",
        "C16 46",
        "Bebop, Skull & Crossbones",
        "TMC 15",
        false,
        false,
    ),
];

fn import_body(text: &str) -> String {
    json!({"text": text, "format": "commander"}).to_string()
}

/// The zone and name of each imported line.
fn zones(reply: &Value) -> Vec<(String, String)> {
    reply["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| {
            (
                line["zone"].as_str().unwrap().to_owned(),
                line["card"]["name"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[sqlx::test(migrations = false)]
async fn a_paired_deck_imports_saves_and_starts_with_both_commanders(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bob = register(server.addr, "bob", PASSWORD).await;
    let bob_deck = store(&server, &bob, &led_by("Questing Beast")).await;

    for (first, first_printing, second, second_printing, pairs, plays) in PAIRS {
        // Moxfield writes the commanders first and marks none.
        let written = |name: &str, printing: &str| {
            let (set, number) = printing.split_once(' ').unwrap();
            format!("1 {name} ({set}) {number}")
        };
        let moxfield = format!(
            "{} *F*\n{}\n98 Forest (MDD) 301\n",
            written(first, first_printing),
            written(second, second_printing)
        );
        let reply = ann
            .post(server.addr, "/api/decks/import", &import_body(&moxfield))
            .await;
        assert_eq!(reply.status, 200, "{}", reply.text());
        let read = zones(&reply.json());
        let second_zone = if pairs { "commander" } else { "main" };
        assert_eq!(
            read[..2],
            [
                ("commander".to_owned(), first.to_owned()),
                (second_zone.to_owned(), second.to_owned()),
            ],
            "{first} and {second}: {}",
            reply.text()
        );
        assert_eq!(read[2].0, "main");

        // A list that marks its commanders keeps both, in any order.
        let arena = format!("Commander\n1 {second}\n1 {first}\n\nDeck\n98 Forest\n");
        let marked = zones(
            &ann.post(server.addr, "/api/decks/import", &import_body(&arena))
                .await
                .json(),
        );
        assert_eq!(marked[0], ("commander".to_owned(), second.to_owned()));
        assert_eq!(marked[1], ("commander".to_owned(), first.to_owned()));

        if !plays {
            continue;
        }
        let paired = deck(
            first,
            "commander",
            &[
                line("commander", first, 1),
                line("commander", second, 1),
                line("main", "Forest", 98),
            ],
        );
        let checked = ann
            .post(
                server.addr,
                "/api/decks/check",
                &json!({"format": "commander", "cards": paired["cards"]}).to_string(),
            )
            .await
            .json();
        assert_eq!(checked["legality"], json!([]), "{first} and {second}");
        let id = store(&server, &ann, &paired).await;

        let seats = [json!({"name": ann.username, "deck": id}), json!({})];
        let (game, mut tokens) = ann.create_game(server.addr, &create_body(&seats, 1)).await;
        let joined = join(&server, &bob, &game, &bob_deck).await;
        assert_eq!(joined.status, 200, "{}", joined.text());
        tokens.push(joined.json()["token"].as_str().unwrap().to_owned());
        lobby::start(&server.registry, &game, 2).await;

        let mut socket = connect(server.addr, &game, &tokens[0]).await;
        let view = snapshot(&mut socket, &game).await;
        let seat = &view.table.players[0];
        assert_eq!(seat.command.count, 2, "{first} and {second}");
        assert_eq!(seat.commanders.len(), 2);
        assert!(seat.commanders.iter().all(|c| c.casts == 0 && c.tax == 0));
        assert_eq!(view.table.players[1].commanders.len(), 1);
        socket.close().await;
    }
}
