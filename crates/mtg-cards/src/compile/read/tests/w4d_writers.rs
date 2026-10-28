//! Goal: the w4d writers give the oracle wording of a delayed trigger that closes an ability and of steps repeated for each player or opponent; another step, owner of the step, duration, event, player group, number, counter kind, comparison or keyword never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn w4d_written(json: &str, clause: &str) -> bool {
    let variants = written_variants(&ability(json));
    variants.iter().any(|wording| same_text(wording, clause))
}

const W4D_ADARKAR: &str = r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"},"Other"]}}],"effects":[{"DelayedTrigger":{"when":{"ZoneChange":{"what":[{"Is":{"Target":0}}],"from":"Battlefield","to":"Graveyard"}},"this_turn":true,"effects":[{"Move":{"what":"Triggering","to":{"Battlefield":{"under":"You"}}}}]}}]}}"#;
const W4D_APPRENTICE: &str = r#"{"Activated":{"cost":[{"Mana":"{B}"},"Tap","SacrificeSelf"],"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Haste"}],"duration":"Forever"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Sacrifice":{"filter":[{"Is":{"Target":0}}]}}]}}]}}"#;
const W4D_KELSIEN: &str = r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}},{"DelayedTrigger":{"when":{"ZoneChange":{"what":[{"Is":{"Target":0}}],"from":"Battlefield","to":"Graveyard"}},"this_turn":true,"effects":[{"PutCounters":{"on":"You","counter":{"Other":"experience"}}}]}}]}}"#;
const W4D_MASSACRE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},"Other"]},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}}],"duration":"UntilEndOfTurn"}},{"DelayedTrigger":{"when":{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}},"once":false,"this_turn":true,"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},"Other"]},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}}],"duration":"UntilEndOfTurn"}}]}}]}}"#;
const W4D_PLASM: &str = r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Counter":{"what":{"Target":0}}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"Main1","whose":"You"}},"effects":[{"AddMana":{"mana":{"AnyCombination":{"count":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}}}]}}]}}"#;
const W4D_PUPPETEER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"Opponents"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"under":"You"}}}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Haste"}],"duration":"Forever"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"Exile":{"what":{"Target":0}}}]}}]}}"#;
const W4D_VIZKOPA: &str = r#"{"Activated":{"cost":"{1}{W}{B}","effects":[{"DelayedTrigger":{"when":{"LifeGained":{"who":"You"}},"once":false,"this_turn":true,"effects":[{"LoseLife":{"who":"Opponents","amount":"Triggering"}}]}}]}}"#;
const W4D_TEMPEST: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"}]}}},{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"LoseLife":{"who":{"Bound":"player"},"amount":{"Destroyed":[{"Type":"Creature"},{"ControlledBy":{"Bound":"player"}}]}}}]}}]}}"#;
const W4D_FEED: &str = r#"{"Spell":{"effects":[{"Draw":{"count":3}},{"LoseLife":{"amount":3}},{"RepeatEach":{"over":"Opponents","as":"opponent","effects":[{"If":{"condition":{"Compare":{"left":{"Counters":{"on":{"Bound":"opponent"},"counter":"poison"}},"op":"Ge","right":3}},"then":[{"LoseLife":{"who":{"Bound":"opponent"},"amount":3}}]}}]}}]}}"#;
const W4D_FIELD: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap","SacrificeSelf"],"targets":[{"what":{"Object":[{"Type":"Land"},{"Not":{"Supertype":"Basic"}},{"ControlledBy":"Opponents"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Search":{"who":{"Bound":"player"},"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{}}}},{"Shuffle":{"who":{"Bound":"player"}}}]}}]}}"#;
const W4D_UNCLEAN: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"LoseLife":{"who":"Opponents","amount":2}},{"RepeatEach":{"over":"Opponents","as":"opponent","effects":[{"If":{"condition":{"Compare":{"left":{"Life":{"Bound":"opponent"}},"op":"Lt","right":{"Life":"You"}}},"then":[{"CreateToken":{"token":{"Custom":{"name":"Plaguebearer of Nurgle","colors":["Black"],"types":["Creature"],"subtypes":["Demon"],"power":1,"toughness":3}}}}]}}]}}]}}"#;
const W4D_KWAIN: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"May":{"who":{"Bound":"player"},"effects":[{"Draw":{"who":{"Bound":"player"}}}]}},{"If":{"condition":"Performed","then":[{"GainLife":{"who":{"Bound":"player"},"amount":1}}]}}]}}]}}"#;

const W4D_ADARKAR_TEXT: &str = "{T}: When target creature other than this creature dies this turn, return that card to the battlefield under your control.";
const W4D_APPRENTICE_TEXT: &str = "{B}, {T}, Sacrifice ~: Return target creature card from your graveyard to the battlefield. That creature gains haste. At the beginning of the next end step, sacrifice it.";
const W4D_KELSIEN_TEXT: &str = "{T}: ~ deals 1 damage to target creature you don't control. When that creature dies this turn, you get an experience counter.";
const W4D_MASSACRE_TEXT: &str = "When ~ enters, each other creature gets -1/-1 until end of turn. Whenever a creature dies this turn, each creature other than ~ gets -1/-1 until end of turn.";
const W4D_PLASM_TEXT: &str = "Counter target spell. At the beginning of your next first main phase, add X mana in any combination of colors, where X is that spell's mana value.";
const W4D_PUPPETEER_TEXT: &str = "When ~ enters, put target creature card from an opponent's graveyard onto the battlefield under your control. It gains haste. At the beginning of your next end step, exile it.";
const W4D_VIZKOPA_TEXT: &str =
    "{1}{W}{B}: Whenever you gain life this turn, each opponent loses that much life.";
const W4D_TEMPEST_TEXT: &str = "Destroy all creatures. Each player loses life equal to the number of creatures they controlled that were destroyed this way.";
const W4D_FEED_TEXT: &str = "You draw three cards and you lose 3 life.\nEach opponent who has three or more poison counters loses 3 life.";
const W4D_FIELD_TEXT: &str = "{2}, {T}, Sacrifice ~: Destroy target nonbasic land an opponent controls. Each player searches their library for a basic land card, puts it onto the battlefield, then shuffles.";
const W4D_UNCLEAN_TEXT: &str = "At the beginning of your end step, each opponent loses 2 life. Then for each opponent who has less life than you, create a 1/3 black Demon creature token named Plaguebearer of Nurgle.";
const W4D_KWAIN_TEXT: &str =
    "{T}: Each player may draw a card, then each player who drew a card this way gains 1 life.";

#[rstest]
#[case::adarkar(W4D_ADARKAR, W4D_ADARKAR_TEXT)]
#[case::apprentice(W4D_APPRENTICE, W4D_APPRENTICE_TEXT)]
#[case::kelsien(W4D_KELSIEN, W4D_KELSIEN_TEXT)]
#[case::massacre(W4D_MASSACRE, W4D_MASSACRE_TEXT)]
#[case::plasm(W4D_PLASM, W4D_PLASM_TEXT)]
#[case::puppeteer(W4D_PUPPETEER, W4D_PUPPETEER_TEXT)]
#[case::puppeteer_with_flying(
    &W4D_PUPPETEER.replace("Haste", "FirstStrike"),
    &W4D_PUPPETEER_TEXT.replace("haste", "first strike")
)]
#[case::vizkopa(W4D_VIZKOPA, W4D_VIZKOPA_TEXT)]
#[case::tempest(W4D_TEMPEST, W4D_TEMPEST_TEXT)]
#[case::feed(W4D_FEED, W4D_FEED_TEXT)]
#[case::feed_by_energy(
    &W4D_FEED.replace(r#""poison""#, r#"{"Other":"energy"}"#),
    &W4D_FEED_TEXT.replace("poison", "energy")
)]
#[case::field(W4D_FIELD, W4D_FIELD_TEXT)]
#[case::unclean(W4D_UNCLEAN, W4D_UNCLEAN_TEXT)]
#[case::kwain(W4D_KWAIN, W4D_KWAIN_TEXT)]
fn w4d_the_oracle_wording_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(
        w4d_written(json, clause),
        "{clause}\n{:#?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::adarkar_for_good(&W4D_ADARKAR.replace(r#""this_turn":true,"#, ""), W4D_ADARKAR_TEXT)]
#[case::adarkar_exiled(&W4D_ADARKAR.replace(r#""to":"Graveyard""#, r#""to":"Exile""#), W4D_ADARKAR_TEXT)]
#[case::adarkar_to_its_owner(&W4D_ADARKAR.replace(r#"{"under":"You"}"#, "{}"), W4D_ADARKAR_TEXT)]
#[case::apprentice_at_upkeep(&W4D_APPRENTICE.replace(r#""step":"End""#, r#""step":"Upkeep""#), W4D_APPRENTICE_TEXT)]
#[case::apprentice_exiles(
    &W4D_APPRENTICE.replace(r#"{"Sacrifice":{"filter":[{"Is":{"Target":0}}]}}"#, r#"{"Exile":{"what":{"Target":0}}}"#),
    W4D_APPRENTICE_TEXT
)]
#[case::apprentice_flies(&W4D_APPRENTICE.replace("Haste", "Flying"), W4D_APPRENTICE_TEXT)]
#[case::apprentice_until_end_of_turn(&W4D_APPRENTICE.replace("Forever", "UntilEndOfTurn"), W4D_APPRENTICE_TEXT)]
#[case::kelsien_for_good(&W4D_KELSIEN.replace(r#""this_turn":true,"#, ""), W4D_KELSIEN_TEXT)]
#[case::kelsien_energy(&W4D_KELSIEN.replace("experience", "energy"), W4D_KELSIEN_TEXT)]
#[case::massacre_once(&W4D_MASSACRE.replace(r#""once":false,"#, ""), W4D_MASSACRE_TEXT)]
#[case::massacre_for_good(&W4D_MASSACRE.replace(r#""this_turn":true,"#, ""), W4D_MASSACRE_TEXT)]
#[case::plasm_at_end(&W4D_PLASM.replace("Main1", "End"), W4D_PLASM_TEXT)]
#[case::plasm_any_player(&W4D_PLASM.replace(r#","whose":"You""#, ""), W4D_PLASM_TEXT)]
#[case::puppeteer_any_player(&W4D_PUPPETEER.replace(r#","whose":"You""#, ""), W4D_PUPPETEER_TEXT)]
#[case::vizkopa_for_good(&W4D_VIZKOPA.replace(r#""this_turn":true,"#, ""), W4D_VIZKOPA_TEXT)]
#[case::vizkopa_each_player(&W4D_VIZKOPA.replace(r#""who":"Opponents""#, r#""who":"EachPlayer""#), W4D_VIZKOPA_TEXT)]
#[case::tempest_opponents(&W4D_TEMPEST.replace("EachPlayer", "Opponents"), W4D_TEMPEST_TEXT)]
#[case::feed_four(&W4D_FEED.replace(r#""right":3"#, r#""right":4"#), W4D_FEED_TEXT)]
#[case::feed_at_most(&W4D_FEED.replace(r#""op":"Ge""#, r#""op":"Le""#), W4D_FEED_TEXT)]
#[case::feed_loses_two(
    &W4D_FEED.replace(r#""who":{"Bound":"opponent"},"amount":3"#, r#""who":{"Bound":"opponent"},"amount":2"#),
    W4D_FEED_TEXT
)]
#[case::field_opponents(&W4D_FIELD.replace(r#""over":"EachPlayer""#, r#""over":"Opponents""#), W4D_FIELD_TEXT)]
#[case::unclean_more_life(&W4D_UNCLEAN.replace(r#""op":"Lt""#, r#""op":"Gt""#), W4D_UNCLEAN_TEXT)]
#[case::unclean_each_player(&W4D_UNCLEAN.replace(r#""over":"Opponents""#, r#""over":"EachPlayer""#), W4D_UNCLEAN_TEXT)]
#[case::kwain_two_life(&W4D_KWAIN.replace(r#""amount":1"#, r#""amount":2"#), W4D_KWAIN_TEXT)]
#[case::kwain_opponents(&W4D_KWAIN.replace("EachPlayer", "Opponents"), W4D_KWAIN_TEXT)]
fn w4d_a_near_miss_never_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!w4d_written(json, clause), "{clause}");
}
