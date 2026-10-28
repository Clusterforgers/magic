//! Goal: the writers of the fifth sample of S to Z give the oracle wording of a card returned with its Aura's creature, a draw that puts the artifact back, draws for strong creatures, energy for life gained, cards exiled for life, damage then a sweep of the damaged, Clues that become creatures, a random reanimation, Treasure offered to each opponent, a scry and a revealed land, a sacrifice by the defending player, populate, counters and an unblockable attacker, a God added, an exile of all graveyards, lands searched by each player, chosen cards returned for a sacrifice, cards returned from an opponent's graveyard, a token of the creature count, a land taken from a graveyard, two modal spells, a land destroyed and replaced, a mana ability until your next turn, an indestructible gift, investigate twice, a win at thirteen cards, a loss per attacker, cards put into a graveyard, an unblockable shroud, lands that lose their abilities, counters for cards drawn, a cast from the graveyard, two lands returned, a condition on spells cast, a red source, a creature with a counter, tokens created, a choice of two types, a blow at fliers and a token for a death; another count, kind, player, zone, duration or condition never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn c5_sz_written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const C5_SZ_SCREAMS_FROM_WITHIN: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Is":"Attached"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}"#;
const C5_SZ_SENSEIS_DIVINING_TOP: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}},{"Move":{"what":"This","to":{"Library":{"place":"Top"}}}}]}}"#;
const C5_SZ_SHATTER_THE_SKY: &str = r#"{"Spell":{"effects":[{"RepeatEach":{"over":"EachPlayer","as":"p","effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":{"Bound":"p"}},{"Compare":{"stat":"Power","op":"Ge","to":4}}]},"op":"Ge","right":1}},"then":[{"Draw":{"who":{"Bound":"p"}}}]}}]}},{"Destroy":{"what":{"Each":[{"Type":"Creature"}]}}}]}}"#;
const C5_SZ_SPHINX_OF_THE_REVELATION: &str = r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"effects":[{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":"Triggering"}}]}}"#;
const C5_SZ_SUFFER_THE_PAST: &str = r#"{"Spell":{"targets":[{"what":"Player"},{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]},"bounds":{"min":"X","max":"X"}}],"effects":[{"Exile":{"what":{"Target":1},"as":"e"}},{"LoseLife":{"who":{"Target":0},"amount":{"Count":[{"Is":{"Bound":"e"}}]}}},{"GainLife":{"amount":{"Count":[{"Is":{"Bound":"e"}}]}}}]}}"#;
const C5_SZ_SWIFT_DEMISE: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":1}},{"Destroy":{"what":{"Each":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}},"DamagedThisTurn"]}}}]}}"#;
const C5_SZ_TANGLETROVE_KELP: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat"}},"effects":[{"Continuous":{"affected":{"Each":[{"Subtype":"Clue"},"Other",{"ControlledBy":"You"}]},"mods":[{"AddType":"Creature"},{"AddSubtype":"Plant"},{"SetPT":{"power":6,"toughness":6}}],"duration":"UntilEndOfTurn"}}]}}"#;
const C5_SZ_TARIEL_RECKONER_OF_SOULS: &str = r#"{"Activated":{"cost":["Tap"],"targets":[{"what":"Opponent"}],"effects":[{"ChooseAtRandom":{"from":{"Each":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]},"as":"c"}},{"Move":{"what":{"Bound":"c"},"to":{"Battlefield":{"under":"You"}}}}]}}"#;
const C5_SZ_TEMPTING_CONTRACT: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"RepeatEach":{"over":"Opponents","as":"p","effects":[{"May":{"who":{"Bound":"p"},"effects":[{"CreateToken":{"who":{"Bound":"p"},"token":{"Named":"treasure"}}}]}},{"If":{"condition":"Performed","then":[{"CreateToken":{"token":{"Named":"treasure"}}}]}}]}}]}}"#;
const C5_SZ_THRASIOS_TRITON_HERO: &str = r#"{"Activated":{"cost":"{4}","effects":[{"Scry":{}},{"LookAtTop":{"count":1,"as":"t"}},{"Reveal":{"what":{"Bound":"t"}}},{"If":{"condition":{"Matches":{"subject":{"Bound":"t"},"selector":[{"Type":"Land"}]}},"then":[{"Move":{"what":{"Bound":"t"},"to":{"Battlefield":{"tapped":true}}}}],"otherwise":[{"Draw":{}}]}}]}}"#;
const C5_SZ_THRAXIMUNDAR: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Sacrifice":{"who":"Defending","filter":[{"Type":"Creature"}]}}]}}"#;
const C5_SZ_TROSTANI_SELESNYAS_VOICE: &str = r#"{"Activated":{"cost":[{"Mana":"{1}{G}{W}"},"Tap"],"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]},"as":"token"}},{"CreateToken":{"token":{"CopyOf":{"of":{"Bound":"token"}}}}}]}}"#;
const C5_SZ_TRYGON_PRIME: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},"Attacking","Other"]},"bounds":{"min":0,"max":1}}],"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}},{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}},{"Continuous":{"affected":{"Target":0},"mods":["CantBeBlocked"],"duration":"UntilEndOfTurn"}}]}}"#;
const C5_SZ_TYRITE_SANCTUM: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"},{"Supertype":"Legendary"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddSubtype":"God"}],"duration":"Forever"}},{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}}"#;
const C5_SZ_ULTIMATE_NULLIFICATION: &str = r#"{"Spell":{"effects":[{"Exile":{"what":{"Each":[{"Type":"Creature"}]}}},{"Exile":{"what":{"Each":[{"In":"Graveyard"}]}}},{"Move":{"what":"This","to":{"Library":{"place":"Bottom"}}}}]}}"#;
const C5_SZ_VETERAN_EXPLORER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"RepeatEach":{"over":"EachPlayer","as":"p","effects":[{"May":{"who":{"Bound":"p"},"effects":[{"Search":{"who":{"Bound":"p"},"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"count":{"min":0,"max":2},"as":"l"}},{"Move":{"what":{"Bound":"l"},"to":{"Battlefield":{}}}},{"Shuffle":{"who":{"Bound":"p"}}}]}}]}}]}}"#;
const C5_SZ_VICTIMIZE: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":2,"max":2}}],"effects":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}},{"If":{"condition":"Performed","then":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"tapped":true}}}}]}}]}}"#;
const C5_SZ_SPURNMAGE_ADVOCATE: &str = r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"Opponents"}]},"bounds":{"min":2,"max":2}},{"what":{"Object":[{"Type":"Creature"},"Attacking"]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}},{"Destroy":{"what":{"Target":1}}}]}}"#;
const C5_SZ_VERNAL_SOVEREIGN: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"Attacks":{"what":["This"]}}]},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Elemental","colors":["Green","White"],"types":["Creature"],"subtypes":["Elemental"],"power":0,"toughness":0,"abilities":[{"Static":{"affected":"This","mods":[{"SetPT":{"power":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"toughness":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}],"defining":true}}]}}}}]}}"#;
const C5_SZ_SOUL_OF_WINDGRACE: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"Attacks":{"what":["This"]}}]},"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Land"},{"In":"Graveyard"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true,"under":"You"}}}}]}}]}}"#;
const C5_SZ_WITHERBLOOM_COMMAND: &str = r#"{"Spell":{"effects":[{"Modal":{"choose":{"min":2,"max":2},"modes":[{"targets":[{"what":"Player"}],"effects":[{"Mill":{"who":{"Target":0},"count":3}},{"Choose":{"from":{"Each":[{"Type":"Land"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":"Hand"}}]},{"targets":[{"what":{"Object":[{"Not":{"Type":"Creature"}},{"Not":{"Type":"Land"}},{"Compare":{"stat":"ManaValue","op":"Le","to":2}}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]},{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":-3,"toughness":-1}}],"duration":"UntilEndOfTurn"}}]},{"targets":[{"what":"Opponent"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":2}},{"GainLife":{"amount":2}}]}]}}]}}"#;
const C5_SZ_VOLATILE_FAULT: &str = r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap","SacrificeSelf"],"targets":[{"what":{"Object":[{"Type":"Land"},{"Not":{"Supertype":"Basic"}},{"ControlledBy":"Opponents"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"May":{"who":{"ControllerOf":{"Target":0}},"effects":[{"Search":{"who":{"ControllerOf":{"Target":0}},"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"l"}},{"Move":{"what":{"Bound":"l"},"to":{"Battlefield":{}}}},{"Shuffle":{"who":{"ControllerOf":{"Target":0}}}}]}},{"CreateToken":{"token":{"Named":"treasure"}}}]}}"#;
const C5_SZ_SONG_OF_FREYALISE: &str = r#"{"Triggered":{"when":{"Chapter":{"number":1}},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}}],"duration":"UntilYourNextTurn"}}]}}"#;
const C5_SZ_WARPING_WAIL: &str = r#"{"Spell":{"effects":[{"Modal":{"modes":[{"targets":[{"what":{"Object":[{"Type":"Creature"},{"AnyOf":[{"Compare":{"stat":"Power","op":"Le","to":1}},{"Compare":{"stat":"Toughness","op":"Le","to":1}}]}]}}],"effects":[{"Exile":{"what":{"Target":0}}}]},{"targets":[{"what":{"Object":["Spell",{"Type":"Sorcery"}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]},{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Eldrazi Scion","colors":[],"types":["Creature"],"subtypes":["Eldrazi","Scion"],"power":1,"toughness":1,"abilities":[{"Activated":{"cost":["SacrificeSelf"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}]}}}}]}]}}]}}"#;
const C5_SZ_THANCRED_WATERS: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Supertype":"Legendary"},"Other",{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Indestructible"}],"duration":"WhileYouControlSource"}}]}}"#;
const C5_SZ_WAVESIFTER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"CreateToken":{"token":{"Named":"clue"},"count":2}}]}}"#;
const C5_SZ_TRISKAIDEKAPHILE: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"op":"Eq","right":13}},"effects":[{"WinGame":{}}]}}"#;
const C5_SZ_WITHIN_RANGE: &str = r#"{"Triggered":{"when":{"PlayerAttacks":{"by":"You","at":"Opponents"}},"effects":[{"RepeatEach":{"over":"Opponents","as":"p","effects":[{"LoseLife":{"who":{"Bound":"p"},"amount":{"Count":["Attacking",{"AttackingPlayer":{"Bound":"p"}}]}}}]}}]}}"#;
const C5_SZ_VULTUROUS_ZOMBIE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"OwnedBy":"Opponents"}],"to":"Graveyard"}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#;
const C5_SZ_WHISPERSILK_CLOAK: &str =
    r#"{"Static":{"affected":"Attached","mods":["CantBeBlocked",{"AddKeyword":"Shroud"}]}}"#;
const C5_SZ_TOXICRENE: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Land"}]},"mods":["LoseAbilities",{"GrantAbility":{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}}]}}"#;
const C5_SZ_THOUGHT_SPONGE: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":{"Tallied":{"who":"Opponents","what":"Drawn"}}}}}}"#;
const C5_SZ_SKAAB_RUINATOR: &str =
    r#"{"Rule":{"rule":{"MayCast":{"what":["This"],"from":"Graveyard"}}}}"#;
const C5_SZ_UYO_SILENT_PROPHET: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},{"ReturnToHand":{"filter":[{"Type":"Land"}],"count":2}}],"targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}]}}],"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}]}}"#;
const C5_SZ_XEREX_STROBE_KNIGHT: &str = r#"{"Activated":{"cost":["Tap"],"condition":{"Compare":{"left":{"Tallied":{"who":"You","what":"SpellsCast"}},"op":"Ge","right":2}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Knight","colors":["White","Blue"],"types":["Creature"],"subtypes":["Knight"],"power":2,"toughness":2,"abilities":[{"Keyword":"Vigilance"}]}}}}]}}"#;
const C5_SZ_THE_RED_TERROR: &str = r#"{"Triggered":{"when":{"Batch":{"DealsDamage":{"source":[{"Color":"Red"},{"ControlledBy":"You"}]}}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#;
const C5_SZ_ZEGANA_UTOPIAN_SPEAKER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Compare":{"left":{"Count":[{"Type":"Creature"},"Other",{"ControlledBy":"You"},{"HasCounter":"+1/+1"}]},"op":"Ge","right":1}},"effects":[{"Draw":{}}]}}"#;
const C5_SZ_STAFF_OF_THE_STORYTELLER: &str = r#"{"Triggered":{"when":{"Batch":{"ZoneChange":{"what":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}],"to":"Battlefield"}}},"effects":[{"PutCounters":{"on":"This","counter":{"Other":"story"}}}]}}"#;
const C5_SZ_SKELETON_CREW: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"},{"AnyOf":[{"Subtype":"Skeleton"},{"Subtype":"Pirate"}]}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}]}}"#;
const C5_SZ_STORM_QUEEN_OF_WAKANDA: &str = r#"{"Triggered":{"when":{"AttacksPlayer":{"what":[{"Type":"Creature"},{"HasKeyword":"Flying"}],"at":"You"}},"effects":[{"DealDamage":{"source":"This","to":"Triggering","amount":{"Stat":{"of":"This","stat":"Power"}}}}]}}"#;
const C5_SZ_WILHELT_THE_ROTCLEAVER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Subtype":"Zombie"},"Other",{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"if":{"Not":{"Matches":{"subject":"Triggering","selector":[{"HasKeyword":"Decayed"}]}}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Zombie","colors":["Black"],"types":["Creature"],"subtypes":["Zombie"],"power":2,"toughness":2,"abilities":[{"Keyword":"Decayed"}]}}}}]}}"#;

#[rstest]
#[case::screams_from_within(
    C5_SZ_SCREAMS_FROM_WITHIN,
    "When enchanted creature dies, return this card from your graveyard to the battlefield."
)]
#[case::senseis_divining_top(
    C5_SZ_SENSEIS_DIVINING_TOP,
    "{T}: Draw a card, then put this artifact on top of its owner's library."
)]
#[case::shatter_the_sky(
    C5_SZ_SHATTER_THE_SKY,
    "Each player who controls a creature with power 4 or greater draws a card. Then destroy all creatures."
)]
#[case::sphinx_of_the_revelation(
    C5_SZ_SPHINX_OF_THE_REVELATION,
    "Whenever you gain life, you get that many {E}."
)]
#[case::suffer_the_past(
    C5_SZ_SUFFER_THE_PAST,
    "Exile X target cards from target player's graveyard. For each card exiled this way, that player loses 1 life and you gain 1 life."
)]
#[case::swift_demise(
    C5_SZ_SWIFT_DEMISE,
    "~ deals 1 damage to target creature. Then destroy each creature you don't control that was dealt damage this turn."
)]
#[case::tangletrove_kelp(
    C5_SZ_TANGLETROVE_KELP,
    "At the beginning of each combat, other Clues you control become 6/6 Plant creatures in addition to their other types until end of turn."
)]
#[case::tariel_reckoner_of_souls(
    C5_SZ_TARIEL_RECKONER_OF_SOULS,
    "{T}: Choose a creature card at random from target opponent's graveyard. Put that card onto the battlefield under your control."
)]
#[case::tempting_contract(
    C5_SZ_TEMPTING_CONTRACT,
    "At the beginning of your upkeep, each opponent may create a Treasure token. For each opponent who does, you create a Treasure token."
)]
#[case::thrasios_triton_hero(
    C5_SZ_THRASIOS_TRITON_HERO,
    "{4}: Scry 1, then reveal the top card of your library. If it's a land card, put it onto the battlefield tapped. Otherwise, draw a card."
)]
#[case::thraximundar(
    C5_SZ_THRAXIMUNDAR,
    "Whenever ~ attacks, defending player sacrifices a creature of their choice."
)]
#[case::trostani_selesnyas_voice(C5_SZ_TROSTANI_SELESNYAS_VOICE, "{1}{G}{W}, {T}: Populate.")]
#[case::trygon_prime(
    C5_SZ_TRYGON_PRIME,
    "Whenever this creature attacks, put a +1/+1 counter on it and a +1/+1 counter on up to one other target attacking creature. That creature can't be blocked this turn."
)]
#[case::tyrite_sanctum(
    C5_SZ_TYRITE_SANCTUM,
    "{2}, {T}: Target legendary creature becomes a God in addition to its other types. Put a +1/+1 counter on it."
)]
#[case::ultimate_nullification(
    C5_SZ_ULTIMATE_NULLIFICATION,
    "Exile all creatures and graveyards. Put ~ on the bottom of its owner's library."
)]
#[case::veteran_explorer(
    C5_SZ_VETERAN_EXPLORER,
    "When this creature dies, each player may search their library for up to two basic land cards, put them onto the battlefield, then shuffle."
)]
#[case::victimize(
    C5_SZ_VICTIMIZE,
    "Choose two target creature cards in your graveyard. Sacrifice a creature. If you do, return the chosen cards to the battlefield tapped."
)]
#[case::spurnmage_advocate(
    C5_SZ_SPURNMAGE_ADVOCATE,
    "{T}: Return two target cards from an opponent's graveyard to their hand. Destroy target attacking creature."
)]
#[case::vernal_sovereign(
    C5_SZ_VERNAL_SOVEREIGN,
    "Whenever this creature enters or attacks, create a green and white Elemental creature token with \"This token's power and toughness are each equal to the number of creatures you control.\""
)]
#[case::soul_of_windgrace(
    C5_SZ_SOUL_OF_WINDGRACE,
    "Whenever ~ enters or attacks, you may put a land card from a graveyard onto the battlefield tapped under your control."
)]
#[case::witherbloom_command(
    C5_SZ_WITHERBLOOM_COMMAND,
    "Choose two —\n• Target player mills three cards, then you return a land card from your graveyard to your hand.\n• Destroy target noncreature, nonland permanent with mana value 2 or less.\n• Target creature gets -3/-1 until end of turn.\n• Target opponent loses 2 life and you gain 2 life."
)]
#[case::volatile_fault(
    C5_SZ_VOLATILE_FAULT,
    "{1}, {T}, Sacrifice this land: Destroy target nonbasic land an opponent controls. That player may search their library for a basic land card, put it onto the battlefield, then shuffle. You create a Treasure token."
)]
#[case::song_of_freyalise(
    C5_SZ_SONG_OF_FREYALISE,
    "I — Until your next turn, creatures you control gain \"{T}: Add one mana of any color.\""
)]
#[case::warping_wail(
    C5_SZ_WARPING_WAIL,
    "Choose one —\n• Exile target creature with power or toughness 1 or less.\n• Counter target sorcery spell.\n• Create a 1/1 colorless Eldrazi Scion creature token. It has \"Sacrifice this token: Add {C}.\""
)]
#[case::thancred_waters(
    C5_SZ_THANCRED_WATERS,
    "When ~ enters, another target legendary permanent you control gains indestructible for as long as you control ~."
)]
#[case::wavesifter(C5_SZ_WAVESIFTER, "When this creature enters, investigate twice.")]
#[case::triskaidekaphile(
    C5_SZ_TRISKAIDEKAPHILE,
    "At the beginning of your upkeep, if you have exactly thirteen cards in your hand, you win the game."
)]
#[case::within_range(
    C5_SZ_WITHIN_RANGE,
    "Whenever you attack, each opponent loses life equal to the number of creatures attacking them."
)]
#[case::vulturous_zombie(
    C5_SZ_VULTUROUS_ZOMBIE,
    "Whenever a card is put into an opponent's graveyard from anywhere, put a +1/+1 counter on this creature."
)]
#[case::whispersilk_cloak(
    C5_SZ_WHISPERSILK_CLOAK,
    "Equipped creature can't be blocked and has shroud."
)]
#[case::toxicrene(
    C5_SZ_TOXICRENE,
    "All lands have \"{T}: Add one mana of any color\" and lose all other abilities."
)]
#[case::thought_sponge(
    C5_SZ_THOUGHT_SPONGE,
    "This creature enters with a number of +1/+1 counters on it equal to the greatest number of cards an opponent has drawn this turn."
)]
#[case::skaab_ruinator(C5_SZ_SKAAB_RUINATOR, "You may cast this card from your graveyard.")]
#[case::uyo_silent_prophet(
    C5_SZ_UYO_SILENT_PROPHET,
    "{2}, Return two lands you control to their owner's hand: Copy target instant or sorcery spell. You may choose new targets for the copy."
)]
#[case::xerex_strobe_knight(
    C5_SZ_XEREX_STROBE_KNIGHT,
    "{T}: Create a 2/2 white and blue Knight creature token with vigilance. Activate only if you've cast two or more spells this turn."
)]
#[case::the_red_terror(
    C5_SZ_THE_RED_TERROR,
    "Whenever a red source you control deals damage to one or more permanents and/or players, put a +1/+1 counter on ~."
)]
#[case::zegana_utopian_speaker(
    C5_SZ_ZEGANA_UTOPIAN_SPEAKER,
    "When ~ enters, if you control another creature with a +1/+1 counter on it, draw a card."
)]
#[case::staff_of_the_storyteller(
    C5_SZ_STAFF_OF_THE_STORYTELLER,
    "Whenever you create one or more creature tokens, put a story counter on this artifact."
)]
#[case::skeleton_crew(
    C5_SZ_SKELETON_CREW,
    "Each other creature you control that's a Skeleton or Pirate gets +1/+1."
)]
#[case::storm_queen_of_wakanda(
    C5_SZ_STORM_QUEEN_OF_WAKANDA,
    "Whenever a creature with flying attacks you, ~ deals damage equal to her power to that creature."
)]
#[case::wilhelt_the_rotcleaver(
    C5_SZ_WILHELT_THE_ROTCLEAVER,
    "Whenever another Zombie you control dies, if it didn't have decayed, create a 2/2 black Zombie creature token with decayed."
)]
fn c5_sz_the_clause_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(c5_sz_written(json, clause), "{clause}");
}

#[rstest]
#[case::screams_from_within(
    C5_SZ_SCREAMS_FROM_WITHIN,
    "\"to\":{\"Battlefield\":{}}",
    "\"to\":\"Hand\"",
    "When enchanted creature dies, return this card from your graveyard to the battlefield."
)]
#[case::senseis_divining_top(
    C5_SZ_SENSEIS_DIVINING_TOP,
    "\"place\":\"Top\"",
    "\"place\":\"Bottom\"",
    "{T}: Draw a card, then put this artifact on top of its owner's library."
)]
#[case::shatter_the_sky(
    C5_SZ_SHATTER_THE_SKY,
    "\"to\":4",
    "\"to\":5",
    "Each player who controls a creature with power 4 or greater draws a card. Then destroy all creatures."
)]
#[case::sphinx_of_the_revelation(
    C5_SZ_SPHINX_OF_THE_REVELATION,
    "\"on\":\"You\"",
    "\"on\":\"Opponents\"",
    "Whenever you gain life, you get that many {E}."
)]
#[case::suffer_the_past(
    C5_SZ_SUFFER_THE_PAST,
    "\"LoseLife\":{\"who\":{\"Target\":0}",
    "\"LoseLife\":{\"who\":\"You\"",
    "Exile X target cards from target player's graveyard. For each card exiled this way, that player loses 1 life and you gain 1 life."
)]
#[case::swift_demise(
    C5_SZ_SWIFT_DEMISE,
    "\"amount\":1",
    "\"amount\":2",
    "~ deals 1 damage to target creature. Then destroy each creature you don't control that was dealt damage this turn."
)]
#[case::tangletrove_kelp(
    C5_SZ_TANGLETROVE_KELP,
    "\"power\":6",
    "\"power\":5",
    "At the beginning of each combat, other Clues you control become 6/6 Plant creatures in addition to their other types until end of turn."
)]
#[case::tariel_reckoner_of_souls(
    C5_SZ_TARIEL_RECKONER_OF_SOULS,
    "{\"Battlefield\":{\"under\":\"You\"}}",
    "{\"Battlefield\":{}}",
    "{T}: Choose a creature card at random from target opponent's graveyard. Put that card onto the battlefield under your control."
)]
#[case::tempting_contract(
    C5_SZ_TEMPTING_CONTRACT,
    "\"treasure\"",
    "\"food\"",
    "At the beginning of your upkeep, each opponent may create a Treasure token. For each opponent who does, you create a Treasure token."
)]
#[case::thrasios_triton_hero(
    C5_SZ_THRASIOS_TRITON_HERO,
    "\"tapped\":true",
    "\"tapped\":false",
    "{4}: Scry 1, then reveal the top card of your library. If it's a land card, put it onto the battlefield tapped. Otherwise, draw a card."
)]
#[case::thraximundar(
    C5_SZ_THRAXIMUNDAR,
    "\"who\":\"Defending\"",
    "\"who\":\"Opponents\"",
    "Whenever ~ attacks, defending player sacrifices a creature of their choice."
)]
#[case::trostani_selesnyas_voice(
    C5_SZ_TROSTANI_SELESNYAS_VOICE,
    "\"Token\",",
    "",
    "{1}{G}{W}, {T}: Populate."
)]
#[case::trygon_prime(
    C5_SZ_TRYGON_PRIME,
    "\"CantBeBlocked\"",
    "\"CantBlock\"",
    "Whenever this creature attacks, put a +1/+1 counter on it and a +1/+1 counter on up to one other target attacking creature. That creature can't be blocked this turn."
)]
#[case::tyrite_sanctum(
    C5_SZ_TYRITE_SANCTUM,
    "\"Forever\"",
    "\"UntilEndOfTurn\"",
    "{2}, {T}: Target legendary creature becomes a God in addition to its other types. Put a +1/+1 counter on it."
)]
#[case::ultimate_nullification(
    C5_SZ_ULTIMATE_NULLIFICATION,
    "\"Bottom\"",
    "\"Top\"",
    "Exile all creatures and graveyards. Put ~ on the bottom of its owner's library."
)]
#[case::veteran_explorer(
    C5_SZ_VETERAN_EXPLORER,
    "\"max\":2",
    "\"max\":3",
    "When this creature dies, each player may search their library for up to two basic land cards, put them onto the battlefield, then shuffle."
)]
#[case::victimize(
    C5_SZ_VICTIMIZE,
    "\"tapped\":true",
    "\"tapped\":false",
    "Choose two target creature cards in your graveyard. Sacrifice a creature. If you do, return the chosen cards to the battlefield tapped."
)]
#[case::spurnmage_advocate(
    C5_SZ_SPURNMAGE_ADVOCATE,
    "\"Attacking\"",
    "\"Blocking\"",
    "{T}: Return two target cards from an opponent's graveyard to their hand. Destroy target attacking creature."
)]
#[case::vernal_sovereign(
    C5_SZ_VERNAL_SOVEREIGN,
    "\"White\"",
    "\"Black\"",
    "Whenever this creature enters or attacks, create a green and white Elemental creature token with \"This token's power and toughness are each equal to the number of creatures you control.\""
)]
#[case::soul_of_windgrace(
    C5_SZ_SOUL_OF_WINDGRACE,
    "{\"Type\":\"Land\"}",
    "{\"Type\":\"Creature\"}",
    "Whenever ~ enters or attacks, you may put a land card from a graveyard onto the battlefield tapped under your control."
)]
#[case::witherbloom_command(
    C5_SZ_WITHERBLOOM_COMMAND,
    "\"count\":3",
    "\"count\":4",
    "Choose two —\n• Target player mills three cards, then you return a land card from your graveyard to your hand.\n• Destroy target noncreature, nonland permanent with mana value 2 or less.\n• Target creature gets -3/-1 until end of turn.\n• Target opponent loses 2 life and you gain 2 life."
)]
#[case::volatile_fault(
    C5_SZ_VOLATILE_FAULT,
    "{\"Named\":\"treasure\"}",
    "{\"Named\":\"clue\"}",
    "{1}, {T}, Sacrifice this land: Destroy target nonbasic land an opponent controls. That player may search their library for a basic land card, put it onto the battlefield, then shuffle. You create a Treasure token."
)]
#[case::song_of_freyalise(
    C5_SZ_SONG_OF_FREYALISE,
    "\"UntilYourNextTurn\"",
    "\"UntilEndOfTurn\"",
    "I — Until your next turn, creatures you control gain \"{T}: Add one mana of any color.\""
)]
#[case::warping_wail(
    C5_SZ_WARPING_WAIL,
    "{\"Type\":\"Sorcery\"}",
    "{\"Type\":\"Instant\"}",
    "Choose one —\n• Exile target creature with power or toughness 1 or less.\n• Counter target sorcery spell.\n• Create a 1/1 colorless Eldrazi Scion creature token. It has \"Sacrifice this token: Add {C}.\""
)]
#[case::thancred_waters(
    C5_SZ_THANCRED_WATERS,
    "\"WhileYouControlSource\"",
    "\"UntilEndOfTurn\"",
    "When ~ enters, another target legendary permanent you control gains indestructible for as long as you control ~."
)]
#[case::wavesifter(
    C5_SZ_WAVESIFTER,
    "\"count\":2",
    "\"count\":3",
    "When this creature enters, investigate twice."
)]
#[case::triskaidekaphile(
    C5_SZ_TRISKAIDEKAPHILE,
    "\"right\":13",
    "\"right\":12",
    "At the beginning of your upkeep, if you have exactly thirteen cards in your hand, you win the game."
)]
#[case::within_range(
    C5_SZ_WITHIN_RANGE,
    "{\"AttackingPlayer\":{\"Bound\":\"p\"}}",
    "\"Blocking\"",
    "Whenever you attack, each opponent loses life equal to the number of creatures attacking them."
)]
#[case::vulturous_zombie(
    C5_SZ_VULTUROUS_ZOMBIE,
    "\"Opponents\"",
    "\"You\"",
    "Whenever a card is put into an opponent's graveyard from anywhere, put a +1/+1 counter on this creature."
)]
#[case::whispersilk_cloak(
    C5_SZ_WHISPERSILK_CLOAK,
    "\"Shroud\"",
    "\"Hexproof\"",
    "Equipped creature can't be blocked and has shroud."
)]
#[case::toxicrene(
    C5_SZ_TOXICRENE,
    "\"AnyColor\":1",
    "\"AnyColor\":2",
    "All lands have \"{T}: Add one mana of any color\" and lose all other abilities."
)]
#[case::thought_sponge(
    C5_SZ_THOUGHT_SPONGE,
    "\"Opponents\"",
    "\"You\"",
    "This creature enters with a number of +1/+1 counters on it equal to the greatest number of cards an opponent has drawn this turn."
)]
#[case::skaab_ruinator(
    C5_SZ_SKAAB_RUINATOR,
    "\"Graveyard\"",
    "\"Exile\"",
    "You may cast this card from your graveyard."
)]
#[case::uyo_silent_prophet(
    C5_SZ_UYO_SILENT_PROPHET,
    "\"count\":2",
    "\"count\":3",
    "{2}, Return two lands you control to their owner's hand: Copy target instant or sorcery spell. You may choose new targets for the copy."
)]
#[case::xerex_strobe_knight(
    C5_SZ_XEREX_STROBE_KNIGHT,
    "\"right\":2",
    "\"right\":3",
    "{T}: Create a 2/2 white and blue Knight creature token with vigilance. Activate only if you've cast two or more spells this turn."
)]
#[case::the_red_terror(
    C5_SZ_THE_RED_TERROR,
    "\"Red\"",
    "\"Green\"",
    "Whenever a red source you control deals damage to one or more permanents and/or players, put a +1/+1 counter on ~."
)]
#[case::zegana_utopian_speaker(
    C5_SZ_ZEGANA_UTOPIAN_SPEAKER,
    "\"Other\",",
    "",
    "When ~ enters, if you control another creature with a +1/+1 counter on it, draw a card."
)]
#[case::staff_of_the_storyteller(
    C5_SZ_STAFF_OF_THE_STORYTELLER,
    "\"story\"",
    "\"lore\"",
    "Whenever you create one or more creature tokens, put a story counter on this artifact."
)]
#[case::skeleton_crew(
    C5_SZ_SKELETON_CREW,
    "\"Pirate\"",
    "\"Zombie\"",
    "Each other creature you control that's a Skeleton or Pirate gets +1/+1."
)]
#[case::storm_queen_of_wakanda(
    C5_SZ_STORM_QUEEN_OF_WAKANDA,
    "\"at\":\"You\"",
    "\"at\":\"Opponents\"",
    "Whenever a creature with flying attacks you, ~ deals damage equal to her power to that creature."
)]
#[case::wilhelt_the_rotcleaver(
    C5_SZ_WILHELT_THE_ROTCLEAVER,
    "{\"Not\":{\"Matches\":{\"subject\":\"Triggering\",\"selector\":[{\"HasKeyword\":\"Decayed\"}]}}}",
    "{\"Matches\":{\"subject\":\"Triggering\",\"selector\":[{\"HasKeyword\":\"Decayed\"}]}}",
    "Whenever another Zombie you control dies, if it didn't have decayed, create a 2/2 black Zombie creature token with decayed."
)]
fn c5_sz_another_wording_does_not_read_back(
    #[case] json: &str,
    #[case] from: &str,
    #[case] to: &str,
    #[case] clause: &str,
) {
    let changed = json.replace(from, to);
    assert_ne!(changed, json, "the change finds nothing");
    assert!(!c5_sz_written(&changed, clause), "{clause}");
}
