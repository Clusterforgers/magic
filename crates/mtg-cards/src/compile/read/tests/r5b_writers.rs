//! Goal: the writers of round 5 pack B read back the control flow of a step offered on trust ("you may draw a card. If you do", "unless its controller pays {X}", "If you do"/"If you don't" over what an optional step bound, "this is the second time this ability has resolved this turn", "choose one or more", "you may choose both instead", "If you search your library this way"), and a rule that drops the "may", ties "if you do" to another step, changes the payer, the count or the number of modes reads Differs.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const DIES: &str =
    r#""when":{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":["This"]}}"#;
const UPKEEP: &str = r#""when":{"StepBegins":{"step":"Upkeep","whose":"You"}}"#;
const ATTACKS: &str = r#""when":{"Attacks":{"what":["This"]}}"#;

fn triggered(when: &str, effects: &str) -> String {
    format!(r#"{{"Triggered":{{{when},"effects":{effects}}}}}"#)
}

fn pay(cost: &str, paid: &str) -> String {
    format!(r#"[{{"Pay":{{"cost":{cost},"paid":{paid}}}}}]"#)
}

#[rstest]
#[case::draw_then_discard(
    &triggered(UPKEEP, &pay(r#"[{"Draw":1}]"#, r#"[{"Discard":{}}]"#)),
    "At the beginning of your upkeep, you may draw a card. If you do, discard a card."
)]
#[case::exile_two_from_graveyard(
    &triggered(UPKEEP, &pay(r#"[{"ExileFromGraveyard":{"count":2,"filter":[]}}]"#, r#"[{"PutCounters":{"counter":"+1/+1","on":"This"}}]"#)),
    "At the beginning of your upkeep, you may exile two cards from your graveyard. If you do, put a +1/+1 counter on this creature."
)]
#[case::exile_creature_cards(
    &triggered(UPKEEP, &pay(r#"[{"ExileFromGraveyard":{"count":2,"filter":[{"Type":"Creature"}]}}]"#, r#"[{"Draw":{}}]"#)),
    "At the beginning of your upkeep, you may exile two creature cards from your graveyard. If you do, draw a card."
)]
#[case::exile_the_dead_card(
    &triggered(DIES, &pay(r#"[{"ExileFromGraveyard":{"any_player":true,"filter":[{"Is":{"Arrived":{"of":"This","zone":"Graveyard"}}}]}}]"#, r#"[{"Draw":{}}]"#)),
    "When this creature dies, you may exile it. If you do, draw a card."
)]
#[case::exile_from_hand(
    &triggered(UPKEEP, &pay(r#"[{"ExileFromHand":{"filter":[]}}]"#, r#"[{"Untap":{"what":"This"}}]"#)),
    "At the beginning of your upkeep, you may exile a card from your hand. If you do, untap this creature."
)]
#[case::tap_another(
    &triggered(ATTACKS, &pay(r#"[{"TapPermanents":{"filter":[{"Type":"Creature"},"Other"]}}]"#, r#"[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":["CantBeBlocked"]}}]"#)),
    "Whenever this creature attacks, you may tap another untapped creature you control. If you do, this creature can't be blocked this turn."
)]
fn r5b_a_payment_that_is_not_mana_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(
        written(json, clause),
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

#[rstest]
// Two cards drawn are not "a card".
#[case::two_drawn(
    &triggered(UPKEEP, &pay(r#"[{"Draw":2}]"#, r#"[{"Discard":{}}]"#)),
    "At the beginning of your upkeep, you may draw a card. If you do, discard a card."
)]
// The "may" dropped: the draw and the discard both happen.
#[case::may_dropped(
    &triggered(UPKEEP, r#"[{"Draw":{}},{"Discard":{}}]"#),
    "At the beginning of your upkeep, you may draw a card. If you do, discard a card."
)]
// One card is not two.
#[case::one_exiled(
    &triggered(UPKEEP, &pay(r#"[{"ExileFromGraveyard":{"filter":[]}}]"#, r#"[{"PutCounters":{"counter":"+1/+1","on":"This"}}]"#)),
    "At the beginning of your upkeep, you may exile two cards from your graveyard. If you do, put a +1/+1 counter on this creature."
)]
// A card of any graveyard is not the card that died.
#[case::any_card_not_it(
    &triggered(DIES, &pay(r#"[{"ExileFromGraveyard":{"any_player":true,"filter":[]}}]"#, r#"[{"Draw":{}}]"#)),
    "When this creature dies, you may exile it. If you do, draw a card."
)]
// Another player pays: not "you may".
#[case::payer_wrong(
    &triggered(UPKEEP, r#"[{"Pay":{"who":"Opponents","cost":[{"Draw":1}],"paid":[{"Discard":{}}]}}]"#),
    "At the beginning of your upkeep, you may draw a card. If you do, discard a card."
)]
// The source itself is not "another".
#[case::tap_not_another(
    &triggered(ATTACKS, &pay(r#"[{"TapPermanents":{"filter":[{"Type":"Creature"}]}}]"#, r#"[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":["CantBeBlocked"]}}]"#)),
    "Whenever this creature attacks, you may tap another untapped creature you control. If you do, this creature can't be blocked this turn."
)]
fn r5b_a_wrong_payment_differs(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}

const COUNTER_X: &str = r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":"{1}","times":"X","unpaid":[{"Counter":{"what":{"Target":0}}}]}}]}}"#;

#[rstest]
#[case::x_times_one(COUNTER_X, true)]
#[case::x_times_two(&COUNTER_X.replace(r#""cost":"{1}""#, r#""cost":"{2}""#), false)]
#[case::you_pay(&COUNTER_X.replace(r#""who":{"ControllerOf":{"Target":0}},"#, ""), false)]
#[case::once(&COUNTER_X.replace(r#""times":"X","#, ""), false)]
fn r5b_one_paid_x_times_is_x(#[case] json: &str, #[case] reads: bool) {
    assert_eq!(
        written(json, "Counter target spell unless its controller pays {X}."),
        reads,
        "{json}"
    );
}

const CROVAX: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Sacrifice":{"as":"sacrificed","filter":[{"Type":"Creature"}]}}]}},{"If":{"condition":{"CountOf":{"op":"Eq","right":1,"selector":[{"Type":"Creature"}],"subject":{"Bound":"sacrificed"}}},"then":[{"PutCounters":{"counter":"+1/+1","on":"This"}}]}},{"If":{"condition":{"CountOf":{"op":"Eq","right":0,"selector":[{"Type":"Creature"}],"subject":{"Bound":"sacrificed"}}},"then":[{"RemoveCounters":{"counter":"+1/+1","from":"This"}}]}}],"when":{"StepBegins":{"step":"Upkeep","whose":"You"}}}}"#;
const CROVAX_TEXT: &str = "At the beginning of your upkeep, you may sacrifice a creature. If you do, put a +1/+1 counter on ~. If you don't, remove a +1/+1 counter from ~.";
const IMP: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Exile":{"as":"exiled","what":{"Target":0}}},{"If":{"condition":{"Compare":{"left":{"Bound":"exiled~moved"},"op":"Eq","right":1}},"then":[{"GainLife":{"amount":2}}]}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#;
const IMP_TEXT: &str = "When this creature enters, you may exile target creature card from a graveyard. If you do, you gain 2 life.";

#[rstest]
#[case::do_and_dont(CROVAX, CROVAX_TEXT, true)]
#[case::moved_count_inside_the_may(IMP, IMP_TEXT, true)]
// "If you do" tied to another binding than the step's.
#[case::other_binding(&CROVAX.replace(r#""subject":{"Bound":"sacrificed"}}},"then":[{"PutCounters""#, r#""subject":{"Bound":"other"}}},"then":[{"PutCounters""#), CROVAX_TEXT, false)]
// Counting two or more is not "you do".
#[case::two_or_more(&IMP.replace(r#""op":"Eq","right":1"#, r#""op":"Ge","right":2"#), IMP_TEXT, false)]
// Exactly one of a group exiled is not "you do".
#[case::one_of_a_group(&IMP.replace(r#""what":{"Target":0}"#, r#""what":{"Each":[{"Type":"Creature"},{"In":"Graveyard"}]}"#), "When this creature enters, you may exile all creature cards from all graveyards. If you do, you gain 2 life.", false)]
// The "may" dropped: the exile always happens.
#[case::may_dropped(r#"{"Triggered":{"effects":[{"Exile":{"as":"exiled","what":{"Target":0}}},{"If":{"condition":{"Compare":{"left":{"Bound":"exiled~moved"},"op":"Eq","right":1}},"then":[{"GainLife":{"amount":2}}]}}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This"]}}}}"#, IMP_TEXT, false)]
// The sacrifice's kind differs from the kind counted: not the same question.
#[case::other_kind(&CROVAX.replace(r#""selector":[{"Type":"Creature"}],"subject":{"Bound":"sacrificed"}}},"then":[{"PutCounters""#, r#""selector":[{"Type":"Artifact"}],"subject":{"Bound":"sacrificed"}}},"then":[{"PutCounters""#), CROVAX_TEXT, false)]
// Do and don't swapped.
#[case::swapped(&CROVAX.replace(r#""op":"Eq","right":1"#, r#""op":"Eq","right":9"#).replace(r#""op":"Eq","right":0"#, r#""op":"Eq","right":1"#).replace(r#""op":"Eq","right":9"#, r#""op":"Eq","right":0"#), CROVAX_TEXT, false)]
fn r5b_if_you_do_asks_what_the_optional_step_bound(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(json, clause), reads, "{json}");
}

const WURMLET: &str = r#"{"Triggered":{"effects":[{"GainLife":{"amount":1}},{"If":{"condition":{"Compare":{"left":"ResolvedThisTurn","op":"Eq","right":1}},"then":[{"PutCounters":{"counter":"+1/+1","on":"This"}}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":[{"Type":"Artifact"},{"ControlledBy":"You"}]}}}}"#;
const WURMLET_TEXT: &str = "Whenever an artifact you control enters, you gain 1 life. If this is the first time this ability has resolved this turn, put a +1/+1 counter on this creature.";
const BELLADONNA: &str = r#"{"Triggered":{"effects":[{"If":{"condition":{"Compare":{"left":"ResolvedThisTurn","op":"Eq","right":1}},"then":[{"GainLife":{"amount":1}}]}},{"If":{"condition":{"Compare":{"left":"ResolvedThisTurn","op":"Eq","right":2}},"then":[{"Draw":{}}]}},{"If":{"condition":{"Compare":{"left":"ResolvedThisTurn","op":"Eq","right":3}},"then":[{"PutCounters":{"counter":"+1/+1","on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":["Token",{"ControlledBy":"You"}]}}}}"#;
const BELLADONNA_TEXT: &str = "Whenever a token you control enters, you gain 1 life if this is the first time this ability has resolved this turn. If it's the second time, draw a card. If it's the third time, put a +1/+1 counter on each creature you control.";

#[rstest]
#[case::first(WURMLET, WURMLET_TEXT, true)]
#[case::three_times(BELLADONNA, BELLADONNA_TEXT, true)]
#[case::second_not_first(&WURMLET.replace(r#""op":"Eq","right":1"#, r#""op":"Eq","right":2"#), WURMLET_TEXT, false)]
#[case::at_least_not_exactly(&WURMLET.replace(r#""op":"Eq","right":1"#, r#""op":"Ge","right":1"#), WURMLET_TEXT, false)]
#[case::fourth_unworded(&WURMLET.replace(r#""op":"Eq","right":1"#, r#""op":"Eq","right":4"#), &WURMLET_TEXT.replace("first", "fourth"), false)]
#[case::times_swapped(&BELLADONNA.replace(r#""right":2}},"then":[{"Draw""#, r#""right":3}},"then":[{"Draw""#), BELLADONNA_TEXT, false)]
fn r5b_the_times_this_ability_resolved(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(written(json, clause), reads, "{json}");
}

const RAIN: &str = r#"{"Spell":{"effects":[{"Modal":{"choose":{"max":3,"min":1},"modes":[{"effects":[{"Destroy":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}]},{"effects":[{"Destroy":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Enchantment"}]}}]},{"effects":[{"Destroy":{"what":{"Target":0}}}],"targets":[{"what":{"Object":[{"Type":"Land"}]}}]}]}}]}}"#;
const RAIN_TEXT: &str = "Choose one or more —\n• Destroy target artifact.\n• Destroy target enchantment.\n• Destroy target land.";
const BOTH: &str = r#"{"Spell":{"effects":[{"Modal":{"choose":{"max":{"When":{"condition":{"Compare":{"left":{"Count":["Commander",{"ControlledBy":"You"}]},"op":"Ge","right":1}},"otherwise":1,"then":2}},"min":1},"modes":[{"effects":[{"Draw":{}}]},{"effects":[{"GainLife":{"amount":3}}]}]}}]}}"#;
const BOTH_TEXT: &str = "Choose one. If you control a commander as you cast this spell, you may choose both instead.\n• Draw a card.\n• You gain 3 life.";

#[rstest]
#[case::one_or_more(RAIN, RAIN_TEXT, true)]
#[case::two_of_three(&RAIN.replace(r#""max":3,"min":1"#, r#""max":2,"min":1"#), RAIN_TEXT, false)]
#[case::may_repeat(&RAIN.replace(r#""max":3,"min":1}"#, r#""max":3,"min":1},"repeat":true"#), RAIN_TEXT, false)]
#[case::one_only(&RAIN.replace(r#""max":3,"min":1"#, r#""max":1,"min":1"#), RAIN_TEXT, false)]
#[case::both_instead(BOTH, BOTH_TEXT, true)]
#[case::both_always(&BOTH.replace(r#""otherwise":1"#, r#""otherwise":2"#), BOTH_TEXT, false)]
#[case::never_both(&BOTH.replace(r#""then":2"#, r#""then":1"#), BOTH_TEXT, false)]
fn r5b_the_number_of_modes(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(json, clause), reads, "{json}");
}

#[rstest]
#[case::searched_your_library(
    r#"{"Searched":{"zone":"Library"}}"#,
    Some("you search your library this way")
)]
#[case::searched_a_graveyard(r#"{"Searched":{"zone":"Graveyard"}}"#, None)]
#[case::another_player_searched(r#"{"Searched":{"who":"Opponents","zone":"Library"}}"#, None)]
#[case::seven_cards(
    r#"{"Compare":{"left":{"Count":[{"OwnedBy":"You"},{"In":"Graveyard"}]},"op":"Ge","right":7}}"#,
    Some("there are seven or more cards in your graveyard")
)]
#[case::cards_of_any_graveyard(
    r#"{"Compare":{"left":{"Count":[{"In":"Graveyard"}]},"op":"Ge","right":7}}"#,
    None
)]
fn r5b_condition_texts(#[case] json: &str, #[case] text: Option<&str>) {
    let condition: crate::value::Condition =
        serde_json::from_str(json).unwrap_or_else(|error| panic!("{json}: {error}"));
    let texts = super::super::conditional::condition_texts(&condition);
    match text {
        Some(text) => assert!(texts.iter().any(|said| said == text), "{texts:?}"),
        None => assert!(
            !texts
                .iter()
                .any(|said| said.contains("search") || said.contains("seven")),
            "{texts:?}"
        ),
    }
}

const MASTERY: &str = r#"{"Spell":{"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"OwnedBy":"You"},{"In":"Graveyard"}]},"op":"Ge","right":2}},"then":[{"Draw":{}}]}}]}}"#;

#[rstest]
#[case::and_or(MASTERY, true)]
#[case::creature_cards(&MASTERY.replace(r#"{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}"#, r#"{"Type":"Creature"}"#), false)]
#[case::three(&MASTERY.replace(r#""right":2"#, r#""right":3"#), false)]
fn r5b_spell_mastery(#[case] json: &str, #[case] reads: bool) {
    assert_eq!(
        written(
            json,
            "If there are two or more instant and/or sorcery cards in your graveyard, draw a card."
        ),
        reads,
        "{json}"
    );
}
