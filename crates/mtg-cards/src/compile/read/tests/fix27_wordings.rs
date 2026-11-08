//! Goal: a printed text that names the card by its kind ("this Equipment", "this Vehicle") or says "enters the battlefield" reads as the written text with `~` and "enters" before any wording is normalized, and a text of another meaning still differs.
use super::*;

#[rstest]
#[case::this_equipment(
    "When this Equipment enters, manifest dread, then attach this Equipment to that creature.",
    "When ~ enters, manifest dread, then attach ~ to that creature.",
    true
)]
#[case::this_equipment_other_target(
    "When this Equipment enters, manifest dread, then attach this Equipment to target creature.",
    "When ~ enters, manifest dread, then attach ~ to that creature.",
    false
)]
#[case::this_vehicle(
    "Whenever this Vehicle deals combat damage to a player, you may cast target instant or sorcery card from that player's graveyard without paying its mana cost. If that spell would be put into a graveyard, exile it instead.",
    "Whenever ~ deals combat damage to a player, you may cast target instant or sorcery card from that player's graveyard without paying its mana cost. If that spell would be put into a graveyard, exile it instead.",
    true
)]
#[case::this_vehicle_other_graveyard(
    "Whenever this Vehicle deals combat damage to a player, you may cast target instant or sorcery card from your graveyard without paying its mana cost. If that spell would be put into a graveyard, exile it instead.",
    "Whenever ~ deals combat damage to a player, you may cast target instant or sorcery card from that player's graveyard without paying its mana cost. If that spell would be put into a graveyard, exile it instead.",
    false
)]
#[case::enters_the_battlefield(
    "When ~ enters the battlefield or attacks, conjure a card named Stormfront Pegasus into your hand.",
    "When ~ enters or attacks, conjure a card named Stormfront Pegasus into your hand.",
    true
)]
#[case::leaves_the_battlefield(
    "When ~ leaves the battlefield or attacks, conjure a card named Stormfront Pegasus into your hand.",
    "When ~ enters or attacks, conjure a card named Stormfront Pegasus into your hand.",
    false
)]
fn fix27_the_card_and_enters_are_one_wording_first(
    #[case] left: &str,
    #[case] right: &str,
    #[case] same: bool,
) {
    assert_eq!(same_written_text(left, right), same, "{right}");
}
