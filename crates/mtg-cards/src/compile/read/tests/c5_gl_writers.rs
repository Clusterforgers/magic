//! Goal: the writers the fifth sample of the names G to L needed give the oracle wording of a loyalty counter moved
//! under a trailing "if", a token copy with an added type, a mill by the owner of a target, a return at the next end
//! step, an exile of any number, a transform after a counter check, a group that explores, a draw when a land is not
//! put, a target of two kinds, a reveal of a shared type, a kicked step after a search, a card of the chosen type, a
//! creature put with haste and sacrificed, mana of a land's type tripled, the largest group as a count, a doubled
//! power of the equipped creature, a forced attack, goad by the equipment, hexproof for the player, afflict, a cost by
//! the life gained, an attack on a player, a prevention after a commander check, a joined sacrifice, a return attached
//! to a target, "also get", "When you attack with" and the modes of a trigger with a tapped and attacking token; a
//! changed count, kind, player, step, zone, group or duration never reads back.
use super::*;
use rstest::rstest;

/// Whether some wording the checker accepts for the rules is the clause.
fn c5_gl_written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const C5_GL_BEACON: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Planeswalker"},{"ControlledBy":"You"}],"to":"Battlefield"}},"if":{"Compare":{"left":{"Counters":{"on":"This","counter":"loyalty"}},"op":"Ge","right":1}},"effects":[{"May":{"effects":[{"MoveCounters":{"from":"This","to":"Triggering","counter":"loyalty"}}]}}]}}"#;
const C5_GL_PATRIARCH: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"HasCounter":{"Other":"infection"}}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"CreateToken":{"token":{"CopyOf":{"of":"Triggering","except":[{"AddSubtype":"Tyranid"}]}}}}]}}"#;
const C5_GL_GETH: &str = r#"{"Activated":{"cost":"{X}{B}","targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]},{"Compare":{"stat":"ManaValue","op":"Eq","to":"X"}},{"In":"Graveyard"},{"OwnedBy":"Opponents"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"tapped":true,"under":"You"}}}},{"Mill":{"who":{"OwnerOf":{"Target":0}},"count":"X"}}]}}"#;
const C5_GL_IMPETUS: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Is":"Attached"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}]}}"#;
const C5_GL_PROTECTOR: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Choose":{"from":{"Each":[{"Not":{"Subtype":"Angel"}},{"Type":"Creature"},{"ControlledBy":"You"}]},"count":{"min":0,"max":null},"as":"chosen"}},{"Exile":{"what":{"Bound":"chosen"},"duration":"WhileSourceOnBattlefield"}}]}}"#;
const C5_GL_CLIMB: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}},{"If":{"condition":{"Compare":{"left":{"Counters":{"on":{"Target":0},"counter":"+1/+1"}},"op":"Ge","right":3}},"then":[{"Transform":{"what":"This"}}]}}]}}"#;
const C5_GL_EXPLORE: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"effects":[{"Explore":{"what":{"Each":[{"Subtype":"Merfolk"},{"Type":"Creature"},{"ControlledBy":"You"}]}}}]}}"#;
const C5_GL_LAND: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Type":"Land"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{}}}}]}},{"If":{"condition":{"Not":"Performed"},"then":[{"Draw":{}}]}}]}}"#;
const C5_GL_HAVEN: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap","SacrificeSelf"],"targets":[{"what":{"Either":[{"Object":[{"Type":"Creature"},{"Subtype":"Dragon"},{"In":"Graveyard"},{"OwnedBy":"You"}]},{"Object":[{"Type":"Planeswalker"},{"Subtype":"Ugin"},{"In":"Graveyard"},{"OwnedBy":"You"}]}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#;
const C5_GL_BLADE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Is":"Attached"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"May":{"effects":[{"RevealUntil":{"filter":[{"Type":"Creature"},{"SharesSubtype":"Triggering"}],"as":"found","revealed_as":"revealed"}},{"Move":{"what":{"Bound":"found"},"to":"Hand"}},{"Move":{"what":{"Bound":"revealed"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}]}}"#;
const C5_GL_WILDS: &str = r#"{"Spell":{"effects":[{"Search":{"filter":[{"Subtype":"Forest"}],"count":{"min":0,"max":2},"as":"found"}},{"Move":{"what":{"Bound":"found"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}},{"If":{"condition":"Kicked","then":[{"Untap":{"what":{"Bound":"found"}}},{"Continuous":{"affected":{"Bound":"found"},"mods":[{"AddType":"Creature"},{"SetPT":{"power":3,"toughness":3}},{"SetColors":["Green"]},{"AddKeyword":"Haste"}],"duration":"Forever"}}]}}]}}"#;
const C5_GL_ICON: &str = r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap"],"effects":[{"LookAtTop":{"count":3,"as":"top"}},{"Choose":{"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}},{"Type":"Creature"},"ChosenSubtype"]},"count":{"min":0,"max":1},"as":"kept"}},{"Reveal":{"what":{"Bound":"kept"}}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#;
const C5_GL_SOULSTOKE: &str = r#"{"Activated":{"cost":[{"Mana":"{1}{R}"},"Tap"],"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Subtype":"Elemental"},{"Type":"Creature"}]},"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":{"Battlefield":{}}}},{"Continuous":{"affected":{"Bound":"card"},"mods":[{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Sacrifice":{"filter":[{"Is":{"Bound":"card"}}]}}]}}]}}]}}"#;
const C5_GL_DRUID: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Instead":{"base":{"ProducedBy":{"among":[{"Type":"Land"},{"ControlledBy":"You"}],"count":1,"colorless":true}},"when":{"Compare":{"left":{"Counters":{"on":"This","counter":"+1/+1"}},"op":"Ge","right":1}},"instead":{"ProducedBy":{"among":[{"Type":"Land"},{"ControlledBy":"You"}],"count":3,"colorless":true}}}}}}]}}"#;
const C5_GL_JOURNAL: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":{"Other":"suspect"},"count":{"Largest":{"who":"EachPlayer","of":[{"Type":"Creature"}]}}}}}}"#;
const C5_GL_JET: &str = r#"{"Activated":{"cost":[{"Mana":"{3}"},{"Sacrifice":{"filter":["Other",{"Type":"Artifact"}]}}],"effects":[{"Continuous":{"affected":"Attached","mods":[{"ModifyPT":{"power":{"Stat":{"of":"Attached","stat":"Power"}},"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#;
const C5_GL_KARDUR: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"mods":["Goaded"],"duration":"UntilYourNextTurn"}}]}}"#;
const C5_GL_KOMAINU: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"AnyOf":["This",{"Is":"Attached"}]}],"to":"EachPlayer","combat":true}},"effects":[{"Goad":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"Triggering"}]}}}]}}"#;
const C5_GL_PLATING: &str = r#"{"Spell":{"effects":[{"Amass":{"subtype":"Zombie","count":1}},{"Rule":{"rule":{"PlayersHave":{"who":"You","keyword":"Hexproof"}},"duration":"UntilEndOfTurn"}},{"Continuous":{"affected":{"Each":[{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Hexproof"}],"duration":"UntilEndOfTurn"}}]}}"#;
const C5_GL_SLIVER: &str = r#"{"Static":{"affected":{"Each":[{"Subtype":"Sliver"},{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Afflict":2}}]}}"#;
const C5_GL_LICIA: &str = r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{1}","per":{"Tallied":{"who":"You","what":"LifeGainedTotal"}}}}"#;
const C5_GL_SENSOR: &str = r#"{"Triggered":{"when":{"PlayerAttacks":{"by":"You","at":"EachPlayer"}},"effects":[{"PutCounters":{"on":"This","counter":"charge"}}]}}"#;
const C5_GL_UNICORN: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"if":{"Compare":{"left":{"Count":["Commander",{"OwnedBy":"You"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"effects":[{"Shield":{"when":{"Damage":{"to":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"combat":true}},"by":"Prevent","duration":"UntilEndOfTurn"}},{"Continuous":{"affected":{"Each":["Other",{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Vigilance"}],"duration":"UntilEndOfTurn"}}]}}"#;
const C5_GL_HEART: &str = r#"{"Activated":{"cost":[{"Mana":"{B}{B}"},"Tap","SacrificeSelf",{"Sacrifice":{"filter":[{"Subtype":"Blood"},"Token"],"count":13}}],"effects":[{"LoseLife":{"who":"Opponents","amount":13}},{"GainLife":{"amount":13}}]}}"#;
const C5_GL_BOON: &str = r#"{"Activated":{"cost":"{3}{W}","timing":"Sorcery","from":"Graveyard","targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}},{"Attach":{"what":"This","to":{"Target":0}}}]}}"#;
const C5_GL_JETMIR: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":0}},{"AddKeyword":"Trample"}],"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":6}}}}"#;
const C5_GL_LANDING: &str = r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"at_least":3}},"effects":[{"Transform":{"what":"This"}}]}}"#;
const C5_GL_GHALTA: &str = r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"effects":[{"Modal":{"modes":[{"effects":[{"CreateToken":{"token":{"Sized":{"size":{"Greatest":{"of":["Other","Attacking",{"Type":"Creature"}],"stat":"Power"}},"token":{"name":"Dinosaur","colors":["Green"],"types":["Creature"],"subtypes":["Dinosaur"],"abilities":[{"Keyword":"Trample"}]}}},"tapped":true,"attacking":true}}]},{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Vampire","colors":["White"],"types":["Creature"],"subtypes":["Vampire"],"power":1,"toughness":1,"abilities":[{"Keyword":"Lifelink"}]}},"count":{"Count":["Other","Attacking",{"Type":"Creature"}]}}}]}]}}]}}"#;

/// The modes of Ghalta and Mavren as the card prints them, after its head.
const C5_GL_GHALTA_MODES: &str = "choose one —\n• Create a tapped and attacking X/X green Dinosaur creature token with trample, where X is the greatest power among other attacking creatures.\n• Create X 1/1 white Vampire creature tokens with lifelink, where X is the number of other attacking creatures.";

#[rstest]
#[case::move_a_loyalty_counter(
    C5_GL_BEACON,
    "Whenever a planeswalker you control enters, if ~ has loyalty counters on it, you may move a loyalty counter from ~ onto that planeswalker."
)]
#[case::copy_with_a_type(
    C5_GL_PATRIARCH,
    "Whenever a creature with an infection counter on it dies, you create a token that's a copy of that creature, except it's a Tyranid in addition to its other types."
)]
#[case::owner_mills(
    C5_GL_GETH,
    "{X}{B}: Put target artifact or creature card with mana value X from an opponent's graveyard onto the battlefield tapped under your control. Then that player mills X cards."
)]
#[case::back_at_the_end_step(
    C5_GL_IMPETUS,
    "When enchanted creature dies, return ~ to the battlefield at the beginning of the next end step."
)]
#[case::exile_any_number(
    C5_GL_PROTECTOR,
    "When ~ enters, you may exile any number of non-Angel creatures you control until ~ leaves the battlefield."
)]
#[case::transform_after_counters(
    C5_GL_CLIMB,
    "At the beginning of combat on your turn, put a +1/+1 counter on target creature you control. Then if that creature has three or more +1/+1 counters on it, transform ~."
)]
#[case::each_explores(
    C5_GL_EXPLORE,
    "At the beginning of combat on your turn, each Merfolk creature you control explores."
)]
#[case::draw_if_you_do_not(
    C5_GL_LAND,
    "Whenever ~ attacks, you may put a land card from your hand onto the battlefield. If you don't, draw a card."
)]
#[case::two_kinds_of_card(
    C5_GL_HAVEN,
    "{2}, {T}, Sacrifice ~: Return target Dragon creature card or Ugin planeswalker card from your graveyard to your hand."
)]
#[case::shares_a_type(
    C5_GL_BLADE,
    "Whenever equipped creature dies, you may reveal cards from the top of your library until you reveal a creature card that shares a creature type with it. Put that card into your hand and the rest on the bottom of your library in a random order."
)]
#[case::kicked_forests(
    C5_GL_WILDS,
    "Search your library for up to two Forest cards, put them onto the battlefield tapped, then shuffle.\nIf this spell was kicked, untap all Forests put onto the battlefield this way. They become 3/3 green creatures with haste that are still lands."
)]
#[case::of_the_chosen_type(
    C5_GL_ICON,
    "{3}, {T}: Look at the top three cards of your library. You may reveal a creature card of the chosen type from among them and put it into your hand. Put the rest on the bottom of your library in a random order."
)]
#[case::haste_then_sacrifice(
    C5_GL_SOULSTOKE,
    "{1}{R}, {T}: You may put an Elemental creature card from your hand onto the battlefield. That creature gains haste until end of turn. Sacrifice it at the beginning of the next end step."
)]
#[case::three_of_that_type(
    C5_GL_DRUID,
    "{T}: Add one mana of any type that a land you control could produce. If ~ has a +1/+1 counter on it, add three mana of that type instead."
)]
#[case::the_largest_group(
    C5_GL_JOURNAL,
    "~ enters with a number of suspect counters on it equal to the greatest number of creatures a player controls."
)]
#[case::double_equipped_power(
    C5_GL_JET,
    "{3}, Sacrifice another artifact: Double equipped creature's power until end of turn."
)]
#[case::forced_attacks(
    C5_GL_KARDUR,
    "When ~ enters, until your next turn, creatures your opponents control attack each combat if able and attack a player other than you if able."
)]
#[case::goad_by_the_equipment(
    C5_GL_KOMAINU,
    "Whenever ~ or equipped creature deals combat damage to a player, goad each creature that player controls."
)]
#[case::you_and_permanents(
    C5_GL_PLATING,
    "Amass Zombies 1.\nYou and permanents you control gain hexproof until end of turn."
)]
#[case::afflict(C5_GL_SLIVER, "Sliver creatures you control have afflict 2.")]
#[case::life_gained(
    C5_GL_LICIA,
    "~ costs {1} less to cast for each 1 life you gained this turn."
)]
#[case::attack_a_player(
    C5_GL_SENSOR,
    "Whenever you attack a player, put a charge counter on ~."
)]
#[case::prevent_then_vigilance(
    C5_GL_UNICORN,
    "At the beginning of combat on your turn, if you control your commander, prevent all combat damage that would be dealt to creatures you control this turn. Other creatures you control gain vigilance until end of turn."
)]
#[case::sacrifice_and(
    C5_GL_HEART,
    "{B}{B}, {T}, Sacrifice ~ and thirteen Blood tokens: Each opponent loses 13 life and you gain 13 life."
)]
#[case::returned_attached(
    C5_GL_BOON,
    "{3}{W}: Return ~ from your graveyard to the battlefield attached to target creature. Activate only as a sorcery."
)]
#[case::also_get(
    C5_GL_JETMIR,
    "Creatures you control also get +1/+0 and have trample as long as you control six or more creatures."
)]
#[case::when_you_attack_with(
    C5_GL_LANDING,
    "When you attack with three or more creatures, transform ~."
)]
fn c5_gl_the_sample_wordings_are_written_as_the_oracle_words_them(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(c5_gl_written(json, clause), "{clause}");
}

#[test]
fn c5_gl_the_modes_of_a_trigger_are_written_one_by_one() {
    let clause = format!("Whenever you attack, {C5_GL_GHALTA_MODES}");
    assert!(c5_gl_written(C5_GL_GHALTA, &clause), "{clause}");
    for near in [
        C5_GL_GHALTA_MODES.replace("choose one", "choose two"),
        C5_GL_GHALTA_MODES.replace("tapped and attacking X/X", "X/X"),
        C5_GL_GHALTA_MODES.replace("Vampire creature tokens", "Bat creature tokens"),
    ] {
        let near = format!("Whenever you attack, {near}");
        assert!(!c5_gl_written(C5_GL_GHALTA, &near), "{near}");
    }
}

#[rstest]
#[case::two_counters_moved(
    C5_GL_BEACON,
    "Whenever a planeswalker you control enters, if ~ has loyalty counters on it, you may move two loyalty counters from ~ onto that planeswalker."
)]
#[case::another_type(
    C5_GL_PATRIARCH,
    "Whenever a creature with an infection counter on it dies, you create a token that's a copy of that creature, except it's a Zombie in addition to its other types."
)]
#[case::you_mill(
    C5_GL_GETH,
    "{X}{B}: Put target artifact or creature card with mana value X from an opponent's graveyard onto the battlefield tapped under your control. Then you mill X cards."
)]
#[case::back_at_the_upkeep(
    C5_GL_IMPETUS,
    "When enchanted creature dies, return ~ to the battlefield at the beginning of your next upkeep."
)]
#[case::exile_up_to_two(
    C5_GL_PROTECTOR,
    "When ~ enters, you may exile up to two non-Angel creatures you control until ~ leaves the battlefield."
)]
#[case::four_counters(
    C5_GL_CLIMB,
    "At the beginning of combat on your turn, put a +1/+1 counter on target creature you control. Then if that creature has four or more +1/+1 counters on it, transform ~."
)]
#[case::elves_explore(
    C5_GL_EXPLORE,
    "At the beginning of combat on your turn, each Elf creature you control explores."
)]
#[case::draw_if_you_do(
    C5_GL_LAND,
    "Whenever ~ attacks, you may put a land card from your hand onto the battlefield. If you do, draw a card."
)]
#[case::another_planeswalker(
    C5_GL_HAVEN,
    "{2}, {T}, Sacrifice ~: Return target Dragon creature card or Bolas planeswalker card from your graveyard to your hand."
)]
#[case::any_order(
    C5_GL_BLADE,
    "Whenever equipped creature dies, you may reveal cards from the top of your library until you reveal a creature card that shares a creature type with it. Put that card into your hand and the rest on the bottom of your library in any order."
)]
#[case::bigger_forests(
    C5_GL_WILDS,
    "Search your library for up to two Forest cards, put them onto the battlefield tapped, then shuffle.\nIf this spell was kicked, untap all Forests put onto the battlefield this way. They become 4/4 green creatures with haste that are still lands."
)]
#[case::top_four(
    C5_GL_ICON,
    "{3}, {T}: Look at the top four cards of your library. You may reveal a creature card of the chosen type from among them and put it into your hand. Put the rest on the bottom of your library in a random order."
)]
#[case::exiled_at_the_end_step(
    C5_GL_SOULSTOKE,
    "{1}{R}, {T}: You may put an Elemental creature card from your hand onto the battlefield. That creature gains haste until end of turn. Exile it at the beginning of the next end step."
)]
#[case::two_of_that_type(
    C5_GL_DRUID,
    "{T}: Add one mana of any type that a land you control could produce. If ~ has a +1/+1 counter on it, add two mana of that type instead."
)]
#[case::the_largest_artifacts(
    C5_GL_JOURNAL,
    "~ enters with a number of suspect counters on it equal to the greatest number of artifacts a player controls."
)]
#[case::double_toughness(
    C5_GL_JET,
    "{3}, Sacrifice another artifact: Double equipped creature's toughness until end of turn."
)]
#[case::attacks_until_end_of_turn(
    C5_GL_KARDUR,
    "When ~ enters, until end of turn, creatures your opponents control attack each combat if able and attack a player other than you if able."
)]
#[case::any_damage(
    C5_GL_KOMAINU,
    "Whenever ~ or equipped creature deals damage to a player, goad each creature that player controls."
)]
#[case::amass_two(
    C5_GL_PLATING,
    "Amass Zombies 2.\nYou and permanents you control gain hexproof until end of turn."
)]
#[case::afflict_three(C5_GL_SLIVER, "Sliver creatures you control have afflict 3.")]
#[case::two_less(
    C5_GL_LICIA,
    "~ costs {2} less to cast for each 1 life you gained this turn."
)]
#[case::two_charge_counters(
    C5_GL_SENSOR,
    "Whenever you attack a player, put two charge counters on ~."
)]
#[case::all_damage(
    C5_GL_UNICORN,
    "At the beginning of combat on your turn, if you control your commander, prevent all damage that would be dealt to creatures you control this turn. Other creatures you control gain vigilance until end of turn."
)]
#[case::twelve_blood(
    C5_GL_HEART,
    "{B}{B}, {T}, Sacrifice ~ and twelve Blood tokens: Each opponent loses 13 life and you gain 13 life."
)]
#[case::attached_to_an_artifact(
    C5_GL_BOON,
    "{3}{W}: Return ~ from your graveyard to the battlefield attached to target artifact. Activate only as a sorcery."
)]
#[case::seven_or_more(
    C5_GL_JETMIR,
    "Creatures you control also get +1/+0 and have trample as long as you control seven or more creatures."
)]
#[case::two_or_more_attackers(
    C5_GL_LANDING,
    "When you attack with two or more creatures, transform ~."
)]
fn c5_gl_a_changed_count_kind_player_step_zone_group_or_duration_never_reads_back(
    #[case] json: &str,
    #[case] clause: &str,
) {
    assert!(!c5_gl_written(json, clause), "{clause}");
}
