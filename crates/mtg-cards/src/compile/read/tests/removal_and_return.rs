//! Goal: the removal and return wordings read in the shapes of the hand-written files and write back as the same text: lists of card types and of negations in a phrase, "another target", a card of your graveyard returned to the battlefield, "sacrifice it unless you pay" on the card itself, and the tokens or clue the controller of a destroyed or exiled permanent gets; a qualifier on a list, other owners, other players and other effects stay with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

fn activated(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Activated, false)
}

#[rstest]
#[case::reanimate(
    "Return target creature card from your graveyard to the battlefield.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}}"#
)]
#[case::two_negations(
    "Destroy target nonartifact, nonblack creature.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"Type":"Artifact"}},{"Not":{"Color":"Black"}}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#
)]
#[case::three_types(
    "Destroy target artifact, creature, or planeswalker.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Planeswalker"}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#
)]
#[case::three_plural_types(
    "Destroy all artifacts, creatures, and enchantments.",
    r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"}]}]}}}]}}"#
)]
#[case::the_controller_gets_a_token(
    "Destroy target permanent. Its controller creates a 3/3 green Beast creature token.",
    r#"{"Spell":{"targets":[{"what":{"Object":[]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"CreateToken":{"who":{"ControllerOf":{"Target":0}},"token":{"Custom":{"name":"Beast","colors":["Green"],"types":["Creature"],"subtypes":["Beast"],"power":3,"toughness":3}}}}]}}"#
)]
#[case::the_controller_investigates(
    "Destroy target creature or planeswalker. Its controller investigates.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"CreateToken":{"who":{"ControllerOf":{"Target":0}},"token":{"Named":"clue"}}}]}}"#
)]
fn the_spells_read_in_the_shape_of_the_hand_written_files(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::unless_you_pay(
    "When ~ enters, sacrifice it unless you pay {R}.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Pay":{"cost":"{R}","unpaid":[{"Sacrifice":{"filter":["This"]}}]}}]}}"#
)]
#[case::another_target_in_a_graveyard(
    "When ~ dies, return another target artifact card from your graveyard to your hand.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"targets":[{"what":{"Object":[{"Type":"Artifact"},"Other",{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#
)]
#[case::another_target_permanent(
    "When ~ enters, destroy another target nonland permanent.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}},"Other"]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#
)]
fn the_triggers_read_in_the_shape_of_the_hand_written_files(
    #[case] text: &str,
    #[case] json: &str,
) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case("Return target creature card from your graveyard to the battlefield.")]
#[case("You may return target creature card from your graveyard to the battlefield.")]
#[case("Return target artifact card from your graveyard to the battlefield.")]
#[case("Destroy target nonartifact, nonblack creature.")]
#[case("Destroy target nonland, nonartifact permanent.")]
#[case("Destroy target artifact, creature, or planeswalker.")]
#[case("Destroy target artifact, enchantment, or planeswalker.")]
#[case("Exile target artifact, creature, or enchantment.")]
#[case("Destroy all artifacts, creatures, and enchantments.")]
#[case("Destroy target permanent. Its controller creates a 3/3 green Elephant creature token.")]
#[case(
    "Destroy target nonland permanent. Its controller creates a 1/1 white Human creature token."
)]
#[case(
    "Destroy target artifact or creature. Its controller creates a 0/1 white Ox creature token."
)]
#[case(
    "Destroy target creature. It can't be regenerated. Its controller creates a 3/3 green Ape creature token."
)]
#[case(
    "Destroy target creature. It can't be regenerated. That creature's controller creates a 3/3 green Frog Lizard creature token."
)]
#[case(
    "Exile target creature or planeswalker. Its controller creates a 4/4 white Angel creature token with flying."
)]
#[case(
    "Exile target creature. Its controller creates a 1/1 colorless Shapeshifter creature token with changeling."
)]
#[case(
    "Exile target nonland permanent. Its controller creates a 1/1 white Soldier creature token."
)]
#[case("Destroy target creature. Its controller creates a Treasure token.")]
#[case("Destroy target creature. Its controller creates two 1/1 white Spirit creature tokens.")]
fn more_spell_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("When ~ enters, sacrifice it unless you pay {R}.")]
#[case("When ~ enters, sacrifice ~ unless you pay {R}.")]
#[case("When ~ enters, sacrifice it unless you pay {1}{U}.")]
#[case("Whenever ~ attacks, sacrifice it unless you pay {R}.")]
#[case("When ~ enters, destroy target nonartifact, nonblack creature.")]
#[case("When ~ enters, destroy target artifact, enchantment, or land.")]
#[case("When ~ enters, destroy another target creature.")]
#[case("When ~ dies, return another target creature card from your graveyard to the battlefield.")]
#[case("When ~ dies, return another target artifact card from your graveyard to your hand.")]
fn more_trigger_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case("{2}{B}, {T}: Destroy all artifacts, creatures, and enchantments.")]
#[case(
    "{B}, {T}, Sacrifice ~: Return target creature card from your graveyard to the battlefield."
)]
#[case(
    "{3}{B}, Sacrifice ~: Return target creature card from your graveyard to the battlefield. Activate only as a sorcery."
)]
fn more_activated_wordings_read_and_write_back(#[case] text: &str) {
    golden(text, ClauseKind::Activated, false);
}

#[rstest]
#[case::tapped("Return target creature card from your graveyard to the battlefield tapped.")]
#[case::under_your_control(
    "Return target creature card from your graveyard to the battlefield under your control."
)]
#[case::an_opponents_graveyard(
    "Return target creature card from an opponent's graveyard to the battlefield."
)]
#[case::any_graveyard("Return target creature card from a graveyard to the battlefield.")]
#[case::a_qualifier_on_the_last_type(
    "Destroy target artifact, enchantment, or creature with flying."
)]
#[case::a_stat_on_the_last_type(
    "Destroy target artifact, enchantment, or creature with power 4 or greater."
)]
#[case::two_types_without_a_connector("Destroy target artifact, creature.")]
#[case::a_comma_between_other_adjectives("Destroy target tapped, nonblack creature.")]
#[case::that_player_creates(
    "Destroy target creature. That player creates a 1/1 white Spirit creature token."
)]
#[case::its_owner_creates(
    "Destroy target creature. Its owner creates a 1/1 white Spirit creature token."
)]
#[case::the_controller_of_nothing(
    "Draw a card. Its controller creates a 1/1 white Spirit creature token."
)]
#[case::the_controllers_of_all(
    "Destroy all creatures. Its controller creates a 1/1 white Spirit creature token."
)]
#[case::its_controller_investigates_twice(
    "Destroy target creature. Its controller investigates twice."
)]
#[case::another_without_a_target("Destroy another creature.")]
#[case::another_player("Destroy another target player.")]
fn near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::another_permanent("Whenever a creature enters, sacrifice it unless you pay {R}.")]
#[case::after_may("When ~ enters, you may sacrifice it unless you pay {R}.")]
#[case::a_second_target(
    "When ~ dies, return another target artifact card from your graveyard to your hand. Destroy another target creature."
)]
#[case::the_controller_of_a_trigger_subject(
    "Whenever a creature dies, its controller creates a 1/1 white Spirit creature token."
)]
fn trigger_near_misses_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[test]
fn an_activation_of_another_graveyard_stays_unread() {
    assert_eq!(
        activated(
            "{B}, {T}: Return target creature card from an opponent's graveyard to the battlefield."
        ),
        None
    );
}

/// "a", "a or b", "a, b, or c": the card types of a phrase as a card lists them.
fn type_list(words: &[String], connector: &str) -> String {
    match words {
        [rest @ .., last] if !rest.is_empty() => {
            let comma = if rest.len() > 1 { "," } else { "" };
            format!("{}{comma} {connector} {last}", rest.join(", "))
        }
        _ => words.join(""),
    }
}

proptest::proptest! {
    #[test]
    fn any_list_of_card_types_reads_and_writes_back(
        picked in proptest::sample::subsequence(
            vec!["artifact", "creature", "enchantment", "land", "planeswalker"],
            1..=5,
        )
    ) {
        let singular: Vec<String> = picked.iter().map(|word| (*word).to_owned()).collect();
        let plural: Vec<String> = picked.iter().map(|word| format!("{word}s")).collect();
        golden(&format!("Destroy target {}.", type_list(&singular, "or")), ClauseKind::Spell, false);
        golden(&format!("Destroy all {}.", type_list(&plural, "and")), ClauseKind::Spell, false);
    }
}
