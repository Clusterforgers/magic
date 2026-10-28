//! Goal: the w5a writers give the oracle wording of the trigger heads the other writers do not word; another card type, player, subject, counter kind, zone, recipient, amount or event never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn w5a_written(json: &str, clause: &str) -> bool {
    let variants = written_variants(&ability(json));
    variants.iter().any(|wording| same_text(wording, clause))
}

const W5A_WASTE_CREATURE: &str = r#"{"Triggered":{"when":{"Discarded":{"what":[{"Type":"Creature"}],"by":"Opponents"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Zombie","colors":["Black"],"types":["Creature"],"subtypes":["Zombie"],"power":2,"toughness":2}}}}]}}"#;
const W5A_WASTE_LAND: &str = r#"{"Triggered":{"when":{"Discarded":{"what":[{"Type":"Land"}],"by":"Opponents"}},"effects":[{"AddMana":{"mana":{"Fixed":["Black","Black"]}}}]}}"#;
const W5A_WASTE_OTHER: &str = r#"{"Triggered":{"when":{"Discarded":{"what":[{"Not":{"Type":"Creature"}},{"Not":{"Type":"Land"}}],"by":"Opponents"}},"effects":[{"Draw":{}}]}}"#;
const W5A_GAHIJI: &str = r#"{"Triggered":{"when":{"AttacksPlayerOrPlaneswalker":{"what":[{"Type":"Creature"}],"at":"Opponents"}},"effects":[{"Continuous":{"affected":"Triggering","mods":[{"ModifyPT":{"power":2,"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#;
const W5A_DECKHAND: &str = r#"{"Triggered":{"when":{"BecomesTarget":{"what":["This"],"by":["Spell"]}},"effects":[{"Sacrifice":{"filter":["This"]}}]}}"#;
const W5A_VIPER: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":{"Each":[{"Type":"Creature"}]},"combat":true}},"effects":[{"DelayedTrigger":{"when":{"StepBegins":{"step":"EndCombat"}},"effects":[{"Destroy":{"what":"Triggering"}}]}}]}}"#;
const W5A_BLADEWING: &str = r#"{"Triggered":{"when":{"AnyOf":[{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},{"DealsDamage":{"source":["This"],"to":{"Each":[{"Type":"Planeswalker"}]},"combat":true}}]},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Zombie Knight","colors":["Black"],"types":["Creature"],"subtypes":["Zombie","Knight"],"power":2,"toughness":2,"abilities":[{"Keyword":"Menace"}]}},"count":{"Count":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}}]}}"#;
const W5A_REYHAN: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Command"}}]},"if":{"Compare":{"left":{"Counters":{"on":"Triggering","counter":"+1/+1"}},"op":"Ge","right":1}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"May":{"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":{"Counters":{"on":"Triggering","counter":"+1/+1"}}}}]}}]}}"#;
const W5A_LOST: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChangeExcept":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"except":"Hand","to":"Battlefield"}},{"SpellCast":{"what":[{"Not":{"CastFrom":"Hand"}}],"by":"You"}}]},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Spawn","colors":["Red"],"types":["Creature"],"subtypes":["Spawn"],"power":3,"toughness":3}}}}]}}"#;
const W5A_VARINA: &str = r#"{"Triggered":{"when":{"Batch":{"Attacks":{"what":[{"Subtype":"Zombie"},{"ControlledBy":"You"}]}}},"effects":[{"Draw":{"count":"Triggering"}},{"Discard":{"count":"Triggering"}},{"GainLife":{"amount":"Triggering"}}]}}"#;
const W5A_RESTORED: &str = r#"{"Triggered":{"when":{"Batch":{"CountersRemoved":{"what":["This"],"counter":{"Other":"time"}}}},"effects":[{"Scry":{}},{"GainLife":{"amount":1}}]}}"#;
const W5A_DRAGONBORN: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"ControlledBy":"You"}],"to":"EachPlayer"}},"if":{"Compare":{"left":"Triggering","op":"Ge","right":5}},"effects":[{"Draw":{}}]}}"#;

const W5A_WASTE_CREATURE_TEXT: &str =
    "Whenever an opponent discards a creature card, create a 2/2 black Zombie creature token.";
const W5A_WASTE_LAND_TEXT: &str = "Whenever an opponent discards a land card, add {B}{B}.";
const W5A_WASTE_OTHER_TEXT: &str =
    "Whenever an opponent discards a noncreature, nonland card, draw a card.";
const W5A_GAHIJI_TEXT: &str = "Whenever a creature attacks one of your opponents or a planeswalker an opponent controls, that creature gets +2/+0 until end of turn.";
const W5A_DECKHAND_TEXT: &str = "When this creature becomes the target of a spell, sacrifice it.";
const W5A_VIPER_TEXT: &str = "Whenever this creature deals combat damage to a creature, destroy that creature at end of combat.";
const W5A_BLADEWING_TEXT: &str = "Whenever ~ deals combat damage to a player or planeswalker, for each creature card in your graveyard, create a 2/2 black Zombie Knight creature token with menace.";
const W5A_REYHAN_TEXT: &str = "Whenever a creature you control dies or is put into the command zone, if it had one or more +1/+1 counters on it, you may put that many +1/+1 counters on target creature.";
const W5A_LOST_TEXT: &str = "Whenever a land you control enters from anywhere other than your hand or you cast a spell from anywhere other than your hand, create a 3/3 red Spawn creature token.";
const W5A_VARINA_TEXT: &str = "Whenever you attack with one or more Zombies, draw that many cards, then discard that many cards. You gain that much life.";
const W5A_RESTORED_TEXT: &str = "Whenever one or more time counters are removed from this enchantment, scry 1 and you gain 1 life.";
const W5A_DRAGONBORN_TEXT: &str =
    "Whenever a source you control deals 5 or more damage to a player, draw a card.";

#[rstest]
#[case::waste_creature(W5A_WASTE_CREATURE, W5A_WASTE_CREATURE_TEXT)]
#[case::waste_land(W5A_WASTE_LAND, W5A_WASTE_LAND_TEXT)]
#[case::waste_other(W5A_WASTE_OTHER, W5A_WASTE_OTHER_TEXT)]
#[case::waste_by_you(
    &W5A_WASTE_LAND.replace(r#""by":"Opponents""#, r#""by":"You""#),
    &W5A_WASTE_LAND_TEXT.replace("an opponent discards", "you discard")
)]
#[case::waste_artifact(
    &W5A_WASTE_LAND.replace("Land", "Artifact"),
    &W5A_WASTE_LAND_TEXT.replace("a land", "an artifact")
)]
#[case::gahiji(W5A_GAHIJI, W5A_GAHIJI_TEXT)]
#[case::deckhand(W5A_DECKHAND, W5A_DECKHAND_TEXT)]
#[case::deckhand_of_an_opponent(
    &W5A_DECKHAND.replace(r#"["Spell"]"#, r#"["Spell",{"ControlledBy":"Opponents"}]"#),
    &W5A_DECKHAND_TEXT.replace("a spell", "a spell an opponent controls")
)]
#[case::viper(W5A_VIPER, W5A_VIPER_TEXT)]
#[case::viper_exiles(
    &W5A_VIPER.replace("Destroy", "Exile"),
    &W5A_VIPER_TEXT.replace("destroy", "exile")
)]
#[case::bladewing(W5A_BLADEWING, W5A_BLADEWING_TEXT)]
#[case::reyhan(W5A_REYHAN, W5A_REYHAN_TEXT)]
#[case::lost(W5A_LOST, W5A_LOST_TEXT)]
#[case::varina(W5A_VARINA, W5A_VARINA_TEXT)]
#[case::restored(W5A_RESTORED, W5A_RESTORED_TEXT)]
#[case::dragonborn(W5A_DRAGONBORN, W5A_DRAGONBORN_TEXT)]
#[case::dragonborn_at_seven(
    &W5A_DRAGONBORN.replace(r#""right":5"#, r#""right":7"#),
    &W5A_DRAGONBORN_TEXT.replace('5', "7")
)]
fn w5a_the_oracle_wording_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(
        w5a_written(json, clause),
        "{clause}\n{:#?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::waste_by_you(&W5A_WASTE_CREATURE.replace(r#""by":"Opponents""#, r#""by":"You""#), W5A_WASTE_CREATURE_TEXT)]
#[case::waste_other_kind(&W5A_WASTE_CREATURE.replace("Creature\"}],\"by", "Enchantment\"}],\"by"), W5A_WASTE_CREATURE_TEXT)]
#[case::waste_other_not(&W5A_WASTE_OTHER.replace(r#"{"Not":{"Type":"Land"}}"#, r#"{"Not":{"Type":"Artifact"}}"#), W5A_WASTE_OTHER_TEXT)]
#[case::gahiji_at_you(&W5A_GAHIJI.replace(r#""at":"Opponents""#, r#""at":"You""#), W5A_GAHIJI_TEXT)]
#[case::gahiji_artifact(&W5A_GAHIJI.replace(r#"[{"Type":"Creature"}]"#, r#"[{"Type":"Artifact"}]"#), W5A_GAHIJI_TEXT)]
#[case::deckhand_of_an_opponent(&W5A_DECKHAND.replace(r#"["Spell"]"#, r#"["Spell",{"ControlledBy":"Opponents"}]"#), W5A_DECKHAND_TEXT)]
#[case::deckhand_any(&W5A_DECKHAND.replace(r#","by":["Spell"]"#, ""), W5A_DECKHAND_TEXT)]
#[case::viper_noncombat(&W5A_VIPER.replace(r#""combat":true"#, r#""combat":false"#), W5A_VIPER_TEXT)]
#[case::viper_artifact(&W5A_VIPER.replace(r#""to":{"Each":[{"Type":"Creature"}]}"#, r#""to":{"Each":[{"Type":"Artifact"}]}"#), W5A_VIPER_TEXT)]
#[case::bladewing_battle(&W5A_BLADEWING.replace("Planeswalker", "Battle"), W5A_BLADEWING_TEXT)]
#[case::bladewing_opponent(&W5A_BLADEWING.replace(r#""to":"EachPlayer""#, r#""to":"Opponents""#), W5A_BLADEWING_TEXT)]
#[case::reyhan_exiled(&W5A_REYHAN.replace(r#""to":"Command""#, r#""to":"Exile""#), W5A_REYHAN_TEXT)]
#[case::reyhan_from_anywhere(&W5A_REYHAN.replace(r#""from":"Battlefield","to":"Command""#, r#""to":"Command""#), W5A_REYHAN_TEXT)]
#[case::lost_from_graveyard(&W5A_LOST.replace(r#""except":"Hand""#, r#""except":"Graveyard""#), W5A_LOST_TEXT)]
#[case::lost_artifact(&W5A_LOST.replace(r#"[{"Type":"Land"},"#, r#"[{"Type":"Artifact"},"#), W5A_LOST_TEXT)]
#[case::viper_end_step(&W5A_VIPER.replace("EndCombat", "End"), W5A_VIPER_TEXT)]
#[case::reyhan_other_counters(&W5A_REYHAN.replace(r#""count":{"Counters":{"on":"Triggering","counter":"+1/+1"}}"#, r#""count":{"Counters":{"on":"Triggering","counter":"-1/-1"}}"#), W5A_REYHAN_TEXT)]
#[case::varina_of_opponents(&W5A_VARINA.replace(r#"{"ControlledBy":"You"}"#, r#"{"ControlledBy":"Opponents"}"#), W5A_VARINA_TEXT)]
#[case::varina_spirits(&W5A_VARINA.replace("Zombie", "Spirit"), W5A_VARINA_TEXT)]
#[case::restored_lore(&W5A_RESTORED.replace(r#"{"Batch":{"CountersRemoved":{"what":["This"],"counter":{"Other":"time"}}}}"#, r#"{"Batch":{"CountersRemoved":{"what":["This"],"counter":{"Other":"lore"}}}}"#), W5A_RESTORED_TEXT)]
#[case::restored_one(&W5A_RESTORED.replace(r#"{"Batch":{"CountersRemoved":{"what":["This"],"counter":{"Other":"time"}}}}"#, r#"{"CountersRemoved":{"what":["This"],"counter":{"Other":"time"}}}"#), W5A_RESTORED_TEXT)]
#[case::dragonborn_at_four(&W5A_DRAGONBORN.replace(r#""right":5"#, r#""right":4"#), W5A_DRAGONBORN_TEXT)]
#[case::dragonborn_greater(&W5A_DRAGONBORN.replace(r#""op":"Ge""#, r#""op":"Gt""#), W5A_DRAGONBORN_TEXT)]
#[case::dragonborn_opponent(&W5A_DRAGONBORN.replace(r#""to":"EachPlayer""#, r#""to":"Opponents""#), W5A_DRAGONBORN_TEXT)]
fn w5a_a_changed_head_does_not_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!w5a_written(json, clause), "{json}");
}

const W5A_MAP: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"Opponents"}],"to":"Battlefield"}},"if":{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":{"ControllerOf":"Triggering"}}]},"op":"Gt","right":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}},"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Land"},{"In":"Hand"},{"OwnedBy":"You"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{}}}}]}}]}}"#;
const W5A_EOWYN: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"if":{"Compare":{"left":{"Count":[{"Subtype":"Human"},"Other",{"ControlledBy":"You"},"EnteredThisTurn"]},"op":"Ge","right":1}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Human Knight","colors":["Red"],"types":["Creature"],"subtypes":["Human","Knight"],"power":2,"toughness":2,"abilities":[{"Keyword":"Trample"},{"Keyword":"Haste"}]}},"count":2}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Subtype":"Human"},{"ControlledBy":"You"}]},"op":"Ge","right":6}},"then":[{"Draw":{}}]}}]}}"#;

const W5A_MAP_TEXT: &str = "Whenever a land an opponent controls enters, if that player controls more lands than you, you may put a land card from your hand onto the battlefield.";
const W5A_EOWYN_TEXT: &str = "At the beginning of combat on your turn, if another Human entered the battlefield under your control this turn, create two 2/2 red Human Knight creature tokens with trample and haste. Then if you control six or more Humans, draw a card.";

#[rstest]
#[case::map(W5A_MAP, W5A_MAP_TEXT)]
#[case::map_fewer(
    &W5A_MAP.replace(r#""op":"Gt""#, r#""op":"Lt""#),
    &W5A_MAP_TEXT.replace("more", "fewer")
)]
#[case::eowyn(W5A_EOWYN, W5A_EOWYN_TEXT)]
#[case::eowyn_any_human(
    &W5A_EOWYN.replace(r#"{"Subtype":"Human"},"Other","#, r#"{"Subtype":"Human"},"#),
    &W5A_EOWYN_TEXT.replace("if another Human", "if a Human")
)]
fn w5a_the_oracle_condition_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(
        w5a_written(json, clause),
        "{clause}\n{:#?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::map_at_least(&W5A_MAP.replace(r#""op":"Gt""#, r#""op":"Ge""#), W5A_MAP_TEXT)]
#[case::map_yours(&W5A_MAP.replace(r#"{"ControllerOf":"Triggering"}"#, r#""Opponents""#), W5A_MAP_TEXT)]
#[case::map_creatures(&W5A_MAP.replace(r#"[{"Type":"Land"},{"ControlledBy":"You"}]"#, r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#), W5A_MAP_TEXT)]
#[case::eowyn_elf(&W5A_EOWYN.replace(r#"[{"Subtype":"Human"},"Other","#, r#"[{"Subtype":"Elf"},"Other","#), W5A_EOWYN_TEXT)]
#[case::eowyn_two(&W5A_EOWYN.replace(r#""EnteredThisTurn"]},"op":"Ge","right":1"#, r#""EnteredThisTurn"]},"op":"Ge","right":2"#), W5A_EOWYN_TEXT)]
#[case::eowyn_opponents(&W5A_EOWYN.replace(r#""Other",{"ControlledBy":"You"}"#, r#""Other",{"ControlledBy":"Opponents"}"#), W5A_EOWYN_TEXT)]
fn w5a_a_changed_condition_does_not_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!w5a_written(json, clause), "{json}");
}
