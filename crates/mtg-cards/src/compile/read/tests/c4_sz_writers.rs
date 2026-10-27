//! Goal: the writers of the fourth sample of S to Z give the oracle wording of damage to the controller of what targeted, a destruction of that player's permanent, an additional beginning phase, a discard by every other player, spells exiled and abilities countered, the mana value of a commander, a later return, control of a tapped artifact, creatures that damage themselves, cards exiled for another player, four kinds of countered objects, a choice of one token of three, tokens that copy the card, a discard that sets a bonus, a discard avoided by another kind, a better return at a graveyard size, statics and prevention under a condition of the card, a count of X looked at, a kind of landwalk, a cast and an attack in one trigger, a cast commander, no creature dying, a shared creature type, a chosen type in triggers, a mill and a land in one mode, an untap of the land of the event, a cycled card that damages, a player who loses life unless another sacrifices, a commander you own and an activation in the upkeep, and a wrong player, count, kind, zone, step, condition or duration never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const THUNDERBREAK: &str = r#"{"Triggered":{"when":{"BecomesTarget":{"what":[{"Subtype":"Dragon"},{"ControlledBy":"You"}],"by":[{"ControlledBy":"Opponents"}]}},"effects":[{"DealDamage":{"source":"This","to":{"ControllerOf":"Dealer"},"amount":3}}]}}"#;
const SCALELORD: &str = r#"{"Triggered":{"when":{"BecomesTarget":{"what":[{"Subtype":"Dragon"},{"ControlledBy":"You"}],"by":[{"ControlledBy":"Opponents"}]}},"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}},{"ControlledBy":{"ControllerOf":"Dealer"}}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#;
const SPHINX: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Main2","whose":"You"}},"effects":[{"AddPhase":{"phases":["Beginning"]}}]}}"#;
const SYPHON: &str = r#"{"Spell":{"effects":[{"Discard":{"who":"Opponents","as":"d"}},{"Draw":{"count":{"Count":[{"Is":{"Bound":"d"}}]}}}]}}"#;
const DISMISSAL: &str = r#"{"Spell":{"effects":[{"Exile":{"what":{"Each":["Spell","Other"]}}},{"Counter":{"what":{"Each":[{"AnyOf":[{"Ability":"Activated"},{"Ability":"Triggered"}]}]}}}]}}"#;
const STUDY: &str = r#"{"Spell":{"effects":[{"Draw":{"count":{"Greatest":{"of":["Commander",{"OwnedBy":"You"},{"AnyOf":[{"In":"Battlefield"},{"In":"Command"}]}],"stat":"ManaValue"}}}},{"LoseLife":{"amount":{"Greatest":{"of":["Commander",{"OwnedBy":"You"},{"AnyOf":[{"In":"Battlefield"},{"In":"Command"}]}],"stat":"ManaValue"}}}}]}}"#;
const TIANA: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"AnyOf":[{"Subtype":"Aura"},{"Subtype":"Equipment"}]},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"May":{"effects":[{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Move":{"what":"Triggering","to":"Hand"}}]}}]}}]}}"#;
const NABBER: &str = r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Artifact"}],"by":"Opponents"}},"effects":[{"Continuous":{"affected":"Triggering","mods":[{"SetController":"You"}],"duration":"UntilEndOfYourNextTurn"}}]}}"#;
const RECKONING: &str = r#"{"Spell":{"effects":[{"RepeatEach":{"over":{"Each":[{"Type":"Creature"}]},"as":"c","effects":[{"DealDamage":{"source":{"Bound":"c"},"to":{"Bound":"c"},"amount":{"Stat":{"of":{"Bound":"c"},"stat":"Power"}}}}]}}]}}"#;
const SIRE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"Opponents"}],"to":"Battlefield"}},"effects":[{"ExileTop":{"who":{"ControllerOf":"Triggering"},"count":2,"as":"x"}},{"Draw":{"count":2}}]}}"#;
const SISTER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Either":[{"Object":["Spell",{"Type":"Instant"}]},{"Object":["Spell",{"Type":"Sorcery"}]},{"Object":[{"Ability":"Activated"}]},{"Object":[{"Ability":"Triggered"}]}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#;
const DOCTOR: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Modal":{"modes":[{"effects":[{"CreateToken":{"token":{"Named":"clue"}}}]},{"effects":[{"CreateToken":{"token":{"Named":"food"}}}]},{"effects":[{"CreateToken":{"token":{"Named":"treasure"}}}]}]}}]}}"#;
const RELIC: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":"Kicked","effects":[{"CreateToken":{"token":{"CopyOf":{"of":"This"}},"count":2,"tapped":true}}]}}"#;
const SPELLBOUND: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Draw":{}},{"Discard":{"as":"d"}},{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":{"Stat":{"of":{"Bound":"d"},"stat":"ManaValue"}},"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#;
const THIRST: &str = r#"{"Spell":{"effects":[{"Draw":{"count":3}},{"Pay":{"cost":[{"Discard":{"filter":[{"Type":"Artifact"}]}}],"unpaid":[{"Discard":{"count":2}}]}}]}}"#;
const MEANING: &str = r#"{"Spell":{"effects":[{"Draw":{"count":3}},{"Pay":{"cost":[{"Discard":{"filter":[{"Type":"Enchantment"}]}}],"unpaid":[{"Discard":{"count":2}}]}}]}}"#;
const STITCH: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"}]},"op":"Ge","right":7}},"then":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}],"otherwise":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}]}}"#;
const WONDER: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Flying"}],"condition":{"Compare":{"left":{"Count":[{"Subtype":"Island"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"from":"Graveyard"}}"#;
const THIEF: &str = r#"{"Static":{"affected":{"Each":[{"Subtype":"Rogue"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":0}}],"condition":{"Compare":{"left":{"Largest":{"who":"Opponents","of":[{"In":"Graveyard"}]}},"op":"Ge","right":8}}}}"#;
const STAFF: &str = r#"{"Replacement":{"when":{"Damage":{"to":"You","source":[{"Type":"Creature"}],"combat":true}},"by":{"PreventUpTo":1},"condition":{"Matches":{"subject":"This","selector":["Untapped"]}}}}"#;
const SOOTHSAYING: &str = r#"{"Activated":{"cost":"{X}","effects":[{"LookAtTop":{"count":"X","as":"top"}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Top","order":"Chosen"}}}}]}}"#;
const BOOTS: &str = r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":{"Landwalk":[{"Type":"Land"},{"Not":{"Supertype":"Basic"}}]}}]}}"#;
const VANGUARD: &str = r#"{"Triggered":{"when":{"AnyOf":[{"SpellCast":{"what":["This"],"by":"You"}},{"Attacks":{"what":["This"]}}]},"effects":[{"PutCounters":{"on":{"Each":["Colorless",{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1"}}]}}"#;
const PHOENIX: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":["Commander"],"by":"You"}},"from":"Graveyard","effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}"#;
const HUNTER: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End"}},"if":{"Compare":{"left":{"Tallied":{"who":"EachPlayer","what":"CreaturesDied"}},"op":"Eq","right":0}},"effects":[{"DealDamage":{"source":"This","to":"Triggering","amount":4}}]}}"#;
const ANIMOSITY: &str = r#"{"Triggered":{"when":{"Attacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"effects":[{"Continuous":{"affected":"Triggering","mods":[{"ModifyPT":{"power":{"Count":["Attacking",{"Not":{"Is":"Triggering"}},{"SharesSubtype":"Triggering"}]},"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#;
const SPECIALIST: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"ChosenSubtype"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"May":{"effects":[{"Draw":{}}]}}]}}"#;
const BANNER: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Type":"Creature"},"ChosenSubtype"],"by":"You"}},"effects":[{"Draw":{}}]}}"#;
const WRECK: &str = r#"{"Spell":{"effects":[{"Modal":{"modes":[{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]},{"effects":[{"Mill":{"count":5}},{"May":{"effects":[{"Choose":{"from":{"Each":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Land"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}}]}}]}]}}]}}"#;
const TILLER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"},"Tapped"],"to":"Battlefield"}},"effects":[{"Modal":{"modes":[{"effects":[{"Untap":{"what":"Triggering"}}]},{"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}},{"ControlledBy":"Opponents"}]}}],"effects":[{"Tap":{"what":{"Target":0}}}]}]}}]}}"#;
const SLICE: &str = r#"{"Triggered":{"when":{"Cycled":{"what":["This"],"by":"You"}},"from":"Graveyard","effects":[{"May":{"effects":[{"DealDamage":{"source":"This","to":{"Each":[{"Type":"Creature"}]},"amount":1}}]}}]}}"#;
const BLADE: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Pay":{"who":"Triggering","cost":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],"unpaid":[{"LoseLife":{"who":"Triggering","amount":{"Count":[{"Type":"Creature"},{"ControlledBy":"Triggering"}]}}}]}}]}}"#;
const SANCTUM: &str = r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap"],"condition":"YourTurn","targets":[{"what":{"Object":["Commander",{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#;
const SCOURGLASS: &str = r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"condition":{"All":[{"Step":"Upkeep"},"YourTurn"]},"effects":[{"Destroy":{"what":{"Each":[{"Not":{"Type":"Artifact"}},{"Not":{"Type":"Land"}}]}}}]}}"#;

#[rstest]
#[case::thunderbreak(
    THUNDERBREAK,
    "Whenever a Dragon you control becomes the target of a spell or ability an opponent controls, this creature deals 3 damage to that player."
)]
#[case::scalelord(
    SCALELORD,
    "Whenever a Dragon you control becomes the target of a spell or ability an opponent controls, destroy target nonland permanent that player controls."
)]
#[case::sphinx(
    SPHINX,
    "At the beginning of each of your postcombat main phases, there is an additional beginning phase after this phase."
)]
#[case::syphon(
    SYPHON,
    "Each other player discards a card. You draw a card for each card discarded this way."
)]
#[case::dismissal(DISMISSAL, "Exile all other spells and counter all abilities.")]
#[case::study(
    STUDY,
    "You draw X cards and you lose X life, where X is the mana value of a commander you own on the battlefield or in the command zone."
)]
#[case::tiana(
    TIANA,
    "Whenever an Aura or Equipment you control is put into a graveyard from the battlefield, you may return that card to its owner's hand at the beginning of the next end step."
)]
#[case::nabber(
    NABBER,
    "Whenever an opponent taps an artifact for mana, gain control of that artifact until the end of your next turn."
)]
#[case::reckoning(RECKONING, "Each creature deals damage to itself equal to its power.")]
#[case::sire(
    SIRE,
    "Whenever a land an opponent controls enters, that player exiles the top two cards of their library and you draw two cards."
)]
#[case::sister(
    SISTER,
    "When this creature enters, counter target instant spell, sorcery spell, activated ability, or triggered ability."
)]
#[case::doctor(
    DOCTOR,
    "When ~ enters, create your choice of a Clue token, a Food token, or a Treasure token."
)]
#[case::relic(
    RELIC,
    "When this artifact enters, if it was kicked, create two tapped tokens that are copies of this artifact."
)]
#[case::spellbound(
    SPELLBOUND,
    "Whenever this creature attacks, draw a card, then discard a card. This creature gets +X/+0 until end of turn, where X is the discarded card's mana value."
)]
#[case::thirst(
    THIRST,
    "Draw three cards. Then discard two cards unless you discard an artifact card."
)]
#[case::meaning(
    MEANING,
    "Draw three cards. Then discard two cards unless you discard an enchantment card."
)]
#[case::stitch(
    STITCH,
    "Return target creature card from your graveyard to your hand.\nThreshold — Return that card from your graveyard to the battlefield instead if there are seven or more cards in your graveyard."
)]
#[case::wonder(
    WONDER,
    "As long as this card is in your graveyard and you control an Island, creatures you control have flying."
)]
#[case::thief(
    THIEF,
    "As long as an opponent has eight or more cards in their graveyard, Rogues you control get +1/+0."
)]
#[case::staff(
    STAFF,
    "As long as this artifact is untapped, if a creature would deal combat damage to you, prevent 1 of that damage."
)]
#[case::soothsaying(
    SOOTHSAYING,
    "{X}: Look at the top X cards of your library, then put them back in any order."
)]
#[case::boots(BOOTS, "Equipped creature has nonbasic landwalk.")]
#[case::vanguard(
    VANGUARD,
    "When you cast this spell and whenever this creature attacks, put a +1/+1 counter on each colorless creature you control."
)]
#[case::phoenix(
    PHOENIX,
    "When you cast your commander, return this card from your graveyard to the battlefield."
)]
#[case::hunter(
    HUNTER,
    "At the beginning of each player's end step, if no creatures died this turn, this creature deals 4 damage to that player."
)]
#[case::animosity(
    ANIMOSITY,
    "Whenever a creature you control attacks, it gets +1/+0 until end of turn for each other attacking creature that shares a creature type with it."
)]
#[case::specialist(
    SPECIALIST,
    "Whenever a creature of the chosen type dies, you may draw a card."
)]
#[case::banner(
    BANNER,
    "Whenever you cast a creature spell of the chosen type, draw a card."
)]
#[case::wreck(
    WRECK,
    "Choose one —\n• Destroy target artifact or enchantment.\n• Mill five cards, then you may put a land card from your graveyard onto the battlefield tapped."
)]
#[case::tiller(
    TILLER,
    "Whenever a land you control enters tapped, choose one —\n• Untap that land.\n• Tap target nonland permanent an opponent controls."
)]
#[case::slice(
    SLICE,
    "When you cycle this card, you may have it deal 1 damage to each creature."
)]
#[case::blade(
    BLADE,
    "Whenever this creature deals combat damage to a player, that player loses life equal to the number of creatures they control unless they sacrifice a creature of their choice."
)]
#[case::sanctum(
    SANCTUM,
    "{2}, {T}: Return target commander you own from the battlefield to your hand. Activate only during your turn."
)]
#[case::scourglass(
    SCOURGLASS,
    "{T}, Sacrifice this artifact: Destroy all permanents except for artifacts and lands. Activate only during your upkeep."
)]
fn the_wording_of_the_card_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(written(json, clause), "{clause}");
}

#[rstest]
#[case::thunderbreak_damage_of_another_size(
    THUNDERBREAK,
    "\"amount\":3",
    "\"amount\":4",
    "Whenever a Dragon you control becomes the target of a spell or ability an opponent controls, this creature deals 3 damage to that player."
)]
#[case::thunderbreak_damage_to_an_opponent(
    THUNDERBREAK,
    "{\"ControllerOf\":\"Dealer\"}",
    "\"Opponents\"",
    "Whenever a Dragon you control becomes the target of a spell or ability an opponent controls, this creature deals 3 damage to that player."
)]
#[case::scalelord_a_creature_instead(
    SCALELORD,
    "{\"Not\":{\"Type\":\"Land\"}}",
    "{\"Type\":\"Creature\"}",
    "Whenever a Dragon you control becomes the target of a spell or ability an opponent controls, destroy target nonland permanent that player controls."
)]
#[case::sphinx_another_phase(
    SPHINX,
    "\"Beginning\"",
    "\"Combat\"",
    "At the beginning of each of your postcombat main phases, there is an additional beginning phase after this phase."
)]
#[case::sphinx_another_step(
    SPHINX,
    "\"Main2\"",
    "\"Main1\"",
    "At the beginning of each of your postcombat main phases, there is an additional beginning phase after this phase."
)]
#[case::syphon_each_player(
    SYPHON,
    "\"who\":\"Opponents\"",
    "\"who\":\"EachPlayer\"",
    "Each other player discards a card. You draw a card for each card discarded this way."
)]
#[case::dismissal_every_spell(
    DISMISSAL,
    "\"Spell\",\"Other\"",
    "\"Spell\"",
    "Exile all other spells and counter all abilities."
)]
#[case::study_a_commander_you_control(
    STUDY,
    "\"OwnedBy\":\"You\"",
    "\"ControlledBy\":\"You\"",
    "You draw X cards and you lose X life, where X is the mana value of a commander you own on the battlefield or in the command zone."
)]
#[case::tiana_another_zone(
    TIANA,
    "\"to\":\"Hand\"",
    "\"to\":\"Graveyard\"",
    "Whenever an Aura or Equipment you control is put into a graveyard from the battlefield, you may return that card to its owner's hand at the beginning of the next end step."
)]
#[case::tiana_another_step(
    TIANA,
    "\"step\":\"End\"",
    "\"step\":\"Upkeep\"",
    "Whenever an Aura or Equipment you control is put into a graveyard from the battlefield, you may return that card to its owner's hand at the beginning of the next end step."
)]
#[case::nabber_another_duration(
    NABBER,
    "UntilEndOfYourNextTurn",
    "UntilEndOfTurn",
    "Whenever an opponent taps an artifact for mana, gain control of that artifact until the end of your next turn."
)]
#[case::reckoning_toughness(
    RECKONING,
    "\"stat\":\"Power\"",
    "\"stat\":\"Toughness\"",
    "Each creature deals damage to itself equal to its power."
)]
#[case::sire_another_count(
    SIRE,
    "\"count\":2",
    "\"count\":3",
    "Whenever a land an opponent controls enters, that player exiles the top two cards of their library and you draw two cards."
)]
#[case::sister_a_kind_missing(
    SISTER,
    "{\"Object\":[{\"Ability\":\"Activated\"}]},",
    "",
    "When this creature enters, counter target instant spell, sorcery spell, activated ability, or triggered ability."
)]
#[case::doctor_another_token(
    DOCTOR,
    "\"food\"",
    "\"map\"",
    "When ~ enters, create your choice of a Clue token, a Food token, or a Treasure token."
)]
#[case::relic_another_count(
    RELIC,
    "\"count\":2",
    "\"count\":3",
    "When this artifact enters, if it was kicked, create two tapped tokens that are copies of this artifact."
)]
#[case::spellbound_power_instead(
    SPELLBOUND,
    "\"stat\":\"ManaValue\"",
    "\"stat\":\"Power\"",
    "Whenever this creature attacks, draw a card, then discard a card. This creature gets +X/+0 until end of turn, where X is the discarded card's mana value."
)]
#[case::thirst_another_kind(
    THIRST,
    "\"Artifact\"",
    "\"Land\"",
    "Draw three cards. Then discard two cards unless you discard an artifact card."
)]
#[case::thirst_another_count(
    THIRST,
    "\"count\":2",
    "\"count\":3",
    "Draw three cards. Then discard two cards unless you discard an artifact card."
)]
#[case::stitch_another_size(
    STITCH,
    "\"right\":7",
    "\"right\":6",
    "Return target creature card from your graveyard to your hand.\nThreshold — Return that card from your graveyard to the battlefield instead if there are seven or more cards in your graveyard."
)]
#[case::wonder_another_land(
    WONDER,
    "\"Island\"",
    "\"Swamp\"",
    "As long as this card is in your graveyard and you control an Island, creatures you control have flying."
)]
#[case::wonder_another_zone(
    WONDER,
    "\"from\":\"Graveyard\"",
    "\"from\":\"Exile\"",
    "As long as this card is in your graveyard and you control an Island, creatures you control have flying."
)]
#[case::thief_another_size(
    THIEF,
    "\"right\":8",
    "\"right\":7",
    "As long as an opponent has eight or more cards in their graveyard, Rogues you control get +1/+0."
)]
#[case::staff_prevents_two(
    STAFF,
    "\"PreventUpTo\":1",
    "\"PreventUpTo\":2",
    "As long as this artifact is untapped, if a creature would deal combat damage to you, prevent 1 of that damage."
)]
#[case::soothsaying_a_fixed_count(
    SOOTHSAYING,
    "\"count\":\"X\"",
    "\"count\":2",
    "{X}: Look at the top X cards of your library, then put them back in any order."
)]
#[case::boots_another_walk(
    BOOTS,
    "\"Basic\"",
    "\"Legendary\"",
    "Equipped creature has nonbasic landwalk."
)]
#[case::vanguard_another_player(
    VANGUARD,
    "\"by\":\"You\"",
    "\"by\":\"Opponents\"",
    "When you cast this spell and whenever this creature attacks, put a +1/+1 counter on each colorless creature you control."
)]
#[case::phoenix_another_spell(
    PHOENIX,
    "\"Commander\"",
    "\"Token\"",
    "When you cast your commander, return this card from your graveyard to the battlefield."
)]
#[case::hunter_a_death(
    HUNTER,
    "\"op\":\"Eq\"",
    "\"op\":\"Ge\"",
    "At the beginning of each player's end step, if no creatures died this turn, this creature deals 4 damage to that player."
)]
#[case::hunter_another_amount(
    HUNTER,
    "\"amount\":4",
    "\"amount\":3",
    "At the beginning of each player's end step, if no creatures died this turn, this creature deals 4 damage to that player."
)]
#[case::animosity_any_creature(
    ANIMOSITY,
    ",{\"SharesSubtype\":\"Triggering\"}",
    "",
    "Whenever a creature you control attacks, it gets +1/+0 until end of turn for each other attacking creature that shares a creature type with it."
)]
#[case::specialist_a_named_type(
    SPECIALIST,
    "\"ChosenSubtype\"",
    "{\"Subtype\":\"Elf\"}",
    "Whenever a creature of the chosen type dies, you may draw a card."
)]
#[case::banner_a_named_type(
    BANNER,
    "\"ChosenSubtype\"",
    "{\"Subtype\":\"Elf\"}",
    "Whenever you cast a creature spell of the chosen type, draw a card."
)]
#[case::wreck_another_mill(
    WRECK,
    "\"count\":5",
    "\"count\":6",
    "Choose one —\n• Destroy target artifact or enchantment.\n• Mill five cards, then you may put a land card from your graveyard onto the battlefield tapped."
)]
#[case::tiller_a_tap(
    TILLER,
    "\"Untap\"",
    "\"Tap\"",
    "Whenever a land you control enters tapped, choose one —\n• Untap that land.\n• Tap target nonland permanent an opponent controls."
)]
#[case::slice_another_blow(
    SLICE,
    "\"amount\":1",
    "\"amount\":2",
    "When you cycle this card, you may have it deal 1 damage to each creature."
)]
#[case::blade_another_player(
    BLADE,
    "\"LoseLife\":{\"who\":\"Triggering\"",
    "\"LoseLife\":{\"who\":\"You\"",
    "Whenever this creature deals combat damage to a player, that player loses life equal to the number of creatures they control unless they sacrifice a creature of their choice."
)]
#[case::sanctum_a_commander_you_control(
    SANCTUM,
    "\"OwnedBy\":\"You\"",
    "\"ControlledBy\":\"You\"",
    "{2}, {T}: Return target commander you own from the battlefield to your hand. Activate only during your turn."
)]
#[case::scourglass_another_kind(
    SCOURGLASS,
    "{\"Not\":{\"Type\":\"Artifact\"}}",
    "{\"Not\":{\"Type\":\"Creature\"}}",
    "{T}, Sacrifice this artifact: Destroy all permanents except for artifacts and lands. Activate only during your upkeep."
)]
fn another_wording_does_not_read_back(
    #[case] json: &str,
    #[case] from: &str,
    #[case] to: &str,
    #[case] clause: &str,
) {
    let changed = json.replace(from, to);
    assert_ne!(changed, json, "the change finds nothing");
    assert!(!written(&changed, clause), "{clause}");
}
