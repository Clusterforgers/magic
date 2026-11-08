//! Goal: r6d writers read back clauses no writer wrote before (if you do after a step you must take, the kind of the one target or of the card, the card or another object entering, a fact of the card's cast, facts of the turn), and a rule of another meaning (other player, other count, other kind, other subject, a missing "another", other fact) reads `Differs`.
use super::*;

/// Whether one wording of the ability is the clause.
fn writes_as(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|written| same_text(written, clause))
}

const FEAST: &str = r#"{"Triggered":{"effects":[{"Sacrifice":{"as":"sacrificed","filter":[{"Type":"Creature"},{"Not":{"Subtype":"Demon"}}]}},{"If":{"condition":{"CountOf":{"op":"Ge","right":1,"selector":[],"subject":{"Bound":"sacrificed"}}},"then":[{"Draw":{}}]}}],"when":{"StepBegins":{"step":"End","whose":"You"}}}}"#;
const FEAST_TEXT: &str =
    "At the beginning of your end step, sacrifice a non-Demon creature. If you do, draw a card.";

#[rstest]
#[case::you_must(FEAST, true)]
// Another player's sacrifice is not "if you do".
#[case::opponent(&FEAST.replace(r#""as":"sacrificed","#, r#""as":"sacrificed","who":"Opponents","#), false)]
// Two sacrificed is not one.
#[case::two(&FEAST.replace(r#""as":"sacrificed","#, r#""as":"sacrificed","count":2,"#), false)]
// None sacrificed is "if you don't".
#[case::none(&FEAST.replace(r#""op":"Ge","right":1"#, r#""op":"Eq","right":0"#), false)]
// A count of another name asks about something else.
#[case::other_name(&FEAST.replace(r#"{"Bound":"sacrificed"}}"#, r#"{"Bound":"other"}}"#), false)]
fn r6d_if_you_do_after_a_step_you_must_take(#[case] json: &str, #[case] reads: bool) {
    assert_eq!(writes_as(json, FEAST_TEXT), reads, "{json}");
}

const ENFORCER: &str = r#"{"Activated":{"cost":"{2}{W}{B}","effects":[{"Exile":{"what":{"Target":0}}},{"If":{"condition":{"CountOf":{"op":"Eq","right":1,"selector":[{"Type":"Creature"}],"subject":{"Target":0}}},"then":[{"Draw":{}}]}}],"targets":[{"what":{"Object":[{"In":"Graveyard"}]}}]}}"#;
const ENFORCER_TEXT: &str =
    "{2}{W}{B}: Exile target card from a graveyard. If it was a creature card, draw a card.";
const SANDS: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Target":0}}},{"If":{"condition":{"CountOf":{"op":"Eq","right":0,"selector":[{"Type":"Land"},{"Supertype":"Basic"}],"subject":{"Target":0}}},"then":[{"Draw":{}}]}}],"targets":[{"what":{"Object":[{"Type":"Land"}]}}]}}"#;
const SANDS_TEXT: &str = "Destroy target land. If that land was nonbasic, draw a card.";
const TOME: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Target":0}}},{"If":{"condition":{"CountOf":{"op":"Ge","right":1,"selector":[{"Compare":{"op":"Le","stat":"ManaValue","to":3}}],"subject":{"Target":0}}},"then":[{"Draw":{}}]}}],"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}]}}"#;

#[rstest]
#[case::creature_card(ENFORCER, ENFORCER_TEXT, true)]
#[case::other_kind(&ENFORCER.replace(r#""selector":[{"Type":"Creature"}]"#, r#""selector":[{"Type":"Land"}]"#), ENFORCER_TEXT, false)]
// None of the kind is "it wasn't".
#[case::none_of_kind(&ENFORCER.replace(r#""op":"Eq","right":1"#, r#""op":"Eq","right":0"#), ENFORCER_TEXT, false)]
// A kind of the card itself is not the target's.
#[case::itself(&ENFORCER.replace(r#""subject":{"Target":0}"#, r#""subject":"This""#), ENFORCER_TEXT, false)]
// Up to two targets: "it" is not one object.
#[case::up_to_two(&ENFORCER.replace(r#"[{"what""#, r#"[{"bounds":{"min":0,"max":2},"what""#), ENFORCER_TEXT, false)]
#[case::nonbasic(SANDS, SANDS_TEXT, true)]
#[case::basic(&SANDS.replace(r#""op":"Eq","right":0"#, r#""op":"Ge","right":1"#), SANDS_TEXT, false)]
// "that creature" names a target that is no creature.
#[case::wrong_noun(
    SANDS,
    "Destroy target land. If that creature was nonbasic, draw a card.",
    false
)]
#[case::mana_value(
    TOME,
    "Destroy target artifact. If its mana value was 3 or less, draw a card.",
    true
)]
#[case::that_artifact(
    TOME,
    "Destroy target artifact. If that artifact has mana value 3 or less, draw a card.",
    true
)]
#[case::more_value(&TOME.replace(r#""op":"Le""#, r#""op":"Ge""#), "Destroy target artifact. If its mana value was 3 or less, draw a card.", false)]
fn r6d_if_the_one_target_is_of_a_kind(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(writes_as(json, clause), reads, "{json}");
}

const LUMARET: &str = r#"{"Triggered":{"effects":[{"GainLife":{"amount":1}}],"when":{"ZoneChange":{"to":"Battlefield","what":[{"AnyOf":["This",{"Not":{"AnyOf":[{"Not":{"Type":"Creature"}},{"Not":"Other"},{"Not":{"ControlledBy":"You"}}]}}]}]}}}}"#;
const LUMARET_TEXT: &str =
    "Whenever this creature or another creature you control enters, you gain 1 life.";
const KICKED: &str = r#"{"Triggered":{"effects":[{"Draw":{"count":2}}],"when":{"ZoneChange":{"to":"Battlefield","what":["This","Kicked"]}}}}"#;
const KICKED_TEXT: &str = "When this creature enters, if it was kicked, draw two cards.";
const STOKER: &str = r#"{"Triggered":{"effects":[{"Draw":{}}],"when":{"ZoneChange":{"to":"Battlefield","what":[{"WasCast":{"by":"You","from":"Hand"}},{"OwnedBy":"You"},"This"]}}}}"#;
const STOKER_TEXT: &str = "When this creature enters, if you cast it from your hand, draw a card.";
const BATTALION_TEXT: &str = "When this creature enters, if you cast it, draw a card.";

#[rstest]
#[case::another(LUMARET, LUMARET_TEXT, true)]
// Without "Other" the card itself is counted twice: no "another".
#[case::not_another(&LUMARET.replace(r#"{"Not":"Other"},"#, ""), LUMARET_TEXT, false)]
#[case::opponents(&LUMARET.replace(r#"{"ControlledBy":"You"}"#, r#"{"ControlledBy":"Opponents"}"#), LUMARET_TEXT, false)]
// "or" of the two kinds, not "and".
#[case::either_kind(&LUMARET.replace(r#"{"Not":{"AnyOf""#, r#"{"AnyOf":[{"Not":{"AnyOf""#).replace("]}}]}]}}", "]}}]}]}]}}"), LUMARET_TEXT, false)]
#[case::kicked(KICKED, KICKED_TEXT, true)]
#[case::not_kicked(&KICKED.replace(r#","Kicked""#, ""), KICKED_TEXT, false)]
#[case::from_hand(STOKER, STOKER_TEXT, true)]
#[case::from_graveyard(&STOKER.replace(r#""from":"Hand""#, r#""from":"Graveyard""#), STOKER_TEXT, false)]
#[case::cast(&STOKER.replace(r#","from":"Hand""#, "").replace(r#"{"OwnedBy":"You"},"#, ""), BATTALION_TEXT, true)]
// Cast from the hand is not any cast.
#[case::cast_anywhere(STOKER, BATTALION_TEXT, false)]
#[case::cast_by_anyone(&STOKER.replace(r#""by":"You","#, ""), STOKER_TEXT, false)]
fn r6d_the_card_or_another_entering_and_its_cast(
    #[case] json: &str,
    #[case] clause: &str,
    #[case] reads: bool,
) {
    assert_eq!(writes_as(json, clause), reads, "{json}");
}

/// A trigger at the beginning of each upkeep that draws a card with the intervening "if" given as JSON.
fn upkeep(condition: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"Draw":{{}}}}],"if":{condition},"when":{{"StepBegins":{{"step":"Upkeep"}}}}}}}}"#
    )
}

const NONE_CAST: &str = r#"{"Compare":{"left":{"CastLastTurn":"EachPlayer"},"op":"Eq","right":0}}"#;
const TWO_CAST: &str = r#"{"Compare":{"left":{"CastLastTurn":"EachPlayer"},"op":"Ge","right":2}}"#;
const REVOLT: &str = r#"{"Compare":{"left":{"Tallied":{"what":"PermanentsLeftUnder","who":"You"}},"op":"Ge","right":1}}"#;
const DESCENDED: &str =
    r#"{"Compare":{"left":{"Tallied":{"what":"Descended","who":"You"}},"op":"Ge","right":1}}"#;
const DIED: &str = r#"{"Compare":{"left":{"Tally":{"event":"Died","filter":[{"Type":"Creature"}],"who":"EachPlayer"}},"op":"Ge","right":1}}"#;
const TAPPED: &str = r#"{"CountOf":{"op":"Ge","right":1,"selector":["Tapped"],"subject":"This"}}"#;

#[rstest]
#[case::no_spells(NONE_CAST, "if no spells were cast last turn", true)]
#[case::one_spell(&NONE_CAST.replace(r#""op":"Eq","right":0"#, r#""op":"Le","right":1"#), "if no spells were cast last turn", false)]
#[case::two_spells(TWO_CAST, "if a player cast two or more spells last turn", true)]
#[case::three_spells(&TWO_CAST.replace(r#""right":2"#, r#""right":3"#), "if a player cast two or more spells last turn", false)]
#[case::revolt(
    REVOLT,
    "if a permanent left the battlefield under your control this turn",
    true
)]
#[case::revolt_past(
    REVOLT,
    "if a permanent you controlled left the battlefield this turn",
    true
)]
#[case::revolt_opponent(&REVOLT.replace(r#""who":"You""#, r#""who":"Opponents""#), "if a permanent left the battlefield under your control this turn", false)]
#[case::descended(DESCENDED, "if you descended this turn", true)]
#[case::descended_twice(&DESCENDED.replace(r#""right":1"#, r#""right":2"#), "if you descended this turn", false)]
#[case::died(DIED, "if a creature died this turn", true)]
#[case::three_died(&DIED.replace(r#""right":1"#, r#""right":3"#), "if three or more creatures died this turn", true)]
#[case::yours_died(&DIED.replace(r#"[{"Type":"Creature"}]"#, r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#), "if a creature died this turn", false)]
#[case::tapped(TAPPED, "if this creature is tapped", true)]
#[case::untapped(&TAPPED.replace("Tapped", "Untapped"), "if this creature is tapped", false)]
fn r6d_facts_of_the_turn_and_of_the_card(
    #[case] condition: &str,
    #[case] fact: &str,
    #[case] reads: bool,
) {
    let clause = format!("At the beginning of each upkeep, {fact}, draw a card.");
    assert_eq!(writes_as(&upkeep(condition), &clause), reads, "{condition}");
}
