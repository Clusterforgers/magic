//! Goal: pack l29-2's wordings ("When ~ enters or the creature it haunts dies", a card you choose from a revealed hand and exile, a card revealed at random whose mana value is lost as life) read into the DSL, write back as the card's own text, and leave the near-misses to the table.
use super::*;

const HAUNT: &str = r#"{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"ZoneChange":{"what":[{"HauntedBy":"This"}],"from":"Battlefield","to":"Graveyard"}}]}"#;

#[rstest]
#[case::a_target(
    "When ~ enters or the creature it haunts dies, destroy target enchantment.",
    r#"[{"what":{"Object":[{"Type":"Enchantment"}]}}]"#,
    r#"[{"Destroy":{"what":{"Target":0}}}]"#
)]
#[case::tokens(
    "When ~ enters or the creature it haunts dies, create two 1/1 black Bat creature tokens with flying.",
    "[]",
    r#"[{"CreateToken":{"token":{"Custom":{"name":"Bat","colors":["Black"],"types":["Creature"],"subtypes":["Bat"],"power":1,"toughness":1,"abilities":[{"Keyword":"Flying"}]}},"count":2}}]"#
)]
fn a_haunt_creature_triggers_on_entering_and_on_its_haunted_creature_dying(
    #[case] text: &str,
    #[case] targets: &str,
    #[case] effects: &str,
) {
    let targets = if targets == "[]" {
        String::new()
    } else {
        format!(r#","targets":{targets}"#)
    };
    let json = format!(r#"{{"Triggered":{{"when":{HAUNT}{targets},"effects":{effects}}}}}"#);
    reads_as(text, ClauseKind::Triggered, false, &json);
}

#[rstest]
#[case::the_creature_it_enchants("When ~ enters or the creature it enchants dies, draw a card.")]
#[case::leaves("When ~ enters or the creature it haunts leaves the battlefield, draw a card.")]
#[case::attacks("When ~ attacks or the creature it haunts dies, draw a card.")]
fn near_misses_of_the_haunt_head_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None);
}

#[rstest]
#[case::nonland(
    "When ~ enters, target opponent reveals their hand. You choose a nonland card from it and exile that card.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Opponent"}],"effects":[{"Reveal":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}},{"Choose":{"from":{"Each":[{"Not":{"Type":"Land"}},{"In":"Hand"},{"OwnedBy":{"Target":0}}]},"as":"picked"}},{"Exile":{"what":{"Bound":"picked"}}}]}}"#
)]
#[case::a_creature_card(
    "When ~ enters, target player reveals their hand. You choose a creature card from it and exile that card.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player"}],"effects":[{"Reveal":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}},{"Choose":{"from":{"Each":[{"Type":"Creature"},{"In":"Hand"},{"OwnedBy":{"Target":0}}]},"as":"picked"}},{"Exile":{"what":{"Bound":"picked"}}}]}}"#
)]
#[case::at_random(
    "When ~ enters, target player reveals a card at random from their hand, then loses life equal to that card's mana value.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player"}],"effects":[{"ChooseAtRandom":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]},"as":"picked"}},{"Reveal":{"what":{"Bound":"picked"}}},{"LoseLife":{"who":{"Target":0},"amount":{"Stat":{"of":{"Bound":"picked"},"stat":"ManaValue"}}}}]}}"#
)]
fn a_card_of_a_revealed_hand_is_exiled_or_costs_life(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::face_down(
    "When ~ enters, target opponent reveals their hand. You choose a nonland card from it and exile that card face down."
)]
#[case::they_choose(
    "When ~ enters, target opponent reveals their hand. They choose a nonland card from it and exile that card."
)]
#[case::power(
    "When ~ enters, target player reveals a card at random from their hand, then loses life equal to that card's power."
)]
#[case::gains(
    "When ~ enters, target player reveals a card at random from their hand, then gains life equal to that card's mana value."
)]
fn near_misses_of_the_hand_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None);
}

#[rstest]
#[case::exile_a_hand(
    "When ~ enters, exile all cards from target opponent's hand.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Opponent"}],"effects":[{"Exile":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}}]}}"#
)]
#[case::exile_a_players_hand(
    "Exile all cards from target player's hand.",
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Exile":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}}]}}"#
)]
#[case::return_the_exiled(
    "When ~ leaves the battlefield, return the exiled cards to their owner's hand.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield"}},"effects":[{"Move":{"what":{"Each":[{"In":"Exile"},{"ExiledWith":"This"}]},"to":"Hand"}}]}}"#
)]
fn a_whole_hand_is_exiled_and_returned(#[case] text: &str, #[case] json: &str) {
    let kind = if text.starts_with("When") {
        ClauseKind::Triggered
    } else {
        ClauseKind::Spell
    };
    reads_as(text, kind, false, json);
}

#[test]
fn a_hand_exiled_only_for_a_card_cast_from_a_hand_writes_back() {
    let json = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This",{"WasCast":{"by":"You","from":"Hand"}}],"to":"Battlefield"}},"targets":[{"what":"Opponent"}],"effects":[{"Exile":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}}]}}"#;
    let text = "When ~ enters, if you cast it from your hand, exile all cards from target opponent's hand.";
    let found = written_variants(&ability(json));
    assert!(found.iter().any(|w| same_text(w, text)), "{found:?}");
}

#[rstest]
#[case::their_graveyard("Exile all cards from target opponent's graveyard.")]
#[case::some_cards("Exile two cards from target opponent's hand.")]
#[case::the_owners("When ~ leaves the battlefield, return the exiled cards to the battlefield.")]
fn near_misses_of_the_whole_hand_stay_with_the_table(#[case] text: &str) {
    let kind = if text.starts_with("When") {
        ClauseKind::Triggered
    } else {
        ClauseKind::Spell
    };
    let read = clause(text, kind, false);
    let json = r#"{"Exile":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}}"#;
    assert!(
        read.as_ref()
            .is_none_or(|read| !serde_json::to_string(read).unwrap().contains(json)),
        "{read:?}"
    );
}

#[rstest]
#[case::with_a_morph_ability(
    r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"AnyOf":[{"HasKeywordNamed":"Morph"},{"HasKeywordNamed":"Megamorph"}]}]}}],"effects":[{"May":{"effects":[{"TurnFaceDown":{"what":{"Target":0}}}]}}]}}"#,
    "When ~ is turned face up, you may turn target creature with a morph ability face down.",
    true
)]
#[case::any_creature_is_not_a_morph(
    r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"May":{"effects":[{"TurnFaceDown":{"what":{"Target":0}}}]}}]}}"#,
    "When ~ is turned face up, you may turn target creature with a morph ability face down.",
    false
)]
#[case::each_opponent_skips(
    r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["This"]}},"effects":[{"RepeatEach":{"over":"Opponents","as":"foe","effects":[{"Rule":{"rule":{"Cant":{"who":{"Bound":"foe"},"action":{"SkipStep":"Untap"}}},"duration":"UntilSkipped"}}]}}]}}"#,
    "When ~ is turned face up, each opponent skips their next untap step.",
    true
)]
#[case::another_player_is_not_each_opponent(
    r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["This"]}},"effects":[{"RepeatEach":{"over":"Opponents","as":"foe","effects":[{"Rule":{"rule":{"Cant":{"who":"You","action":{"SkipStep":"Untap"}}},"duration":"UntilSkipped"}}]}}]}}"#,
    "When ~ is turned face up, each opponent skips their next untap step.",
    false
)]
fn twins_word_a_morph_target_and_each_opponents_skip(
    #[case] json: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let found = written_variants(&ability(json));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}
