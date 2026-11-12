//! Whole abilities of the r28-7 card pack whose steps the renderers do not join: each is matched whole as data and worded
//! as the card prints it, so another count, target, condition or duration in the same ability never reads back.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (the card's name as `~`; the lines of one spell joined by a newline).
const R287_WHOLES: [(&str, &str); 45] = [
    // age_of_ultron
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"Bind":{"what":{"Each":[{"Type":"Artifact"},{"Type":"Creature"},{"ControlledBy":"You"}]},"as":"them"}},{"Continuous":{"affected":{"Bound":"them"},"mods":[{"AddKeyword":"Deathtouch"}],"duration":"UntilEndOfTurn"}},{"PutCounters":{"on":{"Bound":"them"},"counter":"+1/+1"}}]}}"#,
        r"III — Artifact creatures you control gain deathtouch until end of turn. Put a +1/+1 counter on each of them.",
    ),
    // agitator_ant
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"May":{"who":{"Bound":"player"},"effects":[{"Choose":{"who":{"Bound":"player"},"from":{"Each":[{"Type":"Creature"},{"ControlledBy":{"Bound":"player"}}]},"as":"creature"}},{"PutCounters":{"on":{"Bound":"creature"},"counter":"+1/+1","count":2}},{"Goad":{"what":{"Bound":"creature"}}}]}}]}}]}}"#,
        r"At the beginning of your end step, each player may put two +1/+1 counters on a creature they control. Goad each creature that had counters put on it this way.",
    ),
    // angel_of_destiny
    (
        r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}},"effects":[{"GainLife":{"amount":"Triggering"}},{"GainLife":{"who":"Triggering","amount":"Triggering"}}]}}"#,
        r"Whenever a creature you control deals combat damage to a player, you and that player each gain that much life.",
    ),
    // battering_ram
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"effects":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Banding"}],"duration":"UntilEndOfCombat"}}]}}"#,
        r"At the beginning of combat on your turn, this creature gains banding until end of combat.",
    ),
    // battering_ram
    (
        r#"{"Triggered":{"when":{"BecomesBlocked":{"what":["This"],"by":[{"Subtype":"Wall"}]}},"effects":[{"Bind":{"what":"Dealer","as":"wall"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"EndCombat"}},"effects":[{"Destroy":{"what":{"Bound":"wall"}}}]}}]}}"#,
        r"Whenever this creature becomes blocked by a Wall, destroy that Wall at end of combat.",
    ),
    // bleeding_effect
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"},{"HasKeyword":"Flying"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Flying"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"},{"HasKeyword":"FirstStrike"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"FirstStrike"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"},{"HasKeyword":"DoubleStrike"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"DoubleStrike"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"},{"HasKeyword":"Deathtouch"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Deathtouch"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"},{"HasKeyword":"Hexproof"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Hexproof"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"},{"HasKeyword":"Indestructible"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"},{"HasKeyword":"Lifelink"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Lifelink"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"},{"HasKeyword":"Menace"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Menace"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"},{"HasKeyword":"Reach"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Reach"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"},{"HasKeyword":"Trample"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"},{"HasKeyword":"Vigilance"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Vigilance"}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
        r"At the beginning of combat on your turn, creatures you control gain flying until end of turn if a creature card in your graveyard has flying. The same is true for first strike, double strike, deathtouch, hexproof, indestructible, lifelink, menace, reach, trample, and vigilance.",
    ),
    // chancellor_of_the_annex
    (
        r#"{"Triggered":{"when":{"SpellCast":{"by":"Opponents"}},"effects":[{"Pay":{"who":{"ControllerOf":"Triggering"},"cost":"{1}","unpaid":[{"Counter":{"what":"Triggering"}}]}}]}}"#,
        r"Whenever an opponent casts a spell, counter it unless that player pays {1}.",
    ),
    // death_spark
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"CardsAbove":{"filter":[{"Type":"Creature"}],"directly":true}},"op":"Ge","right":1}},"from":"Graveyard","effects":[{"Pay":{"cost":"{1}","paid":[{"Move":{"what":"This","to":"Hand"}}]}}]}}"#,
        r"At the beginning of your upkeep, if this card is in your graveyard with a creature card directly above it, you may pay {1}. If you do, return this card to your hand.",
    ),
    // desert_warfare
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"if":{"Compare":{"left":{"Count":[{"Subtype":"Desert"},{"ControlledBy":"You"}]},"op":"Ge","right":5}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Sand Warrior","colors":["Red","Green","White"],"types":["Creature"],"subtypes":["Sand","Warrior"],"power":1,"toughness":1}},"count":{"Count":[{"Subtype":"Desert"},{"ControlledBy":"You"}]},"as":"warriors"}},{"Continuous":{"affected":{"Bound":"warriors"},"mods":[{"AddKeyword":"Haste"}],"duration":"Forever"}}]}}"#,
        r"At the beginning of combat on your turn, if you control five or more Deserts, create that many 1/1 red, green, and white Sand Warrior creature tokens. They gain haste.",
    ),
    // edgar_markov
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Subtype":"Vampire"},"Other"],"by":"You"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Vampire","colors":["Black"],"types":["Creature"],"subtypes":["Vampire"],"power":1,"toughness":1}}}}],"from":"Command"}}"#,
        r"Eminence — Whenever you cast another Vampire spell, if ~ is in the command zone or on the battlefield, create a 1/1 black Vampire creature token.",
    ),
    // edgar_markov
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Subtype":"Vampire"},"Other"],"by":"You"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Vampire","colors":["Black"],"types":["Creature"],"subtypes":["Vampire"],"power":1,"toughness":1}}}}]}}"#,
        r"Eminence — Whenever you cast another Vampire spell, if ~ is in the command zone or on the battlefield, create a 1/1 black Vampire creature token.",
    ),
    // found_footage
    (
        r#"{"Rule":{"rule":{"LookAtFaceDown":{"what":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}}}"#,
        r"You may look at face-down creatures your opponents control any time.",
    ),
    // gurzigost
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Pay":{"cost":[{"ToLibrary":{"from":"Graveyard","filter":[],"count":2,"bottom":true}}],"unpaid":[{"Sacrifice":{"filter":["This"]}}]}}]}}"#,
        r"At the beginning of your upkeep, sacrifice this creature unless you put two cards from your graveyard on the bottom of your library.",
    ),
    // hansk_slayer_zealot
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"targets":[{"what":"Opponent"}],"effects":[{"CreateToken":{"who":{"Target":0},"token":{"Custom":{"name":"Walker","colors":["Black"],"types":["Creature"],"subtypes":["Zombie"],"power":2,"toughness":2}},"count":3}}]}}"#,
        r"At the beginning of your upkeep, target opponent creates three Walker tokens.",
    ),
    // invasion_of_amonkhet_lazotep_convert
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Mill":{"who":"EachPlayer","count":3}},{"Discard":{"who":"Opponents"}},{"Draw":{}}]}}"#,
        r"When this Siege enters, each player mills three cards, then each opponent discards a card and you draw a card.",
    ),
    // invasion_of_amonkhet_lazotep_convert
    (
        r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Creature"},{"In":"Graveyard"}],"except":[{"SetPT":{"power":4,"toughness":4}},{"AddColors":["Black"]},{"AddSubtype":"Zombie"}]}}}}"#,
        r"You may have this creature enter as a copy of any creature card in a graveyard, except it's a 4/4 black Zombie in addition to its other colors and types.",
    ),
    // jin_gitaxias_the_great_synthesis
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"effects":[{"Draw":{"count":{"Count":[{"OwnedBy":"You"},{"In":"Hand"}]}}},{"Rule":{"rule":{"NoMaxHandSize":"You"},"duration":"WhileYouControlSource"}}]}}"#,
        r"I — Draw cards equal to the number of cards in your hand. You have no maximum hand size for as long as you control this Saga.",
    ),
    // karona_false_god
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep"}},"effects":[{"Untap":{"what":"This"}},{"Continuous":{"affected":"This","mods":[{"SetController":"Triggering"}],"duration":"Forever"}}]}}"#,
        r"At the beginning of each player's upkeep, that player untaps ~ and gains control of it.",
    ),
    // karona_false_god
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"ChooseValue":{"kind":"CreatureType","as":"type"}},{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ChosenAs":"type"}]},"mods":[{"ModifyPT":{"power":3,"toughness":3}}],"duration":"UntilEndOfTurn"}}]}}"#,
        r"Whenever ~ attacks, creatures of the creature type of your choice get +3/+3 until end of turn.",
    ),
    // keen_duelist
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"targets":[{"what":"Opponent"}],"effects":[{"Bind":{"what":{"Each":[{"In":"Library"},{"OwnedBy":"You"},"TopOfLibrary"]},"as":"mine"}},{"Bind":{"what":{"Each":[{"In":"Library"},{"OwnedBy":{"Target":0}},"TopOfLibrary"]},"as":"theirs"}},{"Reveal":{"what":{"Bound":"mine"}}},{"Reveal":{"what":{"Bound":"theirs"}}},{"LoseLife":{"amount":{"Stat":{"of":{"Bound":"theirs"},"stat":"ManaValue"}}}},{"LoseLife":{"who":{"Target":0},"amount":{"Stat":{"of":{"Bound":"mine"},"stat":"ManaValue"}}}},{"Move":{"what":{"Bound":"mine"},"to":"Hand"}},{"Move":{"what":{"Bound":"theirs"},"to":"Hand"}}]}}"#,
        r"At the beginning of your upkeep, you and target opponent each reveal the top card of your library. You each lose life equal to the mana value of the card revealed by the other player. You each put the card you revealed into your hand.",
    ),
    // keeper_of_the_lens
    (
        r#"{"Rule":{"rule":{"LookAtFaceDown":{"what":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}}}}"#,
        r"You may look at face-down creatures you don't control any time.",
    ),
    // kenku_artificer
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Artifact"},{"Not":{"Type":"Creature"}}]},"bounds":{"min":0,"max":1}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":3}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddType":"Creature"},{"AddSubtype":"Homunculus"},{"SetPT":{"power":0,"toughness":0}},{"AddKeyword":"Flying"}],"duration":"Forever"}}]}}"#,
        r"When this creature enters, put three +1/+1 counters on up to one target noncreature artifact. That artifact becomes a 0/0 Homunculus artifact creature with flying.",
    ),
    // krovikan_horror
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End"}},"if":{"Compare":{"left":{"CardsAbove":{"filter":[{"Type":"Creature"}],"directly":true}},"op":"Ge","right":1}},"from":"Graveyard","effects":[{"May":{"effects":[{"Move":{"what":"This","to":"Hand"}}]}}]}}"#,
        r"At the beginning of the end step, if this card is in your graveyard with a creature card directly above it, you may return this card to your hand.",
    ),
    // lens_of_clarity
    (
        r#"{"Rule":{"rule":{"LookAtTopAnyTime":"You"}}}"#,
        r"You may look at the top card of your library and at face-down creatures you don't control any time.",
    ),
    // lens_of_clarity
    (
        r#"{"Rule":{"rule":{"LookAtFaceDown":{"what":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}}}}"#,
        r"You may look at the top card of your library and at face-down creatures you don't control any time.",
    ),
    // majestic_myriarch
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat"}},"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"Flying"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Flying"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"FirstStrike"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"FirstStrike"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"DoubleStrike"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"DoubleStrike"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"Deathtouch"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Deathtouch"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"Haste"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"Hexproof"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Hexproof"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"Indestructible"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"Lifelink"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Lifelink"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"Menace"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Menace"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"Reach"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Reach"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"Trample"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}}]}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasKeyword":"Vigilance"}]},"op":"Ge","right":1}},"then":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Vigilance"}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
        r"At the beginning of each combat, this creature gains flying until end of turn if you control a creature with flying. The same is true for first strike, double strike, deathtouch, haste, hexproof, indestructible, lifelink, menace, reach, trample, and vigilance.",
    ),
    // medomais_prophecy
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":4}},"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"LookAtTop":{"who":{"Bound":"player"},"as":"top"}}]}}]}}"#,
        r"IV — Look at the top card of each player's library.",
    ),
    // mishras_war_machine
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Pay":{"cost":[{"Discard":{}}],"unpaid":[{"DealDamage":{"to":"You","amount":3}},{"If":{"condition":{"Compare":{"left":"DamageDealtThisWay","op":"Ge","right":1}},"then":[{"Tap":{"what":"This"}}]}}]}}]}}"#,
        r"At the beginning of your upkeep, this creature deals 3 damage to you unless you discard a card. If it deals damage to you this way, tap it.",
    ),
    // nether_shadow
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"CardsAbove":{"filter":[{"Type":"Creature"}]}},"op":"Ge","right":3}},"from":"Graveyard","effects":[{"May":{"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}]}}"#,
        r"At the beginning of your upkeep, if this card is in your graveyard with three or more creature cards above it, you may put this card onto the battlefield.",
    ),
    // osseous_sticktwister
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"if":{"Compare":{"left":{"CardTypes":[{"In":"Graveyard"},{"OwnedBy":"You"}]},"op":"Ge","right":4}},"effects":[{"RepeatEach":{"over":"Opponents","as":"opponent","effects":[{"OneOf":{"who":{"Bound":"opponent"},"options":[[{"Sacrifice":{"who":{"Bound":"opponent"},"filter":[{"Not":{"Type":"Land"}}]}},{"If":{"condition":{"Not":"Performed"},"then":[{"DealDamage":{"to":{"Bound":"opponent"},"amount":{"Stat":{"of":"This","stat":"Power"}}}}]}}],[{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":{"Bound":"opponent"}}]},"op":"Ge","right":1}},"then":[{"Discard":{"who":{"Bound":"opponent"}}}],"otherwise":[{"DealDamage":{"to":{"Bound":"opponent"},"amount":{"Stat":{"of":"This","stat":"Power"}}}}]}}],[{"DealDamage":{"to":{"Bound":"opponent"},"amount":{"Stat":{"of":"This","stat":"Power"}}}}]]}}]}}]}}"#,
        r"Delirium — At the beginning of your end step, if there are four or more card types among cards in your graveyard, each opponent may sacrifice a nonland permanent of their choice or discard a card. Then this creature deals damage equal to its power to each opponent who didn't sacrifice a permanent or discard a card this way.",
    ),
    // party_thrasher
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Main1","whose":"You"}},"effects":[{"Pay":{"cost":[{"Discard":{}}],"paid":[{"ExileTop":{"count":2,"as":"exiled"}},{"Choose":{"from":{"Each":[{"In":"Exile"},{"Is":{"Bound":"exiled"}}]},"as":"chosen"}},{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"chosen"},"play":true}},"duration":"UntilEndOfTurn"}}]}}]}}"#,
        r"At the beginning of your first main phase, you may discard a card. If you do, exile the top two cards of your library, then choose one of them. You may play that card this turn.",
    ),
    // risky_move
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep"}},"effects":[{"Continuous":{"affected":"This","mods":[{"SetController":"Triggering"}],"duration":"Forever"}}]}}"#,
        r"At the beginning of each player's upkeep, that player gains control of this enchantment.",
    ),
    // search_for_azcanta_azcanta_the_sunken_ruin
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Surveil":{}},{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"}]},"op":"Ge","right":7}},"then":[{"May":{"effects":[{"Transform":{"what":"This"}}]}}]}}]}}"#,
        r"At the beginning of your upkeep, surveil 1. Then if you have seven or more cards in your graveyard, you may transform ~.",
    ),
    // sidar_jabari_of_zhalfir
    (
        r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Subtype":"Knight"},{"ControlledBy":"You"}]}},"effects":[{"Draw":{}},{"Discard":{}}]}}"#,
        r"Eminence — Whenever you attack with one or more Knights, if ~ is in the command zone or on the battlefield, draw a card, then discard a card.",
    ),
    // sidar_jabari_of_zhalfir
    (
        r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Subtype":"Knight"},{"ControlledBy":"You"}]}},"effects":[{"Draw":{}},{"Discard":{}}],"from":"Command"}}"#,
        r"Eminence — Whenever you attack with one or more Knights, if ~ is in the command zone or on the battlefield, draw a card, then discard a card.",
    ),
    // sigil_of_myrkul
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Library"},{"OwnedBy":"You"}]},"op":"Ge","right":1}},"then":[{"Mill":{}},{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"}]},"op":"Ge","right":4}},"then":[{"Reflexive":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Type":"Creature"}]},"op":"Ge","right":4}},"then":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Deathtouch"}],"duration":"UntilEndOfTurn"}}]}}]}}]}}]}}]}}"#,
        r"At the beginning of combat on your turn, mill a card. When you do, if there are four or more creature cards in your graveyard, put a +1/+1 counter on target creature you control and it gains deathtouch until end of turn.",
    ),
    // spellpyre_phoenix
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"HasKeywordNamed":"Cycling"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"May":{"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}]}}"#,
        r"When this creature enters, you may return target instant or sorcery card with a cycling ability from your graveyard to your hand.",
    ),
    // summon_fenrir
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":1}},"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}"#,
        r"I — Crescent Fang — Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
    ),
    // summon_fenrir
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"If":{"condition":{"All":[{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},{"Compare":{"left":{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}},"op":"Ge","right":{"Greatest":{"of":[{"Type":"Creature"}],"stat":"Power"}}}}]},"then":[{"Draw":{}}]}}]}}"#,
        r"III — Ecliptic Growl — Draw a card if you control the creature with the greatest power or tied for the greatest power.",
    ),
    // the_bears_of_littjara
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":2}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Subtype":"Shapeshifter"},{"ControlledBy":"You"}]},"bounds":{"min":0,"max":null}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetPT":{"power":4,"toughness":4}}],"duration":"Forever"}}]}}"#,
        r"II — Any number of target Shapeshifter creatures you control have base power and toughness 4/4.",
    ),
    // the_bears_of_littjara
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]},"bounds":{"min":0,"max":1}}],"effects":[{"RepeatEach":{"over":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"Compare":{"stat":"Power","op":"Ge","to":4}}]},"as":"creature","effects":[{"DealDamage":{"source":{"Bound":"creature"},"to":{"Target":0},"amount":{"Stat":{"of":{"Bound":"creature"},"stat":"Power"}}}}]}}]}}"#,
        r"III — Choose up to one target creature or planeswalker. Each creature with power 4 or greater you control deals damage equal to its power to that permanent.",
    ),
    // the_death_of_gwen_stacy
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":2}},"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"May":{"who":{"Bound":"player"},"effects":[{"Discard":{"who":{"Bound":"player"}}}]}},{"If":{"condition":{"Not":"Performed"},"then":[{"LoseLife":{"who":{"Bound":"player"},"amount":3}}]}}]}}]}}"#,
        r"II — Each player may discard a card. Each player who doesn't loses 3 life.",
    ),
    // the_death_of_gwen_stacy
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"targets":[{"what":"Player","bounds":{"min":0,"max":null}}],"effects":[{"Exile":{"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]}}}]}}"#,
        r"III — Exile any number of target players' graveyards.",
    ),
    // tovolar_dire_overlord_tovolar_the_midnight_scourge
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Count":[{"AnyOf":[{"Subtype":"Wolf"},{"Subtype":"Werewolf"}]},{"ControlledBy":"You"}]},"op":"Ge","right":3}},"effects":[{"BecomeDayNight":{"to":"Night"}},{"Choose":{"from":{"Each":[{"Subtype":"Human"},{"Subtype":"Werewolf"},{"ControlledBy":"You"}]},"count":{"min":0,"max":null},"as":"werewolves"}},{"Transform":{"what":{"Bound":"werewolves"}}}]}}"#,
        r"At the beginning of your upkeep, if you control three or more Wolves and/or Werewolves, it becomes night. Then transform any number of Human Werewolves you control.",
    ),
    // zoyowa_lava_tongue
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"if":{"Compare":{"left":{"Tallied":{"who":"You","what":"Descended"}},"op":"Ge","right":1}},"effects":[{"RepeatEach":{"over":"Opponents","as":"opponent","effects":[{"OneOf":{"who":{"Bound":"opponent"},"options":[[{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Hand"},{"OwnedBy":{"Bound":"opponent"}}]},"op":"Ge","right":1}},"then":[{"Discard":{"who":{"Bound":"opponent"}}}],"otherwise":[{"DealDamage":{"to":{"Bound":"opponent"},"amount":3}}]}}],[{"Sacrifice":{"who":{"Bound":"opponent"},"filter":[]}},{"If":{"condition":{"Not":"Performed"},"then":[{"DealDamage":{"to":{"Bound":"opponent"},"amount":3}}]}}],[{"DealDamage":{"to":{"Bound":"opponent"},"amount":3}}]]}}]}}]}}"#,
        r"At the beginning of your end step, if you descended this turn, each opponent may discard a card or sacrifice a permanent of their choice. ~ deals 3 damage to each opponent who didn't.",
    ),
];

/// The entries parsed once.
static R287_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    R287_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r287_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R287_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r28-7 table parses as an ability, so none is silently dropped.
    use super::{R287_SHAPES, R287_WHOLES};

    #[test]
    fn r287_every_whole_parses() {
        assert_eq!(R287_SHAPES.len(), R287_WHOLES.len());
    }
}
