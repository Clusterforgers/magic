//! Goal: the copy wordings of a spell that was cast ("copy it", "copy that spell", copy counts, plural "copies", the exceptions of a copy) and of a permanent that becomes a copy of another target creature read into `CopySpell`, `CreateToken` and `BecomeCopy`, are written back as the card's text, and every near-miss stays with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

const CAST: &str = r#""when":{"SpellCast":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"by":"You"}}"#;
const ANY_CAST: &str = r#""when":{"SpellCast":{"by":"You"}}"#;

#[rstest]
#[case::copy_it(
    "Whenever you cast an instant or sorcery spell, copy it. You may choose new targets for the copy.",
    CAST,
    r#"{"CopySpell":{"what":"Triggering","new_targets":true}}"#
)]
#[case::without_new_targets(
    "Whenever you cast an instant or sorcery spell, copy it.",
    CAST,
    r#"{"CopySpell":{"what":"Triggering"}}"#
)]
#[case::that_spell(
    "Whenever you cast an instant or sorcery spell, copy that spell. You may choose new targets for the copy.",
    CAST,
    r#"{"CopySpell":{"what":"Triggering","new_targets":true}}"#
)]
#[case::you_may(
    "Whenever you cast an instant or sorcery spell, you may copy that spell. You may choose new targets for the copy.",
    CAST,
    r#"{"May":{"effects":[{"CopySpell":{"what":"Triggering","new_targets":true}}]}}"#
)]
#[case::you_may_copy_it(
    "Whenever you cast an instant or sorcery spell, you may copy it.",
    CAST,
    r#"{"May":{"effects":[{"CopySpell":{"what":"Triggering"}}]}}"#
)]
#[case::after_a_payment(
    "Whenever you cast an instant or sorcery spell, you may pay {3}. If you do, copy that spell. You may choose new targets for the copy.",
    CAST,
    r#"{"Pay":{"cost":"{3}","paid":[{"CopySpell":{"what":"Triggering","new_targets":true}}]}}"#
)]
#[case::twice(
    "Whenever you cast a spell, copy it twice. You may choose new targets for the copies.",
    ANY_CAST,
    r#"{"CopySpell":{"what":"Triggering","count":2,"new_targets":true}}"#
)]
#[case::three_times(
    "Whenever you cast a spell, copy it three times. You may choose new targets for the copies.",
    ANY_CAST,
    r#"{"CopySpell":{"what":"Triggering","count":3,"new_targets":true}}"#
)]
#[case::twice_without_new_targets(
    "Whenever you cast a spell, copy that spell twice.",
    ANY_CAST,
    r#"{"CopySpell":{"what":"Triggering","count":2}}"#
)]
#[case::an_exception_without_that(
    "Whenever you cast a spell, you may copy it, except the copy is an artifact in addition to its other types.",
    ANY_CAST,
    r#"{"May":{"effects":[{"CopySpell":{"what":"Triggering","except":[{"AddType":"Artifact"}]}}]}}"#
)]
#[case::an_exception_with_that(
    "Whenever you cast a spell, copy it, except that the copy is red.",
    ANY_CAST,
    r#"{"CopySpell":{"what":"Triggering","except":[{"SetColors":["Red"]}]}}"#
)]
#[case::a_size_and_a_subtype(
    "Whenever you cast a spell, copy it, except the copy is a 1/1 Spirit in addition to its other types.",
    ANY_CAST,
    r#"{"CopySpell":{"what":"Triggering","except":[{"SetPT":{"power":1,"toughness":1}},{"AddSubtype":"Spirit"}]}}"#
)]
#[case::a_keyword(
    "Whenever you cast a spell, copy it, except the copy has flying.",
    ANY_CAST,
    r#"{"CopySpell":{"what":"Triggering","except":[{"AddKeyword":"Flying"}]}}"#
)]
#[case::several_exceptions(
    "Whenever you cast a spell, copy it twice, except the copy is red and the copy has haste. You may choose new targets for the copies.",
    ANY_CAST,
    r#"{"CopySpell":{"what":"Triggering","count":2,"new_targets":true,"except":[{"SetColors":["Red"]},{"AddKeyword":"Haste"}]}}"#
)]
fn the_spell_that_was_cast_is_copied(#[case] text: &str, #[case] when: &str, #[case] effect: &str) {
    let json = format!(r#"{{"Triggered":{{{when},"effects":[{effect}]}}}}"#);
    reads_as(text, ClauseKind::Triggered, false, &json);
}

#[rstest]
#[case::an_opponents_spell(
    "Whenever an opponent casts a spell, copy it.",
    r#"{"Triggered":{"when":{"SpellCast":{"by":"Opponents"}},"effects":[{"CopySpell":{"what":"Triggering"}}]}}"#
)]
#[case::the_first_spell_of_the_turn(
    "Whenever you cast your first spell each turn, copy that spell. You may choose new targets for the copy.",
    r#"{"Triggered":{"when":{"Nth":{"event":{"SpellCast":{"by":"You"}},"nth":1}},"effects":[{"CopySpell":{"what":"Triggering","new_targets":true}}]}}"#
)]
fn other_casts_are_copied_the_same_way(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Triggered, false, json);
}

#[test]
fn a_count_of_x_needs_an_x_in_the_cost() {
    let text =
        "Whenever you cast a spell, copy it X times. You may choose new targets for the copies.";
    let json = r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"effects":[{"CopySpell":{"what":"Triggering","count":"X","new_targets":true}}]}}"#;
    reads_as(text, ClauseKind::Triggered, true, json);
    assert_eq!(clause(text, ClauseKind::Triggered, false), None);
}

#[rstest]
#[case::twice(
    "Copy target instant or sorcery spell twice. You may choose new targets for the copies.",
    r#"{"Spell":{TARGETS,"effects":[{"CopySpell":{"what":{"Target":0},"count":2,"new_targets":true}}]}}"#
)]
#[case::four_times(
    "Copy target instant or sorcery spell four times.",
    r#"{"Spell":{TARGETS,"effects":[{"CopySpell":{"what":{"Target":0},"count":4}}]}}"#
)]
#[case::a_keyword_exception(
    "Copy target instant or sorcery spell, except that the copy has flying.",
    r#"{"Spell":{TARGETS,"effects":[{"CopySpell":{"what":{"Target":0},"except":[{"AddKeyword":"Flying"}]}}]}}"#
)]
#[case::a_type_exception(
    "Copy target instant or sorcery spell, except that the copy is a Dragon in addition to its other types.",
    r#"{"Spell":{TARGETS,"effects":[{"CopySpell":{"what":{"Target":0},"except":[{"AddSubtype":"Dragon"}]}}]}}"#
)]
#[case::a_size_exception(
    "Copy target instant or sorcery spell, except that the copy is a 3/3 Dragon creature in addition to its other types.",
    r#"{"Spell":{TARGETS,"effects":[{"CopySpell":{"what":{"Target":0},"except":[{"SetPT":{"power":3,"toughness":3}},{"AddSubtype":"Dragon"},{"AddType":"Creature"}]}}]}}"#
)]
#[case::an_exception_without_that(
    "Copy target instant or sorcery spell, except the copy is red.",
    r#"{"Spell":{TARGETS,"effects":[{"CopySpell":{"what":{"Target":0},"except":[{"SetColors":["Red"]}]}}]}}"#
)]
fn a_targeted_spell_is_copied_more_than_once_or_with_other_exceptions(
    #[case] text: &str,
    #[case] json: &str,
) {
    let targets = r#""targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}]"#;
    reads_as(
        text,
        ClauseKind::Spell,
        false,
        &json.replace("TARGETS", targets),
    );
}

#[rstest]
#[case::not_a_cast("When ~ enters, copy it.")]
#[case::an_attack("Whenever ~ attacks, copy that spell.")]
#[case::once("Whenever you cast a spell, copy it once.")]
#[case::two_times("Whenever you cast a spell, copy it two times.")]
#[case::once_more("Whenever you cast a spell, copy it once more.")]
#[case::a_count_that_is_not_a_number("Whenever you cast a spell, copy it many times.")]
#[case::x_without_a_cost("Whenever you cast a spell, copy it X times.")]
#[case::for_each("Whenever you cast a spell, copy it for each spell cast before it this turn.")]
#[case::the_wrong_copies(
    "Whenever you cast a spell, copy it. You may choose new targets for the copies."
)]
#[case::the_wrong_copy(
    "Whenever you cast a spell, copy it twice. You may choose new targets for the copy."
)]
#[case::an_ability("Whenever you cast a spell, copy that spell or ability.")]
#[case::both("Whenever you cast a spell, copy it and that spell.")]
#[case::a_pronoun("Whenever you cast a spell, copy them.")]
#[case::this("Whenever you cast a spell, copy this.")]
#[case::another_subject("Whenever you cast a spell, copy its.")]
#[case::the_wrong_subject("Whenever you cast a spell, copy it, except it's red.")]
#[case::a_dangling_exception("Whenever you cast a spell, copy it, except the copy is")]
#[case::an_exception_with_a_dangling_and(
    "Whenever you cast a spell, copy it, except the copy is red and"
)]
#[case::a_size_without_kinds(
    "Whenever you cast a spell, copy it, except the copy is a 3/3 in addition to its other types."
)]
#[case::kinds_without_the_end(
    "Whenever you cast a spell, copy it, except the copy is a Golem artifact creature."
)]
#[case::a_color_among_kinds(
    "Whenever you cast a spell, copy it, except the copy is a black Zombie in addition to its other types."
)]
#[case::a_legend("Whenever you cast a spell, copy it, except the copy isn't legendary.")]
#[case::a_name("Whenever you cast a spell, copy it, except the copy's name is Fork.")]
#[case::once_each_turn(
    "Whenever you cast a spell, copy it. This ability triggers only once each turn."
)]
#[case::without_a_comma("Whenever you cast a spell, copy it except the copy is red.")]
#[case::after_new_targets(
    "Whenever you cast a spell, copy it. You may choose new targets for the copy, except the copy is red."
)]
fn near_misses_of_copying_the_spell_cast_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}

#[test]
fn copying_it_needs_a_spell_cast_before() {
    assert_eq!(
        clause("Copy it.", ClauseKind::Spell, false),
        None,
        "a spell has no spell cast to copy"
    );
    assert_eq!(
        clause("{T}: Copy that spell.", ClauseKind::Activated, false),
        None
    );
}

#[rstest]
#[case::an_artifact_creature_with_a_size(
    "Create a token that's a copy of target creature you control, except it's a 3/3 Golem artifact creature in addition to its other types.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"SetPT":{"power":3,"toughness":3}},{"AddSubtype":"Golem"},{"AddType":"Artifact"},{"AddType":"Creature"}]}}}}]}}"#
)]
#[case::two_types(
    "Create a token that's a copy of target creature you control, except it's an artifact creature in addition to its other types.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"AddType":"Artifact"},{"AddType":"Creature"}]}}}}]}}"#
)]
#[case::kinds_and_a_keyword(
    "Create a token that's a copy of target creature you control, except it's a Vehicle artifact in addition to its other types and it has flying.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"AddSubtype":"Vehicle"},{"AddType":"Artifact"},{"AddKeyword":"Flying"}]}}}}]}}"#
)]
fn a_token_copy_may_be_given_several_kinds_at_once(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[test]
fn a_permanent_enters_as_a_copy_with_several_kinds() {
    reads_as(
        "You may have this creature enter as a copy of any creature on the battlefield, except it's a Pirate artifact in addition to its other types and it has flying.",
        ClauseKind::Static,
        false,
        r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Creature"}],"except":[{"AddSubtype":"Pirate"},{"AddType":"Artifact"},{"AddKeyword":"Flying"}]}}}}"#,
    );
}

#[rstest]
#[case::a_color_among_kinds(
    "Create a token that's a copy of target creature you control, except it's a 4/4 black Zombie."
)]
#[case::kinds_without_the_end(
    "Create a token that's a copy of target creature you control, except it's a Golem artifact creature."
)]
#[case::a_size_alone_in_addition(
    "Create a token that's a copy of target creature you control, except it's a 3/3 in addition to its other types."
)]
#[case::several_tokens_with_kinds(
    "Create two tokens that are copies of target creature you control, except they're Golems in addition to their other types."
)]
#[case::the_token_is(
    "Create a token that's a copy of target creature you control, except the token is 1/1."
)]
#[case::not_legendary(
    "Create a token that's a copy of target creature you control, except it isn't legendary."
)]
fn near_misses_of_a_token_copy_exception_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::another_creature_you_control(
    "Create a token that's a copy of another target creature you control.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"},"Other"]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}}}}]}}"#
)]
#[case::with_an_exception(
    "Create a token that's a copy of another target creature you control, except it has haste.",
    r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"},"Other"]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"AddKeyword":"Haste"}]}}}}]}}"#
)]
fn a_token_is_a_copy_of_another_target_creature(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case::another_spell("Create a token that's a copy of another target spell.")]
#[case::another_card_in_a_graveyard(
    "Create a token that's a copy of another target creature card in your graveyard."
)]
#[case::another_without_a_target("Create a token that's a copy of another creature you control.")]
fn near_misses_of_a_token_that_copies_another_target_stay_unread(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}

#[rstest]
#[case::another_creature(
    "Whenever ~ attacks, it becomes a copy of another target creature until end of turn.",
    r#"[{"Type":"Creature"},"Other"]"#
)]
#[case::a_nonlegendary_attacker(
    "Whenever ~ attacks, it becomes a copy of another target nonlegendary attacking creature until end of turn.",
    r#"[{"Type":"Creature"},{"Not":{"Supertype":"Legendary"}},"Attacking","Other"]"#
)]
#[case::any_creature(
    "Whenever ~ attacks, it becomes a copy of target creature until end of turn.",
    r#"[{"Type":"Creature"}]"#
)]
fn an_attacker_becomes_a_copy_of_another_creature(#[case] text: &str, #[case] selector: &str) {
    let json = format!(
        r#"{{"Triggered":{{"when":{{"Attacks":{{"what":["This"]}}}},"targets":[{{"what":{{"Object":{selector}}}}}],"effects":[{{"BecomeCopy":{{"what":"This","of":{{"Target":0}},"duration":"UntilEndOfTurn"}}}}]}}}}"#
    );
    reads_as(text, ClauseKind::Triggered, false, &json);
}

#[test]
fn the_card_itself_becomes_a_copy_of_another_target_creature() {
    reads_as(
        "~ becomes a copy of another target creature until end of turn.",
        ClauseKind::Spell,
        false,
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},"Other"]}}],"effects":[{"BecomeCopy":{"what":"This","of":{"Target":0},"duration":"UntilEndOfTurn"}}]}}"#,
    );
}

#[rstest]
#[case::no_duration("Whenever ~ attacks, it becomes a copy of another target creature.")]
#[case::another_duration(
    "Whenever ~ attacks, it becomes a copy of another target creature until your next turn."
)]
#[case::not_an_attack(
    "When ~ enters, it becomes a copy of another target creature until end of turn."
)]
#[case::another_attacker(
    "Whenever a creature attacks, it becomes a copy of another target creature until end of turn."
)]
#[case::another_spell(
    "Whenever ~ attacks, it becomes a copy of another target spell until end of turn."
)]
#[case::another_card_in_a_graveyard(
    "Whenever ~ attacks, it becomes a copy of another target creature card in a graveyard until end of turn."
)]
#[case::another_without_a_target(
    "Whenever ~ attacks, it becomes a copy of another creature until end of turn."
)]
#[case::another_player(
    "Whenever ~ attacks, it becomes a copy of another target player until end of turn."
)]
#[case::an_exception(
    "Whenever ~ attacks, it becomes a copy of another target creature until end of turn, except it has flying."
)]
fn near_misses_of_becoming_a_copy_of_another_creature_stay_unread(#[case] text: &str) {
    assert_eq!(trigger(text), None, "{text}");
}
