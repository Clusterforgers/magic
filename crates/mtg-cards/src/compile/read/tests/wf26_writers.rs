//! Goal: the wf26 writers give the oracle wording of a hand revealed and the card you choose from it discarded, or every
//! card of a kind discarded, kinship, a creature type or color of your choice, the owner's choice of the top or bottom
//! of their library, a hand looked at, the Ring's temptation and "Enchant creature or Vehicle", and a trigger about "a
//! creature" that is dealt damage names that creature in its steps; another player, chooser, kind, count, value or
//! object never reads back as the clause.
use super::*;
use crate::refs::{Pred, Selector};
use mtg_core::card::{CardType, Subtype};

#[rstest]
#[case::creature_or_vehicle("creature or Vehicle", Some(vec![Pred::Type(CardType::Creature), Pred::Subtype(Subtype("Vehicle".into()))]))]
#[case::artifact_or_land("artifact or land", Some(vec![Pred::Type(CardType::Artifact), Pred::Type(CardType::Land)]))]
#[case::three_words("creature or green Vehicle", None)]
#[case::no_kind("creature or flying", None)]
fn wf26_enchant_either_of_two_kinds(#[case] text: &str, #[case] either: Option<Vec<Pred>>) {
    let expected = either.map(|preds| Selector(vec![Pred::AnyOf(preds)]));
    assert_eq!(enchant_filter(text), expected, "{text}");
}

#[rstest]
#[case::target_player(
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"LookAtHand":{"of":{"Target":0}}}]}}"#,
    "Look at target player's hand.",
    true
)]
#[case::target_opponent(r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Opponent"}],"effects":[{"LookAtHand":{"of":{"Target":0}}}]}}"#, "When ~ enters, look at target opponent's hand.", true)]
#[case::reveal_for_look(r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Reveal":{"what":{"Each":[{"OwnedBy":{"Target":0}},{"In":"Hand"}]}}}]}}"#, "Look at target player's hand.", false)]
#[case::your_hand(
    r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"LookAtHand":{"of":"You"}}]}}"#,
    "Look at target player's hand.",
    false
)]
#[case::ring(r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"RingTempts":{}}]}}"#, "When ~ enters, the Ring tempts you.", true)]
#[case::ring_and_draw(
    r#"{"Spell":{"effects":[{"Draw":{}},{"RingTempts":{}}]}}"#,
    "Draw a card. The Ring tempts you.",
    true
)]
#[case::ring_for_an_opponent(r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"RingTempts":{"who":"Opponents"}}]}}"#, "When ~ enters, the Ring tempts you.", false)]
fn wf26_plain_steps(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

fn reads(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn spell(targets: &str, effects: &str) -> String {
    format!(r#"{{"Spell":{{"targets":[{{"what":"{targets}"}}],"effects":[{REVEAL},{effects}]}}}}"#)
}

const REVEAL: &str = r#"{"Reveal":{"what":{"Each":[{"OwnedBy":{"Target":0}},{"In":"Hand"}]}}}"#;
const NONLAND: &str =
    r#"{"Discard":{"who":{"Target":0},"filter":[{"Not":{"Type":"Land"}}],"by":"You"}}"#;
const DURESS: &str = "Target opponent reveals their hand. You choose a nonland card from it. That player discards that card.";
const SMALL: &str = r#"{"Discard":{"who":{"Target":0},"filter":[{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Le","to":3}}],"by":"You"}}"#;
const ALL: &str = r#"{"Discard":{"who":{"Target":0},"count":{"Count":[{"In":"Hand"},{"OwnedBy":{"Target":0}},{"Not":{"Type":"Land"}}]},"filter":[{"Not":{"Type":"Land"}}]}}"#;
const AMNESIA: &str = "Target player reveals their hand and discards all nonland cards.";

#[rstest]
#[case::chosen(&spell("Opponent", NONLAND), DURESS, true)]
#[case::they_discard(&spell("Opponent", NONLAND), &DURESS.replace("That player discards", "They discard"), true)]
#[case::any_player(&spell("Player", NONLAND), DURESS, false)]
#[case::the_player_chooses(&spell("Opponent", &NONLAND.replace(r#","by":"You""#, "")), DURESS, false)]
#[case::another_kind(&spell("Opponent", &NONLAND.replace("Land", "Creature")), DURESS, false)]
#[case::any_card(&spell("Opponent", &NONLAND.replace(r#""filter":[{"Not":{"Type":"Land"}}],"#, "")), &DURESS.replace("nonland card", "card"), true)]
#[case::two_cards(&spell("Opponent", &NONLAND.replace(r#""by""#, r#""count":2,"by""#)), &DURESS.replace("a nonland card", "two nonland cards"), false)]
#[case::without_the_reveal(&format!(r#"{{"Spell":{{"targets":[{{"what":"Opponent"}}],"effects":[{NONLAND}]}}}}"#), DURESS, false)]
#[case::another_hand(&spell("Opponent", &NONLAND.replace(r#""who":{"Target":0}"#, r#""who":"You""#)), DURESS, false)]
#[case::small_after(&spell("Player", SMALL), "Target player reveals their hand. You choose a nonland card from it with mana value 3 or less. That player discards that card.", true)]
#[case::small_before(&spell("Player", SMALL), "Target player reveals their hand. You choose a nonland card with mana value 3 or less from it. That player discards that card.", true)]
#[case::small_for_greater(&spell("Player", &SMALL.replace("Le", "Ge")), "Target player reveals their hand. You choose a nonland card from it with mana value 3 or less. That player discards that card.", false)]
#[case::every_nonland(&spell("Player", ALL), AMNESIA, true)]
#[case::every_card_of_another_kind(&spell("Player", &ALL.replacen("Land", "Creature", 1)), AMNESIA, false)]
#[case::one_for_all(&spell("Player", &ALL.replace(r#""count":{"Count":[{"In":"Hand"},{"OwnedBy":{"Target":0}},{"Not":{"Type":"Land"}}]},"#, "")), AMNESIA, false)]
#[case::all_chosen(&spell("Player", &ALL.replace("}]}}", r#"}],"by":"You"}}"#)), AMNESIA, false)]
fn wf26_a_revealed_hand_and_its_discard(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

const KIN: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"May":{"effects":[{"LookAtTop":{"as":"top"}},{"Choose":{"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}},{"SharesSubtype":"This"}]},"count":{"min":0,"max":1},"as":"revealed2"}},{"Reveal":{"what":{"Bound":"revealed2"}}},{"If":{"condition":{"CountOf":{"subject":{"Bound":"revealed2"},"selector":[],"op":"Eq","right":1}},"then":[{"GainLife":{"amount":4}}]}}]}}]}}"#;
const GRAYBEARD: &str = "At the beginning of your upkeep, you may look at the top card of your library. If it shares a creature type with ~, you may reveal it. If you do, you gain 4 life.";

#[rstest]
#[case::kinship(KIN, GRAYBEARD, true)]
#[case::other_life(&KIN.replace(r#""amount":4"#, r#""amount":3"#), GRAYBEARD, false)]
#[case::any_card(&KIN.replace(r#",{"SharesSubtype":"This"}"#, ""), GRAYBEARD, false)]
#[case::not_optional(&KIN.replace(r#"[{"May":{"effects":"#, "").replace("}]}}]}}", "}]}}"), GRAYBEARD, false)]
#[case::without_the_reveal(&KIN.replace(r#"{"Reveal":{"what":{"Bound":"revealed2"}}},"#, ""), GRAYBEARD, false)]
#[case::each_upkeep(&KIN.replace(r#","whose":"You""#, ""), GRAYBEARD, false)]
fn wf26_kinship(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

const MISTFORM: &str = r#"{"Activated":{"cost":"{1}","effects":[{"ChooseValue":{"kind":"CreatureType","as":"chosen"}},{"Continuous":{"affected":"This","mods":[{"SetCreatureTypesChosen":"chosen"}],"duration":"UntilEndOfTurn"}}]}}"#;
const DREAMER: &str = "{1}: ~ becomes the creature type of your choice until end of turn.";
const CHAMELEON: &str = r#"{"Activated":{"cost":"{G}","effects":[{"ChooseValue":{"kind":"Color","as":"chosen"}},{"Continuous":{"affected":"This","mods":[{"SetColorsChosen":"chosen"}],"duration":"UntilEndOfTurn"}}]}}"#;
const KAVU: &str = "{G}: ~ becomes the color of your choice until end of turn.";
const MOTES: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"ChooseValue":{"who":{"OwnerOf":{"Target":0}},"kind":{"Options":["Top","Bottom"]},"as":"way"}},{"If":{"condition":{"ChosenWord":{"name":"way","word":"Bottom"}},"then":[{"Move":{"what":{"Target":0},"to":{"Library":{"place":"Bottom"}}}}],"otherwise":[{"Move":{"what":{"Target":0},"to":{"Library":{"place":"Top"}}}}]}}]}}"#;
const OWNER: &str =
    "Target creature's owner puts it on their choice of the top or bottom of their library.";

#[rstest]
#[case::creature_type(MISTFORM, DREAMER, true)]
#[case::creature_type_for_good(&MISTFORM.replace(r#","duration":"UntilEndOfTurn""#, r#","duration":"UntilYourNextTurn""#), DREAMER, false)]
#[case::creature_type_for_color(&MISTFORM.replace("SetCreatureTypesChosen", "SetColorsChosen"), DREAMER, false)]
#[case::creature_type_of_another_value(&MISTFORM.replace(r#"[{"SetCreatureTypesChosen":"chosen"}]"#, r#"[{"SetCreatureTypesChosen":"other"}]"#), DREAMER, false)]
#[case::color(CHAMELEON, KAVU, true)]
#[case::color_of_an_opponent(&CHAMELEON.replace(r#"{"kind":"Color""#, r#"{"who":"Opponents","kind":"Color""#), KAVU, false)]
#[case::land_type(&MISTFORM.replace("CreatureType", "LandType").replace("SetCreatureTypesChosen", "SetLandTypesChosen"), "{1}: ~ becomes the basic land type of your choice until end of turn.", false)]
#[case::top_or_bottom(MOTES, OWNER, true)]
#[case::bottom_or_top(&MOTES.replace(r#""word":"Bottom""#, r#""word":"Top""#), OWNER, false)]
#[case::always_the_top(&MOTES.replace(r#""place":"Bottom""#, r#""place":"Top""#), OWNER, false)]
#[case::your_choice(&MOTES.replace(r#""who":{"OwnerOf":{"Target":0}},"#, ""), OWNER, false)]
#[case::of_a_nonland_permanent(&MOTES.replace(r#"{"Type":"Creature"}"#, r#"{"Not":{"Type":"Land"}}"#), "Target nonland permanent's owner puts it on their choice of the top or bottom of their library.", true)]
fn wf26_a_value_of_your_choice(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

fn dealt(to: &str, effects: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"DealsDamage":{{"source":[],"to":{{"Each":{to}}}}}}},"effects":{effects}}}}}"#
    )
}

const CREATURE: &str = r#"[{"Type":"Creature"}]"#;
const MINE: &str = r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#;
const DESTROY: &str = r#"[{"Destroy":{"what":"Triggering","regenerate":false}}]"#;
const COUNTER: &str = r#"[{"PutCounters":{"on":"Triggering","counter":"+1/+1"}}]"#;

#[rstest]
#[case::destroyed(&dealt(CREATURE, DESTROY), "Whenever a creature is dealt damage, destroy it. It can't be regenerated.", true)]
#[case::regenerable(&dealt(CREATURE, &DESTROY.replace(r#","regenerate":false"#, "")), "Whenever a creature is dealt damage, destroy it. It can't be regenerated.", false)]
#[case::the_card_destroyed(&dealt(CREATURE, &DESTROY.replace("Triggering", "This")), "Whenever a creature is dealt damage, destroy it. It can't be regenerated.", false)]
#[case::countered(&dealt(MINE, COUNTER), "Whenever a creature you control is dealt damage, put a +1/+1 counter on it.", true)]
#[case::any_creature_countered(&dealt(CREATURE, COUNTER), "Whenever a creature you control is dealt damage, put a +1/+1 counter on it.", false)]
#[case::the_card_countered(&dealt(MINE, &COUNTER.replace("Triggering", "This")), "Whenever a creature you control is dealt damage, put a +1/+1 counter on it.", false)]
fn wf26_a_creature_dealt_damage_is_named_in_its_steps(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

#[rstest]
#[case::the_card(r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[],"to":"This"}}},"effects":[{"Destroy":{"what":"This","regenerate":false}}]}}"#, "When ~ is dealt damage, destroy it. It can't be regenerated.", true)]
#[case::the_card_as_they(r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[],"to":"This"}}},"effects":[{"Destroy":{"what":"This","regenerate":false}}]}}"#, "When ~ is dealt damage, destroy it. They can't be regenerated.", false)]
#[case::all_creatures(r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"}]},"regenerate":false}}]}}"#, "Destroy all creatures. It can't be regenerated.", false)]
fn wf26_the_card_that_cant_be_regenerated_is_it(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] same: bool,
) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

const STUNNED: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":2}}],"effects":[{"Tap":{"what":{"Target":0}}},{"PutCounters":{"on":{"Target":0},"counter":{"Other":"stun"}}}]}}"#;
const STALL: &str =
    "Tap up to two target creatures. Put a stun counter on each of those creatures.";

#[rstest]
#[case::those_creatures(STUNNED, STALL, true)]
#[case::those_lands(STUNNED, &STALL.replace("those creatures", "those lands"), false)]
#[case::those_of_another_target(&STUNNED.replace(r#"{"PutCounters":{"on":{"Target":0}"#, r#"{"PutCounters":{"on":"This""#), STALL, false)]
#[case::those_permanents(&STUNNED.replace(r#"[{"Type":"Creature"}]"#, "[]"), "Tap up to two target permanents. Put a stun counter on each of those permanents.", true)]
fn wf26_each_of_those(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

const BRAGGART: &str = r#"{"Activated":{"cost":"{1}{W}","condition":{"Matches":{"subject":"This","selector":["AttackedThisTurn"]}},"limit":"OncePerTurn","effects":[{"Untap":{"what":"This"}}]}}"#;
const BOAST: &str = "Boast \u{2014} {1}{W}: Untap ~.";
const DRAKE: &str = r#"{"Activated":{"cost":[{"Mana":"{1}"},{"DiscardRandom":1}],"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#;
const RANDOM: &str = "{1}, Discard a card at random: ~ gets +2/+0 until end of turn.";

#[rstest]
#[case::boast(BRAGGART, BOAST, true)]
#[case::boast_without_the_attack(&BRAGGART.replace(r#""condition":{"Matches":{"subject":"This","selector":["AttackedThisTurn"]}},"#, ""), BOAST, false)]
#[case::boast_without_the_limit(&BRAGGART.replace(r#""limit":"OncePerTurn","#, ""), BOAST, false)]
#[case::boast_once_a_game(&BRAGGART.replace("OncePerTurn", "OncePerGame"), BOAST, false)]
#[case::random_discard(DRAKE, RANDOM, true)]
#[case::chosen_discard(&DRAKE.replace(r#"{"DiscardRandom":1}"#, r#"{"Discard":{}}"#), RANDOM, false)]
#[case::two_at_random(&DRAKE.replace(r#"{"DiscardRandom":1}"#, r#"{"DiscardRandom":2}"#), RANDOM, false)]
fn wf26_activation_costs(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}

const SOUND: &str = r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":"{2}","paid":[{"Surveil":{"count":2}}],"unpaid":[{"Counter":{"what":{"Target":0}}}]}}]}}"#;
const SURVEIL: &str = "Counter target spell unless its controller pays {2}. If they do, surveil 2.";

#[rstest]
#[case::paid(SOUND, SURVEIL, true)]
#[case::paid_by_you(SOUND, &SURVEIL.replace("If they do, ", "If they do, you "), true)]
#[case::paid_otherwise(&SOUND.replace(r#""count":2"#, r#""count":1"#), SURVEIL, false)]
#[case::nothing_paid(&SOUND.replace(r#""paid":[{"Surveil":{"count":2}}],"#, ""), SURVEIL, false)]
#[case::paid_for_nothing(&SOUND.replace(r#","unpaid":[{"Counter":{"what":{"Target":0}}}]"#, ""), SURVEIL, false)]
#[case::paid_by_the_payer(&SOUND.replace(r#""paid":[{"Surveil":{"count":2}}]"#, r#""paid":[{"Surveil":{"who":{"ControllerOf":{"Target":0}},"count":2}}]"#), SURVEIL, false)]
fn wf26_if_they_pay(#[case] json: &str, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(reads(json, clause), same, "{clause}");
}
