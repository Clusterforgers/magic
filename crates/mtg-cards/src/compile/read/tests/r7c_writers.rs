//! Goal: round 7's read-back wordings of on-trust conditional, optional and paid steps accept exactly their rule: a search for a card by name, a search of library and graveyard with its shuffle, a condition held as a number, the mana spent on a spell, the gift, and "unless"; each wrong rule (other name, zone, number, color, negation, missing "may") reads `Differs`.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const AVARAX: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Search":{"as":"found","filter":[{"Named":"Avarax"}],"reveal":true}},{"Move":{"to":"Hand","what":{"Bound":"found"}}},{"Shuffle":{}}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;
const AVARAX_TEXT: &str = "When ~ enters, you may search your library for a card named Avarax, reveal it, put it into your hand, then shuffle.";
const WOLF: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Search":{"as":"found","count":{"max":3,"min":0},"filter":[{"Named":"Howling Wolf"}],"reveal":true}},{"Move":{"to":"Hand","what":{"Bound":"found"}}},{"Shuffle":{}}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;
const WOLF_TEXT: &str = "When ~ enters, you may search your library for up to three cards named Howling Wolf, reveal them, put them into your hand, then shuffle.";

#[rstest]
#[case::named(AVARAX, AVARAX_TEXT, true)]
#[case::other_name(&AVARAX.replace("\"Avarax\"", "\"Avalanche\""), AVARAX_TEXT, false)]
#[case::other_place(&AVARAX.replace("\"to\":\"Hand\"", "\"to\":\"Graveyard\""), AVARAX_TEXT, false)]
#[case::up_to_three(WOLF, WOLF_TEXT, true)]
#[case::up_to_two(&WOLF.replace("\"max\":3", "\"max\":2"), WOLF_TEXT, false)]
fn r7c_a_search_names_the_card(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(
        written(json, clause),
        reads,
        "{:?}",
        written_variants(&ability(json))
    );
}

const DEVOTEE: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Search":{"also":["Graveyard"],"as":"found","filter":[{"OwnedBy":"You"},{"Named":"Elspeth, Undaunted Hero"}],"reveal":true}},{"Move":{"to":"Hand","what":{"Bound":"found"}}},{"If":{"condition":{"Searched":{"zone":"Library"}},"then":[{"Shuffle":{}}]}}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;
const DEVOTEE_TEXT: &str = "When ~ enters, you may search your library and/or graveyard for a card named Elspeth, Undaunted Hero, reveal it, and put it into your hand. If you search your library this way, shuffle.";

#[rstest]
#[case::and_put(DEVOTEE, DEVOTEE_TEXT, true)]
#[case::then_put(DEVOTEE, &DEVOTEE_TEXT.replace("reveal it, and put", "reveal it, then put"), true)]
// The library alone is no search of the graveyard.
#[case::library_only(&DEVOTEE.replace(r#""also":["Graveyard"],"#, ""), DEVOTEE_TEXT, false)]
#[case::exile_too(&DEVOTEE.replace(r#"["Graveyard"]"#, r#"["Graveyard","Exile"]"#), DEVOTEE_TEXT, false)]
// A shuffle after any search is not the shuffle after a search of the library.
#[case::always_shuffles(&DEVOTEE.replace(r#"{"If":{"condition":{"Searched":{"zone":"Library"}},"then":[{"Shuffle":{}}]}}"#, r#"{"Shuffle":{}}"#), DEVOTEE_TEXT, false)]
#[case::no_reveal(&DEVOTEE.replace(r#","reveal":true"#, ""), DEVOTEE_TEXT, false)]
#[case::not_optional(&DEVOTEE.replace(r#"{"May":{"effects":"#, r#"{"Together":{"effects":"#), DEVOTEE_TEXT, false)]
fn r7c_a_search_of_library_and_graveyard_shuffles_after_the_library(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        written(json, clause),
        reads,
        "{:?}",
        written_variants(&ability(json))
    );
}

const VISION: &str = r#"{"Spell":{"effects":[{"Draw":{"count":3}},{"If":{"condition":{"Compare":{"left":{"When":{"condition":{"Compare":{"left":{"ManaSpent":{"of":"Blue"}},"op":"Ge","right":3}},"otherwise":0,"then":1}},"op":"Ge","right":1}},"then":[{"Scry":{"count":3}}]}}]}}"#;
const VISION_TEXT: &str =
    "Draw three cards. If at least three blue mana was spent to cast this spell, scry 3.";

#[rstest]
#[case::held_as_a_number(VISION, true)]
#[case::plain(r#"{"Spell":{"effects":[{"Draw":{"count":3}},{"If":{"condition":{"Compare":{"left":{"ManaSpent":{"of":"Blue"}},"op":"Ge","right":3}},"then":[{"Scry":{"count":3}}]}}]}}"#, true)]
// The number 0 is the condition that does not hold.
#[case::negated(&VISION.replace(r#""op":"Ge","right":1"#, r#""op":"Eq","right":0"#), false)]
#[case::other_numbers(&VISION.replace(r#""otherwise":0,"then":1"#, r#""otherwise":0,"then":2"#), false)]
#[case::other_color(&VISION.replace("Blue", "Red"), false)]
#[case::other_amount(&VISION.replace(r#""op":"Ge","right":3"#, r#""op":"Ge","right":2"#), false)]
fn r7c_a_condition_held_as_a_number_is_the_condition(#[case] json: &str, #[case] reads: bool) {
    assert_eq!(
        written(json, VISION_TEXT),
        reads,
        "{:?}",
        written_variants(&ability(json))
    );
}

const ALTAR: &str = r#"{"Triggered":{"effects":[{"Sacrifice":{"as":"sacrificed","filter":[{"Type":"Creature"}]}},{"If":{"condition":{"CountOf":{"op":"Ge","right":1,"selector":[],"subject":{"Bound":"sacrificed"}}},"then":[{"Draw":{}}]}}],"when":{"StepBegins":{"step":"Upkeep","whose":"You"}}}}"#;
const ALTAR_TEXT: &str =
    "At the beginning of your upkeep, sacrifice a creature. If you do, draw a card.";

#[rstest]
#[case::you_do(ALTAR, true)]
// A count of what another step moved, or of a kind of it, is no "you do".
#[case::other_bind(&ALTAR.replace(r#"{"Bound":"sacrificed"}"#, r#"{"Bound":"other"}"#), false)]
#[case::of_a_kind(&ALTAR.replace(r#""selector":[]"#, r#""selector":[{"Subtype":"Human"}]"#), false)]
#[case::two_or_more(&ALTAR.replace(r#""right":1"#, r#""right":2"#), false)]
fn r7c_one_or_more_moved_by_the_step_before_is_you_do(#[case] json: &str, #[case] reads: bool) {
    assert_eq!(
        written(json, ALTAR_TEXT),
        reads,
        "{:?}",
        written_variants(&ability(json))
    );
}

const INSTEAD: &str = r#"{"Spell":{"effects":[{"If":{"condition":{"Not":{"Monarch":"You"}},"then":[{"Draw":{}}]}},{"If":{"condition":{"Monarch":"You"},"then":[{"Draw":{"count":2}}]}}]}}"#;
const INSTEAD_TEXT: &str = "Draw a card. If you're the monarch, instead draw two cards.";
const DELIRIUM: &str = r#"{"Spell":{"effects":[{"If":{"condition":{"Not":{"Compare":{"left":{"CardTypes":[{"In":"Graveyard"},{"OwnedBy":"You"}]},"op":"Ge","right":4}}},"then":[{"Draw":{}}]}},{"If":{"condition":{"Compare":{"left":{"CardTypes":[{"In":"Graveyard"},{"OwnedBy":"You"}]},"op":"Ge","right":4}},"then":[{"Draw":{"count":2}}]}}]}}"#;
const DELIRIUM_TEXT: &str = "Draw a card. If there are four or more card types among cards in your graveyard, instead draw two cards.";

#[rstest]
#[case::monarch(INSTEAD, INSTEAD_TEXT, true)]
// A first step that can change the condition is not the other branch of one test.
#[case::becomes_monarch(&INSTEAD.replacen(r#"[{"Draw":{}}]"#, r#"[{"Draw":{}},{"BecomeMonarch":{}}]"#, 1), &INSTEAD_TEXT.replace("Draw a card.", "Draw a card and become the monarch."), false)]
#[case::same_condition(&INSTEAD.replacen(r#"{"Not":{"Monarch":"You"}}"#, r#"{"Monarch":"You"}"#, 1), INSTEAD_TEXT, false)]
#[case::delirium(DELIRIUM, DELIRIUM_TEXT, true)]
#[case::delirium_mill(&DELIRIUM.replacen(r#"[{"Draw":{}}]"#, r#"[{"Mill":{}}]"#, 1), &DELIRIUM_TEXT.replace("Draw a card.", "Mill a card."), false)]
fn r7c_a_step_and_its_replacement_are_one_test(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        written(json, clause),
        reads,
        "{:?}",
        written_variants(&ability(json))
    );
}

const STALLION: &str = r#"{"Triggered":{"effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":1,"toughness":1}}]}},{"If":{"condition":{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"ManaSpent"}},"op":"Ge","right":5}},"then":[{"Draw":{}}]}}],"when":{"SpellCast":{"by":"You","what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}}}"#;
const STALLION_TEXT: &str = "Whenever you cast an instant or sorcery spell, ~ gets +1/+1 until end of turn. If five or more mana was spent to cast that spell, draw a card.";
const BLAST: &str = r#"{"Spell":{"effects":[{"DealDamage":{"amount":2,"to":{"Target":0}}},{"If":{"condition":{"Compare":{"left":"Promised","op":"Eq","right":1}},"then":[{"Draw":{}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}]}}"#;
const BLAST_TEXT: &str =
    "~ deals 2 damage to target creature. If the gift was promised, draw a card.";
const CHART: &str = r#"{"Spell":{"effects":[{"Draw":{"count":2}},{"If":{"condition":{"Compare":{"left":{"Tallied":{"what":"Attackers","who":"You"}},"op":"Eq","right":0}},"then":[{"Discard":{}}]}}]}}"#;
const CHART_TEXT: &str = "Draw two cards. Then discard a card unless you attacked this turn.";

#[rstest]
#[case::opus(STALLION, STALLION_TEXT, true)]
#[case::opus_four(&STALLION.replace(r#""right":5"#, r#""right":4"#), STALLION_TEXT, false)]
#[case::opus_own_spell(&STALLION.replace(r#""of":"Triggering""#, r#""of":"This""#), STALLION_TEXT, false)]
#[case::gift(BLAST, BLAST_TEXT, true)]
#[case::gift_not(&BLAST.replace(r#""right":1"#, r#""right":0"#), BLAST_TEXT, false)]
#[case::gift_not_text(&BLAST.replace(r#""right":1"#, r#""right":0"#), &BLAST_TEXT.replace("was", "wasn't"), true)]
#[case::unless(CHART, CHART_TEXT, true)]
#[case::unless_opposite(&CHART.replace(r#""op":"Eq","right":0"#, r#""op":"Ge","right":1"#), CHART_TEXT, false)]
fn r7c_the_conditions_of_a_step_read_as_the_oracle_words_them(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(
        written(json, clause),
        reads,
        "{:?}",
        written_variants(&ability(json))
    );
}
