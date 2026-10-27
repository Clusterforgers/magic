//! Goal: the writers of the proposal checker give the oracle wording of the fourth sample of the names M to R (sizes of twice a number, foretold steps, copies returned, switches of a group, tokens that copy the permanent of an event, counters checked after they were put, activation limits and free casts), and a wrong number, object, player, kind or step never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// "Whenever enchanted creature attacks, its controller loses 2 life and you gain 2 life."
const PARASITIC_IMPETUS: &str = r#"{"Triggered":{"when":{"Attacks":{"what":[{"Is":"Attached"}]}},"effects":[{"LoseLife":{"who":{"ControllerOf":"Triggering"},"amount":2}},{"GainLife":{"amount":2}}]}}"#;

/// "At the beginning of your upkeep, put a +1/+1 counter on equipped creature if it's white."
const RING_OF_THUNE: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"If":{"condition":{"Matches":{"subject":"Attached","selector":[{"Color":"White"}]}},"then":[{"PutCounters":{"on":"Attached","counter":"+1/+1"}}]}}]}}"#;

/// "When ~ deals combat damage to a player, sacrifice it and create that many Treasure tokens."
const RMS_TITANIC: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Sacrifice":{"filter":["This"]}},{"CreateToken":{"token":{"Named":"treasure"},"count":"Triggering"}}]}}"#;

/// "At the beginning of each upkeep, if you don't control a Pest creature token, create a 1/1 black and green Pest creature token with "When this token dies, you gain 1 life.""
const PEST_RESCUER: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep"}},"if":{"Compare":{"left":{"Count":[{"Type":"Creature"},"Token",{"Subtype":"Pest"},{"ControlledBy":"You"}]},"op":"Eq","right":0}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Pest","colors":["Black","Green"],"types":["Creature"],"subtypes":["Pest"],"power":1,"toughness":1,"abilities":[{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"GainLife":{"amount":1}}]}}]}}}}]}}"#;

/// "Whenever ~ deals combat damage to a player, choose any number —\n• Each player discards a card.\n• Each player loses 1 life and draws a card.\n• Each player sacrifices a creature of their choice."
const RANKLE: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Modal":{"choose":{"min":0,"max":3},"modes":[{"effects":[{"Discard":{"who":"EachPlayer"}}]},{"effects":[{"LoseLife":{"who":"EachPlayer","amount":1}},{"Draw":{"who":"EachPlayer"}}]},{"effects":[{"Sacrifice":{"who":"EachPlayer","filter":[{"Type":"Creature"}]}}]}]}}]}}"#;

/// "Choose one —\n• Counter all other spells.\n• Switch each creature's power and toughness until end of turn.\n• Creatures can't be blocked this turn."
const REVERSE_THE_POLARITY: &str = r#"{"Spell":{"effects":[{"Modal":{"modes":[{"effects":[{"Counter":{"what":{"Each":["Spell","Other"]}}}]},{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":["SwitchPT"],"duration":"UntilEndOfTurn"}}]},{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":["CantBeBlocked"],"duration":"UntilEndOfTurn"}}]}]}}]}}"#;

/// "Each creature gets twice -X/-X until end of turn. Each player gets X rad counters."
const NUCLEAR_FALLOUT: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":{"Times":{"by":-2,"of":"X"}},"toughness":{"Times":{"by":-2,"of":"X"}}}}],"duration":"UntilEndOfTurn"}},{"PutCounters":{"on":"EachPlayer","counter":{"Other":"rad"},"count":"X"}}]}}"#;

/// "Destroy target creature. If this spell was foretold, scry 2."
const POISON_THE_CUP: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"If":{"condition":{"CastWith":"Foretold"},"then":[{"Scry":{"count":2}}]}}]}}"#;

/// "You draw X cards and lose X life, where X is the number of colors of mana spent to cast this spell."
const PAINFUL_TRUTHS: &str = r#"{"Spell":{"effects":[{"Draw":{"count":"ColorsSpent"}},{"LoseLife":{"amount":"ColorsSpent"}}]}}"#;

/// "Copy target instant or sorcery spell, then return it to its owner's hand. You may choose new targets for the copy."
const NARSETS_REVERSAL: &str = r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}],"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}},{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#;

/// "{2}, Return ~ to its owner's hand: Copy target instant or sorcery spell you control. You may choose new targets for the copy."
const ROOTHA: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},{"ReturnToHand":{"filter":["This"]}}],"targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"ControlledBy":"You"}]}}],"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}]}}"#;

/// "Whenever an enchantment you control enters, you may create a token that's a copy of it. Do this only once each turn."
const ONDU: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Enchantment"},{"ControlledBy":"You"}],"to":"Battlefield"}},"limit":"OncePerTurn","effects":[{"May":{"effects":[{"CreateToken":{"token":{"CopyOf":{"of":"Triggering"}}}}]}}]}}"#;

/// "At the beginning of your first main phase, you may discard two cards. If you do, return this card from your graveyard to your hand."
const OLD_ONE_EYE: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Main1","whose":"You"}},"from":"Graveyard","effects":[{"Pay":{"cost":[{"Discard":{"count":2}}],"paid":[{"Move":{"what":"This","to":"Hand"}}]}}]}}"#;

/// "Whenever you gain life, put that many ribbon counters on this creature. Then if there are three or more ribbon counters on this creature, remove those counters and untap it."
const PRIZE_PIG: &str = r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"effects":[{"PutCounters":{"on":"This","counter":{"Other":"ribbon"},"count":"Triggering"}},{"If":{"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"ribbon"}}},"op":"Ge","right":3}},"then":[{"RemoveCounters":{"from":"This","counter":{"Other":"ribbon"},"count":{"Counters":{"on":"This","counter":{"Other":"ribbon"}}}}},{"Untap":{"what":"This"}}]}}]}}"#;

/// "Whenever ~ attacks, if you control three or more Dragons, until end of turn, ~ becomes a Dragon with base power and toughness 5/5 and gains flying."
const NOGI: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"if":{"Compare":{"left":{"Count":[{"Subtype":"Dragon"},{"ControlledBy":"You"}]},"op":"Ge","right":3}},"effects":[{"Continuous":{"affected":"This","mods":[{"SetCreatureTypes":["Dragon"]},{"SetPT":{"power":5,"toughness":5}},{"AddKeyword":"Flying"}],"duration":"UntilEndOfTurn"}}]}}"#;

/// "You draw cards equal to the sacrificed creature's power, then you gain life equal to its toughness."
const MOMENTOUS_FALL: &str = r#"{"Spell":{"effects":[{"Draw":{"count":{"Stat":{"of":"Paid","stat":"Power"}}}},{"GainLife":{"amount":{"Stat":{"of":"Paid","stat":"Toughness"}}}}]}}"#;

/// "Whenever ~ becomes tapped, each opponent who has three or more poison counters loses 1 life."
const PHYREXIAN_ATLAS: &str = r#"{"Triggered":{"when":{"Tapped":{"what":["This"]}},"effects":[{"RepeatEach":{"over":"Opponents","as":"opp","effects":[{"If":{"condition":{"Compare":{"left":{"Counters":{"on":{"Bound":"opp"},"counter":"poison"}},"op":"Ge","right":3}},"then":[{"LoseLife":{"who":{"Bound":"opp"},"amount":1}}]}}]}}]}}"#;

/// "Draw cards equal to the number of cards in target opponent's hand."
const RECURRING_INSIGHT: &str = r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"Draw":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}}]}}"#;

/// "Sacrifice ~: Draw a card. Activate only if an opponent has eight or more cards in their graveyard."
const WINDROBBER: &str = r#"{"Activated":{"cost":["SacrificeSelf"],"condition":{"Compare":{"left":{"Largest":{"who":"Opponents","of":[{"In":"Graveyard"}]}},"op":"Ge","right":8}},"effects":[{"Draw":{}}]}}"#;

/// "You may cast target instant or sorcery card from an opponent's graveyard without paying its mana cost."
const MEMORY_PLUNDER: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"Opponents"},{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}],"effects":[{"CastFromZone":{"what":{"Target":0},"free":true}}]}}"#;

/// "Shuffle your library. Then exile the top card of your library. Until end of turn, you may play that card without paying its mana cost."
const MINDS_DESIRE: &str = r#"{"Spell":{"effects":[{"Shuffle":{}},{"ExileTop":{"as":"card"}},{"CastFromZone":{"what":{"Bound":"card"},"free":true,"play":true}}]}}"#;

/// "At the beginning of your upkeep, if your devotion to white and black is seven or greater, create a 4/3 white and black Vampire Demon creature token with flying."
const MARCH_OF_THE_CANONIZED: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Devotion":["White","Black"]},"op":"Ge","right":7}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Vampire Demon","colors":["White","Black"],"types":["Creature"],"subtypes":["Vampire","Demon"],"power":4,"toughness":3,"abilities":[{"Keyword":"Flying"}]}}}}]}}"#;

/// "Whenever a creature an opponent controls dies, if its toughness was less than 1, draw a card."
const MASSACRE_GIRL: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"Opponents"}],"from":"Battlefield","to":"Graveyard"}},"if":{"Matches":{"subject":"Triggering","selector":[{"Compare":{"stat":"Toughness","op":"Lt","to":1}}]}},"effects":[{"Draw":{}}]}}"#;

/// "At the beginning of your upkeep, if ~ is in the command zone, you gain 2 life."
const OLORO: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"from":"Command","effects":[{"GainLife":{"amount":2}}]}}"#;

/// "Whenever a creature you control enters, it explores."
const PATH_OF_DISCOVERY: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"Explore":{"what":"Triggering"}}]}}"#;

/// "Destroy target creature. Its controller loses life equal to its power plus its toughness."
const PHTHISIS: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"LoseLife":{"who":{"ControllerOf":{"Target":0}},"amount":{"Sum":[{"Stat":{"of":{"Target":0},"stat":"Power"}},{"Stat":{"of":{"Target":0},"stat":"Toughness"}}]}}}]}}"#;

/// "Whenever another nontoken artifact you control enters, you may pay {2}. If you do, create a token that's a copy of that artifact."
const MIRRORWORKS: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Artifact"},"Other",{"Not":"Token"},{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"Pay":{"cost":"{2}","paid":[{"CreateToken":{"token":{"CopyOf":{"of":"Triggering"}}}}]}}]}}"#;

#[rstest]
#[case::its_controller(
    PARASITIC_IMPETUS,
    "Whenever enchanted creature attacks, its controller loses 2 life and you gain 2 life."
)]
#[case::equipped_creature(
    RING_OF_THUNE,
    "At the beginning of your upkeep, put a +1/+1 counter on equipped creature if it's white."
)]
#[case::when_it_sacrifices(
    RMS_TITANIC,
    "When ~ deals combat damage to a player, sacrifice it and create that many Treasure tokens."
)]
#[case::token_quote(
    PEST_RESCUER,
    "At the beginning of each upkeep, if you don't control a Pest creature token, create a 1/1 black and green Pest creature token with \"When this token dies, you gain 1 life.\""
)]
#[case::modal_any_number(
    RANKLE,
    "Whenever ~ deals combat damage to a player, choose any number —\n• Each player discards a card.\n• Each player loses 1 life and draws a card.\n• Each player sacrifices a creature of their choice."
)]
#[case::switch_each(
    REVERSE_THE_POLARITY,
    "Choose one —\n• Counter all other spells.\n• Switch each creature's power and toughness until end of turn.\n• Creatures can't be blocked this turn."
)]
#[case::twice_x(
    NUCLEAR_FALLOUT,
    "Each creature gets twice -X/-X until end of turn. Each player gets X rad counters."
)]
#[case::foretold(
    POISON_THE_CUP,
    "Destroy target creature. If this spell was foretold, scry 2."
)]
#[case::converge_draw(
    PAINFUL_TRUTHS,
    "You draw X cards and lose X life, where X is the number of colors of mana spent to cast this spell."
)]
#[case::copy_returned(
    NARSETS_REVERSAL,
    "Copy target instant or sorcery spell, then return it to its owner's hand. You may choose new targets for the copy."
)]
#[case::return_self(
    ROOTHA,
    "{2}, Return ~ to its owner's hand: Copy target instant or sorcery spell you control. You may choose new targets for the copy."
)]
#[case::copy_it(
    ONDU,
    "Whenever an enchantment you control enters, you may create a token that's a copy of it. Do this only once each turn."
)]
#[case::graveyard_return(
    OLD_ONE_EYE,
    "At the beginning of your first main phase, you may discard two cards. If you do, return this card from your graveyard to your hand."
)]
#[case::counter_check(
    PRIZE_PIG,
    "Whenever you gain life, put that many ribbon counters on this creature. Then if there are three or more ribbon counters on this creature, remove those counters and untap it."
)]
#[case::until_first(
    NOGI,
    "Whenever ~ attacks, if you control three or more Dragons, until end of turn, ~ becomes a Dragon with base power and toughness 5/5 and gains flying."
)]
#[case::sacrificed_stats(
    MOMENTOUS_FALL,
    "You draw cards equal to the sacrificed creature's power, then you gain life equal to its toughness."
)]
#[case::poison_loss(
    PHYREXIAN_ATLAS,
    "Whenever ~ becomes tapped, each opponent who has three or more poison counters loses 1 life."
)]
#[case::hand_size_draw(
    RECURRING_INSIGHT,
    "Draw cards equal to the number of cards in target opponent's hand."
)]
#[case::graveyard_limit(
    WINDROBBER,
    "Sacrifice ~: Draw a card. Activate only if an opponent has eight or more cards in their graveyard."
)]
#[case::free_cast_target(
    MEMORY_PLUNDER,
    "You may cast target instant or sorcery card from an opponent's graveyard without paying its mana cost."
)]
#[case::free_play_exiled(
    MINDS_DESIRE,
    "Shuffle your library. Then exile the top card of your library. Until end of turn, you may play that card without paying its mana cost."
)]
#[case::devotion_if(
    MARCH_OF_THE_CANONIZED,
    "At the beginning of your upkeep, if your devotion to white and black is seven or greater, create a 4/3 white and black Vampire Demon creature token with flying."
)]
#[case::last_known(
    MASSACRE_GIRL,
    "Whenever a creature an opponent controls dies, if its toughness was less than 1, draw a card."
)]
#[case::command_zone(
    OLORO,
    "At the beginning of your upkeep, if ~ is in the command zone, you gain 2 life."
)]
#[case::explores_it(
    PATH_OF_DISCOVERY,
    "Whenever a creature you control enters, it explores."
)]
#[case::power_plus_toughness(
    PHTHISIS,
    "Destroy target creature. Its controller loses life equal to its power plus its toughness."
)]
#[case::copy_that_artifact(
    MIRRORWORKS,
    "Whenever another nontoken artifact you control enters, you may pay {2}. If you do, create a token that's a copy of that artifact."
)]
fn a_card_says_it_the_way_the_text_does(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::its_controller_other(
    PARASITIC_IMPETUS,
    "Whenever enchanted creature attacks, its controller loses 3 life and you gain 2 life."
)]
#[case::equipped_creature_other(
    RING_OF_THUNE,
    "At the beginning of your upkeep, put a +1/+1 counter on equipped creature if it's red."
)]
#[case::when_it_sacrifices_other(
    RMS_TITANIC,
    "When ~ deals combat damage to a player, sacrifice it and create a Treasure token."
)]
#[case::token_quote_other(
    PEST_RESCUER,
    "At the beginning of each upkeep, if you don't control a Pest creature token, create a 1/1 black and green Pest creature token with \"When this token dies, you gain 2 life.\""
)]
#[case::modal_any_number_other(
    RANKLE,
    "Whenever ~ deals combat damage to a player, choose any number —\n• Each player discards a card.\n• Each player loses 2 life and draws a card.\n• Each player sacrifices a creature of their choice."
)]
#[case::switch_each_other(
    REVERSE_THE_POLARITY,
    "Choose one —\n• Counter all other spells.\n• Switch each creature's power and toughness until end of turn.\n• Creatures can't be blocked until end of turn."
)]
#[case::twice_x_other(
    NUCLEAR_FALLOUT,
    "Each creature gets thrice -X/-X until end of turn. Each player gets X rad counters."
)]
#[case::foretold_other(
    POISON_THE_CUP,
    "Destroy target creature. If this spell was foretold, scry 3."
)]
#[case::converge_draw_other(
    PAINFUL_TRUTHS,
    "You draw X cards and gain X life, where X is the number of colors of mana spent to cast this spell."
)]
#[case::copy_returned_other(
    NARSETS_REVERSAL,
    "Copy target instant or sorcery spell, then return it to your hand. You may choose new targets for the copy."
)]
#[case::return_self_other(
    ROOTHA,
    "{2}, Return another creature you control to its owner's hand: Copy target instant or sorcery spell you control. You may choose new targets for the copy."
)]
#[case::copy_it_other(
    ONDU,
    "Whenever an artifact you control enters, you may create a token that's a copy of it. Do this only once each turn."
)]
#[case::graveyard_return_other(
    OLD_ONE_EYE,
    "At the beginning of your first main phase, you may discard three cards. If you do, return this card from your graveyard to your hand."
)]
#[case::counter_check_other(
    PRIZE_PIG,
    "Whenever you gain life, put that many ribbon counters on this creature. Then if there are four or more ribbon counters on this creature, remove those counters and untap it."
)]
#[case::until_first_other(
    NOGI,
    "Whenever ~ attacks, if you control two or more Dragons, until end of turn, ~ becomes a Dragon with base power and toughness 5/5 and gains flying."
)]
#[case::sacrificed_stats_other(
    MOMENTOUS_FALL,
    "You draw cards equal to the sacrificed creature's toughness, then you gain life equal to its power."
)]
#[case::poison_loss_other(
    PHYREXIAN_ATLAS,
    "Whenever ~ becomes tapped, each opponent who has four or more poison counters loses 1 life."
)]
#[case::hand_size_draw_other(
    RECURRING_INSIGHT,
    "Draw cards equal to the number of cards in target player's hand."
)]
#[case::graveyard_limit_other(
    WINDROBBER,
    "Sacrifice ~: Draw a card. Activate only if an opponent has seven or more cards in their graveyard."
)]
#[case::free_cast_target_other(
    MEMORY_PLUNDER,
    "You may cast target instant or sorcery card from your graveyard without paying its mana cost."
)]
#[case::free_play_exiled_other(
    MINDS_DESIRE,
    "Shuffle your library. Then exile the top card of your library. Until end of turn, you may play that card."
)]
#[case::devotion_if_other(
    MARCH_OF_THE_CANONIZED,
    "At the beginning of your upkeep, if your devotion to white and black is six or greater, create a 4/3 white and black Vampire Demon creature token with flying."
)]
#[case::last_known_other(
    MASSACRE_GIRL,
    "Whenever a creature an opponent controls dies, if its toughness was less than 2, draw a card."
)]
#[case::command_zone_other(
    OLORO,
    "At the beginning of your upkeep, if ~ is in the graveyard, you gain 2 life."
)]
#[case::explores_it_other(
    PATH_OF_DISCOVERY,
    "Whenever a creature an opponent controls enters, it explores."
)]
#[case::power_plus_toughness_other(
    PHTHISIS,
    "Destroy target creature. Its controller loses life equal to its power plus its power."
)]
#[case::copy_that_artifact_other(
    MIRRORWORKS,
    "Whenever another nontoken artifact you control enters, you may pay {2}. If you do, create a token that's a copy of that land."
)]
fn a_wrong_number_object_player_or_step_never_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}
