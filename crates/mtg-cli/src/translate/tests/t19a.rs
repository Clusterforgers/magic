//! Goal: pack t19a reads `GenericChoice | TempRemember$ Chooser` over a group of players as one choice in a pass for each
//! player when every part asks that player to pay, and leaves the neighbouring shapes gaps.
#![allow(clippy::unwrap_used)]

use rstest::rstest;

use super::{abilities, gaps};

fn card(lines: &str) -> (String, Vec<String>) {
    let text = format!("Name:Subject\nTypes:Sorcery\n{lines}\nOracle:x\n");
    (
        serde_json::to_string(&abilities(&text)).unwrap(),
        gaps(&text),
    )
}

const PARTS: &str = "SVar:S:DB$ LoseLife | Defined$ Remembered | LifeAmount$ 3 | UnlessCost$ Sac<1/Creature> | UnlessPayer$ Remembered
SVar:D:DB$ LoseLife | Defined$ Remembered | LifeAmount$ 3 | UnlessCost$ Discard<1/Card> | UnlessPayer$ Remembered";

#[rstest]
#[case::opponents("Opponent", r#""over":"Opponents""#)]
#[case::players("Player", r#""over":"EachPlayer""#)]
fn each_chooser_pays_or_suffers(#[case] who: &str, #[case] over: &str) {
    let (json, gaps) = card(&format!(
        "A:SP$ GenericChoice | Defined$ {who} | TempRemember$ Chooser | Choices$ S,D\n{PARTS}"
    ));
    assert!(gaps.is_empty(), "{gaps:?}");
    assert!(json.contains(over), "{json}");
    assert!(json.contains(r#""OneOf":{"options""#), "{json}");
}

#[rstest]
#[case::a_part_that_is_not_a_payment(
    "A:SP$ GenericChoice | Defined$ Opponent | TempRemember$ Chooser | Choices$ S,N\nSVar:S:DB$ LoseLife | Defined$ Remembered | LifeAmount$ 3 | UnlessCost$ Discard<1/Card> | UnlessPayer$ Remembered\nSVar:N:DB$ Draw | Defined$ Remembered"
)]
#[case::the_payer_is_another(
    "A:SP$ GenericChoice | Defined$ Opponent | TempRemember$ Chooser | Choices$ S,D\nSVar:S:DB$ LoseLife | Defined$ Remembered | LifeAmount$ 3 | UnlessCost$ Discard<1/Card> | UnlessPayer$ You\nSVar:D:DB$ LoseLife | Defined$ Remembered | LifeAmount$ 3 | UnlessCost$ Sac<1/Creature> | UnlessPayer$ Remembered"
)]
#[case::a_secret_choice(
    "A:SP$ GenericChoice | Defined$ Opponent | TempRemember$ Chooser | Secretly$ True | Choices$ S,D\nSVar:S:DB$ LoseLife | Defined$ Remembered | LifeAmount$ 3 | UnlessCost$ Discard<1/Card> | UnlessPayer$ Remembered\nSVar:D:DB$ LoseLife | Defined$ Remembered | LifeAmount$ 3 | UnlessCost$ Sac<1/Creature> | UnlessPayer$ Remembered"
)]
#[case::a_hidden_choice(
    "A:SP$ GenericChoice | Defined$ Opponent | TempRemember$ Chooser | ShowChoice$ Never | Choices$ S,D\nSVar:S:DB$ LoseLife | Defined$ Remembered | LifeAmount$ 3 | UnlessCost$ Discard<1/Card> | UnlessPayer$ Remembered\nSVar:D:DB$ LoseLife | Defined$ Remembered | LifeAmount$ 3 | UnlessCost$ Sac<1/Creature> | UnlessPayer$ Remembered"
)]
fn near_misses_stay_gaps(#[case] lines: &str) {
    let (_, gaps) = card(lines);
    assert!(
        gaps.iter().any(|gap| gap.contains("GenericChoice")),
        "{gaps:?}"
    );
}
