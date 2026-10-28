//! Whole abilities of the stored proposals of the names A to L that no writer words: each entry is an ability matched
//! whole as data and worded as the card prints it ("Whenever a creature attacks one of your opponents, that player loses
//! 1 life.").

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording.
const C11F_WHOLES: [(&str, &str); 98] = [
    (
        r#"{"Triggered":{"when":{"AttacksPlayer":{"what":[{"Type":"Creature"}],"at":"Opponents"}},"effects":[{"LoseLife":{"who":"Attacked","amount":1}}]}}"#,
        "Whenever a creature attacks one of your opponents, that player loses 1 life.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetColors":["Red"]},{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}},{"Draw":{}}]}}"#,
        "Target creature becomes red and gains haste until end of turn.\nDraw a card.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Exile":{"what":{"Each":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]},"as":"exiled"}},{"GainLife":{"amount":{"Times":{"by":3,"of":{"Count":[{"Is":{"Bound":"exiled"}}]}}}}}]}}"#,
        "Exile all creature cards from target player's graveyard. You gain 3 life for each card exiled this way.",
    ),
    (
        r#"{"Spell":{"effects":[{"LookAtTop":{"count":5,"as":"top"}},{"Choose":{"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}}]},"as":"kept","rest_as":"rest"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":{"Library":{"place":"Top","order":"Chosen"}}}}]}}"#,
        "Look at the top five cards of your library. Put one of them into your hand and the rest on top of your library in any order.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Tap":{"what":{"Target":0}}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"op":"Ge","right":3}},"then":[{"Exile":{"what":{"Target":0}}}]}}]}}"#,
        "Tap target creature.\nIf you control three or more artifacts, exile that creature.",
    ),
    (
        r#"{"Activated":{"cost":[{"Mana":"{2}{C}"}],"targets":[{"what":{"Object":[{"Type":"Creature"},"Other"]}}],"effects":[{"Exile":{"what":{"Target":0},"as":"exiled"}},{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{"tapped":true}}}}]}}"#,
        "{2}{C}: Exile another target creature, then return it to the battlefield tapped under its owner's control.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},{"AnyOf":[{"Color":"Green"},{"Color":"White"}]},{"ControlledBy":"You"}]},"as":"returned"}},{"Move":{"what":{"Bound":"returned"},"to":"Hand"}}]}}"#,
        "When this creature enters, return a green or white creature you control to its owner's hand.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"May":{"effects":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"as":"discarded"}},{"Draw":{"count":{"Count":[{"Is":{"Bound":"discarded"}}]}}}]}}]}}"#,
        "At the beginning of your upkeep, you may discard all the cards in your hand. If you do, draw that many cards.",
    ),
    (
        r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Creature"}],"except":[{"GrantAbility":{"Activated":{"cost":"{X}","effects":[{"Continuous":{"affected":"This","mods":[{"SetPT":{"power":"X","toughness":"X"}}],"duration":"Forever"}}]}}}]}}}}"#,
        "You may have this creature enter as a copy of any creature on the battlefield, except it has \"{X}: This creature has base power and toughness X/X.\"",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Land Mine","types":["Artifact"],"abilities":[{"Activated":{"cost":[{"Mana":"{R}"},"SacrificeSelf"],"targets":[{"what":{"Object":[{"Type":"Creature"},"Attacking",{"Not":{"HasKeyword":"Flying"}}]}}],"effects":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":2}}]}}]}}}},{"FlipCoin":{"lose":[{"DealDamage":{"source":"This","to":"This","amount":2}}]}}]}}"#,
        "At the beginning of your upkeep, create a colorless artifact token named Land Mine with \"{R}, Sacrifice this token: This token deals 2 damage to target attacking creature without flying.\" Then flip a coin. If you lose the flip, this creature deals 2 damage to itself.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"Not":{"Color":"Black"}}],"to":"Battlefield"}},"effects":[{"Sacrifice":{"filter":["This"]}},{"If":{"condition":"Performed","then":[{"Destroy":{"what":"Triggering"}}]}}]}}"#,
        "When a nonblack creature enters, sacrifice this enchantment. If you do, destroy that creature.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"targets":[{"what":{"Object":["Token",{"ControlledBy":"You"}]}}],"effects":[{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Subtype":"Squirrel"}]}},"then":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}},"count":2}}],"otherwise":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0}}}}}]}}]}}"#,
        "At the beginning of your end step, create a token that's a copy of target token you control. If that token is a Squirrel, instead create two tokens that are copies of it.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Counters":{"on":"This","counter":"+1/+1"}},"op":"Ge","right":1}},"effects":[{"Pay":{"cost":[{"RemoveCounters":{"counter":"+1/+1"}}],"paid":[{"Draw":{}}]}}]}}"#,
        "At the beginning of your upkeep, if this creature has one or more +1/+1 counters on it, you may remove a +1/+1 counter from it. If you do, draw a card.",
    ),
    (
        r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap"],"condition":{"Compare":{"left":{"Largest":{"who":"Opponents","of":[{"Type":"Land"}]}},"op":"Ge","right":{"Sum":[{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]},2]}}},"effects":[{"Scry":{}},{"May":{"effects":[{"LookAtTop":{"as":"top"}},{"Reveal":{"what":{"Bound":"top"}}},{"If":{"condition":{"Matches":{"subject":{"Bound":"top"},"selector":[{"Supertype":"Basic"},{"Type":"Land"}]}},"then":[{"Move":{"what":{"Bound":"top"},"to":{"Battlefield":{"tapped":true}}}}]}}]}}]}}"#,
        "{2}, {T}: Scry 1, then you may reveal the top card of your library. If a basic land card is revealed this way, put it onto the battlefield tapped. Activate only if an opponent controls at least two more lands than you.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"You"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]}]},"bounds":{"min":0,"max":1}}],"effects":[{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":2}},{"Pay":{"cost":[{"PayEnergy":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}],"paid":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}}]}}"#,
        "Choose up to one target artifact or creature card in your graveyard. You get {E}{E}. Then you may pay an amount of {E} equal to that card's mana value. If you do, return it from your graveyard to the battlefield.",
    ),
    (
        r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":5,"toughness":5}},{"AddKeyword":"FirstStrike"},{"AddKeyword":"Trample"},{"AddKeyword":"Indestructible"},{"AddKeyword":"Haste"},{"GrantAbility":{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":{"Each":[{"Type":"Creature"}]},"combat":true}},"effects":[{"Exile":{"what":"Triggering"}}]}}}]}}"#,
        "Equipped creature gets +5/+5 and has first strike, trample, indestructible, haste, and \"Whenever this creature deals combat damage to a creature, exile that creature.\"",
    ),
    (
        r#"{"Spell":{"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"}]},"op":"Ge","right":7}},"then":[{"Destroy":{"what":{"Each":[{"Type":"Creature"}]},"regenerate":false}},{"CreateToken":{"token":{"Custom":{"name":"Spirit","colors":["White"],"types":["Creature"],"subtypes":["Spirit"],"power":1,"toughness":1,"abilities":[{"Keyword":"Flying"}]}},"count":2}}],"otherwise":[{"Destroy":{"what":{"Each":[{"Type":"Creature"}]},"regenerate":false}}]}}]}}"#,
        "Destroy all creatures. They can't be regenerated.\nIf there are seven or more cards in your graveyard, instead destroy all creatures, then create two 1/1 white Spirit creature tokens with flying. Creatures destroyed this way can't be regenerated.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"LookAtTop":{"count":"X","as":"top"}},{"Reveal":{"what":{"Bound":"top"}}},{"Choose":{"from":{"Each":[{"Is":{"Bound":"top"}},{"Subtype":"Aura"},{"Compare":{"stat":"ManaValue","op":"Le","to":"X"}}]},"count":{"min":0,"max":null},"as":"auras","rest_as":"rest"}},{"Move":{"what":{"Bound":"auras"},"to":{"Battlefield":{}}}},{"Move":{"what":{"Each":[{"Is":{"Bound":"top"}},{"Not":{"Is":{"Bound":"auras"}}}]},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#,
        "When this creature enters, reveal the top X cards of your library. You may put any number of Aura cards with mana value X or less from among them onto the battlefield. Then put all cards revealed this way that weren't put onto the battlefield on the bottom of your library in a random order.",
    ),
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Artifact"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"If":{"condition":"Performed","then":[{"Draw":{"who":{"ControllerOf":{"Target":0}}}}]}}]}}"#,
        "Whenever this creature attacks, destroy up to one target artifact. If an artifact is destroyed this way, its controller draws a card.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"Not":"Token"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Move":{"what":"Triggering","to":{"Battlefield":{"under":"You"}}}}]}}]}}"#,
        "Whenever ~ or another nontoken creature you control dies, return that card to the battlefield under your control at the beginning of the next end step.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["Other",{"ControlledBy":"You"},{"Not":"Token"},{"Type":"Artifact"},{"AnyOf":[{"Type":"Creature"},{"Subtype":"Vehicle"}]}],"to":"Battlefield"}},"effects":[{"Draw":{}}]}}"#,
        "Whenever another nontoken artifact creature or Vehicle you control enters, draw a card.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Subtype":"Hero"},"Other",{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"Continuous":{"affected":"Triggering","mods":[{"AddKeyword":"Vigilance"},{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}},{"PutCounters":{"on":"Triggering","counter":"+1/+1"}},{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#,
        "Whenever another Hero you control enters, it gains vigilance and haste until end of turn. Put a +1/+1 counter on that Hero and a +1/+1 counter on ~.",
    ),
    (
        r#"{"Triggered":{"when":{"PlayerAttacks":{"by":"You","at":"Opponents"}},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasCounter":{"Other":"x"}}]},"mods":[{"AddKeyword":"DoubleStrike"}],"duration":"UntilEndOfTurn"}}]}}"#,
        "Whenever you attack, creatures you control with counters on them gain double strike until end of turn.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Exile":{"what":{"Target":0}}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Subtype":"Human"},{"ControlledBy":"You"}]},"op":"Eq","right":0}},"then":[{"LoseLife":{"amount":{"Stat":{"of":{"Target":0},"stat":"Toughness"}}}}]}}]}}"#,
        "Exile target creature. If you don't control a Human, you lose life equal to that creature's toughness.",
    ),
    (
        r#"{"Activated":{"cost":"{X}{B}{B}","targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":0,"toughness":{"Times":{"by":-1,"of":"X"}}}}],"duration":"UntilEndOfTurn"}},{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":"X","toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#,
        "{X}{B}{B}: Target creature gets -0/-X until end of turn and ~ gets +X/+0 until end of turn.",
    ),
    (
        r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Bird","colors":["White"],"types":["Creature"],"subtypes":["Bird"],"power":1,"toughness":1,"abilities":[{"Keyword":"Flying"}]}},"count":"Triggering"}},{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#,
        "Whenever ~ deal combat damage to a player, create that many 1/1 white Bird creature tokens with flying, then put a +1/+1 counter on ~.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Is":"Attached"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Move":{"what":"Triggering","to":{"Battlefield":{"under":"You"}}}}]}}"#,
        "When enchanted creature dies, return that card to the battlefield under your control.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}},"targets":[{"what":"Player"}],"effects":[{"Goad":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":{"Target":0}}]}}}]}}"#,
        "Whenever a land you control enters, goad each creature target player controls.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"targets":[{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"May":{"effects":[{"Exile":{"what":"This"}}]}},{"If":{"condition":"Performed","then":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}]}}"#,
        "When this creature dies, you may exile it. If you do, return target card from your graveyard to your hand.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Subtype":"Equipment"},{"ControlledBy":"You"}],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"May":{"effects":[{"Attach":{"what":"Triggering","to":{"Target":0}}}]}}]}}"#,
        "Whenever ~ or another Equipment you control enters, you may attach that Equipment to target creature you control.",
    ),
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"Attacks":{"what":["This"]}},{"Blocks":{"what":["This"]}}]},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Cat Warrior","colors":["Green"],"types":["Creature"],"subtypes":["Cat","Warrior"],"power":2,"toughness":2,"abilities":[{"Keyword":{"Landwalk":[{"Subtype":"Forest"}]}}]}}}}]}}"#,
        "Whenever Jedit Ojanen attacks or blocks, create a 2/2 green Cat Warrior creature token with forestwalk.",
    ),
    (
        r#"{"Activated":{"cost":[{"Mana":"{G}{U}"}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":["CantBeBlocked"],"duration":"UntilEndOfTurn"}},{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Subtype":"Snake"}]}},"then":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":2,"toughness":2}}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
        "{G}{U}: Target creature can't be blocked this turn. If that creature is a Snake, it gets +2/+2 until end of turn.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"GainLife":{"amount":{"Stat":{"of":"This","stat":"Power"}}}},{"Draw":{"count":{"Stat":{"of":"This","stat":"Power"}}}}]}}"#,
        "When this creature dies, you gain life and draw cards equal to its power.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Opponent"}],"effects":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":{"Count":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}}]}}"#,
        "When this creature enters, it deals 1 damage to target opponent for each creature card in your graveyard.",
    ),
    (
        r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"effects":[{"GainLife":{"amount":{"Tallied":{"who":"You","what":"SpellsCast"}}}}]}}"#,
        "Whenever you cast a spell, you gain 1 life for each spell you've cast this turn.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}},"targets":[{"what":"AnyTarget"}],"effects":[{"If":{"condition":{"Matches":{"subject":"Triggering","selector":[{"Subtype":"Mountain"}]}},"then":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":2}}],"otherwise":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":1}}]}}]}}"#,
        "Whenever a land you control enters, this creature deals 1 damage to any target. If that land is a Mountain, this creature deals 2 damage instead.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"from":"Graveyard","effects":[{"RollDice":{"sides":4,"results":[{"from":1,"to":1,"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}]}}]}}"#,
        "At the beginning of your upkeep, if ~ is in your graveyard, roll a four-sided die. If the result is 1, return ~ from your graveyard to the battlefield.",
    ),
    (
        r#"{"Replacement":{"when":{"Enters":{"what":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]}},"by":{"WithCounters":{"counter":"+1/+1","count":{"Stat":{"of":"This","stat":"Toughness"}}}}}}"#,
        "Each other creature you control enters with a number of additional +1/+1 counters on it equal to ~'s toughness.",
    ),
    (
        r#"{"Activated":{"cost":[{"Mana":"{1}{U}"},"Tap"],"effects":[{"Rule":{"rule":{"MustAttack":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}},"duration":"UntilEndOfTurn"}}]}}"#,
        "{1}{U}, {T}: Creatures your opponents control attack this turn if able.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"Not":"Token"}],"to":"Battlefield"}},"effects":[{"Pay":{"cost":"{1}{G}","paid":[{"CreateToken":{"who":{"ControllerOf":"Triggering"},"token":{"CopyOf":{"of":"Triggering"}}}}]}}]}}"#,
        "Whenever another nontoken creature enters, you may pay {1}{G}. If you do, that creature's controller creates a token that's a copy of that creature.",
    ),
    (
        r#"{"Rule":{"rule":{"MayCast":{"what":["Spell",{"Subtype":"Spirit"}],"flash":true}}}}"#,
        "You may cast Spirit spells as though they had flash.",
    ),
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Type":"Creature"}],"by":"You"}},"if":{"Not":"YourTurn"},"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Tap":{"what":{"Target":0}}}]}}"#,
        "Whenever you cast a creature spell during an opponent's turn, tap up to one target creature.",
    ),
    (
        r#"{"Spell":{"effects":[{"Modal":{"modes":[{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}},{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]},{"ControlledBy":"Opponents"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":1,"toughness":0}}],"duration":"UntilEndOfTurn"}},{"DealDamage":{"source":{"Target":0},"to":{"Target":1},"amount":{"Stat":{"of":{"Target":0},"stat":"Power"}}}}]},{"targets":[{"what":{"Object":[{"Type":"Enchantment"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]},{"effects":[{"Draw":{"count":2}}]}]}}]}}"#,
        "Choose one —\n• Target creature you control gets +1/+0 until end of turn. It deals damage equal to its power to target creature or planeswalker an opponent controls.\n• Destroy target enchantment.\n• Draw two cards.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"Sacrifice":{"who":{"Target":0},"filter":[{"Type":"Creature"}]}},{"Discard":{"who":{"Target":0},"count":3}},{"LoseLife":{"who":{"Target":0},"amount":5}},{"Choose":{"from":{"Each":[{"In":"Graveyard"},{"Type":"Creature"},{"OwnedBy":"You"}]},"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}},{"Draw":{"count":3}},{"GainLife":{"amount":5}}]}}"#,
        "Target opponent sacrifices a creature of their choice, discards three cards, then loses 5 life. You return a creature card from your graveyard to your hand, draw three cards, then gain 5 life.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"Not":"Token"},{"ControlledBy":"AttachedPlayer"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Exile":{"what":"Triggering"}},{"CreateToken":{"token":{"Custom":{"name":"Spider","colors":["Green"],"types":["Creature"],"subtypes":["Spider"],"power":1,"toughness":2,"abilities":[{"Keyword":"Reach"}]}}}}]}}"#,
        "Whenever a nontoken creature enchanted player controls dies, exile it and you create a 1/2 green Spider creature token with reach.",
    ),
    (
        r#"{"Static":{"affected":{"Each":[{"Type":"Artifact"},{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Afflict":3}}]}}"#,
        "Artifact creatures you control have afflict 3.",
    ),
    (
        r#"{"Triggered":{"when":{"State":{"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"ice"}}},"op":"Eq","right":0}}}},"effects":[{"Sacrifice":{"filter":["This"]}},{"If":{"condition":"Performed","then":[{"CreateToken":{"token":{"Custom":{"name":"Marit Lage","supertypes":["Legendary"],"colors":["Black"],"types":["Creature"],"subtypes":["Avatar"],"power":20,"toughness":20,"abilities":[{"Keyword":"Flying"},{"Keyword":"Indestructible"}]}}}}]}}]}}"#,
        "When ~ has no ice counters on it, sacrifice it. If you do, create Marit Lage, a legendary 20/20 black Avatar creature token with flying and indestructible.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Opponent"}],"effects":[{"Pay":{"who":{"Target":0},"cost":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],"unpaid":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":5}}]}}]}}"#,
        "When this creature enters, it deals 5 damage to target opponent unless that player sacrifices a creature of their choice.",
    ),
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":"AnyTarget"},{"what":"AnyTarget","bounds":{"min":0,"max":2}}],"effects":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":4}},{"DealDamage":{"source":"This","to":{"Target":1},"amount":3}}]}}"#,
        "Whenever ~ attacks, ~ deals 4 damage to any target and 3 damage to each of up to two other targets.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Enchantment"}]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":[{"Sacrifice":{"filter":[{"Is":{"Target":0}}]}}],"unpaid":[{"DealDamage":{"source":{"Target":0},"to":{"ControllerOf":{"Target":0}},"amount":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}]}}]}}"#,
        "At the beginning of your end step, target enchantment deals damage equal to its mana value to its controller unless that player sacrifices it.",
    ),
    (
        r#"{"Spell":{"effects":[{"Mill":{"who":"EachPlayer","count":2}},{"Choose":{"from":{"Each":[{"In":"Graveyard"},{"Type":"Creature"}]},"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":{"Battlefield":{"under":"You"}}}}]}}"#,
        "Each player mills two cards. Then you put a creature card from a graveyard onto the battlefield under your control.",
    ),
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"if":{"Compare":{"left":{"Count":[{"Subtype":"Pirate"},"Other",{"Not":"Token"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"effects":[{"Pay":{"cost":"{2}","paid":[{"CreateToken":{"token":{"Custom":{"name":"Pirate","colors":["Black"],"types":["Creature"],"subtypes":["Pirate"],"power":2,"toughness":2,"abilities":[{"Keyword":"Menace"}]}}}}]}}]}}"#,
        "Whenever this creature attacks, if you control another nontoken Pirate, you may pay {2}. If you do, create a 2/2 black Pirate creature token with menace.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"Not":"Token"},{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"Pay":{"cost":"{R}","paid":[{"CreateToken":{"token":{"CopyOf":{"of":"Triggering","except":[{"AddKeyword":"Haste"}]}},"as":"copy"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Exile":{"what":{"Bound":"copy"}}}]}}]}}]}}"#,
        "Whenever a nontoken creature you control enters, you may pay {R}. If you do, create a token that's a copy of that creature. That token gains haste. Exile it at the beginning of the next end step.",
    ),
    (
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Commander",{"OwnedBy":"You"}]},"mods":[{"GrantAbility":{"Triggered":{"when":{"SpellCast":{"what":[{"SharesSubtype":"This"}],"by":"You"}},"limit":"OncePerTurn","effects":[{"Draw":{}}]}}}]}}"#,
        "Commander creatures you own have \"Whenever you cast a spell that shares a creature type with this creature, draw a card. This ability triggers only once each turn.\"",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"May":{"effects":[{"Exile":{"what":"This"}}]}},{"If":{"condition":"Performed","then":[{"RevealUntil":{"filter":[{"Type":"Creature"}],"as":"found","revealed_as":"revealed"}},{"Move":{"what":{"Bound":"found"},"to":{"Battlefield":{}}}},{"Move":{"what":{"Bound":"revealed"},"to":"Graveyard"}}]}}]}}"#,
        "When this creature dies, you may exile it. If you do, reveal cards from the top of your library until you reveal a creature card. Put that card onto the battlefield and put all other cards revealed this way into your graveyard.",
    ),
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Continuous":{"affected":{"Each":[{"Named":"Gary Clone"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":1,"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#,
        "Whenever this creature attacks, each creature you control named ~ gets +1/+0 until end of turn.",
    ),
    (
        r#"{"Triggered":{"when":{"AttacksPlayer":{"what":[{"Type":"Creature"}],"at":"You"}},"effects":[{"Continuous":{"affected":"Triggering","mods":[{"ModifyPT":{"power":-1,"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#,
        "Whenever a creature attacks you, it gets -1/-0 until end of turn.",
    ),
    (
        r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"May":{"effects":[{"CastFromZone":{"what":{"Each":["Commander",{"In":"Command"},{"OwnedBy":"You"}]},"free":true}}]}}]}}"#,
        "Whenever this creature deals combat damage to a player, you may cast your commander from the command zone without paying its mana cost.",
    ),
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"Attacks":{"what":["This"]}},{"Blocks":{"what":["This"]}},{"BecomesTarget":{"what":["This"],"by":["Spell"]}}]},"effects":[{"DealDamage":{"source":"This","to":"Opponents","amount":{"Stat":{"of":"This","stat":"Power"}}}}]}}"#,
        "Whenever this creature attacks, blocks, or becomes the target of a spell, it deals damage equal to its power to each opponent.",
    ),
    (
        r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Type":"Creature"}],"to":"Opponents","combat":true}},"effects":[{"Pay":{"who":{"ControllerOf":"Triggering"},"cost":[{"PayLife":1}],"paid":[{"Draw":{"who":{"ControllerOf":"Triggering"}}}]}}]}}"#,
        "Whenever a creature deals combat damage to one of your opponents, its controller may pay 1 life. If they do, they draw a card.",
    ),
    (
        r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"at_least":2}},"if":{"Matches":{"subject":"Attached","selector":["Attacking"]}},"effects":[{"Draw":{}}]}}"#,
        "Whenever equipped creature and at least one other creature attack, draw a card.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"Opponents"}},"effects":[{"CreateToken":{"who":"Triggering","token":{"Custom":{"name":"Goblin","colors":["Red"],"types":["Creature"],"subtypes":["Goblin"],"power":1,"toughness":1,"abilities":[{"Rule":{"rule":{"MustAttack":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]}}}}]}}}}]}}"#,
        "At the beginning of each opponent's end step, that player creates a 1/1 red Goblin creature token with \"Creatures you control attack each combat if able.\"",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"under":"You"}}}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Haste"}],"duration":"Forever"}}]}}"#,
        "Put target creature card from a graveyard onto the battlefield under your control. It gains haste.",
    ),
    (
        r#"{"Activated":{"cost":["Tap"],"timing":"Sorcery","condition":{"Compare":{"left":{"Tallied":{"who":"You","what":"InstantsAndSorceriesCast"}},"op":"Ge","right":1}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}}"#,
        "{T}: Put a +1/+1 counter on target creature. Activate only as a sorcery and only if you've cast an instant or sorcery spell this turn.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"tower"}}},"op":"Ge","right":100}},"effects":[{"WinGame":{}}]}}"#,
        "At the beginning of your upkeep, if there are 100 or more tower counters on this enchantment, you win the game.",
    ),
    (
        r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Artifact"},{"ControlledBy":"Triggering"}]},"mods":[{"SetController":"You"}],"duration":"Forever"}}]}}"#,
        "Whenever this creature deals combat damage to a player, gain control of all artifacts that player controls.",
    ),
    (
        r#"{"Spell":{"effects":[{"Shield":{"when":{"Damage":{"source":[{"Type":"Creature"},{"Not":{"HasCounter":"+1/+1"}}],"combat":true}},"by":"Prevent","duration":"UntilEndOfTurn"}}]}}"#,
        "Prevent all combat damage that would be dealt this turn by creatures with no +1/+1 counters on them.",
    ),
    (
        r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"effects":[{"Modal":{"choose":{"min":0,"max":1},"modes":[{"targets":[{"what":{"Object":["Spell",{"Not":{"ControlledBy":"You"}}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]},{"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}]}}]}}"#,
        "Whenever you cast a spell, choose up to one —\n• Return target spell you don't control to its owner's hand.\n• Return target nonland permanent to its owner's hand.",
    ),
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":["This"],"by":"You"}},"from":"Stack","effects":[{"GainLife":{"amount":{"Half":{"of":"X","up":false}}}},{"Draw":{"count":{"Half":{"of":"X","up":false}}}}]}}"#,
        "When you cast this spell, you gain half X life and draw half X cards. Round down each time.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"targets":[{"what":"Opponent"}],"effects":[{"Pay":{"who":{"Target":0},"cost":[{"OneOf":[[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}],[{"PayLife":3}]]}],"unpaid":[{"Draw":{}}]}}]}}"#,
        "At the beginning of your upkeep, draw a card unless target opponent sacrifices a creature of their choice or pays 3 life.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"DelayedTrigger":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"Not":"Token"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"once":false,"effects":[{"CreateToken":{"token":{"Custom":{"name":"Saproling","colors":["Green"],"types":["Creature"],"subtypes":["Saproling"],"power":1,"toughness":1}},"count":{"Stat":{"of":"Triggering","stat":"Power"}}}}]}}]}}"#,
        "When this creature enters, until end of turn, whenever a nontoken creature you control dies, create a number of 1/1 green Saproling creature tokens equal to that creature's power.",
    ),
    (
        r#"{"Keyword":{"Enchant":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"},{"Subtype":"Clue"}]}]}}"#,
        "Enchant creature, planeswalker, or Clue",
    ),
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"AttacksAlone":{"what":["This"]}},{"AttacksAlone":{"what":["Commander",{"ControlledBy":"You"}]}}]},"targets":[{"what":{"Object":[{"Type":"Artifact"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}}"#,
        "Whenever this creature or a commander you control attacks alone, return target artifact card from your graveyard to the battlefield.",
    ),
    (
        r#"{"Triggered":{"when":{"Sacrificed":{"what":[{"Not":"Token"}],"by":"Opponents"}},"effects":[{"Move":{"what":"Triggering","to":{"Battlefield":{"under":"You"}}}}]}}"#,
        "Whenever an opponent sacrifices a nontoken permanent, put that card onto the battlefield under your control.",
    ),
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Ragavan","supertypes":["Legendary"],"colors":["Red"],"types":["Creature"],"subtypes":["Monkey"],"power":2,"toughness":1}},"tapped":true,"attacking":true,"as":"ragavan"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"EndCombat"}},"effects":[{"Exile":{"what":{"Bound":"ragavan"}}}]}}]}}"#,
        "Whenever ~ attacks, create Ragavan, a legendary 2/1 red Monkey creature token. Ragavan enters tapped and attacking. Exile that token at end of combat.",
    ),
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":["Other",{"HasKeyword":"Convoke"}],"by":"You"}},"effects":[{"Scry":{"count":2}},{"Draw":{}}]}}"#,
        "Whenever you cast another spell that has convoke, scry 2, then draw a card.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"Opponents"}},"if":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"Triggering"}]},"op":"Gt","right":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}}}}]}}"#,
        "At the beginning of each opponent's end step, if that player controls more creatures than you, create a 1/1 white Soldier creature token.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"Opponents"}},"if":{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":"Triggering"}]},"op":"Gt","right":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}},"effects":[{"May":{"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Subtype":"Plains"}],"count":{"min":0,"max":1},"as":"found"}},{"Move":{"what":{"Bound":"found"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}]}}"#,
        "At the beginning of each opponent's end step, if that player controls more lands than you, you may search your library for a basic Plains card, put it onto the battlefield tapped, then shuffle.",
    ),
    (
        r#"{"Activated":{"cost":"{2}{U}","effects":[{"Continuous":{"affected":{"Each":[{"Named":"Leitmotif Composer"}]},"mods":["CantBeBlocked"],"duration":"UntilEndOfTurn"}}]}}"#,
        "{2}{U}: Creatures named ~ can't be blocked this turn.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"UntilEndOfTurn"}},{"Untap":{"what":{"Target":0}}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddSubtype":"Villain"},{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}}"#,
        "When ~ enters, gain control of target creature until end of turn. Untap that creature. Until end of turn, it becomes a Villain in addition to its other types and gains haste.",
    ),
    (
        r#"{"Activated":{"cost":[{"Mana":"{G}{W}{W}"},"Tap","SacrificeSelf"],"timing":"Sorcery","targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]},"bounds":{"min":0,"max":2}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Vigilance"},{"AddKeyword":"Lifelink"}],"duration":"UntilEndOfTurn"}}]}}"#,
        "{G}{W}{W}, {T}, Sacrifice this land: Put a +1/+1 counter on each of up to two target creatures you control. They gain vigilance and lifelink until end of turn. Activate only as a sorcery.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"LookAtTop":{"count":2,"as":"top"}},{"Reveal":{"what":{"Bound":"top"}}},{"Choose":{"from":{"Bound":"top"},"as":"card","rest_as":"rest"}},{"Move":{"what":{"Bound":"card"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":"Graveyard"}},{"If":{"condition":{"Matches":{"subject":{"Bound":"card"},"selector":[{"Type":"Artifact"}]}},"then":[{"GainLife":{"amount":3}}]}}]}}"#,
        "When this creature enters, reveal the top two cards of your library. Put one of them into your hand and the other into your graveyard. If you put an artifact card into your hand this way, you gain 3 life.",
    ),
    (
        r#"{"Triggered":{"when":{"Cycled":{"what":["This"],"by":"You"}},"effects":[{"Pay":{"cost":"{X}","paid":[{"CreateToken":{"token":{"Custom":{"name":"Soldier","colors":["White"],"types":["Creature"],"subtypes":["Soldier"],"power":1,"toughness":1}},"count":"X"}}]}}]}}"#,
        "When you cycle this card, you may pay {X}. If you do, create X 1/1 white Soldier creature tokens.",
    ),
    (
        r#"{"Activated":{"cost":[{"Mana":"{U}"},"Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"}]}},{"what":"Opponent"}],"effects":[{"Rule":{"rule":{"MustAttackPlayers":{"who":{"Target":0},"players":{"Target":1}}},"duration":"UntilEndOfTurn"}}]}}"#,
        "{U}, {T}: Target creature attacks target opponent this turn if able.",
    ),
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Pay":{"cost":"{X}{W}{U}{B}","paid":[{"Draw":{"count":"X"}}]}}]}}"#,
        "Whenever ~ attacks, you may pay {X}{W}{U}{B}. If you do, draw X cards.",
    ),
    (
        r#"{"Activated":{"cost":"{1}{G}","targets":[{"what":{"Object":[{"In":"Graveyard"}]}}],"effects":[{"Exile":{"what":{"Target":0},"as":"card"}},{"If":{"condition":{"Matches":{"subject":{"Bound":"card"},"selector":[{"Type":"Creature"}]}},"then":[{"CreateToken":{"token":{"Custom":{"name":"Pest","colors":["Black","Green"],"types":["Creature"],"subtypes":["Pest"],"power":1,"toughness":1,"abilities":[{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"GainLife":{"amount":1}}]}}]}}}}]}}]}}"#,
        "{1}{G}: Exile target card from a graveyard. If a creature card is exiled this way, create a 1/1 black and green Pest creature token with \"When this token dies, you gain 1 life.\"",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":99}},{"what":{"Object":[{"Type":"Artifact"}]},"bounds":{"min":0,"max":99}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}},{"PutCounters":{"on":{"Target":1},"counter":"charge"}}]}}"#,
        "When this creature enters, put a +1/+1 counter on each of any number of target creatures and a charge counter on each of any number of target artifacts.",
    ),
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":"AnyTarget"}],"effects":[{"Pay":{"cost":"{X}{R}","paid":[{"DealDamage":{"to":{"Target":0},"amount":"X"}}]}}]}}"#,
        "Whenever this creature attacks, you may pay {X}{R}. If you do, it deals X damage to any target.",
    ),
    (
        r#"{"Activated":{"cost":"{W}{U}{B}{R}{G}","targets":[{"what":{"Object":[{"Subtype":"Elemental"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"CastFromZone":{"what":{"Target":0},"free":true,"play":true}}]}}"#,
        "{W}{U}{B}{R}{G}: You may play target Elemental card from your graveyard without paying its mana cost.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Is":"Attached"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Move":{"what":"Triggering","to":{"Battlefield":{"under":"You"}}}},{"Move":{"what":"This","to":{"Battlefield":{"under":"You","transformed":true}}}}]}}"#,
        "When enchanted creature dies, return it to the battlefield under your control, then return this card to the battlefield transformed under your control.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"AttachedPlayer"}},"effects":[{"RevealUntil":{"who":"AttachedPlayer","filter":[{"Type":"Creature"}],"as":"found","revealed_as":"revealed"}},{"Move":{"what":{"Bound":"found"},"to":{"Battlefield":{"under":"You"}}}},{"Move":{"what":{"Bound":"revealed"},"to":"Graveyard"}}]}}"#,
        "At the beginning of enchanted player's upkeep, that player reveals cards from the top of their library until they reveal a creature card. Put that card onto the battlefield under your control. That player puts the rest of the revealed cards into their graveyard.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Land"},{"ControlledBy":"You"}]}}],"effects":[{"May":{"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddType":"Creature"},{"AddSubtype":"Elemental"},{"SetPT":{"power":3,"toughness":3}},{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
        "Whenever a land you control enters, you may have target land you control become a 3/3 Elemental creature with haste until end of turn. It's still a land.",
    ),
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":[{"Is":"Attached"}]}},"effects":[{"PutCounters":{"on":"Defending","counter":{"Other":"rad"},"count":2}}]}}"#,
        "Whenever enchanted creature attacks, defending player gets two rad counters.",
    ),
    (
        r#"{"Keyword":{"Morph":[{"ReturnToHand":{"filter":[{"Subtype":"Island"},{"ControlledBy":"You"}],"count":2}}]}}"#,
        "Morph—Return two Islands you control to their owner's hand.",
    ),
    (
        r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"targets":[{"what":{"Object":[{"Type":"Creature"},"Attacking"]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":{"Other":"shield"}}},{"Continuous":{"affected":{"Target":0},"mods":[{"GrantAbility":{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"RemoveCounters":{"from":"This","counter":{"Other":"shield"}}},{"If":{"condition":"Performed","then":[{"Draw":{}}]}}]}}}],"duration":"UntilEndOfTurn"}}]}}"#,
        "Whenever you attack, put a shield counter on target attacking creature. Until end of turn, it gains \"Whenever this creature deals combat damage to a player, remove a shield counter from it. If you do, draw a card.\"",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End"}},"if":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"Named":"Biovisionary"},{"ControlledBy":"You"}]},"op":"Ge","right":4}},"effects":[{"WinGame":{}}]}}"#,
        "At the beginning of the end step, if you control four or more creatures named ~, you win the game.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"Choose":{"from":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"as":"mine"}},{"PutCounters":{"on":{"Bound":"mine"},"counter":{"Other":"menace"}}}]}}"#,
        "Destroy target creature. Put a menace counter on a creature you control.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]}]}}],"effects":[{"PutCounters":{"on":"You","counter":{"Other":"energy"},"count":4}},{"Pay":{"cost":[{"PayEnergy":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}],"paid":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"Forever"}}]}}]}}"#,
        "Choose target artifact or creature. You get {E}{E}{E}{E}, then you may pay an amount of {E} equal to that permanent's mana value. If you do, gain control of it.",
    ),
];

/// The abilities of the table read once, each with its wording.
static C11F_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    C11F_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn c11f_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in C11F_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}
