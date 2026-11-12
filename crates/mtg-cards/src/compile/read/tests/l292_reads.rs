//! Goal: pack l29-2's wordings ("When ~ enters or the creature it haunts dies", a card you choose from a revealed hand and exile, a card revealed at random whose mana value is lost as life, each kind of counter on a target put again or removed, steps on one group named again with "them", "they" or "those", a spell copied for each kind of counter among permanents, a counter for each flip won) read into the DSL, write back as the card's own text, and leave the near-misses to the table.
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

#[test]
fn each_kind_of_counter_is_put_again_or_removed() {
    reads_as(
        "When ~ enters, for each kind of counter on target permanent, put another counter of that kind on it or remove one from it.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[]}}],"effects":[{"PutCountersOf":{"on":{"Target":0},"of":{"Target":0},"each":1,"per_kind":"PutOrRemove"}}]}}"#,
    );
}

#[rstest]
#[case::give_only(
    "When ~ enters, for each kind of counter on target permanent, put another counter of that kind on it or remove two from it."
)]
#[case::another_holder(
    "When ~ enters, for each kind of counter on target permanent, put another counter of that kind on ~ or remove one from it."
)]
fn near_misses_of_put_or_remove_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None);
}

const UNTAP_YOURS: &str =
    r#"{"Untap":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}"#;

#[rstest]
#[case::those_creatures(
    r#"{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}}"#,
    "+2: Untap all creatures you control. Those creatures get +1/+1 until end of turn.",
    true
)]
#[case::another_group_is_not_those(
    r#"{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}}"#,
    "+2: Untap all creatures you control. Those creatures get +1/+1 until end of turn.",
    false
)]
fn steps_on_one_group_name_it_again(#[case] second: &str, #[case] text: &str, #[case] reads: bool) {
    let json =
        format!(r#"{{"Loyalty":{{"cost":{{"Plus":2}},"effects":[{UNTAP_YOURS},{second}]}}}}"#);
    let found = written_variants(&ability(&json));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

#[test]
fn control_of_a_group_is_gained_and_it_is_untapped_and_hasted() {
    let all = r#"{"Each":[{"Type":"Creature"}]}"#;
    let json = format!(
        r#"{{"Loyalty":{{"cost":{{"Minus":9}},"effects":[{{"Continuous":{{"affected":{all},"mods":[{{"SetController":"You"}}],"duration":"UntilEndOfTurn"}}}},{{"Untap":{{"what":{all}}}}},{{"Continuous":{{"affected":{all},"mods":[{{"AddKeyword":"Haste"}}],"duration":"UntilEndOfTurn"}}}}]}}}}"#
    );
    let text = "−9: Gain control of all creatures until end of turn. Untap them. They gain haste until end of turn.";
    let found = written_variants(&ability(&json));
    assert!(found.iter().any(|w| same_text(w, text)), "{found:?}");
}

#[test]
fn a_plus_one_counter_or_a_counter_of_each_kind_you_have() {
    reads_as(
        "When ~ enters, for each kind of counter on permanents you control, you may put your choice of a +1/+1 counter or a counter of that kind on ~.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"PutCountersOf":{"on":"This","of":{"Each":[{"ControlledBy":"You"}]},"each":1,"per_kind":"MayPlusOneInstead"}}]}}"#,
    );
}

#[rstest]
#[case::creatures_only(
    "When ~ enters, for each kind of counter on creatures you control, you may put your choice of a +1/+1 counter or a counter of that kind on ~."
)]
#[case::a_minus_one(
    "When ~ enters, for each kind of counter on permanents you control, you may put your choice of a -1/-1 counter or a counter of that kind on ~."
)]
fn near_misses_of_the_choice_of_a_plus_one_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None);
}

#[rstest]
#[case::kinds_of_counters(
    "CounterKinds",
    "When you cast this spell, copy it for each kind of counter among permanents you control. You may choose new targets for the copies.",
    true
)]
#[case::names_are_not_kinds(
    "Name",
    "When you cast this spell, copy it for each kind of counter among permanents you control. You may choose new targets for the copies.",
    false
)]
fn a_spell_is_copied_for_each_kind_of_counter(
    #[case] stat: &str,
    #[case] text: &str,
    #[case] reads: bool,
) {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"SpellCast":{{"what":["This"],"by":"You"}}}},"from":"Stack","effects":[{{"CopySpell":{{"what":"This","count":{{"Distinct":{{"of":[{{"ControlledBy":"You"}}],"stat":"{stat}"}}}},"new_targets":true}}}}]}}}}"#
    );
    let found = written_variants(&ability(&json));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}

#[test]
fn a_counter_for_each_flip_won() {
    reads_as(
        "When ~ enters, flip a coin until you lose a flip. Put a +1/+1 counter on ~ for each flip you won.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"FlipCoin":{"until":true,"won_as":"won"}},{"PutCounters":{"on":"This","counter":"+1/+1","count":{"Bound":"won"}}}]}}"#,
    );
}

#[rstest]
#[case::each_flip(
    "When ~ enters, flip a coin until you lose a flip. Put a +1/+1 counter on ~ for each flip."
)]
#[case::lost(
    "When ~ enters, flip a coin until you lose a flip. Put a +1/+1 counter on ~ for each flip you lost."
)]
fn near_misses_of_the_flips_won_stay_with_the_table(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Triggered, false), None);
}

const HYPNOX_ENTERS: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This",{"WasCast":{"by":"You","from":"Hand"}}],"to":"Battlefield"}},"targets":[{"what":"Opponent"}],"effects":[{"Exile":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}}]}}"#;
const HYPNOX_LEAVES: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield"}},"effects":[{"Move":{"what":{"Each":[{"In":"Exile"},{"ExiledWith":"This"}]},"to":"Hand"}}]}}"#;

#[rstest]
#[case::exiles_a_hand(
    HYPNOX_ENTERS,
    "When ~ enters, if you cast it from your hand, exile all cards from target opponent's hand.",
    true
)]
#[case::returns_the_exiled(
    HYPNOX_LEAVES,
    "When ~ leaves the battlefield, return the exiled cards to their owner's hand.",
    true
)]
#[case::a_hand_and_a_graveyard_is_not_a_hand(
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Exile":{"what":{"Each":[{"OwnedBy":{"Target":0}},{"AnyOf":[{"In":"Hand"},{"In":"Graveyard"}]}]}}}]}}"#,
    "Exile all cards from target player's hand.",
    false
)]
#[case::to_a_graveyard_is_not_a_hand(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield"}},"effects":[{"Move":{"what":{"Each":[{"In":"Exile"},{"ExiledWith":"This"}]},"to":"Graveyard"}}]}}"#,
    "When ~ leaves the battlefield, return the exiled cards to their owner's hand.",
    false
)]
#[case::the_chosen_card_in_two_sentences(
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Reveal":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}},{"Choose":{"from":{"Each":[{"Not":{"Type":"Land"}},{"In":"Hand"},{"OwnedBy":{"Target":0}}]},"as":"picked"}},{"Exile":{"what":{"Bound":"picked"}}}]}}"#,
    "Target player reveals their hand. You choose a nonland card from it. Exile that card.",
    true
)]
fn whole_abilities_write_as_printed(#[case] json: &str, #[case] text: &str, #[case] reads: bool) {
    let found = written_variants(&ability(json));
    assert_eq!(found.iter().any(|w| same_text(w, text)), reads, "{found:?}");
}
