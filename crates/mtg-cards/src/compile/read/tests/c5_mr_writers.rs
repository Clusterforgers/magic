//! Goal: the writers of the proposal checker give the oracle wording of the fifth sample of the names M to R (a return to the bottom of the library, up to X kicked targets, each of X targets, a doubled size, a quoted grant after an evasion, a sacrifice with a discard for who can't, a copy that gains a trigger, a move to a graveyard or to exile, a wheel each player may do, the draw step of another player, damage dealt this way, damage to the controller of the event, equip as long as, an exile from another player's library, a blink until the next end step, a size for each of two groups, lands returned as a cost and a table of swapped words), and a wrong number, object, player, kind, zone or step never reads back.
use rstest::rstest;

use super::*;

/// Whether some wording the checker accepts for the rules is the clause, with the wordings for a failed assertion.
fn c5_mr_written(json: &str, clause: &str) -> (bool, Vec<String>) {
    let wordings = written_variants(&ability(json));
    let found = wordings.iter().any(|wording| same_text(wording, clause));
    (found, wordings)
}

/// "When this creature dies, put it on the bottom of its owner's library."
const C5_MR_MURDEROUS_RIDER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Move":{"what":"This","to":{"Library":{"place":"Bottom"}}}}]}}"#;

/// "When this enchantment enters, return up to X target creature cards from your graveyard to the battlefield, where X is the number of times this enchantment was kicked."
const C5_MR_MARSHALS_ANTHEM: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":"Kicks"}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}}]}}"#;

/// "~ deals 4 damage to each of X targets."
const C5_MR_METEOR_BLAST: &str = r#"{"Spell":{"targets":[{"what":"AnyTarget","bounds":{"min":"X","max":"X"}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":4}}]}}"#;

/// "Whenever this creature or another enchantment you control enters, double target creature's power and toughness until end of turn."
const C5_MR_NYLEAS_COLOSSUS: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"ZoneChange":{"what":[{"Type":"Enchantment"},"Other",{"ControlledBy":"You"}],"to":"Battlefield"}}]},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":{"Stat":{"of":{"Target":0},"stat":"Power"}},"toughness":{"Stat":{"of":{"Target":0},"stat":"Toughness"}}}}],"duration":"UntilEndOfTurn"}}]}}"#;

/// "X target creatures can't be blocked this turn. Until end of turn, those creatures gain "Whenever this creature deals combat damage to a player, draw a card.""
const C5_MR_OPEN_INTO_WONDER: &str = r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":"X","max":"X"}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":["CantBeBlocked"],"duration":"UntilEndOfTurn"}},{"Continuous":{"affected":{"Target":0},"mods":[{"GrantAbility":{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Draw":{}}]}}}],"duration":"UntilEndOfTurn"}}]}}"#;

/// "When this creature enters, each player sacrifices a creature or planeswalker of their choice. Each player who can't discards a card."
const C5_MR_PLAGUECRAFTER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Sacrifice":{"who":{"Bound":"player"},"filter":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]}},{"If":{"condition":{"Not":"Performed"},"then":[{"Discard":{"who":{"Bound":"player"}}}]}}]}}]}}"#;

/// "You may have this creature enter as a copy of any creature on the battlefield, except it has "At the beginning of your upkeep, if this creature isn't a token, create a token that's a copy of this creature.""
const C5_MR_PROGENITOR_MIMIC: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Creature"}],"except":[{"GrantAbility":{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Not":{"Matches":{"subject":"This","selector":["Token"]}}},"effects":[{"CreateToken":{"token":{"CopyOf":{"of":"This"}}}}]}}}]}}}}"#;

/// "Whenever this creature or another nontoken artifact you control is put into a graveyard from the battlefield or is put into exile from the battlefield, target opponent loses 1 life and you gain 1 life."
const C5_MR_PSYCHOMANCER: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},{"ZoneChange":{"what":[{"Type":"Artifact"},{"Not":"Token"},"Other",{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Exile"}},{"ZoneChange":{"what":[{"Type":"Artifact"},{"Not":"Token"},"Other",{"ControlledBy":"You"}],"from":"Battlefield","to":"Exile"}}]},"targets":[{"what":"Opponent"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":1}},{"GainLife":{"amount":1}}]}}"#;

/// "When this creature dies, each player may discard their hand and draw seven cards."
const C5_MR_RUIN_GRINDER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"May":{"who":{"Bound":"player"},"effects":[{"Discard":{"who":{"Bound":"player"},"count":{"Count":[{"In":"Hand"},{"OwnedBy":{"Bound":"player"}}]}}},{"Draw":{"who":{"Bound":"player"},"count":7}}]}}]}}]}}"#;

/// "At the beginning of the draw step of enchanted creature's controller, that player draws an additional card."
const C5_MR_RIGHTEOUS_DRAW: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Draw","whose":{"ControllerOf":"Attached"}}},"effects":[{"Draw":{"who":{"ControllerOf":"Attached"}}}]}}"#;

/// "Enchanted creature gets +1/+1 for each card in its controller's hand."
const C5_MR_RIGHTEOUS_SIZE: &str = r#"{"Static":{"affected":"Attached","mods":[{"ModifyPT":{"power":{"Count":[{"In":"Hand"},{"OwnedBy":{"ControllerOf":"Attached"}}]},"toughness":{"Count":[{"In":"Hand"},{"OwnedBy":{"ControllerOf":"Attached"}}]}}}]}}"#;

/// "Whenever another creature you control enters, this creature deals 2 damage to it. If a Dinosaur is dealt damage this way, this creature gets +2/+0 until end of turn."
const C5_MR_MARAUDING_RAPTOR: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"DealDamage":{"to":"Triggering","amount":2}},{"If":{"condition":{"Matches":{"subject":"Triggering","selector":[{"Subtype":"Dinosaur"}]}},"then":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}]}}"#;

/// "Whenever an artifact an opponent controls is put into a graveyard from the battlefield, this creature deals 1 damage to that player."
const C5_MR_PAIN_DAMAGE: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Artifact"},{"ControlledBy":"Opponents"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"DealDamage":{"to":{"ControllerOf":"Triggering"},"amount":1}}]}}"#;

/// "Equipment you control have equip {0} as long as you control three or more artifacts."
const C5_MR_PURESTEEL_PALADIN: &str = r#"{"Static":{"affected":{"Each":[{"Subtype":"Equipment"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":{"Equip":"{0}"}}],"condition":{"Compare":{"left":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"op":"Ge","right":3}}}}"#;

/// "Whenever an opponent casts their first spell each turn, that player exiles the top card of their library. If it's a nonland card, you may cast it without paying its mana cost."
const C5_MR_MINDS_DILATION: &str = r#"{"Triggered":{"when":{"Nth":{"event":{"SpellCast":{"by":"Opponents"}},"nth":1}},"effects":[{"ExileTop":{"who":{"ControllerOf":"Triggering"},"as":"card"}},{"If":{"condition":{"Matches":{"subject":{"Bound":"card"},"selector":[{"Not":{"Type":"Land"}}]}},"then":[{"CastFromZone":{"what":{"Bound":"card"},"free":true}}]}}]}}"#;

/// "Discard three cards: Exile ~. Return it to the battlefield tapped under its owner's control at the beginning of the next end step."
const C5_MR_NEZAHAL: &str = r#"{"Activated":{"cost":[{"Discard":{"count":3}}],"effects":[{"Exile":{"what":"This","as":"exiled"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{"tapped":true}}}}]}}]}}"#;

/// "~ gets +1/+1 for each land you control and each land card in your graveyard."
const C5_MR_MULTANI_SIZE: &str = r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":{"Sum":[{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]},{"Count":[{"Type":"Land"},{"In":"Graveyard"},{"OwnedBy":"You"}]}]},"toughness":{"Sum":[{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]},{"Count":[{"Type":"Land"},{"In":"Graveyard"},{"OwnedBy":"You"}]}]}}}]}}"#;

/// "{1}{G}, Return two lands you control to their owner's hand: Return this card from your graveyard to your hand."
const C5_MR_MULTANI_RETURN: &str = r#"{"Activated":{"cost":[{"Mana":"{1}{G}"},{"ReturnToHand":{"filter":[{"Type":"Land"}],"count":2}}],"from":"Graveyard","effects":[{"Move":{"what":"This","to":"Hand"}}]}}"#;

/// "Creatures can't block this turn."
const C5_MR_CHAOS: &str = r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"}]},"mods":["CantBlock"],"duration":"UntilEndOfTurn"}}]}}"#;

/// "At the beginning of your end step, you may return target creature card with power less than ~'s from your graveyard to the battlefield with a finality counter on it."
const C5_MR_MIRKO: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"},{"Compare":{"stat":"Power","op":"Lt","to":{"Stat":{"of":"This","stat":"Power"}}}}]}}],"effects":[{"May":{"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{"counters":[[{"Other":"finality"},1]]}}}}]}}]}}"#;

/// "When a creature you control dies, sacrifice this enchantment. If you do, create four 1/1 colorless Spirit creature tokens."
const C5_MR_PROMISE_OF_BUNREI: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"Sacrifice":{"filter":["This"]}},{"If":{"condition":"Performed","then":[{"CreateToken":{"token":{"Custom":{"name":"Spirit","colors":[],"types":["Creature"],"subtypes":["Spirit"],"power":1,"toughness":1}},"count":4}}]}}]}}"#;

/// "Whenever a creature an opponent controls with a -1/-1 counter on it dies, you may return that card to the battlefield under your control."
const C5_MR_NECROSKITTER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"Opponents"},{"HasCounter":"-1/-1"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"May":{"effects":[{"Move":{"what":"Triggering","to":{"Battlefield":{"under":"You"}}}}]}}]}}"#;

/// "Whenever a player casts their first spell each turn, they create a Treasure token."
const C5_MR_PAIN_TREASURE: &str = r#"{"Triggered":{"when":{"Nth":{"event":{"SpellCast":{}},"nth":1}},"effects":[{"CreateToken":{"who":{"ControllerOf":"Triggering"},"token":{"Named":"treasure"}}}]}}"#;

/// "{T}: Goad target creature an opponent controls. That player draws a card. You add {R}."
const C5_MR_RED_DEATH: &str = r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"Goad":{"what":{"Target":0}}},{"Draw":{"who":{"ControllerOf":{"Target":0}}}},{"AddMana":{"mana":{"Fixed":["Red"]}}}]}}"#;

/// "Whenever ~ deals combat damage to a player, investigate that many times."
const C5_MR_PIPER_WRIGHT: &str = r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"CreateToken":{"token":{"Named":"clue"},"count":"Triggering"}}]}}"#;

/// "Whenever one or more creatures attack, you may pay {1}{R}. If you do, creatures attacking your opponents and/or planeswalkers they control get +2/+0 until end of turn."
const C5_MR_ROAR_OF_RESISTANCE: &str = r#"{"Triggered":{"when":{"Batch":{"Attacks":{"what":[{"Type":"Creature"}]}}},"effects":[{"Pay":{"cost":"{1}{R}","paid":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},"Attacking"]},"mods":[{"ModifyPT":{"power":2,"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}]}}"#;

/// "Whenever this Vehicle attacks, create three 1/1 red Hamster creature tokens, then it deals X damage to any target, where X is the number of Hamsters you control."
const C5_MR_ROLLING_HAMSPHERE: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":"AnyTarget"}],"effects":[{"CreateToken":{"token":{"Custom":{"name":"Hamster","colors":["Red"],"types":["Creature"],"subtypes":["Hamster"],"power":1,"toughness":1}},"count":3}},{"DealDamage":{"to":{"Target":0},"amount":{"Count":[{"Subtype":"Hamster"},{"ControlledBy":"You"}]}}}]}}"#;

/// "Whenever another creature enters, this creature deals 1 damage to that creature's controller."
const C5_MR_FEROCIDON: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["Other",{"Type":"Creature"}],"to":"Battlefield"}},"effects":[{"DealDamage":{"to":{"ControllerOf":"Triggering"},"amount":1}}]}}"#;

/// The rules of each card of the sample with its oracle clause (the card's name written `~`).
#[rstest]
#[case(
    C5_MR_MURDEROUS_RIDER,
    "When this creature dies, put it on the bottom of its owner's library."
)]
#[case(
    C5_MR_MARSHALS_ANTHEM,
    "When this enchantment enters, return up to X target creature cards from your graveyard to the battlefield, where X is the number of times this enchantment was kicked."
)]
#[case(C5_MR_METEOR_BLAST, "~ deals 4 damage to each of X targets.")]
#[case(
    C5_MR_NYLEAS_COLOSSUS,
    "Whenever this creature or another enchantment you control enters, double target creature's power and toughness until end of turn."
)]
#[case(
    C5_MR_OPEN_INTO_WONDER,
    "X target creatures can't be blocked this turn. Until end of turn, those creatures gain \"Whenever this creature deals combat damage to a player, draw a card.\""
)]
#[case(
    C5_MR_PLAGUECRAFTER,
    "When this creature enters, each player sacrifices a creature or planeswalker of their choice. Each player who can't discards a card."
)]
#[case(
    C5_MR_PROGENITOR_MIMIC,
    "You may have this creature enter as a copy of any creature on the battlefield, except it has \"At the beginning of your upkeep, if this creature isn't a token, create a token that's a copy of this creature.\""
)]
#[case(
    C5_MR_PSYCHOMANCER,
    "Whenever this creature or another nontoken artifact you control is put into a graveyard from the battlefield or is put into exile from the battlefield, target opponent loses 1 life and you gain 1 life."
)]
#[case(
    C5_MR_RUIN_GRINDER,
    "When this creature dies, each player may discard their hand and draw seven cards."
)]
#[case(
    C5_MR_RIGHTEOUS_DRAW,
    "At the beginning of the draw step of enchanted creature's controller, that player draws an additional card."
)]
#[case(
    C5_MR_RIGHTEOUS_SIZE,
    "Enchanted creature gets +1/+1 for each card in its controller's hand."
)]
#[case(
    C5_MR_MARAUDING_RAPTOR,
    "Whenever another creature you control enters, this creature deals 2 damage to it. If a Dinosaur is dealt damage this way, this creature gets +2/+0 until end of turn."
)]
#[case(
    C5_MR_PAIN_DAMAGE,
    "Whenever an artifact an opponent controls is put into a graveyard from the battlefield, this creature deals 1 damage to that player."
)]
#[case(
    C5_MR_PURESTEEL_PALADIN,
    "Equipment you control have equip {0} as long as you control three or more artifacts."
)]
#[case(
    C5_MR_MINDS_DILATION,
    "Whenever an opponent casts their first spell each turn, that player exiles the top card of their library. If it's a nonland card, you may cast it without paying its mana cost."
)]
#[case(
    C5_MR_NEZAHAL,
    "Discard three cards: Exile ~. Return it to the battlefield tapped under its owner's control at the beginning of the next end step."
)]
#[case(
    C5_MR_MULTANI_SIZE,
    "~ gets +1/+1 for each land you control and each land card in your graveyard."
)]
#[case(
    C5_MR_MULTANI_RETURN,
    "{1}{G}, Return two lands you control to their owner's hand: Return this card from your graveyard to your hand."
)]
#[case(C5_MR_CHAOS, "Creatures can't block this turn.")]
#[case(
    C5_MR_MIRKO,
    "At the beginning of your end step, you may return target creature card with power less than ~'s from your graveyard to the battlefield with a finality counter on it."
)]
#[case(
    C5_MR_PROMISE_OF_BUNREI,
    "When a creature you control dies, sacrifice this enchantment. If you do, create four 1/1 colorless Spirit creature tokens."
)]
#[case(
    C5_MR_NECROSKITTER,
    "Whenever a creature an opponent controls with a -1/-1 counter on it dies, you may return that card to the battlefield under your control."
)]
#[case(
    C5_MR_PAIN_TREASURE,
    "Whenever a player casts their first spell each turn, they create a Treasure token."
)]
#[case(
    C5_MR_RED_DEATH,
    "{T}: Goad target creature an opponent controls. That player draws a card. You add {R}."
)]
#[case(
    C5_MR_PIPER_WRIGHT,
    "Whenever ~ deals combat damage to a player, investigate that many times."
)]
#[case(
    C5_MR_ROAR_OF_RESISTANCE,
    "Whenever one or more creatures attack, you may pay {1}{R}. If you do, creatures attacking your opponents and/or planeswalkers they control get +2/+0 until end of turn."
)]
#[case(
    C5_MR_ROLLING_HAMSPHERE,
    "Whenever this Vehicle attacks, create three 1/1 red Hamster creature tokens, then it deals X damage to any target, where X is the number of Hamsters you control."
)]
#[case(
    C5_MR_FEROCIDON,
    "Whenever another creature enters, this creature deals 1 damage to that creature's controller."
)]
fn c5_mr_golden(#[case] json: &str, #[case] clause: &str) {
    let (found, wordings) = c5_mr_written(json, clause);
    assert!(found, "{clause}\n{wordings:#?}");
}

/// A near-miss of each wording: the rules with another number, object, player, kind, zone or step keep the clause.
#[rstest]
#[case(
    C5_MR_MURDEROUS_RIDER,
    r#""place":"Bottom""#,
    r#""place":"Top""#,
    "When this creature dies, put it on the bottom of its owner's library."
)]
#[case(
    C5_MR_MARSHALS_ANTHEM,
    r#""max":"Kicks""#,
    r#""max":"X""#,
    "When this enchantment enters, return up to X target creature cards from your graveyard to the battlefield, where X is the number of times this enchantment was kicked."
)]
#[case(
    C5_MR_METEOR_BLAST,
    r#""min":"X","max":"X""#,
    r#""min":0,"max":"X""#,
    "~ deals 4 damage to each of X targets."
)]
#[case(
    C5_MR_NYLEAS_COLOSSUS,
    r#""stat":"Toughness""#,
    r#""stat":"Power""#,
    "Whenever this creature or another enchantment you control enters, double target creature's power and toughness until end of turn."
)]
#[case(
    C5_MR_OPEN_INTO_WONDER,
    r#""effects":[{"Draw":{}}]"#,
    r#""effects":[{"Draw":{"count":2}}]"#,
    "X target creatures can't be blocked this turn. Until end of turn, those creatures gain \"Whenever this creature deals combat damage to a player, draw a card.\""
)]
#[case(
    C5_MR_PLAGUECRAFTER,
    r#""then":[{"Discard""#,
    r#""then":[{"Mill""#,
    "When this creature enters, each player sacrifices a creature or planeswalker of their choice. Each player who can't discards a card."
)]
#[case(
    C5_MR_PROGENITOR_MIMIC,
    r#""step":"Upkeep""#,
    r#""step":"End""#,
    "You may have this creature enter as a copy of any creature on the battlefield, except it has \"At the beginning of your upkeep, if this creature isn't a token, create a token that's a copy of this creature.\""
)]
#[case(
    C5_MR_PSYCHOMANCER,
    r#""to":"Exile"}},{"#,
    r#""to":"Hand"}},{"#,
    "Whenever this creature or another nontoken artifact you control is put into a graveyard from the battlefield or is put into exile from the battlefield, target opponent loses 1 life and you gain 1 life."
)]
#[case(
    C5_MR_RUIN_GRINDER,
    r#""count":7"#,
    r#""count":3"#,
    "When this creature dies, each player may discard their hand and draw seven cards."
)]
#[case(
    C5_MR_RIGHTEOUS_DRAW,
    r#""step":"Draw""#,
    r#""step":"Upkeep""#,
    "At the beginning of the draw step of enchanted creature's controller, that player draws an additional card."
)]
#[case(
    C5_MR_RIGHTEOUS_SIZE,
    r#""OwnedBy":{"ControllerOf":"Attached"}}]},"toughness""#,
    r#""OwnedBy":"Opponents"}]},"toughness""#,
    "Enchanted creature gets +1/+1 for each card in its controller's hand."
)]
#[case(
    C5_MR_MARAUDING_RAPTOR,
    r#""Subtype":"Dinosaur""#,
    r#""Subtype":"Dragon""#,
    "Whenever another creature you control enters, this creature deals 2 damage to it. If a Dinosaur is dealt damage this way, this creature gets +2/+0 until end of turn."
)]
#[case(
    C5_MR_PAIN_DAMAGE,
    r#""amount":1"#,
    r#""amount":2"#,
    "Whenever an artifact an opponent controls is put into a graveyard from the battlefield, this creature deals 1 damage to that player."
)]
#[case(
    C5_MR_PURESTEEL_PALADIN,
    r#""right":3"#,
    r#""right":4"#,
    "Equipment you control have equip {0} as long as you control three or more artifacts."
)]
#[case(
    C5_MR_MINDS_DILATION,
    r#""Not":{"Type":"Land"}"#,
    r#""Type":"Creature""#,
    "Whenever an opponent casts their first spell each turn, that player exiles the top card of their library. If it's a nonland card, you may cast it without paying its mana cost."
)]
#[case(
    C5_MR_NEZAHAL,
    r#""tapped":true"#,
    r#""tapped":false"#,
    "Discard three cards: Exile ~. Return it to the battlefield tapped under its owner's control at the beginning of the next end step."
)]
#[case(
    C5_MR_MULTANI_SIZE,
    r#"{"Type":"Land"},{"In":"Graveyard"}"#,
    r#"{"Type":"Creature"},{"In":"Graveyard"}"#,
    "~ gets +1/+1 for each land you control and each land card in your graveyard."
)]
#[case(
    C5_MR_MULTANI_RETURN,
    r#""count":2"#,
    r#""count":3"#,
    "{1}{G}, Return two lands you control to their owner's hand: Return this card from your graveyard to your hand."
)]
#[case(
    C5_MR_CHAOS,
    r#""mods":["CantBlock"]"#,
    r#""mods":["CantAttack"]"#,
    "Creatures can't block this turn."
)]
#[case(
    C5_MR_MIRKO,
    r#""op":"Lt""#,
    r#""op":"Gt""#,
    "At the beginning of your end step, you may return target creature card with power less than ~'s from your graveyard to the battlefield with a finality counter on it."
)]
#[case(
    C5_MR_PROMISE_OF_BUNREI,
    r#""count":4"#,
    r#""count":3"#,
    "When a creature you control dies, sacrifice this enchantment. If you do, create four 1/1 colorless Spirit creature tokens."
)]
#[case(
    C5_MR_NECROSKITTER,
    r#""under":"You""#,
    r#""tapped":true"#,
    "Whenever a creature an opponent controls with a -1/-1 counter on it dies, you may return that card to the battlefield under your control."
)]
#[case(
    C5_MR_PAIN_TREASURE,
    r#""Named":"treasure""#,
    r#""Named":"food""#,
    "Whenever a player casts their first spell each turn, they create a Treasure token."
)]
#[case(
    C5_MR_RED_DEATH,
    r#"{"Fixed":["Red"]}"#,
    r#"{"Fixed":["Blue"]}"#,
    "{T}: Goad target creature an opponent controls. That player draws a card. You add {R}."
)]
#[case(
    C5_MR_PIPER_WRIGHT,
    r#""combat":true"#,
    r#""combat":false"#,
    "Whenever ~ deals combat damage to a player, investigate that many times."
)]
#[case(
    C5_MR_ROAR_OF_RESISTANCE,
    r#""power":2"#,
    r#""power":1"#,
    "Whenever one or more creatures attack, you may pay {1}{R}. If you do, creatures attacking your opponents and/or planeswalkers they control get +2/+0 until end of turn."
)]
#[case(
    C5_MR_ROLLING_HAMSPHERE,
    r#"{"Subtype":"Hamster"},{"ControlledBy":"You"}"#,
    r#"{"Subtype":"Hamster"},{"ControlledBy":"Opponents"}"#,
    "Whenever this Vehicle attacks, create three 1/1 red Hamster creature tokens, then it deals X damage to any target, where X is the number of Hamsters you control."
)]
#[case(
    C5_MR_FEROCIDON,
    r#""amount":1"#,
    r#""amount":3"#,
    "Whenever another creature enters, this creature deals 1 damage to that creature's controller."
)]
fn c5_mr_near_miss(#[case] json: &str, #[case] from: &str, #[case] to: &str, #[case] clause: &str) {
    assert!(json.contains(from), "{from}");
    let (found, wordings) = c5_mr_written(&json.replacen(from, to, 1), clause);
    assert!(!found, "{clause}\n{wordings:#?}");
}
