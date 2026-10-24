//! Goal: the name of a token that starts with the card's own first name is shown as `~` by the clause and read back as such by the checker, and a token with another name is a mismatch.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use mtg_cards::Caps;
use mtg_cards::load::Library;
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::{PrintedCard, PrintedFace};
use rstest::rstest;

use crate::common::token_sources;

fn tuktuk() -> PrintedCard {
    PrintedCard {
        oracle_id: "00000000-0000-4000-8000-000000000001".to_owned(),
        name: "Tuktuk the Explorer".to_owned(),
        keywords: vec!["Haste".to_owned()],
        layout: "normal".to_owned(),
        faces: vec![PrintedFace {
            name: "Tuktuk the Explorer".to_owned(),
            mana_cost: "{2}{R}".to_owned(),
            type_line: "Legendary Creature \u{2014} Goblin".to_owned(),
            oracle_text: "Haste\nWhen Tuktuk the Explorer dies, create Tuktuk the Returned, a legendary 5/5 colorless Goblin Golem artifact creature token.".to_owned(),
            power: Some("2".to_owned()),
            toughness: Some("2".to_owned()),
            loyalty: None,
            defense: None,
            colors: vec!["R".to_owned()],
        }],
    }
}

fn rules(token: &str, power: u32) -> String {
    format!(
        r#"{{"faces":[{{"abilities":[{{"Keyword":"Haste"}},{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"from":"Battlefield","to":"Graveyard"}}}},"effects":[{{"CreateToken":{{"token":{{"Custom":{{"name":"{token}","supertypes":["Legendary"],"types":["Artifact","Creature"],"subtypes":["Goblin","Golem"],"power":{power},"toughness":5}}}}}}}}]}}}}]}}]}}"#
    )
}

#[rstest]
#[case::the_first_name_of_the_card("Tuktuk the Returned", 5, Verdict::Accepted)]
#[case::another_name("Rahzar the Returned", 5, Verdict::Mismatch)]
#[case::another_size("Tuktuk the Returned", 4, Verdict::Mismatch)]
fn a_token_named_after_the_card_is_shown_as_a_tilde(
    #[case] token: &str,
    #[case] power: u32,
    #[case] verdict: Verdict,
) {
    let tokens = Library::from_sources(&token_sources()).unwrap();
    let names: BTreeSet<String> = BTreeSet::from(["tuktuk the explorer".to_owned()]);
    let report = check(
        &rules(token, power),
        &tuktuk(),
        &tokens,
        &names,
        &Caps::everything(),
    );
    assert_eq!(report.verdict(), verdict, "{}", report.render(true));
}
