//! Goal: the copy wordings ("create a token that's a copy of ...", with and without "except it ...", the count forms, the copy that is exiled at the end step, "populate" and "enter as a copy of ...") read into `CopyOf`, `Choose` and `AsCopy` and write back as the card's text; every near-miss stays with the table.
use super::*;
use crate::effect::Effect;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

fn statics(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Static, false)
}

const OF_TARGET: &str =
    r#""targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}]"#;

#[rstest]
#[case::cackling_counterpart(
    "Create a token that's a copy of target creature you control.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}}}}]}}"#
)]
#[case::any_creature(
    "Create a token that's a copy of target creature.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}}}}]}}"#
)]
#[case::with_haste(
    "Create a token that's a copy of target creature you control, except it has haste.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"AddKeyword":"Haste"}]}}}}]}}"#
)]
#[case::with_two_keywords(
    "Create a token that's a copy of target creature you control, except it has flying and haste.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"AddKeyword":"Flying"},{"AddKeyword":"Haste"}]}}}}]}}"#
)]
#[case::an_extra_subtype(
    "Create a token that's a copy of target creature you control, except it's a Zombie in addition to its other types.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"AddSubtype":"Zombie"}]}}}}]}}"#
)]
#[case::an_extra_type(
    "Create a token that's a copy of target creature you control, except it's an artifact in addition to its other types.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"AddType":"Artifact"}]}}}}]}}"#
)]
#[case::a_color(
    "Create a token that's a copy of target creature you control, except it's blue.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"SetColors":["Blue"]}]}}}}]}}"#
)]
#[case::two_colors(
    "Create a token that's a copy of target creature you control, except it's blue and black.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"SetColors":["Blue","Black"]}]}}}}]}}"#
)]
#[case::colorless(
    "Create a token that's a copy of target creature you control, except it's colorless.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"SetColors":[]}]}}}}]}}"#
)]
#[case::power_and_toughness(
    "Create a token that's a copy of target creature you control, except it's 1/1.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"SetPT":{"power":1,"toughness":1}}]}}}}]}}"#
)]
#[case::an_ability(
    "Create a token that's a copy of target creature you control, except it has \"When this creature becomes the target of a spell or ability, sacrifice it.\"",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"GrantAbility":{"Triggered":{"when":{"BecomesTarget":{"what":["This"]}},"effects":[{"Sacrifice":{"filter":["This"]}}]}}}]}}}}]}}"#
)]
#[case::several_exceptions(
    "Create a token that's a copy of target creature you control, except it's a Zombie in addition to its other types and it has haste.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"AddSubtype":"Zombie"},{"AddKeyword":"Haste"}]}}}}]}}"#
)]
#[case::two_copies(
    "Create two tokens that are copies of target creature you control.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}},"count":2}}]}}"#
)]
#[case::two_copies_with_haste(
    "Create three tokens that are copies of target creature you control, except they have haste.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"AddKeyword":"Haste"}]}},"count":3}}]}}"#
)]
#[case::exiled_at_the_end_step(
    "Create a token that's a copy of target creature you control, except it has haste. Exile it at the beginning of the next end step.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"AddKeyword":"Haste"}]}},"as":"copy"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Exile":{"what":{"Bound":"copy"}}}]}}]}}"#
)]
#[case::sacrificed_at_the_end_step(
    "Create a token that's a copy of target creature you control, except it has haste. Sacrifice it at the beginning of the next end step.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"AddKeyword":"Haste"}]}},"as":"copy"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Sacrifice":{"filter":[{"Is":{"Bound":"copy"}}]}}]}}]}}"#
)]
#[case::the_copies_are_exiled_too(
    "Create two tokens that are copies of target creature you control, except they have haste. Exile them at the beginning of the next end step.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"AddKeyword":"Haste"}]}},"count":2,"as":"copy"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Exile":{"what":{"Bound":"copy"}}}]}}]}}"#
)]
fn a_spell_that_copies_a_permanent_reads_as_a_token_copy(#[case] text: &str, #[case] json: &str) {
    reads_as(
        text,
        ClauseKind::Spell,
        false,
        &json.replace("TARGETS", OF_TARGET),
    );
}

#[rstest]
#[case::any_number(
    "Create X tokens that are copies of target creature you control.",
    r#"{"Spell":{TARGETS,"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}},"count":"X"}}]}}"#
)]
fn a_copy_may_be_made_x_times_when_the_cost_has_an_x(#[case] text: &str, #[case] json: &str) {
    reads_as(
        text,
        ClauseKind::Spell,
        true,
        &json.replace("TARGETS", OF_TARGET),
    );
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::followed_footsteps(
    "At the beginning of your upkeep, create a token that's a copy of enchanted creature.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"CreateToken":{"token":{"CopyOf":{"of":"Attached"}}}}]}}"#
)]
#[case::equipped(
    "At the beginning of your upkeep, create a token that's a copy of equipped creature.",
    r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"CreateToken":{"token":{"CopyOf":{"of":"Attached"}}}}]}}"#
)]
#[case::itself(
    "Whenever ~ attacks, create a token that's a copy of ~, except it has haste.",
    r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"CreateToken":{"token":{"CopyOf":{"of":"This","except":[{"AddKeyword":"Haste"}]}}}}]}}"#
)]
#[case::may(
    "When ~ enters, you may create a token that's a copy of target creature you control.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"May":{"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}}}}]}}]}}"#
)]
fn a_trigger_that_copies_reads_the_creature_it_names(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::it("Tap target creature you control. Create a token that's a copy of it.")]
#[case::that_creature(
    "Tap target creature you control. Create a token that's a copy of that creature."
)]
fn a_copy_of_what_the_sentence_before_named_is_a_copy_of_it(#[case] text: &str) {
    let read = golden(text, ClauseKind::Spell, false);
    let Ability::Spell { targets, effects } = &read else {
        panic!("not a spell: {read:?}");
    };
    assert_eq!(targets.len(), 1);
    assert!(matches!(
        effects.as_slice(),
        [Effect::Tap { .. }, Effect::CreateToken { .. }]
    ));
    assert_eq!(
        render_ability(&read).as_deref(),
        Some("Tap target creature you control. Create a token that's a copy of it.")
    );
}

#[rstest]
#[case::rootborn_defenses(
    "Populate. Creatures you control gain indestructible until end of turn.",
    r#"{"Spell":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]},"as":"token"}},{"CreateToken":{"token":{"CopyOf":{"of":{"Bound":"token"}}}}},{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}]}}"#
)]
#[case::alone(
    "Populate.",
    r#"{"Spell":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]},"as":"token"}},{"CreateToken":{"token":{"CopyOf":{"of":{"Bound":"token"}}}}}]}}"#
)]
#[case::after_a_comma(
    "Destroy target artifact, then populate.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"Choose":{"from":{"Each":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]},"as":"token"}},{"CreateToken":{"token":{"CopyOf":{"of":{"Bound":"token"}}}}}]}}"#
)]
fn populate_chooses_a_token_and_copies_it(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[test]
fn populate_in_a_trigger_is_growing_ranks() {
    reads_as(
        "At the beginning of your upkeep, populate.",
        ClauseKind::Triggered,
        false,
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]},"as":"token"}},{"CreateToken":{"token":{"CopyOf":{"of":{"Bound":"token"}}}}}]}}"#,
    );
}

#[rstest]
#[case::clone(
    "You may have this creature enter as a copy of any creature on the battlefield.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Creature"}]}}}}"#
)]
#[case::mirror_image(
    "You may have this creature enter as a copy of a creature you control.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}}"#
)]
#[case::phantasmal_image(
    "You may have this creature enter as a copy of any creature on the battlefield, except it's an Illusion in addition to its other types and it has \"When this creature becomes the target of a spell or ability, sacrifice it.\"",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Creature"}],"except":[{"AddSubtype":"Illusion"},{"GrantAbility":{"Triggered":{"when":{"BecomesTarget":{"what":["This"]}},"effects":[{"Sacrifice":{"filter":["This"]}}]}}}]}}}}"#
)]
#[case::an_artifact_or_a_creature(
    "You may have this creature enter as a copy of any artifact or creature on the battlefield, except it's an artifact in addition to its other types.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]}],"except":[{"AddType":"Artifact"}]}}}}"#
)]
#[case::a_fixed_size(
    "You may have this creature enter as a copy of any creature on the battlefield, except it's 7/7.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Creature"}],"except":[{"SetPT":{"power":7,"toughness":7}}]}}}}"#
)]
#[case::a_keyword_and_a_color(
    "You may have ~ enter as a copy of any creature on the battlefield, except it's blue and it has flying.",
    r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Creature"}],"except":[{"SetColors":["Blue"]},{"AddKeyword":"Flying"}]}}}}"#
)]
fn a_permanent_may_enter_as_a_copy(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::another_kind_of_ability_in_the_quotes(
    "Create a token that's a copy of target creature you control, except it has \"{T}: Add {G}.\""
)]
#[case::an_unknown_keyword(
    "Create a token that's a copy of target creature you control, except it has ward."
)]
#[case::a_type_without_in_addition(
    "Create a token that's a copy of target creature you control, except it's a Zombie."
)]
#[case::the_token(
    "Create a token that's a copy of target creature you control, except the token has haste."
)]
#[case::another_exception(
    "Create a token that's a copy of target creature you control, except its name is Bob."
)]
#[case::an_exception_without_a_comma(
    "Create a token that's a copy of target creature you control except it has haste."
)]
#[case::a_dangling_and(
    "Create a token that's a copy of target creature you control, except it has haste and."
)]
#[case::a_spell("Create a token that's a copy of target spell.")]
#[case::a_card_in_a_graveyard(
    "Create a token that's a copy of target creature card in your graveyard."
)]
#[case::a_second_target(
    "Create a token that's a copy of target creature you control. Create a token that's a copy of target land."
)]
#[case::tapped_and_attacking(
    "Create a token that's a copy of target creature you control that's tapped and attacking."
)]
#[case::the_wrong_count_word("Create two tokens that's a copy of target creature you control.")]
#[case::one_of_a_plural("Create a token that are copies of target creature you control.")]
#[case::a_plural_with_a_type(
    "Create two tokens that are copies of target creature you control, except they're artifacts in addition to their other types."
)]
#[case::an_x_without_one_in_the_cost(
    "Create X tokens that are copies of target creature you control."
)]
#[case::the_next_combat(
    "Create a token that's a copy of target creature you control, except it has haste. Exile it at the beginning of the next combat."
)]
#[case::exile_them_for_one(
    "Create a token that's a copy of target creature you control, except it has haste. Exile them at the beginning of the next end step."
)]
#[case::a_delay_with_nothing_to_delay("Exile it at the beginning of the next end step.")]
#[case::the_wording_with_is("Create a token that is a copy of target creature you control.")]
#[case::an_opponents_token(
    "Target opponent creates a token that's a copy of target creature you control."
)]
#[case::it_without_a_sentence_before("Create a token that's a copy of it.")]
#[case::populate_twice("Populate twice.")]
#[case::populate_a_token("Populate target creature.")]
#[case::populate_with_a_condition("Populate if you control a creature.")]
fn near_misses_of_a_token_copy_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::a_land(
    "You may have this land enter as a copy of any land on the battlefield, except it's a creature."
)]
#[case::target("You may have this creature enter as a copy of target creature.")]
#[case::without_may("This creature enters as a copy of any creature on the battlefield.")]
#[case::another_zone(
    "You may have this creature enter as a copy of any creature card in a graveyard."
)]
#[case::no_exception_after_the_comma(
    "You may have this creature enter as a copy of any creature on the battlefield,"
)]
#[case::controlled_and_anywhere(
    "You may have this creature enter as a copy of any creature you control on the battlefield."
)]
#[case::an_opponents_creature(
    "You may have this creature enter as a copy of a creature an opponent controls, except it's blue."
)]
#[case::until_end_of_turn(
    "You may have this creature become a copy of any creature until end of turn."
)]
#[case::a_name(
    "You may have this creature enter as a copy of any creature on the battlefield, except its name is Bob."
)]
#[case::an_activated_ability(
    "You may have this creature enter as a copy of any creature on the battlefield, except it has \"{T}: Add {G}.\""
)]
#[case::unmatched_quote(
    "You may have this creature enter as a copy of any creature on the battlefield, except it has \"When this creature dies, draw a card."
)]
#[case::an_extra_counter(
    "You may have this creature enter as a copy of any creature on the battlefield, except it enters with an additional +1/+1 counter."
)]
fn near_misses_of_entering_as_a_copy_stay_unread(#[case] text: &str) {
    assert_eq!(statics(text), None, "{text}");
}

#[rstest]
#[case::enchanted_land(
    "At the beginning of your upkeep, create a token that's a copy of enchanted land."
)]
#[case::each(
    "At the beginning of your upkeep, create a token that's a copy of each creature you control."
)]
fn near_misses_of_a_copying_trigger_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[rstest]
#[case::that_creature(
    "a token that's a copy of that creature,",
    "a token that's a copy of it,"
)]
#[case::that_permanent("copies of that permanent.", "copies of it.")]
#[case::not_after_a_copy("exile that creature.", "exile that creature.")]
#[case::another_noun("a copy of that card.", "a copy of that card.")]
fn that_creature_after_a_copy_is_it(#[case] text: &str, #[case] expected: &str) {
    assert_eq!(canonical(text), expected);
}

#[rstest]
#[case::a_quote_that_opens("it has \"When", &["it", "has", "\"", "when"])]
#[case::a_quote_that_closes("sacrifice it.\" Draw", &["sacrifice", "it", ".", "\"", "draw"])]
fn a_quote_that_opens_an_ability_starts_a_sentence(#[case] text: &str, #[case] words: &[&str]) {
    let read: Vec<String> = tokenize(text)
        .into_iter()
        .map(|token| match token {
            Token::Word(word) | Token::Name(word) => word,
            Token::Punct(mark) => mark.to_string(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(read, words);
}

#[rstest]
#[case::when_targeted(
    "When ~ becomes the target of a spell or ability, sacrifice it.",
    r#"{"Triggered":{"when":{"BecomesTarget":{"what":["This"]}},"effects":[{"Sacrifice":{"filter":["This"]}}]}}"#
)]
#[case::whenever_targeted(
    "Whenever ~ becomes the target of a spell or ability, sacrifice ~.",
    r#"{"Triggered":{"when":{"BecomesTarget":{"what":["This"]}},"effects":[{"Sacrifice":{"filter":["This"]}}]}}"#
)]
#[case::when_it_enters_as_it(
    "When ~ enters, sacrifice it.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Sacrifice":{"filter":["This"]}}]}}"#
)]
#[case::when_it_enters(
    "When ~ enters, sacrifice ~.",
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Sacrifice":{"filter":["This"]}}]}}"#
)]
fn a_card_that_sacrifices_itself_reads_it_as_the_card(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[rstest]
#[case::another_creature("Whenever a creature enters, sacrifice it.")]
#[case::each_turn(
    "When ~ becomes the target of a spell or ability for the first time each turn, draw a card."
)]
fn sacrificing_it_needs_the_card_to_be_what_it_is(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}
