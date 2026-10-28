//! Goal: the writers of the sixth sample of S to Z give the oracle wording of a copy from the graveyard, a return of all but five types, an Aura revealed, a bounce by toughness, searches for up to X, a basic type or a Plains, life then a draw, an artifact animated by its mana value, a mill then a land, six mana, a blow at fliers, a modal choice of two, controls returned, two sacrifices, a destroy and a counter, deathtouch shared, a sacrifice or a token, an artifact animated with counters, a Food or a Treasure, blight, a draw for the greatest power, a granted draw, a Mutant made, a win at growth counters, conditions on life and spells cast, a sacrifice in your turn, Thopters returned, counters by colours, an escape kept, another player's draw, a commander's attack, an attack trigger doubled, a seven-card exile, an attack pump, unearth for energy, an exile with an ability word, a Zombie copy and "and you" joins; another count, kind, player, zone, duration or condition never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn c6_sz_written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const C6_SZ_SEVINNES_RECLAMATION: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]},{"In":"Graveyard"},{"OwnedBy":"You"},{"Compare":{"stat":"ManaValue","op":"Le","to":3}}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}},{"If":{"condition":{"Matches":{"subject":"This","selector":[{"CastFrom":"Graveyard"}]}},"then":[{"May":{"effects":[{"CopySpell":{"what":"This","new_targets":true}}]}}]}}]}}"#;
const C6_SZ_SLINN_VODA_THE_RISING_DEEP: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":"Kicked","effects":[{"Move":{"what":{"Each":[{"Type":"Creature"},{"Not":{"Subtype":"Merfolk"}},{"Not":{"Subtype":"Kraken"}},{"Not":{"Subtype":"Leviathan"}},{"Not":{"Subtype":"Octopus"}},{"Not":{"Subtype":"Serpent"}}]},"to":"Hand"}}]}}"#;
const C6_SZ_SONGBIRDS_BLESSING: &str = r#"{"Triggered":{"when":{"Attacks":{"what":[{"Is":"Attached"}]}},"effects":[{"RevealUntil":{"filter":[{"Subtype":"Aura"}],"as":"aura","revealed_as":"rest"}},{"May":{"effects":[{"Move":{"what":{"Bound":"aura"},"to":{"Battlefield":{}}}}]}},{"If":{"condition":{"Not":"Performed"},"then":[{"Move":{"what":{"Bound":"aura"},"to":"Hand"}}]}},{"Move":{"what":{"Bound":"rest"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#;
const C6_SZ_SPECTRAL_DELUGE: &str = r#"{"Spell":{"effects":[{"Move":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"},{"Compare":{"stat":"Toughness","op":"Le","to":{"Count":[{"Subtype":"Island"},{"ControlledBy":"You"}]}}}]},"to":"Hand"}}]}}"#;
const C6_SZ_SPOROCYST: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"count":{"min":0,"max":"X"},"as":"lands"}},{"Move":{"what":{"Bound":"lands"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#;
const C6_SZ_SPROUTING_GOBLIN: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":"Kicked","effects":[{"Search":{"filter":[{"Type":"Land"},{"AnyOf":[{"Subtype":"Plains"},{"Subtype":"Island"},{"Subtype":"Swamp"},{"Subtype":"Mountain"},{"Subtype":"Forest"}]}],"reveal":true,"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":"Hand"}},{"Shuffle":{}}]}}"#;
const C6_SZ_STOIC_FARMER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Subtype":"Plains"}],"reveal":true,"as":"land"}},{"If":{"condition":{"Compare":{"left":{"Largest":{"who":"Opponents","of":[{"Type":"Land"}]}},"op":"Gt","right":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}},"then":[{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}}],"otherwise":[{"Move":{"what":{"Bound":"land"},"to":"Hand"}}]}},{"Shuffle":{}}]}}"#;
const C6_SZ_SURVIVAL_CACHE: &str = r#"{"Spell":{"effects":[{"GainLife":{"amount":2}},{"If":{"condition":{"Compare":{"left":{"Life":"You"},"op":"Gt","right":{"LowestLife":"Opponents"}}},"then":[{"Draw":{}}]}}]}}"#;
const C6_SZ_SYDRI_GALVANIC_GENIUS: &str = r#"{"Activated":{"cost":"{U}","targets":[{"what":{"Object":[{"Type":"Artifact"},{"Not":{"Type":"Creature"}}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddType":"Creature"},{"SetPT":{"power":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}},"toughness":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}],"duration":"UntilEndOfTurn"}}]}}"#;
const C6_SZ_TEVAL_THE_BALANCED_SCALE: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Mill":{"count":3}},{"May":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Land"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}}]}}]}}"#;
const C6_SZ_THE_FLUX: &str = r#"{"Triggered":{"when":{"Chapter":{"number":6}},"effects":[{"AddMana":{"mana":{"Fixed":["Red","Red","Red","Red","Red","Red"]}}}]}}"#;
const C6_SZ_THUNDERMAW_HELLKITE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"},{"HasKeyword":"Flying"},{"ControlledBy":"Opponents"}]},"amount":1}},{"Tap":{"what":{"Each":[{"Type":"Creature"},{"HasKeyword":"Flying"},{"ControlledBy":"Opponents"}]}}}]}}"#;
const C6_SZ_TITAN_OF_INDUSTRY: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Modal":{"choose":{"min":2,"max":2},"modes":[{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]},{"targets":[{"what":"Player"}],"effects":[{"GainLife":{"who":{"Target":0},"amount":5}}]},{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Rhino Warrior","colors":["Green"],"types":["Creature"],"subtypes":["Rhino","Warrior"],"power":4,"toughness":4,"abilities":[]}}}}]},{"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"as":"creature"}},{"PutCounters":{"on":{"Bound":"creature"},"counter":{"Other":"shield"}}}]}]}}]}}"#;
const C6_SZ_TROSTANI_DISCORDANT: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"OwnedBy":{"Bound":"player"}}]},"mods":[{"SetController":{"Bound":"player"}}],"duration":"Forever"}}]}}]}}"#;
const C6_SZ_VILE_MUTILATOR: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Sacrifice":{"who":"Opponents","filter":[{"Type":"Enchantment"},{"Not":"Token"}]}},{"Sacrifice":{"who":"Opponents","filter":[{"Type":"Creature"},{"Not":"Token"}]}}]}}"#;
const C6_SZ_WAKKA_DEVOTED_GUARDIAN: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"targets":[{"what":{"Object":[{"Type":"Artifact"},{"ControlledBy":"Triggering"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#;
const C6_SZ_WAND_OF_ORCUS: &str = r#"{"Triggered":{"when":{"AnyOf":[{"Attacks":{"what":[{"Is":"Attached"}]}},{"Blocks":{"what":[{"Is":"Attached"}]}}]},"effects":[{"Continuous":{"affected":"Attached","mods":[{"AddKeyword":"Deathtouch"}],"duration":"UntilEndOfTurn"}},{"Continuous":{"affected":{"Each":[{"Subtype":"Zombie"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Deathtouch"}],"duration":"UntilEndOfTurn"}}]}}"#;
const C6_SZ_WASITORA_NEKORU_QUEEN: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Sacrifice":{"who":"Triggering","filter":[{"Type":"Creature"}]}},{"If":{"condition":{"Not":"Performed"},"then":[{"CreateToken":{"token":{"Custom":{"name":"Cat Dragon","colors":["Black","Red","Green"],"types":["Creature"],"subtypes":["Cat","Dragon"],"power":3,"toughness":3,"abilities":[{"Keyword":"Flying"}]}}}}]}}]}}"#;
const C6_SZ_WORKSHOP_ELDERS: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Artifact"},{"Not":{"Type":"Creature"}},{"ControlledBy":"You"}]}}],"effects":[{"May":{"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddType":"Creature"},{"SetPT":{"power":0,"toughness":0}}],"duration":"Forever"}}]}},{"If":{"condition":"Performed","then":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":4}}]}}]}}"#;
const C6_SZ_TIRELESS_PROVISIONER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"Modal":{"modes":[{"effects":[{"CreateToken":{"token":{"Named":"food"}}}]},{"effects":[{"CreateToken":{"token":{"Named":"treasure"}}}]}]}}]}}"#;
const C6_SZ_SINISTER_GNARLBARK: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"Draw":{}},{"Choose":{"from":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"as":"blighted"}},{"PutCounters":{"on":{"Bound":"blighted"},"counter":"-1/-1"}}]}}"#;
const C6_SZ_THICKEST_IN_THE_THICKET: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"If":{"condition":{"All":[{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},{"Compare":{"left":{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}},"op":"Ge","right":{"Greatest":{"of":[{"Type":"Creature"}],"stat":"Power"}}}}]},"then":[{"Draw":{"count":2}}]}}]}}"#;
const C6_SZ_SUBTERFUGE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Flying"},{"GrantAbility":{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Draw":{"count":"Triggering"}}]}}}],"duration":"UntilEndOfTurn"}}]}}"#;
const C6_SZ_VAULT_87_FORCED_EVOLUTION: &str = r#"{"Triggered":{"when":{"Chapter":{"number":2}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddSubtype":"Mutant"}],"duration":"Forever"}}]}}"#;
const C6_SZ_SIMIC_ASCENDANCY: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"growth"}}},"op":"Ge","right":20}},"effects":[{"WinGame":{}}]}}"#;
const C6_SZ_SPEAKER_OF_THE_HEAVENS: &str = r#"{"Activated":{"cost":["Tap"],"timing":"Sorcery","condition":{"Compare":{"left":{"Life":"You"},"op":"Ge","right":{"Sum":[{"StartingLife":"You"},7]}}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Angel","colors":["White"],"types":["Creature"],"subtypes":["Angel"],"power":4,"toughness":4,"abilities":[{"Keyword":"Flying"}]}}}}]}}"#;
const C6_SZ_STELLA_LEE_WILD_CARD: &str = r#"{"Activated":{"cost":["Tap"],"condition":{"Compare":{"left":{"Tallied":{"who":"You","what":"SpellsCast"}},"op":"Ge","right":3}},"targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"ControlledBy":"You"}]}}],"effects":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}]}}"#;
const C6_SZ_SZAREL_GENESIS_SHEPHERD: &str = r#"{"Triggered":{"when":{"Sacrificed":{"what":["Other",{"Not":"Token"}],"by":"You"}},"if":"YourTurn","targets":[{"what":{"Object":[{"Type":"Creature"},"Other"]},"bounds":{"min":0,"max":1}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":{"Stat":{"of":"This","stat":"Power"}}}}]}}"#;
const C6_SZ_THOPTER_ASSEMBLY: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Count":[{"Subtype":"Thopter"},"Other",{"ControlledBy":"You"}]},"op":"Eq","right":0}},"effects":[{"Move":{"what":"This","to":"Hand"}},{"CreateToken":{"count":5,"token":{"Custom":{"name":"Thopter","colors":[],"types":["Artifact","Creature"],"subtypes":["Thopter"],"power":1,"toughness":1,"abilities":[{"Keyword":"Flying"}]}}}}]}}"#;
const C6_SZ_ULASHT_THE_HATE_SEED: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":{"Sum":[{"Count":[{"Type":"Creature"},"Other",{"Color":"Red"},{"ControlledBy":"You"}]},{"Count":[{"Type":"Creature"},"Other",{"Color":"Green"},{"ControlledBy":"You"}]}]}}}}}"#;
const C6_SZ_URO_TITAN_OF_NATURES_WRATH: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Not":{"CastWith":"Escape"}},"effects":[{"Sacrifice":{"filter":["This"]}}]}}"#;
const C6_SZ_WELL_OF_IDEAS: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Draw","whose":"Opponents"}},"effects":[{"Draw":{"who":"Triggering"}}]}}"#;
const C6_SZ_WKABI_SHIELD_OF_THE_NATION: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["Commander",{"ControlledBy":"You"}]}},"if":{"Compare":{"left":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"},{"Compare":{"stat":"ManaValue","op":"Ge","to":4}}]},"op":"Ge","right":1}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Rhino","colors":["Green"],"types":["Creature"],"subtypes":["Rhino"],"power":4,"toughness":4,"abilities":[{"Keyword":"Trample"}]}}}}]}}"#;
const C6_SZ_WULFGAR_OF_ICEWIND_DALE: &str = r#"{"Rule":{"rule":{"TriggerAgain":{"cause":{"Attacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}}}}"#;
const C6_SZ_XENAGOS_THE_REVELER: &str = r#"{"Loyalty":{"cost":{"Minus":6},"effects":[{"ExileTop":{"count":7,"as":"exiled"}},{"Choose":{"from":{"Each":[{"Is":{"Bound":"exiled"}},{"AnyOf":[{"Type":"Creature"},{"Type":"Land"}]}]},"count":{"min":0,"max":null},"as":"chosen"}},{"Move":{"what":{"Bound":"chosen"},"to":{"Battlefield":{}}}}]}}"#;
const C6_SZ_SALVATION_COLOSSUS: &str = r#"{"Triggered":{"when":{"PlayerAttacks":{"by":"You","at":"Opponents"}},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}]}}"#;
const C6_SZ_SALVATION_COLOSSUS_UNEARTH: &str = r#"{"Keyword":{"Unearth":[{"PayEnergy":8}]}}"#;
const C6_SZ_SUMMON_IXION: &str = r#"{"Triggered":{"when":{"Chapter":{"number":1}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Exile":{"what":{"Target":0},"duration":"WhileSourceOnBattlefield"}}]}}"#;
const C6_SZ_THE_SCARAB_GOD: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"LoseLife":{"who":"Opponents","amount":{"Count":[{"Subtype":"Zombie"},{"ControlledBy":"You"}]}}},{"Scry":{"count":{"Count":[{"Subtype":"Zombie"},{"ControlledBy":"You"}]}}}]}}"#;
const C6_SZ_THE_SCARAB_GOD_COPY: &str = r#"{"Activated":{"cost":"{2}{U}{B}","targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],"effects":[{"Exile":{"what":{"Target":0}}},{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"SetPT":{"power":4,"toughness":4}},{"SetColors":["Black"]},{"SetCreatureTypes":["Zombie"]}]}}}}]}}"#;
const C6_SZ_WITCH_OF_THE_MOORS: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"if":{"Compare":{"left":{"Tallied":{"who":"You","what":"LifeGained"}},"op":"Ge","right":1}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Sacrifice":{"who":"Opponents","filter":[{"Type":"Creature"}]}},{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#;

#[rstest]
#[case::sevinnes_reclamation(
    C6_SZ_SEVINNES_RECLAMATION,
    "Return target permanent card with mana value 3 or less from your graveyard to the battlefield. If this spell was cast from a graveyard, you may copy this spell and may choose a new target for the copy."
)]
#[case::slinn_voda_the_rising_deep(
    C6_SZ_SLINN_VODA_THE_RISING_DEEP,
    "When ~ enters, if it was kicked, return all creatures to their owners' hands except for Merfolk, Krakens, Leviathans, Octopuses, and Serpents."
)]
#[case::songbirds_blessing(
    C6_SZ_SONGBIRDS_BLESSING,
    "Whenever enchanted creature attacks, reveal cards from the top of your library until you reveal an Aura card. You may put that card onto the battlefield. If you don't, put it into your hand. Put the rest on the bottom of your library in a random order."
)]
#[case::spectral_deluge(
    C6_SZ_SPECTRAL_DELUGE,
    "Return each creature your opponents control with toughness X or less to its owner's hand, where X is the number of Islands you control."
)]
#[case::sporocyst(
    C6_SZ_SPOROCYST,
    "When this creature enters, search your library for up to X basic land cards, put them onto the battlefield tapped, then shuffle."
)]
#[case::sprouting_goblin(
    C6_SZ_SPROUTING_GOBLIN,
    "When this creature enters, if it was kicked, search your library for a land card with a basic land type, reveal it, put it into your hand, then shuffle."
)]
#[case::stoic_farmer(
    C6_SZ_STOIC_FARMER,
    "When this creature enters, search your library for a basic Plains card and reveal it. If an opponent controls more lands than you, put it onto the battlefield tapped. Otherwise, put it into your hand. Then shuffle."
)]
#[case::survival_cache(
    C6_SZ_SURVIVAL_CACHE,
    "You gain 2 life. Then if you have more life than an opponent, draw a card."
)]
#[case::sydri_galvanic_genius(
    C6_SZ_SYDRI_GALVANIC_GENIUS,
    "{U}: Target noncreature artifact becomes an artifact creature with power and toughness each equal to its mana value until end of turn."
)]
#[case::teval_the_balanced_scale(
    C6_SZ_TEVAL_THE_BALANCED_SCALE,
    "Whenever ~ attacks, mill three cards. Then you may return a land card from your graveyard to the battlefield tapped."
)]
#[case::the_flux(C6_SZ_THE_FLUX, "VI \u{2014} Add six {R}.")]
#[case::thundermaw_hellkite(
    C6_SZ_THUNDERMAW_HELLKITE,
    "When this creature enters, it deals 1 damage to each creature with flying your opponents control. Tap those creatures."
)]
#[case::titan_of_industry(
    C6_SZ_TITAN_OF_INDUSTRY,
    "When this creature enters, choose two \u{2014}\n\u{2022} Destroy target artifact or enchantment.\n\u{2022} Target player gains 5 life.\n\u{2022} Create a 4/4 green Rhino Warrior creature token.\n\u{2022} Put a shield counter on a creature you control."
)]
#[case::trostani_discordant(
    C6_SZ_TROSTANI_DISCORDANT,
    "At the beginning of your end step, each player gains control of all creatures they own."
)]
#[case::vile_mutilator(
    C6_SZ_VILE_MUTILATOR,
    "When this creature enters, each opponent sacrifices a nontoken enchantment of their choice, then sacrifices a nontoken creature of their choice."
)]
#[case::wakka_devoted_guardian(
    C6_SZ_WAKKA_DEVOTED_GUARDIAN,
    "Whenever ~ deals combat damage to a player, destroy up to one target artifact that player controls and put a +1/+1 counter on ~."
)]
#[case::wand_of_orcus(
    C6_SZ_WAND_OF_ORCUS,
    "Whenever equipped creature attacks or blocks, it and Zombies you control gain deathtouch until end of turn."
)]
#[case::wasitora_nekoru_queen(
    C6_SZ_WASITORA_NEKORU_QUEEN,
    "Whenever ~ deals combat damage to a player, that player sacrifices a creature of their choice. If the player can't, you create a 3/3 black, red, and green Cat Dragon creature token with flying."
)]
#[case::workshop_elders(
    C6_SZ_WORKSHOP_ELDERS,
    "At the beginning of combat on your turn, you may have target noncreature artifact you control become a 0/0 artifact creature. If you do, put four +1/+1 counters on it."
)]
#[case::tireless_provisioner(
    C6_SZ_TIRELESS_PROVISIONER,
    "Whenever a land you control enters, create a Food token or a Treasure token."
)]
#[case::sinister_gnarlbark(
    C6_SZ_SINISTER_GNARLBARK,
    "At the beginning of your end step, draw a card and blight 1."
)]
#[case::thickest_in_the_thicket(
    C6_SZ_THICKEST_IN_THE_THICKET,
    "At the beginning of your end step, draw two cards if you control the creature with the greatest power or tied for the greatest power."
)]
#[case::subterfuge(
    C6_SZ_SUBTERFUGE,
    "When this creature enters, target creature gains flying and \"Whenever this creature deals combat damage to a player, draw that many cards\" until end of turn."
)]
#[case::vault_87_forced_evolution(
    C6_SZ_VAULT_87_FORCED_EVOLUTION,
    "II \u{2014} Put a +1/+1 counter on target creature you control. It becomes a Mutant in addition to its other types."
)]
#[case::simic_ascendancy(
    C6_SZ_SIMIC_ASCENDANCY,
    "At the beginning of your upkeep, if this enchantment has twenty or more growth counters on it, you win the game."
)]
#[case::speaker_of_the_heavens(
    C6_SZ_SPEAKER_OF_THE_HEAVENS,
    "{T}: Create a 4/4 white Angel creature token with flying. Activate only if you have at least 7 life more than your starting life total and only as a sorcery."
)]
#[case::stella_lee_wild_card(
    C6_SZ_STELLA_LEE_WILD_CARD,
    "{T}: Copy target instant or sorcery spell you control. You may choose new targets for the copy. Activate only if you've cast three or more spells this turn."
)]
#[case::szarel_genesis_shepherd(
    C6_SZ_SZAREL_GENESIS_SHEPHERD,
    "Whenever you sacrifice another nontoken permanent during your turn, put a number of +1/+1 counters equal to ~'s power on up to one other target creature."
)]
#[case::thopter_assembly(
    C6_SZ_THOPTER_ASSEMBLY,
    "At the beginning of your upkeep, if you control no Thopters other than this creature, return this creature to its owner's hand and create five 1/1 colorless Thopter artifact creature tokens with flying."
)]
#[case::ulasht_the_hate_seed(
    C6_SZ_ULASHT_THE_HATE_SEED,
    "~ enters with a +1/+1 counter on it for each other red creature you control and a +1/+1 counter on it for each other green creature you control."
)]
#[case::uro_titan_of_natures_wrath(
    C6_SZ_URO_TITAN_OF_NATURES_WRATH,
    "When ~ enters, sacrifice it unless it escaped."
)]
#[case::well_of_ideas(
    C6_SZ_WELL_OF_IDEAS,
    "At the beginning of each other player's draw step, that player draws an additional card."
)]
#[case::wkabi_shield_of_the_nation(
    C6_SZ_WKABI_SHIELD_OF_THE_NATION,
    "Whenever you attack with your commander, if you control an artifact with mana value 4 or greater, create a 4/4 green Rhino creature token with trample."
)]
#[case::wulfgar_of_icewind_dale(
    C6_SZ_WULFGAR_OF_ICEWIND_DALE,
    "If a creature you control attacking causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time."
)]
#[case::xenagos_the_reveler(
    C6_SZ_XENAGOS_THE_REVELER,
    "\u{2212}6: Exile the top seven cards of your library. You may put any number of creature and/or land cards from among them onto the battlefield."
)]
#[case::salvation_colossus(
    C6_SZ_SALVATION_COLOSSUS,
    "Whenever you attack, other creatures you control get +2/+2 and gain indestructible until end of turn."
)]
#[case::salvation_colossus_unearth(
    C6_SZ_SALVATION_COLOSSUS_UNEARTH,
    "Unearth\u{2014}Pay eight {E}."
)]
#[case::summon_ixion(
    C6_SZ_SUMMON_IXION,
    "I \u{2014} Aerospark \u{2014} Exile target creature an opponent controls until this Saga leaves the battlefield."
)]
#[case::the_scarab_god(
    C6_SZ_THE_SCARAB_GOD,
    "At the beginning of your upkeep, each opponent loses X life and you scry X, where X is the number of Zombies you control."
)]
#[case::the_scarab_god_copy(
    C6_SZ_THE_SCARAB_GOD_COPY,
    "{2}{U}{B}: Exile target creature card from a graveyard. Create a token that's a copy of it, except it's a 4/4 black Zombie."
)]
#[case::witch_of_the_moors(
    C6_SZ_WITCH_OF_THE_MOORS,
    "At the beginning of your end step, if you gained life this turn, each opponent sacrifices a creature of their choice and you return up to one target creature card from your graveyard to your hand."
)]
fn c6_sz_the_clause_reads_back(#[case] json: &str, #[case] clause: &str) {
    assert!(c6_sz_written(json, clause), "{clause}");
}

#[rstest]
#[case::sevinnes_reclamation(
    C6_SZ_SEVINNES_RECLAMATION,
    "\"op\":\"Le\",\"to\":3",
    "\"op\":\"Le\",\"to\":2",
    "Return target permanent card with mana value 3 or less from your graveyard to the battlefield. If this spell was cast from a graveyard, you may copy this spell and may choose a new target for the copy."
)]
#[case::slinn_voda_the_rising_deep(
    C6_SZ_SLINN_VODA_THE_RISING_DEEP,
    "Serpent",
    "Spirit",
    "When ~ enters, if it was kicked, return all creatures to their owners' hands except for Merfolk, Krakens, Leviathans, Octopuses, and Serpents."
)]
#[case::songbirds_blessing(
    C6_SZ_SONGBIRDS_BLESSING,
    "\"Subtype\":\"Aura\"",
    "\"Subtype\":\"Equipment\"",
    "Whenever enchanted creature attacks, reveal cards from the top of your library until you reveal an Aura card. You may put that card onto the battlefield. If you don't, put it into your hand. Put the rest on the bottom of your library in a random order."
)]
#[case::spectral_deluge(
    C6_SZ_SPECTRAL_DELUGE,
    "\"Subtype\":\"Island\"",
    "\"Subtype\":\"Swamp\"",
    "Return each creature your opponents control with toughness X or less to its owner's hand, where X is the number of Islands you control."
)]
#[case::sporocyst(
    C6_SZ_SPOROCYST,
    "\"tapped\":true",
    "\"tapped\":false",
    "When this creature enters, search your library for up to X basic land cards, put them onto the battlefield tapped, then shuffle."
)]
#[case::sprouting_goblin(
    C6_SZ_SPROUTING_GOBLIN,
    "\"to\":\"Hand\"",
    "\"to\":\"Graveyard\"",
    "When this creature enters, if it was kicked, search your library for a land card with a basic land type, reveal it, put it into your hand, then shuffle."
)]
#[case::stoic_farmer(
    C6_SZ_STOIC_FARMER,
    "\"op\":\"Gt\"",
    "\"op\":\"Ge\"",
    "When this creature enters, search your library for a basic Plains card and reveal it. If an opponent controls more lands than you, put it onto the battlefield tapped. Otherwise, put it into your hand. Then shuffle."
)]
#[case::survival_cache(
    C6_SZ_SURVIVAL_CACHE,
    "\"amount\":2",
    "\"amount\":3",
    "You gain 2 life. Then if you have more life than an opponent, draw a card."
)]
#[case::sydri_galvanic_genius(
    C6_SZ_SYDRI_GALVANIC_GENIUS,
    "\"stat\":\"ManaValue\"",
    "\"stat\":\"Power\"",
    "{U}: Target noncreature artifact becomes an artifact creature with power and toughness each equal to its mana value until end of turn."
)]
#[case::teval_the_balanced_scale(
    C6_SZ_TEVAL_THE_BALANCED_SCALE,
    "\"count\":3",
    "\"count\":2",
    "Whenever ~ attacks, mill three cards. Then you may return a land card from your graveyard to the battlefield tapped."
)]
#[case::the_flux(C6_SZ_THE_FLUX, ",\"Red\"]", ",\"Green\"]", "VI \u{2014} Add six {R}.")]
#[case::thundermaw_hellkite(
    C6_SZ_THUNDERMAW_HELLKITE,
    "\"amount\":1",
    "\"amount\":2",
    "When this creature enters, it deals 1 damage to each creature with flying your opponents control. Tap those creatures."
)]
#[case::titan_of_industry(
    C6_SZ_TITAN_OF_INDUSTRY,
    "\"amount\":5",
    "\"amount\":4",
    "When this creature enters, choose two \u{2014}\n\u{2022} Destroy target artifact or enchantment.\n\u{2022} Target player gains 5 life.\n\u{2022} Create a 4/4 green Rhino Warrior creature token.\n\u{2022} Put a shield counter on a creature you control."
)]
#[case::trostani_discordant(
    C6_SZ_TROSTANI_DISCORDANT,
    "\"OwnedBy\"",
    "\"ControlledBy\"",
    "At the beginning of your end step, each player gains control of all creatures they own."
)]
#[case::vile_mutilator(
    C6_SZ_VILE_MUTILATOR,
    "{\"Type\":\"Enchantment\"}",
    "{\"Type\":\"Artifact\"}",
    "When this creature enters, each opponent sacrifices a nontoken enchantment of their choice, then sacrifices a nontoken creature of their choice."
)]
#[case::wakka_devoted_guardian(
    C6_SZ_WAKKA_DEVOTED_GUARDIAN,
    "\"max\":1",
    "\"max\":2",
    "Whenever ~ deals combat damage to a player, destroy up to one target artifact that player controls and put a +1/+1 counter on ~."
)]
#[case::wand_of_orcus(
    C6_SZ_WAND_OF_ORCUS,
    "\"Zombie\"",
    "\"Skeleton\"",
    "Whenever equipped creature attacks or blocks, it and Zombies you control gain deathtouch until end of turn."
)]
#[case::wasitora_nekoru_queen(
    C6_SZ_WASITORA_NEKORU_QUEEN,
    "\"power\":3",
    "\"power\":2",
    "Whenever ~ deals combat damage to a player, that player sacrifices a creature of their choice. If the player can't, you create a 3/3 black, red, and green Cat Dragon creature token with flying."
)]
#[case::workshop_elders(
    C6_SZ_WORKSHOP_ELDERS,
    "\"count\":4",
    "\"count\":3",
    "At the beginning of combat on your turn, you may have target noncreature artifact you control become a 0/0 artifact creature. If you do, put four +1/+1 counters on it."
)]
#[case::tireless_provisioner(
    C6_SZ_TIRELESS_PROVISIONER,
    "\"treasure\"",
    "\"clue\"",
    "Whenever a land you control enters, create a Food token or a Treasure token."
)]
#[case::sinister_gnarlbark(
    C6_SZ_SINISTER_GNARLBARK,
    "\"-1/-1\"",
    "\"+1/+1\"",
    "At the beginning of your end step, draw a card and blight 1."
)]
#[case::thickest_in_the_thicket(
    C6_SZ_THICKEST_IN_THE_THICKET,
    "\"count\":2",
    "\"count\":3",
    "At the beginning of your end step, draw two cards if you control the creature with the greatest power or tied for the greatest power."
)]
#[case::subterfuge(
    C6_SZ_SUBTERFUGE,
    "\"Flying\"",
    "\"Reach\"",
    "When this creature enters, target creature gains flying and \"Whenever this creature deals combat damage to a player, draw that many cards\" until end of turn."
)]
#[case::vault_87_forced_evolution(
    C6_SZ_VAULT_87_FORCED_EVOLUTION,
    "\"Mutant\"",
    "\"Zombie\"",
    "II \u{2014} Put a +1/+1 counter on target creature you control. It becomes a Mutant in addition to its other types."
)]
#[case::simic_ascendancy(
    C6_SZ_SIMIC_ASCENDANCY,
    "\"right\":20",
    "\"right\":10",
    "At the beginning of your upkeep, if this enchantment has twenty or more growth counters on it, you win the game."
)]
#[case::speaker_of_the_heavens(
    C6_SZ_SPEAKER_OF_THE_HEAVENS,
    "7]",
    "6]",
    "{T}: Create a 4/4 white Angel creature token with flying. Activate only if you have at least 7 life more than your starting life total and only as a sorcery."
)]
#[case::stella_lee_wild_card(
    C6_SZ_STELLA_LEE_WILD_CARD,
    "\"right\":3",
    "\"right\":2",
    "{T}: Copy target instant or sorcery spell you control. You may choose new targets for the copy. Activate only if you've cast three or more spells this turn."
)]
#[case::szarel_genesis_shepherd(
    C6_SZ_SZAREL_GENESIS_SHEPHERD,
    "\"if\":\"YourTurn\",",
    "",
    "Whenever you sacrifice another nontoken permanent during your turn, put a number of +1/+1 counters equal to ~'s power on up to one other target creature."
)]
#[case::thopter_assembly(
    C6_SZ_THOPTER_ASSEMBLY,
    "\"count\":5",
    "\"count\":4",
    "At the beginning of your upkeep, if you control no Thopters other than this creature, return this creature to its owner's hand and create five 1/1 colorless Thopter artifact creature tokens with flying."
)]
#[case::ulasht_the_hate_seed(
    C6_SZ_ULASHT_THE_HATE_SEED,
    "\"Green\"",
    "\"Blue\"",
    "~ enters with a +1/+1 counter on it for each other red creature you control and a +1/+1 counter on it for each other green creature you control."
)]
#[case::uro_titan_of_natures_wrath(
    C6_SZ_URO_TITAN_OF_NATURES_WRATH,
    "\"Escape\"",
    "\"Flashback\"",
    "When ~ enters, sacrifice it unless it escaped."
)]
#[case::well_of_ideas(
    C6_SZ_WELL_OF_IDEAS,
    "\"Opponents\"",
    "\"EachPlayer\"",
    "At the beginning of each other player's draw step, that player draws an additional card."
)]
#[case::wkabi_shield_of_the_nation(
    C6_SZ_WKABI_SHIELD_OF_THE_NATION,
    "\"to\":4",
    "\"to\":3",
    "Whenever you attack with your commander, if you control an artifact with mana value 4 or greater, create a 4/4 green Rhino creature token with trample."
)]
#[case::wulfgar_of_icewind_dale(
    C6_SZ_WULFGAR_OF_ICEWIND_DALE,
    "\"Attacks\"",
    "\"Blocks\"",
    "If a creature you control attacking causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time."
)]
#[case::xenagos_the_reveler(
    C6_SZ_XENAGOS_THE_REVELER,
    "\"count\":7",
    "\"count\":6",
    "\u{2212}6: Exile the top seven cards of your library. You may put any number of creature and/or land cards from among them onto the battlefield."
)]
#[case::salvation_colossus(
    C6_SZ_SALVATION_COLOSSUS,
    "\"power\":2",
    "\"power\":1",
    "Whenever you attack, other creatures you control get +2/+2 and gain indestructible until end of turn."
)]
#[case::salvation_colossus_unearth(
    C6_SZ_SALVATION_COLOSSUS_UNEARTH,
    "\"PayEnergy\":8",
    "\"PayEnergy\":7",
    "Unearth\u{2014}Pay eight {E}."
)]
#[case::summon_ixion(
    C6_SZ_SUMMON_IXION,
    "\"WhileSourceOnBattlefield\"",
    "\"Forever\"",
    "I \u{2014} Aerospark \u{2014} Exile target creature an opponent controls until this Saga leaves the battlefield."
)]
#[case::the_scarab_god(
    C6_SZ_THE_SCARAB_GOD,
    "\"Zombie\"",
    "\"Vampire\"",
    "At the beginning of your upkeep, each opponent loses X life and you scry X, where X is the number of Zombies you control."
)]
#[case::the_scarab_god_copy(
    C6_SZ_THE_SCARAB_GOD_COPY,
    "\"power\":4",
    "\"power\":5",
    "{2}{U}{B}: Exile target creature card from a graveyard. Create a token that's a copy of it, except it's a 4/4 black Zombie."
)]
#[case::witch_of_the_moors(
    C6_SZ_WITCH_OF_THE_MOORS,
    "\"LifeGained\"",
    "\"Drawn\"",
    "At the beginning of your end step, if you gained life this turn, each opponent sacrifices a creature of their choice and you return up to one target creature card from your graveyard to your hand."
)]
fn c6_sz_another_wording_does_not_read_back(
    #[case] json: &str,
    #[case] from: &str,
    #[case] to: &str,
    #[case] clause: &str,
) {
    let changed = json.replace(from, to);
    assert_ne!(changed, json, "the change finds nothing");
    assert!(!c6_sz_written(&changed, clause), "{clause}");
}
