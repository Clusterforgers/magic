//! Goal: an effect counted by "for each" (cards, life, mana, tokens, counters; things you hold, the cards of a zone, opponents, the counters on the card) reads into a `Count` of a selector (a `Times` of it for a number above one), writes back in the same words, and leaves every near-miss to the table.
use super::*;

const CREATURES: &str = r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#;

#[rstest]
#[case::draw(
    "Draw a card for each creature you control.",
    ClauseKind::Spell,
    &format!(r#"{{"Spell":{{"effects":[{{"Draw":{{"count":{CREATURES}}}}}]}}}}"#)
)]
#[case::other(
    "Draw a card for each other creature you control.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Draw":{"count":{"Count":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::times(
    "Draw two cards for each Elf on the battlefield.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Draw":{"count":{"Times":{"by":2,"of":{"Count":[{"Subtype":"Elf"}]}}}}}]}}"#
)]
#[case::graveyard(
    "Draw a card for each creature card in your graveyard.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Draw":{"count":{"Count":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}}]}}"#
)]
#[case::hand(
    "Draw a card for each card in your hand.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Draw":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}}]}}"#
)]
#[case::opponents(
    "Draw a card for each tapped creature your opponents control.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Draw":{"count":{"Count":[{"Type":"Creature"},"Tapped",{"ControlledBy":"Opponents"}]}}}]}}"#
)]
#[case::attacking(
    "Draw a card for each attacking creature.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Draw":{"count":{"Count":[{"Type":"Creature"},"Attacking"]}}}]}}"#
)]
#[case::power(
    "Draw a card for each creature you control with power 4 or greater.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Draw":{"count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"Compare":{"stat":"Power","op":"Ge","to":4}}]}}}]}}"#
)]
#[case::the_players(
    "Draw a card for each opponent.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"Draw":{"count":{"Players":"Opponents"}}}]}}"#
)]
#[case::a_trigger(
    "When ~ enters, draw a card for each creature you control.",
    ClauseKind::Triggered,
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Draw":{"count":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::mill(
    "Target player mills a card for each Locus you control.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Mill":{"who":{"Target":0},"count":{"Count":[{"Subtype":"Locus"},{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::life(
    "You gain 1 life for each creature you control.",
    ClauseKind::Spell,
    &format!(r#"{{"Spell":{{"effects":[{{"GainLife":{{"amount":{CREATURES}}}}}]}}}}"#)
)]
#[case::life_times(
    "You gain 2 life for each creature you control with flying.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"GainLife":{"amount":{"Times":{"by":2,"of":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"Flying"}]}}}}}]}}"#
)]
#[case::counters_on_the_card(
    "You gain 1 life for each +1/+1 counter on ~.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"GainLife":{"amount":{"Counters":{"on":"This","counter":"+1/+1"}}}}]}}"#
)]
#[case::each_opponent_loses(
    "Each opponent loses 1 life for each Spider you control.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"LoseLife":{"who":"Opponents","amount":{"Count":[{"Subtype":"Spider"},{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::mana(
    "Add {R} for each Goblin you control.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"AddMana":{"mana":{"Fixed":["Red"]},"times":{"Count":[{"Subtype":"Goblin"},{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::mana_of_a_group(
    "{T}: Add {G} for each Elf on the battlefield.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Green"]},"times":{"Count":[{"Subtype":"Elf"}]}}}]}}"#
)]
#[case::mana_for_counters(
    "{T}: Add {C} for each charge counter on this artifact.",
    ClauseKind::Activated,
    r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]},"times":{"Counters":{"on":"This","counter":"charge"}}}}]}}"#
)]
#[case::token(
    "Create a 1/1 green Elf Warrior creature token for each Elf you control.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Elf Warrior","colors":["Green"],"types":["Creature"],"subtypes":["Elf","Warrior"],"power":1,"toughness":1}},"count":{"Count":[{"Subtype":"Elf"},{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::treasure(
    "Create a Treasure token for each artifact you control.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"treasure"},"count":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::tokens_times(
    "Create two 1/1 white Soldier creature tokens for each creature you control.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}},"count":{"Times":{"by":2,"of":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}}}]}}"#
)]
#[case::counters(
    "Put a +1/+1 counter on target creature for each Elf you control.",
    ClauseKind::Spell,
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":{"Count":[{"Subtype":"Elf"},{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::counters_on_the_card_itself(
    "Put a +1/+1 counter on ~ for each creature card in your graveyard.",
    ClauseKind::Spell,
    r#"{"Spell":{"effects":[{"PutCounters":{"on":"This","counter":"+1/+1","count":{"Count":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}}]}}"#
)]
fn a_count_by_for_each_reads_as_the_count_of_a_selector(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] json: &str,
) {
    reads_as(text, kind, false, json);
}

#[rstest]
#[case::two_verbs("Target player draws a card and loses 1 life for each creature you control.")]
#[case::a_target_holds("Draw a card for each tapped creature target opponent controls.")]
#[case::another_player_zone("Target opponent loses 1 life for each card in their graveyard.")]
#[case::a_compound_type(
    "Target opponent loses 2 life for each instant and sorcery card in your graveyard."
)]
#[case::a_quantity("Draw a card for each different power among creatures you control.")]
#[case::damage_dealt("Draw a card for each creature destroyed this way.")]
#[case::no_thing("Draw a card for each.")]
#[case::x_for_each("Draw X cards for each creature you control.")]
#[case::zero_for_each("Draw zero cards for each creature you control.")]
#[case::a_counter_of_another("You gain 1 life for each +1/+1 counter on target creature.")]
#[case::any_counter("You gain 1 life for each counter on ~.")]
#[case::mana_times("Add {R}{R} for each Goblin you control and {G}.")]
#[case::for_each_first("For each creature you control, draw a card.")]
#[case::tapped_tokens(
    "Create a tapped 1/1 white Soldier creature token for each creature you control."
)]
fn a_count_the_templates_do_not_model_stays_unread(#[case] text: &str) {
    assert_eq!(clause(text, ClauseKind::Spell, false), None, "{text}");
}

#[rstest]
#[case::equipment(
    "Equipped creature gets +1/+1 for each artifact you control.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"toughness":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::power_only(
    "Equipped creature gets +1/+0 for each card in your hand.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"toughness":0}}]}}"#
)]
#[case::times(
    "Enchanted creature gets +2/+2 for each enchantment you control.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":{"Times":{"by":2,"of":{"Count":[{"Type":"Enchantment"},{"ControlledBy":"You"}]}}},"toughness":{"Times":{"by":2,"of":{"Count":[{"Type":"Enchantment"},{"ControlledBy":"You"}]}}}}}]}}"#
)]
#[case::and_a_keyword(
    "Enchanted creature gets +1/+1 for each enchantment you control and has first strike.",
    r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":{"Count":[{"Type":"Enchantment"},{"ControlledBy":"You"}]},"toughness":{"Count":[{"Type":"Enchantment"},{"ControlledBy":"You"}]}}},{"AddKeyword":"FirstStrike"}]}}"#
)]
#[case::the_card_itself(
    "~ gets +1/+1 for each other Giant you control.",
    r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":{"Count":[{"Subtype":"Giant"},"Other",{"ControlledBy":"You"}]},"toughness":{"Count":[{"Subtype":"Giant"},"Other",{"ControlledBy":"You"}]}}}]}}"#
)]
#[case::a_group(
    "Creatures you control get +1/+0 for each charge counter on ~.",
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":{"Counters":{"on":"This","counter":"charge"}},"toughness":0}}]}}"#
)]
fn a_bonus_counted_by_for_each_is_a_static_over_a_count(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::minus(
    "Target creature gets -1/-1 until end of turn for each Swamp you control.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":{"Times":{"by":-1,"of":{"Count":[{"Subtype":"Swamp"},{"ControlledBy":"You"}]}}},"toughness":{"Times":{"by":-1,"of":{"Count":[{"Subtype":"Swamp"},{"ControlledBy":"You"}]}}}}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::all(
    "All creatures get -1/-1 until end of turn for each Swamp you control.",
    r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":{"Times":{"by":-1,"of":{"Count":[{"Subtype":"Swamp"},{"ControlledBy":"You"}]}}},"toughness":{"Times":{"by":-1,"of":{"Count":[{"Subtype":"Swamp"},{"ControlledBy":"You"}]}}}}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::power(
    "Target creature gets +1/+0 until end of turn for each creature you control.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#
)]
fn a_pump_counted_by_for_each_is_an_effect_over_a_count(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::static_zero("Equipped creature gets +0/+0 for each artifact you control.", ClauseKind::Static)]
#[case::static_x("Equipped creature gets +X/+X for each artifact you control.", ClauseKind::Static)]
#[case::static_mixed("Equipped creature gets +1/+2 for each artifact you control.", ClauseKind::Static)]
#[case::static_keyword("Equipped creature has flying for each artifact you control.", ClauseKind::Static)]
#[case::pump_keyword(
    "Target creature gains flying until end of turn for each creature you control.",
    ClauseKind::Spell
)]
#[case::pump_before(
    "Target creature gets +1/+1 for each creature you control until end of turn.",
    ClauseKind::Spell
)]
fn a_bonus_the_templates_do_not_model_stays_unread(#[case] text: &str, #[case] kind: ClauseKind) {
    assert_eq!(clause(text, kind, false), None, "{text}");
}
