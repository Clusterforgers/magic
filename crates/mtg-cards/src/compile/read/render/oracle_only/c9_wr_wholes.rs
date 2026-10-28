//! Whole abilities of the third samples (`proposals/r2ba` to `r2bd`) whose wording no renderer writes: the ability is
//! matched whole as data and worded as the card prints it ("Each player sacrifices all permanents they control that
//! are one or more colors.").

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording.
const C9_WR_WHOLES: [(&str, &str); 65] = [
    (
        r#"{"Spell":{"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Sacrifice":{"who":{"Bound":"player"},"filter":[{"Not":"Colorless"}],"count":{"Count":[{"Not":"Colorless"},{"ControlledBy":{"Bound":"player"}}]}}}]}}]}}"#,
        "Each player sacrifices all permanents they control that are one or more colors.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"},{"AnyOf":["This",{"HasKeyword":"Flying"}]}],"to":"Battlefield"}},"effects":[{"May":{"effects":[{"GainLife":{"amount":{"Stat":{"of":"Triggering","stat":"Power"}}}}]}}]}}"#,
        "Whenever this creature or another creature you control with flying enters, you may gain life equal to that creature's power.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Pay":{"who":{"Bound":"player"},"cost":[{"Sacrifice":{"filter":[{"Type":"Creature"},{"Not":"Token"}]}}],"unpaid":[{"LoseLife":{"who":{"Bound":"player"},"amount":4}}]}}]}}]}}"#,
        "At the beginning of your end step, each player loses 4 life unless they sacrifice a nontoken creature of their choice.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"If":{"condition":{"Matches":{"subject":"Attached","selector":[{"Subtype":"Vampire"}]}},"then":[{"PutCounters":{"on":"Attached","counter":"+1/+1","count":2}}],"otherwise":[{"PutCounters":{"on":"Attached","counter":"+1/+1"}}]}}]}}"#,
        "Whenever a creature dies, put a +1/+1 counter on equipped creature. If equipped creature is a Vampire, put two +1/+1 counters on it instead.",
    ),
    (
        r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":6,"toughness":6}},{"AddKeyword":"Flying"}],"condition":{"All":[{"Compare":{"left":{"Life":"You"},"op":"Ge","right":30}},{"Compare":{"left":{"LowestLife":"Opponents"},"op":"Le","right":10}}]}}}"#,
        "As long as you have 30 or more life and an opponent has 10 or less life, this creature gets +6/+6 and has flying.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Opponent"}],"effects":[{"Discard":{"who":{"Target":0},"count":"Kicks"}}]}}"#,
        "When this creature enters, target opponent discards a card for each time it was kicked.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End"}},"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Continuous":{"affected":{"Each":[{"OwnedBy":{"Bound":"player"}},{"Not":"Token"}]},"mods":[{"SetController":{"Bound":"player"}}],"duration":"Forever"}}]}}]}}"#,
        "At the beginning of each end step, each player gains control of all nontoken permanents they own.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"DealDamage":{"source":{"Target":0},"to":{"Each":[{"Type":"Creature"},{"Not":{"Is":{"Target":0}}}]},"amount":{"Stat":{"of":{"Target":0},"stat":"Power"}}}},{"DealDamage":{"source":{"Target":0},"to":"Opponents","amount":{"Stat":{"of":{"Target":0},"stat":"Power"}}}}]}}"#,
        "Target creature you control deals damage equal to its power to each other creature and each opponent.",
    ),
    (
        r#"{"Spell":{"effects":[{"ExileTop":{"count":"X","as":"impulse"}},{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"impulse"},"play":true}},"duration":"UntilEndOfYourNextTurn"}}]}}"#,
        "Exile the top X cards of your library. Until the end of your next turn, you may play those cards.",
    ),
    (
        r#"{"Spell":{"effects":[{"Draw":{"count":"X"}},{"PutCounters":{"on":"You","counter":{"Other":"rad"},"count":{"Half":{"of":"X","up":true}}}}]}}"#,
        "Draw X cards, then you get half X rad counters, rounded up.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"targets":[{"what":"Player"}],"effects":[{"Draw":{"count":{"Count":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]}}},{"LoseLife":{"amount":{"Count":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]}}}]}}"#,
        "When this creature dies, you draw X cards and you lose X life, where X is the number of creature cards in target player's graveyard.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"If":{"condition":{"Compare":{"left":{"Life":"You"},"op":"Gt","right":{"StartingLife":"You"}}},"then":[{"Draw":{}}],"otherwise":[{"GainLife":{"amount":2}}]}}]}}"#,
        "At the beginning of your end step, draw a card if your life total is greater than your starting life total. Otherwise, you gain 2 life.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"targets":[{"what":"AnyTarget"}],"effects":[{"If":{"condition":{"Monarch":"You"},"then":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":7}}],"otherwise":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":2}}]}}]}}"#,
        "At the beginning of your upkeep, this enchantment deals 2 damage to any target. If you're the monarch, it deals 7 damage instead.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End"}},"effects":[{"If":{"condition":{"Compare":{"left":{"Count":[{"Commander":null},{"ControlledBy":"Triggering"}]},"op":"Eq","right":0}},"then":[{"DealDamage":{"source":"This","to":"Triggering","amount":4}}]}}]}}"#,
        "At the beginning of each player's end step, this creature deals 4 damage to that player unless they control a commander.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"All":[{"Matches":{"subject":"This","selector":[{"CastFrom":"Hand"}]}},{"Compare":{"left":{"Count":[{"Type":"Creature"},"Other"]},"op":"Ge","right":5}}]},"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},"Other"]}}}]}}"#,
        "When this creature enters, if you cast it from your hand and there are five or more other creatures on the battlefield, destroy all other creatures.",
    ),
    (
        r#"{"Spell":{"effects":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"as":"discarded"}},{"Draw":{"count":{"Count":[{"Is":{"Bound":"discarded"}}]}}}]}}"#,
        "Discard all the cards in your hand, then draw that many cards.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}},{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Subtype":"Angel"}]}},"then":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":2}}]}}]}}"#,
        "Return target creature card from your graveyard to the battlefield. If it's an Angel, put two +1/+1 counters on it.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"targets":[{"what":{"Object":[]}}],"effects":[{"May":{"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"UntilEndOfTurn"}},{"Untap":{"what":{"Target":0}}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
        "At the beginning of your upkeep, you may gain control of target permanent until end of turn. If you do, untap it and it gains haste until end of turn.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]},{"ControlledBy":"Opponents"}]},"bounds":{"min":1,"max":null},"divide":5}],"effects":[{"DealDamage":{"source":"This","to":{"Target":0},"amount":"Divided"}}]}}"#,
        "When ~ enters, it deals 5 damage divided as you choose among any number of target creatures and/or planeswalkers your opponents control.",
    ),
    (
        r#"{"Spell":{"effects":[{"LookAtTop":{"count":7,"as":"seen"}},{"Choose":{"from":{"Bound":"seen"},"count":{"min":2,"max":2},"as":"kept","rest_as":"rest"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#,
        "Look at the top seven cards of your library. Put two of them into your hand and the rest on the bottom of your library in a random order.",
    ),
    (
        r#"{"Spell":{"effects":[{"Scry":{"count":3}},{"May":{"effects":[{"LookAtTop":{"as":"top"}},{"Reveal":{"what":{"Bound":"top"}}},{"If":{"condition":{"Matches":{"subject":{"Bound":"top"},"selector":[{"Type":"Creature"}]}},"then":[{"Draw":{}}]}}]}}]}}"#,
        "Scry 3, then you may reveal the top card of your library. If a creature card is revealed this way, draw a card.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Zombie","colors":["Black"],"types":["Creature"],"subtypes":["Zombie"],"power":2,"toughness":2}},"count":{"Half":{"of":{"Count":[{"Subtype":"Zombie"},{"ControlledBy":"You"}]},"up":false}}}}]}}"#,
        "At the beginning of your upkeep, create X 2/2 black Zombie creature tokens, where X is half the number of Zombies you control, rounded down.",
    ),
    (
        r#"{"Triggered":{"when":{"State":{"condition":{"Compare":{"left":{"Count":[{"Subtype":"Thrull"},{"ControlledBy":"You"}]},"op":"Ge","right":7}}}},"effects":[{"Sacrifice":{"filter":["This"]}}]}}"#,
        "When you control seven or more Thrulls, sacrifice ~.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"under":"You"}}}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Indestructible"}],"duration":"Forever"}},{"If":{"condition":"YourTurn","then":[{"Scry":{"count":2}}]}}]}}"#,
        "Put target creature card from a graveyard onto the battlefield under your control. It gains indestructible. If it's your turn, scry 2.",
    ),
    (
        r#"{"Rule":{"rule":{"MayCast":{"flash":true}},"condition":{"All":[{"Step":"End"},{"Not":"YourTurn"}]}}}"#,
        "During each opponent's end step, you may cast spells as though they had flash.",
    ),
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":[{"AnyOf":[{"Not":{"Type":"Creature"}},{"Subtype":"Dragon"}]}],"by":"You"}},"effects":[{"DealDamage":{"source":"This","to":"Opponents","amount":1}}]}}"#,
        "Whenever you cast a noncreature or Dragon spell, this creature deals 1 damage to each opponent.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"as":"returned"}},{"Move":{"what":{"Bound":"returned"},"to":"Hand"}},{"PutCounters":{"on":"This","counter":"+1/+1","count":{"Stat":{"of":{"Bound":"returned"},"stat":"Power"}}}}]}}]}}"#,
        "At the beginning of your end step, you may return another creature you control to its owner's hand, then put a number of +1/+1 counters equal to that creature's power on this creature.",
    ),
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"Attacks":{"what":["This"]}},{"BecomesTarget":{"what":["This"],"by":["Spell"]}}]},"effects":[{"CreateToken":{"token":{"Named":"treasure"}}}]}}"#,
        "Whenever this creature attacks or becomes the target of a spell, create a Treasure token.",
    ),
    (
        r#"{"Spell":{"effects":[{"Draw":{"count":2}},{"If":{"condition":{"Matches":{"subject":"Paid","selector":[{"Not":{"Type":"Land"}}]}},"then":[{"DealDamage":{"source":"This","to":"Opponents","amount":2}}]}}]}}"#,
        "Draw two cards. If the discarded card wasn't a land card, ~ deals 2 damage to each opponent.",
    ),
    (
        r#"{"Triggered":{"when":{"BecomesBlocked":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"effects":[{"May":{"effects":[{"Move":{"what":"Triggering","to":"Hand"}}]}}]}}"#,
        "Whenever a creature you control becomes blocked, you may return it to its owner's hand.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player"}],"effects":[{"Exile":{"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]},"as":"exiled"}},{"PutCounters":{"on":"This","counter":"+1/+1","count":{"Count":[{"Is":{"Bound":"exiled"}},{"Type":"Creature"}]}}}]}}"#,
        "When this creature enters, exile target player's graveyard. Put a number of +1/+1 counters on this creature equal to the number of creature cards exiled this way.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},"Attacking"]}}],"effects":[{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Subtype":"Demon"}]}},"then":[{"Exile":{"what":{"Target":0}}}],"otherwise":[{"Destroy":{"what":{"Target":0}}}]}}]}}"#,
        "When this creature enters, destroy target attacking creature. If that creature is a Demon, exile it instead.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"Opponents"}],"from":"Battlefield","to":"Graveyard"}},"if":{"Matches":{"subject":"Triggering","selector":[{"DamagedBy":"This"}]}},"effects":[{"Draw":{"count":1}}]}}"#,
        "Whenever a creature an opponent controls dies, if ~ dealt damage to it this turn, draw a card.",
    ),
    (
        r#"{"Spell":{"effects":[{"Modal":{"choose":{"min":1,"max":1},"modes":[{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]}]},"bounds":{"min":"X","max":"X"}}],"effects":[{"Destroy":{"what":{"Target":0}}}]},{"targets":[{"what":"Player"}],"effects":[{"GainLife":{"who":{"Target":0},"amount":{"Times":{"by":2,"of":"X"}}}}]}]}}]}}"#,
        "Choose one \u{2014}\n\u{2022} Destroy X target artifacts and/or enchantments.\n\u{2022} Target player gains twice X life.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"Supertype":"Legendary"},{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"May":{"effects":[{"Attach":{"what":"This","to":"Triggering"}}]}}]}}"#,
        "Whenever a legendary creature you control enters, you may attach this Equipment to it.",
    ),
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"SpellCast":{"what":[{"Not":{"Type":"Creature"}}],"by":"You"}}]},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":1,"toughness":0}},"CantBeBlocked"],"duration":"UntilEndOfTurn"}}]}}"#,
        "When ~ enters and whenever you cast a noncreature spell, target creature gets +1/+0 until end of turn and can't be blocked this turn.",
    ),
    (
        r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Type":"Creature"},{"ControlledBy":"You"}],"to":"EachPlayer","combat":true}},"effects":[{"GainLife":{"amount":{"Stat":{"of":"Dealer","stat":"Toughness"}}}}]}}"#,
        "Whenever a creature you control deals combat damage to a player, you gain life equal to that creature's toughness.",
    ),
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Compare":{"stat":"Colors","op":"Ge","to":2}}],"by":"You"}},"effects":[{"Scry":{"count":1}},{"If":{"condition":{"Matches":{"subject":"Triggering","selector":[{"Compare":{"stat":"Colors","op":"Eq","to":5}}]}},"then":[{"CreateToken":{"token":{"Custom":{"name":"Angel","colors":["White"],"types":["Creature"],"subtypes":["Angel"],"power":4,"toughness":4,"abilities":[{"Keyword":"Flying"},{"Keyword":"Vigilance"}]}}}}]}}]}}"#,
        "Whenever you cast a multicolored spell, scry 1. If that spell was all colors, create a 4/4 white Angel creature token with flying and vigilance.",
    ),
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Type":"Creature"},{"Compare":{"stat":"ManaValue","op":"Ge","to":5}}],"by":"You"}},"effects":[{"PutCounters":{"on":"You","counter":{"Other":"experience"}}}]}}"#,
        "Whenever you cast a creature spell with mana value 5 or greater, you get an experience counter.",
    ),
    (
        r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":{"Counters":{"on":"You","counter":{"Other":"experience"}}},"toughness":{"Counters":{"on":"You","counter":{"Other":"experience"}}}}}]}}"#,
        "~ gets +1/+1 for each experience counter you have.",
    ),
    (
        r#"{"Activated":{"cost":[{"Mana":"{3}{U}"}],"effects":[{"Draw":{"count":1}},{"May":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Land"},{"ControlledBy":"You"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":"Hand"}}]}}]}}"#,
        "{3}{U}: Draw a card, then you may return a land you control to its owner's hand.",
    ),
    (
        r#"{"Activated":{"cost":[{"Mana":"{1}{W}"}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"},"ChosenSubtype"]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":{"Other":"divinity"}}}]}}"#,
        "{1}{W}: Put a divinity counter on target creature you control of the chosen type.",
    ),
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"},"ChosenSubtype"],"to":"Battlefield"}},{"Attacks":{"what":[{"Type":"Creature"},{"ControlledBy":"You"},"ChosenSubtype"]}}]},"effects":[{"Draw":{"count":1}}]}}"#,
        "Whenever a creature you control of the chosen type enters or attacks, draw a card.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"OwnedBy":"You"}],"from":"Graveyard"}},"if":"YourTurn","limit":"OncePerTurn","effects":[{"Draw":{"count":1}}]}}"#,
        "Whenever a card leaves your graveyard during your turn, draw a card. This ability triggers only once each turn.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Not":{"OwnedBy":"You"}}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Draw":{"count":1}},{"LoseLife":{"amount":1}}]}}"#,
        "Whenever a permanent owned by another player is put into a graveyard from the battlefield, you draw a card and you lose 1 life.",
    ),
    (
        r#"{"Spell":{"effects":[{"LookAtTop":{"count":6,"as":"top"}},{"Reveal":{"what":{"Bound":"top"}}},{"Choose":{"from":{"Each":[{"Is":{"Bound":"top"}},{"Type":"Enchantment"}]},"count":{"min":0,"max":3},"as":"picked"}},{"Move":{"what":{"Bound":"picked"},"to":"Hand"}},{"Move":{"what":{"Each":[{"Is":{"Bound":"top"}},{"Not":{"Is":{"Bound":"picked"}}}]},"to":"Graveyard"}}]}}"#,
        "Reveal the top six cards of your library. Put up to three enchantment cards from among them into your hand and the rest of the revealed cards into your graveyard.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Move":{"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]},"to":{"Library":{"place":"Shuffled"}}}},{"Draw":{"count":1}}]}}"#,
        "Target player shuffles their graveyard into their library.\nDraw a card.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Compare":{"left":{"HighestLife":"Opponents"},"op":"Gt","right":{"Life":"You"}}},"effects":[{"GainLife":{"amount":5}}]}}"#,
        "When ~ enters, if an opponent has more life than you, you gain 5 life.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"},{"Not":{"HasKeyword":"Flying"}}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Move":{"what":"Triggering","to":{"Battlefield":{"counters":[[{"Other":"flying"},1]]}}}}]}}"#,
        "Whenever a creature you control without flying dies, return it to the battlefield under its owner's control with a flying counter on it.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"}],"duration":"UntilEndOfTurn"}},{"Untap":{"what":{"Target":0}}},{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":2,"toughness":0}},{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}}"#,
        "Gain control of target creature until end of turn. Untap that creature. It gets +2/+0 and gains haste until end of turn.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"as":"discarded"}},{"Sacrifice":{"who":"Opponents","filter":[{"Type":"Creature"}],"count":{"Count":[{"Is":{"Bound":"discarded"}}]}}}]}}"#,
        "When ~ enters, discard your hand. Each opponent sacrifices a creature of their choice for each card discarded this way.",
    ),
    (
        r#"{"Rule":{"rule":{"MayCast":{"what":["This"],"from":"Graveyard"}},"condition":{"Compare":{"left":{"Count":[{"AnyOf":[{"Color":"Black"},{"Color":"Green"}]},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"from":"Graveyard"}}"#,
        "You may cast this card from your graveyard as long as you control a black or green permanent.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]},{"ControlledBy":"Opponents"}]}}],"effects":[{"Pay":{"cost":[{"ExileFromGraveyard":{"filter":[{"Type":"Creature"}]}}],"paid":[{"Exile":{"what":{"Target":0}}}]}}]}}"#,
        "When this creature enters, you may exile a creature card from your graveyard. If you do, exile target artifact or enchantment an opponent controls.",
    ),
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"ExileTop":{"count":4,"as":"exiled"}},{"Rule":{"rule":{"MayCast":{"what":[{"Not":{"Type":"Creature"}}],"from":"Exile","free":true,"only":{"Bound":"exiled"}}},"duration":"UntilEndOfTurn"}}]}}"#,
        "Whenever ~ attacks, exile the top four cards of your library. Until end of turn, you may cast noncreature spells from among those cards without paying their mana costs.",
    ),
    (
        r#"{"Spell":{"effects":[{"Modal":{"modes":[{"effects":[{"Exile":{"what":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Gt","to":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}}]}}}]},{"effects":[{"Exile":{"what":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Lt","to":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}}]}}}]}]}}]}}"#,
        "Choose one \u{2014}\n\u{2022} Exile each creature with power greater than the number of cards in your hand.\n\u{2022} Exile each creature with power less than the number of cards in your hand.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"In":"Graveyard"},{"Type":"Creature"},{"OwnedBy":"Opponents"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"tapped":true,"under":"You"}}}},{"Exile":{"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":{"OwnerOf":{"Target":0}}}]}}}]}}"#,
        "Put target creature card from an opponent's graveyard onto the battlefield tapped under your control, then exile that player's graveyard.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Is":"Attached"}],"from":"Battlefield","to":"Graveyard"}},"from":"Graveyard","effects":[{"Move":{"what":"This","to":"Hand"}},{"CreateToken":{"token":{"Custom":{"name":"Plaguebearer of Nurgle","colors":["Black"],"types":["Creature"],"subtypes":["Demon"],"power":1,"toughness":3}}}}]}}"#,
        "When enchanted creature dies, return this card to its owner's hand and you create a 1/3 black Demon creature token named Plaguebearer of Nurgle.",
    ),
    (
        r#"{"Activated":{"cost":["ExileSelf"],"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"AnyOf":[{"Type":"Artifact"},{"Subtype":"Human"}]}]},"mods":[{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}]}}"#,
        "Exile ~: Each creature you control that's an artifact or Human gains indestructible until end of turn.",
    ),
    (
        r#"{"Activated":{"cost":[{"PayEnergy":3}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Rule":{"rule":{"MustBlock":{"Target":0}},"duration":"UntilEndOfTurn"}}]}}"#,
        "Pay {E}{E}{E}: Target creature blocks this turn if able.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Phyrexian Insect","colors":["Green"],"types":["Creature"],"subtypes":["Phyrexian","Insect"],"power":1,"toughness":1,"abilities":[{"Keyword":"Infect"}]}},"count":{"Counters":{"on":"Opponents","counter":"poison"}}}}]}}"#,
        "At the beginning of your upkeep, create a 1/1 green Phyrexian Insect creature token with infect for each poison counter your opponents have.",
    ),
    (
        r#"{"Spell":{"effects":[{"LookAtTop":{"count":3,"as":"top"}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Top","order":"Chosen"}}}},{"May":{"effects":[{"Shuffle":{}}]}},{"Draw":{}}]}}"#,
        "Look at the top three cards of your library, then put them back in any order. You may shuffle.\nDraw a card.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"Opponents"}},"effects":[{"DealDamage":{"source":"This","to":"Triggering","amount":{"Count":[{"In":"Hand"},{"OwnedBy":"Triggering"}]}}}]}}"#,
        "At the beginning of each opponent's upkeep, this enchantment deals damage to that player equal to the number of cards in that player's hand.",
    ),
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"by":"You"}},"effects":[{"If":{"condition":{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"ManaValue"}},"op":"Ge","right":5}},"then":[{"CreateToken":{"token":{"Custom":{"name":"Elemental","colors":["Blue","Red"],"types":["Creature"],"subtypes":["Elemental"],"power":1,"toughness":1}},"count":3}}],"otherwise":[{"CreateToken":{"token":{"Custom":{"name":"Elemental","colors":["Blue","Red"],"types":["Creature"],"subtypes":["Elemental"],"power":1,"toughness":1}}}}]}}]}}"#,
        "Whenever you cast an instant or sorcery spell, create a 1/1 blue and red Elemental creature token. If that spell's mana value is 5 or greater, create three of those tokens instead.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End"}},"if":{"Compare":{"left":{"Count":[{"Type":"Creature"}]},"op":"Eq","right":0}},"effects":[{"Sacrifice":{"filter":["This"]}}]}}"#,
        "At the beginning of the end step, if no creatures are on the battlefield, sacrifice this enchantment.",
    ),
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},{"ZoneChange":{"what":[{"Type":"Artifact"},{"ControlledBy":"You"},"Other"],"from":"Battlefield","to":"Graveyard"}}]},"targets":[{"what":{"Object":[{"Type":"Artifact"},{"In":"Graveyard"},{"OwnedBy":"You"},{"Compare":{"stat":"ManaValue","op":"Lt","to":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#,
        "Whenever this creature dies or another artifact you control is put into a graveyard from the battlefield, return to your hand target artifact card in your graveyard with lesser mana value.",
    ),
];

/// The wordings with their abilities as the data writes them, read once.
static C9_WR_SHAPES: LazyLock<Vec<(serde_json::Value, &str)>> = LazyLock::new(|| {
    C9_WR_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((c9_wr_shape(shape)?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(mut data) = serde_json::to_value(ability) else {
        return;
    };
    c9_wr_own_source(&mut data);
    for (shape, text) in C9_WR_SHAPES.iter() {
        if *shape == data {
            texts.push((*text).to_owned());
        }
    }
}

/// The JSON of an ability as the data writes it, defaults filled in the way the rules files hold them.
fn c9_wr_shape(shape: &str) -> Option<serde_json::Value> {
    let ability: Ability = serde_json::from_str(shape).ok()?;
    let mut data = serde_json::to_value(ability).ok()?;
    c9_wr_own_source(&mut data);
    Some(data)
}

/// Drops the `"source": "This"` of every damage: an absent source is the ability's own, so both write one wording.
fn c9_wr_own_source(data: &mut serde_json::Value) {
    match data {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::Object(damage)) = map.get_mut("DealDamage")
                && damage.get("source").and_then(serde_json::Value::as_str) == Some("This")
            {
                damage.remove("source");
            }
            map.values_mut().for_each(c9_wr_own_source);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(c9_wr_own_source),
        _ => {}
    }
}
