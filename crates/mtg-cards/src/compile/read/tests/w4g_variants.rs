//! Goal: the r3w7 writers give the printed wording of an ability the renderers write in other words (a quote that ends the sentence, a keyword list before a quote, a token's name before its quote, "a number of ... equal to", "each creature with a counter on it", "When you sacrifice", "it deals", "they control", "an amount of {C}", "Double all damage"); a changed number, kind, player, zone or condition never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn w4g_written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const W4G_NECROPLASM: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Eq","to":{"Counters":{"on":"This","counter":"+1/+1"}}}}]}}}]}}"#;
const W4G_ORDEAL_ATTACKS: &str = r#"{"Triggered":{"when":{"Attacks":{"what":[{"Is":"Attached"}]}},"effects":[{"PutCounters":{"on":"Attached","counter":"+1/+1"}},{"If":{"condition":{"Compare":{"left":{"Counters":{"on":"Attached","counter":"+1/+1"}},"op":"Ge","right":3}},"then":[{"Sacrifice":{"filter":["This"]}}]}}]}}"#;
const W4G_ORDEAL_SACRIFICED: &str = r#"{"Triggered":{"when":{"Sacrificed":{"what":["This"],"by":"You"}},"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"count":{"min":0,"max":2},"as":"lands"}},{"Move":{"what":{"Bound":"lands"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#;
const W4G_ORIMS_THUNDER: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]}]}},{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"If":{"condition":"Kicked","then":[{"DealDamage":{"to":{"Target":1},"amount":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}]}}]}}"#;
const W4G_CONSTRICTOR: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"Opponents"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Triggering"}]},"bounds":{"min":0,"max":1}}],"effects":[{"LoseLife":{"who":"Triggering","amount":1}},{"PutCounters":{"on":{"Target":0},"counter":"-1/-1"}}]}}"#;
const W4G_PINK_HORROR: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Blue Horror","colors":["Blue","Red"],"types":["Creature"],"subtypes":["Demon","Horror"],"power":2,"toughness":2,"abilities":[{"Triggered":{"when":{"SpellCast":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"by":"You"}},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]}}]}},"count":2}}]}}"#;
const W4G_RAPHAEL: &str = r#"{"Replacement":{"when":{"Damage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"},"HasAnyCounter"]}},"by":{"Multiply":2}}}"#;
const W4G_FIREPLACE: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]},"times":{"Counters":{"on":"This","counter":{"Other":"time"}}}}}]}}"#;
const W4G_DOCTOR: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"Untap":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},"HasAnyCounter"]}}}]}}"#;
const W4G_WAKE_THE_DRAGON: &str = r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Dragon","colors":["Black","Red"],"types":["Creature"],"subtypes":["Dragon"],"power":6,"toughness":6,"abilities":[{"Keyword":"Flying"},{"Keyword":"Menace"},{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"targets":[{"what":{"Object":[{"Type":"Artifact"},{"ControlledBy":"Triggering"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"Forever"}}]}}]}}}}]}}"#;
const W4G_WHITE_SUN: &str = r#"{"Spell":{"effects":[{"GainLife":{"amount":"X"}},{"CreateToken":{"token":{"Custom":{"name":"Phyrexian Mite","types":["Artifact","Creature"],"subtypes":["Phyrexian","Mite"],"power":1,"toughness":1,"abilities":[{"Keyword":{"Toxic":1}},{"Static":{"affected":"This","mods":["CantBlock"]}}]}},"count":"X"}},{"If":{"condition":{"Compare":{"left":"X","op":"Ge","right":5}},"then":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},"Other"]}}}]}}]}}"#;
const W4G_MARDU: &str = r#"{"Spell":{"effects":[{"Modal":{"all_if":{"Compare":{"left":{"Count":["Commander",{"ControlledBy":"You"}]},"op":"Ge","right":1}},"modes":[{"targets":[{"what":"Player"}],"effects":[{"CreateToken":{"token":{"Custom":{"name":"Warrior","colors":["Red"],"types":["Creature"],"subtypes":["Warrior"],"power":1,"toughness":1}},"count":{"Count":[{"Type":"Creature"},{"ControlledBy":{"Target":0}}]}}}]},{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}]}]}}]}}"#;
const W4G_SCRIPTURES: &str = r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"Exile":{"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":"Opponents"}]}}}]}}"#;

const W4G_NECROPLASM_TEXT: &str = "At the beginning of your end step, destroy each creature with mana value equal to the number of +1/+1 counters on this creature.";
const W4G_ORDEAL_ATTACKS_TEXT: &str = "Whenever enchanted creature attacks, put a +1/+1 counter on it. Then if it has three or more +1/+1 counters on it, sacrifice this Aura.";
const W4G_ORDEAL_SACRIFICED_TEXT: &str = "When you sacrifice this Aura, search your library for up to two basic land cards, put them onto the battlefield tapped, then shuffle.";
const W4G_ORIMS_TEXT: &str = "Destroy target artifact or enchantment. If this spell was kicked, it deals damage equal to that permanent's mana value to target creature.";
const W4G_CONSTRICTOR_TEXT: &str = "At the beginning of each opponent's upkeep, they lose 1 life and you put a -1/-1 counter on up to one target creature they control.";
const W4G_PINK_TEXT: &str = "When this creature dies, create two 2/2 blue and red Demon Horror creature tokens named Blue Horror with \"Whenever you cast an instant or sorcery spell, this token deals 1 damage to any target.\"";
const W4G_RAPHAEL_TEXT: &str =
    "Double all damage that creatures you control with counters on them would deal.";
const W4G_FIREPLACE_TEXT: &str =
    "{T}: Add an amount of {C} equal to the number of time counters on this artifact.";
const W4G_DOCTOR_TEXT: &str =
    "At the beginning of your end step, untap each creature you control with a counter on it.";
const W4G_WAKE_TEXT: &str = "Create a 6/6 black and red Dragon creature token with flying, menace, and \"Whenever this token deals combat damage to a player, gain control of target artifact that player controls.\"";
const W4G_WHITE_SUN_TEXT: &str = "You gain X life. Create X 1/1 colorless Phyrexian Mite artifact creature tokens with toxic 1 and \"This token can't block.\" If X is 5 or more, destroy all other creatures.";
const W4G_MARDU_TEXT: &str = "Choose one. If you control a commander as you cast this spell, you may choose both instead.\n• Create a number of 1/1 red Warrior creature tokens equal to the number of creatures target player controls.\n• ~ deals damage to target creature equal to the number of creatures you control.";
const W4G_SCRIPTURES_TEXT: &str = "III — Exile all opponents' graveyards.";

#[rstest]
#[case::necroplasm(W4G_NECROPLASM, W4G_NECROPLASM_TEXT)]
#[case::necroplasm_all(
    W4G_NECROPLASM,
    "At the beginning of your end step, destroy all creatures with mana value equal to the number of +1/+1 counters on ~."
)]
#[case::ordeal_attacks(W4G_ORDEAL_ATTACKS, W4G_ORDEAL_ATTACKS_TEXT)]
#[case::ordeal_sacrificed(W4G_ORDEAL_SACRIFICED, W4G_ORDEAL_SACRIFICED_TEXT)]
#[case::orims_thunder(W4G_ORIMS_THUNDER, W4G_ORIMS_TEXT)]
#[case::constrictor(W4G_CONSTRICTOR, W4G_CONSTRICTOR_TEXT)]
#[case::pink_horror(W4G_PINK_HORROR, W4G_PINK_TEXT)]
#[case::raphael(W4G_RAPHAEL, W4G_RAPHAEL_TEXT)]
#[case::raphael_combat(
    &W4G_RAPHAEL.replace(r#""HasAnyCounter"]"#, r#""HasAnyCounter"],"combat":true"#),
    "Double all combat damage that creatures you control with counters on them would deal."
)]
#[case::fireplace(W4G_FIREPLACE, W4G_FIREPLACE_TEXT)]
#[case::doctor(W4G_DOCTOR, W4G_DOCTOR_TEXT)]
#[case::wake_the_dragon(W4G_WAKE_THE_DRAGON, W4G_WAKE_TEXT)]
#[case::white_sun(W4G_WHITE_SUN, W4G_WHITE_SUN_TEXT)]
#[case::mardu(W4G_MARDU, W4G_MARDU_TEXT)]
#[case::scriptures(W4G_SCRIPTURES, W4G_SCRIPTURES_TEXT)]
fn w4g_the_printed_wording_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(
        w4g_written(json, clause),
        "{clause}\n{:#?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::necroplasm_by_another_counter(&W4G_NECROPLASM.replace("+1/+1", "-1/-1"), W4G_NECROPLASM_TEXT)]
#[case::necroplasm_by_a_greater_value(&W4G_NECROPLASM.replace(r#""op":"Eq""#, r#""op":"Ge""#), W4G_NECROPLASM_TEXT)]
#[case::ordeal_at_two(&W4G_ORDEAL_ATTACKS.replace(r#""right":3"#, r#""right":2"#), W4G_ORDEAL_ATTACKS_TEXT)]
#[case::ordeal_on_itself(&W4G_ORDEAL_ATTACKS.replace(r#"{"on":"Attached","counter":"+1/+1"}}"#, r#"{"on":"This","counter":"+1/+1"}}"#), W4G_ORDEAL_ATTACKS_TEXT)]
#[case::ordeal_another_sacrificed(&W4G_ORDEAL_SACRIFICED.replace(r#"["This"]"#, r#"[{"Type":"Artifact"}]"#), W4G_ORDEAL_SACRIFICED_TEXT)]
#[case::ordeal_three_lands(&W4G_ORDEAL_SACRIFICED.replace(r#""max":2"#, r#""max":3"#), W4G_ORDEAL_SACRIFICED_TEXT)]
#[case::orims_by_power(&W4G_ORIMS_THUNDER.replace("ManaValue", "Power"), W4G_ORIMS_TEXT)]
#[case::orims_to_a_player(&W4G_ORIMS_THUNDER.replace(r#"{"what":{"Object":[{"Type":"Creature"}]}}"#, r#"{"what":"Player"}"#), W4G_ORIMS_TEXT)]
#[case::constrictor_your_creature(&W4G_CONSTRICTOR.replace(r#"{"ControlledBy":"Triggering"}"#, r#"{"ControlledBy":"You"}"#), W4G_CONSTRICTOR_TEXT)]
#[case::constrictor_two_life(&W4G_CONSTRICTOR.replace(r#""amount":1"#, r#""amount":2"#), W4G_CONSTRICTOR_TEXT)]
#[case::pink_another_name(&W4G_PINK_HORROR.replace("Blue Horror", "Pink Horror"), W4G_PINK_TEXT)]
#[case::pink_two_damage(&W4G_PINK_HORROR.replace(r#""amount":1"#, r#""amount":2"#), W4G_PINK_TEXT)]
#[case::pink_one_token(&W4G_PINK_HORROR.replace(r#","count":2"#, ""), W4G_PINK_TEXT)]
#[case::raphael_tripled(&W4G_RAPHAEL.replace(r#""Multiply":2"#, r#""Multiply":3"#), W4G_RAPHAEL_TEXT)]
#[case::raphael_to_you(&W4G_RAPHAEL.replace(r#""HasAnyCounter"]"#, r#""HasAnyCounter"],"to":"You""#), W4G_RAPHAEL_TEXT)]
#[case::raphael_without_counters(&W4G_RAPHAEL.replace(r#","HasAnyCounter""#, ""), W4G_RAPHAEL_TEXT)]
#[case::raphael_noncombat(&W4G_RAPHAEL.replace(r#""HasAnyCounter"]"#, r#""HasAnyCounter"],"combat":false"#), W4G_RAPHAEL_TEXT)]
#[case::fireplace_green(&W4G_FIREPLACE.replace("Colorless", "Green"), W4G_FIREPLACE_TEXT)]
#[case::fireplace_other_counter(&W4G_FIREPLACE.replace(r#""time""#, r#""fade""#), W4G_FIREPLACE_TEXT)]
#[case::doctor_opponents_creatures(&W4G_DOCTOR.replace(r#"{"ControlledBy":"You"}"#, r#"{"ControlledBy":"Opponents"}"#), W4G_DOCTOR_TEXT)]
#[case::doctor_every_creature(&W4G_DOCTOR.replace(r#","HasAnyCounter""#, ""), W4G_DOCTOR_TEXT)]
#[case::wake_without_menace(&W4G_WAKE_THE_DRAGON.replace(r#"{"Keyword":"Menace"},"#, ""), W4G_WAKE_TEXT)]
#[case::wake_any_damage(&W4G_WAKE_THE_DRAGON.replace(r#","combat":true"#, ""), W4G_WAKE_TEXT)]
#[case::white_sun_at_four(&W4G_WHITE_SUN.replace(r#""right":5"#, r#""right":4"#), W4G_WHITE_SUN_TEXT)]
#[case::white_sun_toxic_two(&W4G_WHITE_SUN.replace(r#""Toxic":1"#, r#""Toxic":2"#), W4G_WHITE_SUN_TEXT)]
#[case::mardu_your_creatures(&W4G_MARDU.replace(r#"{"ControlledBy":{"Target":0}}"#, r#"{"ControlledBy":"You"}"#), W4G_MARDU_TEXT)]
#[case::mardu_two_power(&W4G_MARDU.replace(r#""power":1,"#, r#""power":2,"#), W4G_MARDU_TEXT)]
#[case::scriptures_your_graveyard(&W4G_SCRIPTURES.replace(r#""Opponents""#, r#""You""#), W4G_SCRIPTURES_TEXT)]
fn w4g_a_near_miss_never_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(
        !w4g_written(json, clause),
        "{clause}\n{:#?}",
        written_variants(&ability(json))
    );
}
