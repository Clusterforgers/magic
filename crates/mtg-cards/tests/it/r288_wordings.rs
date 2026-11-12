//! Goal: the writers of pack r288 word each shape as the Oracle text does (a chapter worded as its steps after "When ~
//! enters", a life total set to half the starting life total, X cards drawn from counters with X life lost if you do,
//! the evolve test as an intervening "if", a delayed trigger with targets of its own, the final chapter of a Saga, an opponent who attacks another, a tap and goad of a creature the attacked player controls, the creature with the lowest toughness returned, a life gain caused by a spell of a kind, a target creature or player) and
//! refuse the near misses.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::written_variants;
use rstest::rstest;

fn says(json: &str, text: &str) -> bool {
    let ability: Ability = serde_json::from_str(json).unwrap();
    written_variants(&ability).iter().any(|said| said == text)
}

const DISTRIBUTE: &str =
    "II \u{2014} Distribute seven +1/+1 counters among any number of target creatures you control.";
const HALF: &str = "When ~ enters, up to one target player's life total becomes half their starting life total, rounded down.";
const BEBOP: &str = "Whenever ~ deals combat damage to a player, you may draw X cards, where X is the number of counters on ~. If you do, you lose X life.";
const EVOLVING: &str = "Whenever another creature you control enters, if that creature has greater power or toughness than this creature, put an oil counter on ~.";
const HARALD: &str = "III \u{2014} Whenever an Elf you control attacks this turn, target creature an opponent controls gets -1/-1 until end of turn.";
const NARCI: &str = "Whenever the final chapter ability of a Saga you control resolves, each opponent loses X life and you gain X life, where X is that Saga's mana value.";
const BOON: &str = "Whenever the final chapter ability of a Saga you control triggers, create a 4/4 white Angel creature token with flying and vigilance.";
const KARAZIKAR: &str = "Whenever an opponent attacks another one of your opponents, you and the attacking player each draw a card and lose 1 life.";
const GOAD: &str =
    "Whenever you attack a player, tap target creature that player controls and goad it.";
const BOUNCE: &str = "Visit \u{2014} Return a creature you don't control with the lowest toughness among creatures you don't control to its owner's hand.";
const FIRESONG: &str = "Whenever a white instant or sorcery spell causes you to gain life, ~ deals 3 damage to target creature or player.";

#[rstest]
#[case::chapter_distributes(
    r#"{"Triggered":{"when":{"Chapter":{"number":2}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]},"bounds":{"min":0,"max":null},"divide":7}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":"Divided"}}]}}"#,
    DISTRIBUTE,
    true
)]
#[case::another_chapter(
    r#"{"Triggered":{"when":{"Chapter":{"number":3}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]},"bounds":{"min":0,"max":null},"divide":7}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":"Divided"}}]}}"#,
    DISTRIBUTE,
    false
)]
#[case::another_total(
    r#"{"Triggered":{"when":{"Chapter":{"number":2}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]},"bounds":{"min":0,"max":null},"divide":6}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":"Divided"}}]}}"#,
    DISTRIBUTE,
    false
)]
#[case::chapter_of_a_plain_step(
    r#"{"Triggered":{"when":{"Chapter":{"number":1}},"effects":[{"Draw":{"count":2}}]}}"#,
    "I \u{2014} Draw two cards.",
    true
)]
#[case::half_starting_life(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player","bounds":{"min":0,"max":1}}],"effects":[{"SetLife":{"who":{"Target":0},"amount":{"Half":{"of":{"StartingLife":{"Target":0}},"up":false}}}}]}}"#,
    HALF,
    true
)]
#[case::half_rounded_up(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player","bounds":{"min":0,"max":1}}],"effects":[{"SetLife":{"who":{"Target":0},"amount":{"Half":{"of":{"StartingLife":{"Target":0}},"up":true}}}}]}}"#,
    HALF,
    false
)]
#[case::exactly_one_player(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player"}],"effects":[{"SetLife":{"who":{"Target":0},"amount":{"Half":{"of":{"StartingLife":{"Target":0}},"up":false}}}}]}}"#,
    HALF,
    false
)]
#[case::half_of_your_starting_life(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player","bounds":{"min":0,"max":1}}],"effects":[{"SetLife":{"who":{"Target":0},"amount":{"Half":{"of":{"StartingLife":"You"},"up":false}}}}]}}"#,
    HALF,
    false
)]
#[case::draw_and_lose(
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"May":{"effects":[{"Draw":{"count":{"AnyCounters":"This"}}},{"LoseLife":{"amount":{"AnyCounters":"This"}}}]}}]}}"#,
    BEBOP,
    true
)]
#[case::loss_of_another_amount(
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"May":{"effects":[{"Draw":{"count":{"AnyCounters":"This"}}},{"LoseLife":{"amount":2}}]}}]}}"#,
    BEBOP,
    false
)]
#[case::draw_without_may(
    r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Draw":{"count":{"AnyCounters":"This"}}},{"LoseLife":{"amount":{"AnyCounters":"This"}}}]}}"#,
    BEBOP,
    false
)]
#[case::evolve_test(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}],"to":"Battlefield"}},"if":{"Any":[{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"Power"}},"op":"Gt","right":{"Stat":{"of":"This","stat":"Power"}}}},{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"Toughness"}},"op":"Gt","right":{"Stat":{"of":"This","stat":"Toughness"}}}}]},"effects":[{"PutCounters":{"on":"This","counter":{"Other":"oil"}}}]}}"#,
    EVOLVING,
    true
)]
#[case::power_only(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}],"to":"Battlefield"}},"if":{"Any":[{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"Power"}},"op":"Gt","right":{"Stat":{"of":"This","stat":"Power"}}}}]},"effects":[{"PutCounters":{"on":"This","counter":{"Other":"oil"}}}]}}"#,
    EVOLVING,
    false
)]
#[case::greater_or_equal(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}],"to":"Battlefield"}},"if":{"Any":[{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"Power"}},"op":"Ge","right":{"Stat":{"of":"This","stat":"Power"}}}},{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"Toughness"}},"op":"Ge","right":{"Stat":{"of":"This","stat":"Toughness"}}}}]},"effects":[{"PutCounters":{"on":"This","counter":{"Other":"oil"}}}]}}"#,
    EVOLVING,
    false
)]
#[case::spell_causes_gain(
    r#"{"Triggered":{"when":{"Caused":{"inner":{"LifeGained":{"who":"You"}},"source":[{"Color":"White"},{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}},"targets":[{"what":{"Either":["Player",{"Object":[{"Type":"Creature"}]}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#,
    FIRESONG,
    true
)]
#[case::spell_of_another_color(
    r#"{"Triggered":{"when":{"Caused":{"inner":{"LifeGained":{"who":"You"}},"source":[{"Color":"Red"},{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}},"targets":[{"what":{"Either":["Player",{"Object":[{"Type":"Creature"}]}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#,
    FIRESONG,
    false
)]
#[case::any_target_is_not_creature_or_player(
    r#"{"Triggered":{"when":{"Caused":{"inner":{"LifeGained":{"who":"You"}},"source":[{"Color":"White"},{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#,
    FIRESONG,
    false
)]
#[case::gain_by_any_cause(
    r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"targets":[{"what":{"Either":["Player",{"Object":[{"Type":"Creature"}]}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}"#,
    FIRESONG,
    false
)]
#[case::delayed_with_targets(
    r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"DelayedTrigger":{"when":{"Attacks":{"what":[{"Subtype":"Elf"},{"ControlledBy":"You"}]}},"once":false,"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
    HARALD,
    true
)]
#[case::delayed_once(
    r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"DelayedTrigger":{"when":{"Attacks":{"what":[{"Subtype":"Elf"},{"ControlledBy":"You"}]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
    HARALD,
    false
)]
#[case::delayed_until_your_next_turn(
    r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"DelayedTrigger":{"when":{"Attacks":{"what":[{"Subtype":"Elf"},{"ControlledBy":"You"}]}},"once":false,"until":"UntilYourNextTurn","targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
    HARALD,
    false
)]
#[case::final_chapter_resolves(
    r#"{"Triggered":{"when":{"FinalChapter":{"what":[{"Subtype":"Saga"},{"ControlledBy":"You"}],"resolves":true}},"effects":[{"LoseLife":{"who":"Opponents","amount":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}},{"GainLife":{"amount":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}}]}}"#,
    NARCI,
    true
)]
#[case::final_chapter_triggers_is_not_resolves(
    r#"{"Triggered":{"when":{"FinalChapter":{"what":[{"Subtype":"Saga"},{"ControlledBy":"You"}]}},"effects":[{"LoseLife":{"who":"Opponents","amount":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}},{"GainLife":{"amount":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}}]}}"#,
    NARCI,
    false
)]
#[case::drain_of_another_amount(
    r#"{"Triggered":{"when":{"FinalChapter":{"what":[{"Subtype":"Saga"},{"ControlledBy":"You"}],"resolves":true}},"effects":[{"LoseLife":{"who":"Opponents","amount":{"Stat":{"of":"Triggering","stat":"Power"}}}},{"GainLife":{"amount":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}}]}}"#,
    NARCI,
    false
)]
#[case::final_chapter_triggers(
    r#"{"Triggered":{"when":{"FinalChapter":{"what":[{"Subtype":"Saga"},{"ControlledBy":"You"}]}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Angel","colors":["White"],"types":["Creature"],"subtypes":["Angel"],"power":4,"toughness":4,"abilities":[{"Keyword":"Flying"},{"Keyword":"Vigilance"}]}}}}]}}"#,
    BOON,
    true
)]
#[case::an_opponents_saga(
    r#"{"Triggered":{"when":{"FinalChapter":{"what":[{"Subtype":"Saga"},{"ControlledBy":"Opponents"}]}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Angel","colors":["White"],"types":["Creature"],"subtypes":["Angel"],"power":4,"toughness":4,"abilities":[{"Keyword":"Flying"},{"Keyword":"Vigilance"}]}}}}]}}"#,
    BOON,
    false
)]
#[case::another_opponent_attacked(
    r#"{"Triggered":{"when":{"PlayerAttacks":{"by":"Opponents","at":"Opponents"}},"effects":[{"Draw":{}},{"Draw":{"who":"Triggering"}},{"LoseLife":{"amount":1}},{"LoseLife":{"who":"Triggering","amount":1}}]}}"#,
    KARAZIKAR,
    true
)]
#[case::you_attacked(
    r#"{"Triggered":{"when":{"PlayerAttacks":{"by":"Opponents","at":"You"}},"effects":[{"Draw":{}},{"Draw":{"who":"Triggering"}},{"LoseLife":{"amount":1}},{"LoseLife":{"who":"Triggering","amount":1}}]}}"#,
    KARAZIKAR,
    false
)]
#[case::you_lose_more(
    r#"{"Triggered":{"when":{"PlayerAttacks":{"by":"Opponents","at":"Opponents"}},"effects":[{"Draw":{}},{"Draw":{"who":"Triggering"}},{"LoseLife":{"amount":2}},{"LoseLife":{"who":"Triggering","amount":1}}]}}"#,
    KARAZIKAR,
    false
)]
#[case::tap_and_goad(
    r#"{"Triggered":{"when":{"GroupAttacksAt":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"at":"EachPlayer"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Against"}]}}],"effects":[{"Tap":{"what":{"Target":0}}},{"Goad":{"what":{"Target":0}}}]}}"#,
    GOAD,
    true
)]
#[case::goad_without_tap(
    r#"{"Triggered":{"when":{"GroupAttacksAt":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"at":"EachPlayer"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Against"}]}}],"effects":[{"Goad":{"what":{"Target":0}}}]}}"#,
    GOAD,
    false
)]
#[case::lowest_toughness(
    r#"{"Triggered":{"when":{"Visit":{}},"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}},{"LeastAmong":{"stat":"Toughness","group":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}}]},"as":"bounced"}},{"Move":{"what":{"Bound":"bounced"},"to":"Hand"}}]}}"#,
    BOUNCE,
    true
)]
#[case::lowest_power(
    r#"{"Triggered":{"when":{"Visit":{}},"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}},{"LeastAmong":{"stat":"Power","group":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}}]},"as":"bounced"}},{"Move":{"what":{"Bound":"bounced"},"to":"Hand"}}]}}"#,
    BOUNCE,
    false
)]
#[case::any_creature_you_dont_control(
    r#"{"Triggered":{"when":{"Visit":{}},"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]},"as":"bounced"}},{"Move":{"what":{"Bound":"bounced"},"to":"Hand"}}]}}"#,
    BOUNCE,
    false
)]
#[case::its_power_or_its_toughness(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"Battlefield"}},"if":{"Any":[{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"Power"}},"op":"Gt","right":{"Stat":{"of":"This","stat":"Power"}}}},{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"Toughness"}},"op":"Gt","right":{"Stat":{"of":"This","stat":"Toughness"}}}}]},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#,
    "Whenever a creature you control enters, if its power is greater than this creature's power or its toughness is greater than this creature's toughness, put a +1/+1 counter on ~.",
    true
)]
fn r288_writers_word_the_shape_and_refuse_near_misses(
    #[case] json: &str,
    #[case] text: &str,
    #[case] expected: bool,
) {
    assert_eq!(says(json, text), expected, "{json}");
}
