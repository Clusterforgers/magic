//! Goal: the round 6 trigger wordings read a printed clause back to the same rule (filter order, "the end step", the implied "you", "cycle or discard", "and" for ", then", "When", the attached permanent's names, "its controller", "a source", "it" for the card) and a rule of another meaning still reads Differs.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const PACKLEADER: &str = r#"{"Triggered":{"effects":[{"May":{"effects":[{"Draw":{}}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":[{"Type":"Creature"},{"Compare":{"op":"Ge","stat":"Power","to":3}},"Other",{"ControlledBy":"You"}]}}}}"#;
const BALL: &str = r#"{"Triggered":{"effects":[{"Sacrifice":{"filter":["This"]}}],"when":{"StepBegins":{"step":"End"}}}}"#;
const FLESHTAKER: &str = r#"{"Triggered":{"effects":[{"GainLife":{"amount":1}},{"Scry":{}}],"when":{"Sacrificed":{"by":"You","what":[{"Type":"Creature"},"Other"]}}}}"#;
const VIZIER: &str = r#"{"Triggered":{"effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}],"when":{"Discarded":{"by":"You"}}}}"#;
const BOAR: &str = r#"{"Triggered":{"effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Vigilance"}]}}],"when":{"ZoneChange":{"to":"Battlefield","what":[{"Type":"Creature"},{"Compare":{"op":"Ge","stat":"Power","to":4}},{"ControlledBy":"You"}]}}}}"#;
const GWENNA: &str = r#"{"Triggered":{"effects":[{"PutCounters":{"counter":"+1/+1","on":"This"}},{"Untap":{"what":"This"}}],"when":{"SpellCast":{"by":"You","what":[{"Type":"Creature"},{"Compare":{"op":"Ge","stat":"Power","to":5}}]}}}}"#;
const KEEPER: &str = r#"{"Triggered":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Spirit","power":1,"subtypes":["Spirit"],"toughness":1,"types":["Creature"]}}}}],"when":{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":[{"Type":"Creature"},{"ControlledBy":"You"},"Modified",{"Not":"Token"}]}}}}"#;

#[rstest]
#[case::controller_first(
    PACKLEADER,
    "Whenever another creature you control with power 3 or greater enters, you may draw a card.",
    true
)]
#[case::other_controller(&PACKLEADER.replace(r#""ControlledBy":"You""#, r#""ControlledBy":"Opponents""#), "Whenever another creature you control with power 3 or greater enters, you may draw a card.", false)]
#[case::the_end_step(
    BALL,
    "At the beginning of the end step, sacrifice this creature.",
    true
)]
#[case::your_end_step(&BALL.replace(r#""step":"End""#, r#""step":"End","whose":"You""#), "At the beginning of the end step, sacrifice this creature.", false)]
#[case::implied_you(
    FLESHTAKER,
    "Whenever you sacrifice another creature, you gain 1 life and scry 1.",
    true
)]
#[case::other_scry(&FLESHTAKER.replace(r#"{"Scry":{}}"#, r#"{"Scry":{"count":2}}"#), "Whenever you sacrifice another creature, you gain 1 life and scry 1.", false)]
#[case::cycle_or_discard(
    VIZIER,
    "Whenever you cycle or discard a card, this creature gets +1/+1 until end of turn.",
    true
)]
#[case::opponent_discards(&VIZIER.replace(r#""by":"You""#, r#""by":"Opponents""#), "Whenever you cycle or discard a card, this creature gets +1/+1 until end of turn.", false)]
#[case::until_after(
    BOAR,
    "Whenever a creature you control with power 4 or greater enters, this creature gets +1/+1 and gains vigilance until end of turn.",
    true
)]
#[case::gains_first(
    BOAR,
    "Whenever a creature you control with power 4 or greater enters, this creature gains vigilance and gets +1/+1 until end of turn.",
    true
)]
#[case::other_keyword(&BOAR.replace("Vigilance", "Trample"), "Whenever a creature you control with power 4 or greater enters, this creature gets +1/+1 and gains vigilance until end of turn.", false)]
#[case::untap_it(
    GWENNA,
    "Whenever you cast a creature spell with power 5 or greater, put a +1/+1 counter on ~ and untap it.",
    true
)]
#[case::then_and(
    GWENNA,
    "Whenever you cast a creature spell with power 5 or greater, put a +1/+1 counter on ~ and untap ~.",
    true
)]
#[case::tap_it(&GWENNA.replace("Untap", "Tap"), "Whenever you cast a creature spell with power 5 or greater, put a +1/+1 counter on ~ and untap it.", false)]
// "When" is printed for a trigger that ends the card; one that repeats keeps "Whenever".
#[case::when_repeating(
    GWENNA,
    "When you cast a creature spell with power 5 or greater, put a +1/+1 counter on ~ and untap it.",
    false
)]
#[case::when_ending(
    r#"{"Triggered":{"effects":[{"Sacrifice":{"filter":["This"]}}],"when":{"SpellCast":{"by":"You","what":[{"Type":"Creature"}]}}}}"#,
    "When you cast a creature spell, sacrifice this creature.",
    true
)]
#[case::adjectives(
    KEEPER,
    "Whenever a nontoken modified creature you control dies, create a 1/1 colorless Spirit creature token.",
    true
)]
#[case::token_too(&KEEPER.replace(r#",{"Not":"Token"}"#, ""), "Whenever a nontoken modified creature you control dies, create a 1/1 colorless Spirit creature token.", false)]
fn r6a_a_reordered_or_implied_word_reads_the_same_rule(
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

const FERTILE: &str = r#"{"Triggered":{"effects":[{"AddMana":{"mana":{"AnyColor":1},"who":"Triggering"}}],"when":{"TappedForMana":{"what":[{"Is":"Attached"}]}}}}"#;
const ARCHFIEND: &str = r#"{"Triggered":{"effects":[{"LoseLife":{"amount":2,"who":{"ControllerOf":"Triggering"}}}],"when":{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}}}"#;
const PYRELING: &str = r#"{"Triggered":{"effects":[{"Continuous":{"affected":"This","duration":"UntilEndOfTurn","mods":[{"ModifyPT":{"power":1,"toughness":0}},{"AddKeyword":"DoubleStrike"}]}}],"when":{"DealsDamage":{"combat":false,"source":[{"ControlledBy":"You"}],"to":"Opponents"}}}}"#;
const DROPLET: &str = r#"{"Triggered":{"effects":[{"PutCounters":{"count":"Triggering","counter":"charge","on":"This"}}],"when":{"Batch":{"DealsDamage":{"source":[],"to":"You"}}}}}"#;
const GROVEDANCER: &str = r#"{"Triggered":{"effects":[{"GainLife":{"amount":1}}],"when":{"ZoneChange":{"to":"Graveyard","what":[{"Type":"Land"},{"Not":"Token"},{"OwnedBy":"You"}]}}}}"#;
const ONI: &str = r#"{"Triggered":{"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"Subtype":"Ogre"},{"ControlledBy":"You"}]},"op":"Eq","right":0}},"then":[{"Discard":{}}]}}],"when":{"StepBegins":{"step":"Upkeep","whose":"You"}}}}"#;

#[rstest]
#[case::enchanted_land(
    FERTILE,
    "Whenever enchanted land is tapped for mana, its controller adds an additional one mana of any color.",
    true
)]
#[case::the_card_itself(&FERTILE.replace(r#"{"Is":"Attached"}"#, r#""This""#), "Whenever enchanted land is tapped for mana, its controller adds an additional one mana of any color.", false)]
#[case::its_controller(
    ARCHFIEND,
    "Whenever a creature an opponent controls dies, its controller loses 2 life.",
    true
)]
#[case::you_lose(&ARCHFIEND.replace(r#"{"ControllerOf":"Triggering"}"#, r#""You""#), "Whenever a creature an opponent controls dies, its controller loses 2 life.", false)]
#[case::a_source(
    PYRELING,
    "Whenever a source you control deals noncombat damage to an opponent, this creature gets +1/+0 and gains double strike until end of turn.",
    true
)]
#[case::creature_source(&PYRELING.replace(r#"[{"ControlledBy":"You"}]"#, r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#), "Whenever a source you control deals noncombat damage to an opponent, this creature gets +1/+0 and gains double strike until end of turn.", false)]
#[case::dealt_damage(
    DROPLET,
    "Whenever you're dealt damage, put that many charge counters on this artifact.",
    true
)]
#[case::by_creatures(&DROPLET.replace(r#""source":[]"#, r#""source":[{"Type":"Creature"}]"#), "Whenever you're dealt damage, put that many charge counters on this artifact.", false)]
#[case::land_card(
    GROVEDANCER,
    "Whenever a land card is put into your graveyard from anywhere, you gain 1 life.",
    true
)]
#[case::opponents_land(&GROVEDANCER.replace(r#""OwnedBy":"You""#, r#""OwnedBy":"Opponents""#), "Whenever a land card is put into your graveyard from anywhere, you gain 1 life.", false)]
#[case::no_ogre(
    ONI,
    "At the beginning of your upkeep, discard a card if you don't control an Ogre.",
    true
)]
#[case::one_ogre(&ONI.replace(r#""right":0"#, r#""right":1"#), "At the beginning of your upkeep, discard a card if you don't control an Ogre.", false)]
fn r6a_a_named_object_or_player_reads_the_same_rule(
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
