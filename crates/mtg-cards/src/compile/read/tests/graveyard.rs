//! Goal: the abilities a card has in its graveyard read into the DSL and write back as the card's own text (a return of the card to the battlefield, tapped or not, or to the hand, and an exile of the card from the graveyard as a cost), as do the triggers on the card being put into a graveyard (from anywhere, or from the battlefield as a dying Aura), and the wordings that need a rule the table lacks stay with the table.
use super::*;

#[rstest]
#[case::tapped(
    "{1}{B}: Return ~ from your graveyard to the battlefield tapped.",
    r#"{"Activated":{"cost":"{1}{B}","from":"Graveyard","effects":[{"Move":{"what":"This","to":{"Battlefield":{"tapped":true}}}}]}}"#
)]
#[case::untapped(
    "{3}{B}: Return ~ from your graveyard to the battlefield.",
    r#"{"Activated":{"cost":"{3}{B}","from":"Graveyard","effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}"#
)]
#[case::to_the_hand(
    "{2}{B}: Return ~ from your graveyard to your hand.",
    r#"{"Activated":{"cost":"{2}{B}","from":"Graveyard","effects":[{"Move":{"what":"This","to":"Hand"}}]}}"#
)]
#[case::a_sacrifice_cost(
    "Sacrifice two artifacts: Return ~ from your graveyard to your hand.",
    r#"{"Activated":{"cost":[{"Sacrifice":{"filter":[{"Type":"Artifact"}],"count":2}}],"from":"Graveyard","effects":[{"Move":{"what":"This","to":"Hand"}}]}}"#
)]
#[case::a_tap_cost(
    "{2}{U}{R}, Tap an untapped Rebel you control: Return ~ from your graveyard to the battlefield tapped.",
    r#"{"Activated":{"cost":[{"Mana":"{2}{U}{R}"},{"TapPermanents":{"filter":[{"Subtype":"Rebel"}]}}],"from":"Graveyard","effects":[{"Move":{"what":"This","to":{"Battlefield":{"tapped":true}}}}]}}"#
)]
#[case::an_exile_cost(
    "{3}{G}{G}, Exile ~ from your graveyard: Create a 3/3 green Beast creature token.",
    r#"{"Activated":{"cost":[{"Mana":"{3}{G}{G}"},"ExileSelf"],"from":"Graveyard","effects":[{"CreateToken":{"token":{"Custom":{"name":"Beast","colors":["Green"],"types":["Creature"],"subtypes":["Beast"],"power":3,"toughness":3}}}}]}}"#
)]
#[case::an_exile_cost_and_a_target(
    "{4}{W}{W}, Exile ~ from your graveyard: Return target planeswalker card from your graveyard to the battlefield.",
    r#"{"Activated":{"cost":[{"Mana":"{4}{W}{W}"},"ExileSelf"],"from":"Graveyard","targets":[{"what":{"Object":[{"Type":"Planeswalker"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}}"#
)]
#[case::an_exile_cost_for_a_group(
    "{5}, Exile ~ from your graveyard: Permanents you control gain indestructible until end of turn.",
    r#"{"Activated":{"cost":[{"Mana":"{5}"},"ExileSelf"],"from":"Graveyard","effects":[{"Continuous":{"affected":{"Each":[{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::exiled_from_the_battlefield(
    "Exile ~: Creatures you control gain indestructible until end of turn.",
    r#"{"Activated":{"cost":["ExileSelf"],"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn an_ability_of_the_graveyard_reads_into_the_dsl_and_writes_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Activated, false, json);
}

#[rstest]
#[case::shuffled_from_anywhere(
    "When ~ is put into a graveyard from anywhere, shuffle it into its owner's library.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Graveyard"}},"from":"Graveyard","effects":[{"Move":{"what":{"Each":[{"In":"Graveyard"},"This"]},"to":{"Library":{"place":"Shuffled"}}}}]}}"#
)]
#[case::an_aura_back_to_the_hand(
    "When ~ is put into a graveyard from the battlefield, return it to its owner's hand.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Move":{"what":{"Each":[{"In":"Graveyard"},"This"]},"to":"Hand"}}]}}"#
)]
#[case::a_creature_back_to_the_hand(
    "When ~ dies, return it to its owner's hand.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Move":{"what":{"Each":[{"In":"Graveyard"},"This"]},"to":"Hand"}}]}}"#
)]
#[case::tapped_from_the_graveyard(
    "Whenever a land you control enters, return ~ from your graveyard to the battlefield tapped.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}},"from":"Graveyard","effects":[{"Move":{"what":"This","to":{"Battlefield":{"tapped":true}}}}]}}"#
)]
fn a_trigger_on_the_card_in_a_graveyard_reads_into_the_dsl_and_writes_back(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::the_other_wording_of_the_aura(
    "When this Aura is put into a graveyard from the battlefield, return it to its owner's hand."
)]
#[case::the_other_wording_of_a_creature("When this creature dies, return it to its owner's hand.")]
#[case::the_name_of_the_card(
    "When ~ is put into a graveyard from anywhere, shuffle it into its owner's library."
)]
fn the_card_names_itself_in_any_of_its_words(#[case] text: &str) {
    let tokens = tokenize(text);
    let mut reader = Reader::new(&tokens, false);
    assert!(
        abilities::triggered(&mut reader).is_some() && reader.end(),
        "{text}"
    );
}

#[rstest]
#[case::until_the_upkeep(
    "{3}{W}{W}: Return ~ from your graveyard to your hand. Activate only during your upkeep.",
    ClauseKind::Activated
)]
#[case::attached_to_a_target(
    "{3}{W}: Return ~ from your graveyard to the battlefield attached to target creature. Activate only as a sorcery.",
    ClauseKind::Activated
)]
#[case::with_a_counter(
    "{4}{B}{B}: Return ~ from your graveyard to the battlefield with a +1/+1 counter on it.",
    ClauseKind::Activated
)]
#[case::from_the_battlefield_not_the_graveyard_it_needs_the_card(
    "{2}{B}: Return it from your graveyard to your hand.",
    ClauseKind::Activated
)]
#[case::an_exile_of_another_card(
    "{2}, Exile another card from your graveyard: Draw a card.",
    ClauseKind::Activated
)]
#[case::from_a_hand(
    "When ~ is put into a graveyard from your hand, shuffle it into its owner's library.",
    ClauseKind::Triggered
)]
fn a_graveyard_wording_with_more_than_the_table_models_stays_unread(
    #[case] text: &str,
    #[case] kind: ClauseKind,
) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
