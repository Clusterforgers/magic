//! Whole abilities of the rmt5 card pack whose steps the renderers do not join: each is matched whole as data and worded
//! as the card prints it, so another count, target, condition or duration in the same ability never reads back.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (the card's name as `~`; the lines of one spell joined by a newline).
const RMT5_WHOLES: [(&str, &str); 101] = [
    // artificers_hex
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"if":{"Compare":{"left":{"Count":[{"Type":"Creature"},{"HasAttached":[{"Is":"Attached"}]}]},"op":"Ge","right":1}},"effects":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},{"HasAttached":[{"Is":"Attached"}]}]}}}]}}"#,
        r"At the beginning of your upkeep, if enchanted Equipment is attached to a creature, destroy that creature.",
    ),
    // break_the_spell
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Enchantment"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"If":{"condition":{"Compare":{"left":{"Destroyed":[{"AnyOf":[{"ControlledBy":"You"},"Token"]}]},"op":"Ge","right":1}},"then":[{"Draw":{}}]}}]}}"#,
        r"Destroy target enchantment. If a permanent you controlled or a token was destroyed this way, draw a card.",
    ),
    // burnout
    (
        r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"Type":"Instant"}]}}],"effects":[{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Color":"Blue"}]}},"then":[{"Counter":{"what":{"Target":0}}}]}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"Upkeep"}},"effects":[{"Draw":{}}]}}]}}"#,
        r"Counter target instant spell if it's blue.
Draw a card at the beginning of the next turn's upkeep.",
    ),
    // drag_to_the_roots
    (
        r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{2}","condition":{"Compare":{"left":{"CardTypes":[{"In":"Graveyard"},{"OwnedBy":"You"}]},"op":"Ge","right":4}}}}"#,
        r"Delirium — This spell costs {2} less to cast as long as there are four or more card types among cards in your graveyard.",
    ),
    // elbrus_the_binding_blade_withengar_unbound
    (
        r#"{"Triggered":{"when":{"DealsDamage":{"source":[{"Is":"Attached"}],"to":"EachPlayer","combat":true}},"effects":[{"Unattach":{"what":"This"}},{"Transform":{"what":"This"}}]}}"#,
        r"When equipped creature deals combat damage to a player, unattach ~, then transform it.",
    ),
    // feast_of_worms
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Land"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Supertype":"Legendary"}]}},"then":[{"Sacrifice":{"who":{"ControllerOf":{"Target":0}},"filter":[{"Type":"Land"},{"Not":{"Is":{"Target":0}}}]}}]}}]}}"#,
        r"Destroy target land. If that land was legendary, its controller sacrifices another land of their choice.",
    ),
    // flare_of_faith
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Subtype":"Human"}]}},"then":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":3,"toughness":3}},{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}],"otherwise":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":2,"toughness":2}}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
        r"Target creature gets +2/+2 until end of turn. If it's a Human, instead it gets +3/+3 and gains indestructible until end of turn.",
    ),
    // fyndhorn_druid
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"if":{"Matches":{"subject":"This","selector":[{"DidThisTurn":{"event":"Blocked"}}]}},"effects":[{"GainLife":{"amount":4}}]}}"#,
        r"When this creature dies, if it was blocked this turn, you gain 4 life.",
    ),
    // gleeful_demolition
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"ControlledBy":"You"}]}},"then":[{"CreateToken":{"token":{"Custom":{"name":"Phyrexian Goblin","colors":["Red"],"types":["Creature"],"subtypes":["Phyrexian","Goblin"],"power":1,"toughness":1}},"count":3}}]}}]}}"#,
        r"Destroy target artifact. If you controlled that artifact, create three 1/1 red Phyrexian Goblin creature tokens.",
    ),
    // hog_monkey_rampage
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}},{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}],"effects":[{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Compare":{"stat":"Power","op":"Ge","to":4}}]}},"then":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}]}},{"Fight":{"what":{"Target":0},"with":{"Target":1}}}]}}"#,
        r"Choose target creature you control and target creature an opponent controls. Put a +1/+1 counter on the creature you control if it has power 4 or greater. Then those creatures fight each other.",
    ),
    // homarid_warrior
    (
        r#"{"Activated":{"cost":"{U}","effects":[{"Continuous":{"affected":"This","mods":[{"AddKeyword":"Shroud"}],"duration":"UntilEndOfTurn"}},{"Continuous":{"affected":"This","mods":["DoesntUntap"],"duration":"NextUntapStep"}},{"Tap":{"what":"This"}}]}}"#,
        r"{U}: This creature gains shroud until end of turn and doesn't untap during your next untap step. Tap it.",
    ),
    // kraul_harpooner
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}},{"HasKeyword":"Flying"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":{"Count":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"toughness":0}}],"duration":"UntilEndOfTurn"}},{"May":{"effects":[{"Fight":{"what":"This","with":{"Target":0}}}]}}]}}"#,
        r"Undergrowth — When this creature enters, choose up to one target creature you don't control with flying. This creature gets +X/+0 until end of turn, where X is the number of creature cards in your graveyard, then you may have this creature fight that creature.",
    ),
    // rebel_salvo
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":5}},{"Continuous":{"affected":{"Target":0},"mods":[{"RemoveKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}]}}"#,
        r"~ deals 5 damage to target creature or planeswalker. That permanent loses indestructible until end of turn.",
    ),
    // reign_of_the_pit
    (
        r#"{"Spell":{"effects":[{"Sacrifice":{"who":"EachPlayer","filter":[{"Type":"Creature"}],"as":"sacrificed"}},{"CreateToken":{"token":{"Sized":{"size":{"Total":{"of":[{"Is":{"Bound":"sacrificed"}}],"stat":"Power"}},"token":{"name":"Demon","colors":["Black"],"types":["Creature"],"subtypes":["Demon"],"abilities":[{"Keyword":"Flying"}]}}}}}]}}"#,
        r"Each player sacrifices a creature of their choice. Create an X/X black Demon creature token with flying, where X is the total power of the creatures sacrificed this way.",
    ),
    // searing_blaze
    (
        r#"{"Spell":{"targets":[{"what":{"Either":["Player",{"Object":[{"Type":"Planeswalker"}]}]}},{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":{"OrController":{"Target":0}}}]}}],"effects":[{"If":{"condition":{"Compare":{"left":{"Tally":{"event":"Entered","who":"You","filter":[{"Type":"Land"}]}},"op":"Ge","right":1}},"then":[{"DealDamage":{"to":{"Target":0},"amount":3}},{"DealDamage":{"to":{"Target":1},"amount":3}}],"otherwise":[{"DealDamage":{"to":{"Target":0},"amount":1}},{"DealDamage":{"to":{"Target":1},"amount":1}}]}}]}}"#,
        r"~ deals 1 damage to target player or planeswalker and 1 damage to target creature that player or that planeswalker's controller controls.
If you had a land enter the battlefield under your control this turn, ~ deals 3 damage to that player or planeswalker and 3 damage to that creature instead.",
    ),
    // spoils_of_evil
    (
        r#"{"Spell":{"targets":[{"what":"Opponent"}],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]},"times":{"Count":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]},{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]}}},{"GainLife":{"amount":{"Count":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]},{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]}}}]}}"#,
        r"For each artifact or creature card in target opponent's graveyard, add {C} and you gain 1 life.",
    ),
    // storm_the_festival
    (
        r#"{"Spell":{"effects":[{"LookAtTop":{"count":5,"as":"top"}},{"Choose":{"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}},{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"},{"Type":"Battle"}]},{"Compare":{"stat":"ManaValue","op":"Le","to":5}}]},"count":{"min":0,"max":2},"as":"kept"}},{"Move":{"what":{"Bound":"kept"},"to":{"Battlefield":{}}}},{"Move":{"what":{"Bound":"top"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}}"#,
        r"Look at the top five cards of your library. You may put up to two permanent cards with mana value 5 or less from among them onto the battlefield. Put the rest on the bottom of your library in a random order.",
    ),
    // tectonic_instability
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"}],"to":"Battlefield"}},"effects":[{"Tap":{"what":{"Each":[{"Type":"Land"},{"ControlledBy":{"ControllerOf":"Triggering"}}]}}}]}}"#,
        r"Whenever a land enters, tap all lands its controller controls.",
    ),
    // triceraton_commander
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"Continuous":{"affected":{"Each":[{"Subtype":"Dinosaur"},{"ControlledBy":"You"},"Other"]},"mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddKeyword":"Flying"}],"duration":"UntilEndOfTurn"}}]}}"#,
        r"Whenever this creature attacks, Dinosaurs you control other than this creature get +1/+1 and gain flying until end of turn.",
    ),
    // winged_hive_tyrant
    (
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},"Other",{"ControlledBy":"You"},"HasAnyCounter"]},"mods":[{"AddKeyword":"Flying"},{"AddKeyword":"Haste"}]}}"#,
        r"The Will of the Hive Mind — Other creatures you control with counters on them have flying and haste.",
    ),
    // word_of_blasting
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Subtype":"Wall"}]}}],"effects":[{"Destroy":{"what":{"Target":0},"regenerate":false}},{"DealDamage":{"to":{"ControllerOf":{"Target":0}},"amount":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}]}}"#,
        r"Destroy target Wall. It can't be regenerated. ~ deals damage equal to that Wall's mana value to the Wall's controller.",
    ),
    // zur_eternal_schemer
    (
        r#"{"Activated":{"cost":"{1}{W}","targets":[{"what":{"Object":[{"Type":"Enchantment"},{"Not":{"Subtype":"Aura"}},{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddType":"Creature"},{"SetPT":{"power":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}},"toughness":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}],"duration":"Forever"}}]}}"#,
        r"{1}{W}: Target non-Aura enchantment you control becomes a creature in addition to its other types and has base power and base toughness each equal to its mana value.",
    ),
    // anje_maid_of_dishonor
    (
        r#"{"Triggered":{"when":{"Batch":{"ZoneChange":{"what":[{"AnyOf":["This",{"Subtype":"Vampire"}]},{"ControlledBy":"You"}],"to":"Battlefield"}}},"limit":"OncePerTurn","effects":[{"CreateToken":{"token":{"Named":"blood"}}}]}}"#,
        r"Whenever ~ and/or one or more other Vampires you control enter, create a Blood token. This ability triggers only once each turn.",
    ),
    // anje_maid_of_dishonor
    (
        r#"{"Activated":{"cost":[{"Mana":"{2}"},{"Sacrifice":{"filter":["Other",{"AnyOf":[{"Type":"Creature"},{"Subtype":"Blood"}]}]}}],"effects":[{"LoseLife":{"who":"Opponents","amount":2}},{"GainLife":{"amount":2}}]}}"#,
        r"{2}, Sacrifice another creature or a Blood token: Each opponent loses 2 life and you gain 2 life.",
    ),
    // endless_evil
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"CreateToken":{"token":{"CopyOf":{"of":"Attached","except":[{"SetPT":{"power":1,"toughness":1}}]}}}}]}}"#,
        r"At the beginning of your upkeep, create a token that's a copy of enchanted creature, except the token is 1/1.",
    ),
    // endless_evil
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Is":"Attached"}],"from":"Battlefield","to":"Graveyard"}},"if":{"Matches":{"subject":"Triggering","selector":[{"Subtype":"Horror"}]}},"effects":[{"Move":{"what":"This","to":"Hand"}}]}}"#,
        r"When enchanted creature dies, if that creature was a Horror, return this card to its owner's hand.",
    ),
    // firefright_mage
    (
        r#"{"Activated":{"cost":[{"Mana":"{1}{R}"},"Tap",{"Discard":{"filter":[]}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"CantBeBlockedBy":[{"Not":{"AnyOf":[{"Type":"Artifact"},{"Color":"Red"}]}}]}],"duration":"UntilEndOfTurn"}}]}}"#,
        r"{1}{R}, {T}, Discard a card: Target creature can't be blocked this turn except by artifact creatures and/or red creatures.",
    ),
    // heat_shimmer
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"AddKeyword":"Haste"},{"GrantAbility":{"Triggered":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Exile":{"what":"This"}}]}}}]}}}}]}}"#,
        r#"Create a token that's a copy of target creature, except it has haste and "At the beginning of the end step, exile this token.""#,
    ),
    // phyrexian_devourer
    (
        r#"{"Triggered":{"when":{"State":{"condition":{"Matches":{"subject":"This","selector":[{"Compare":{"stat":"Power","op":"Ge","to":7}}]}}}},"effects":[{"Sacrifice":{"filter":["This"]}}]}}"#,
        r"When this creature's power is 7 or greater, sacrifice it.",
    ),
    // phyrexian_devourer
    (
        r#"{"Activated":{"cost":[{"ExileTop":1}],"effects":[{"PutCounters":{"on":"This","counter":"+1/+1","count":{"Stat":{"of":"Paid","stat":"ManaValue"}}}}]}}"#,
        r"Exile the top card of your library: Put X +1/+1 counters on this creature, where X is the exiled card's mana value.",
    ),
    // saproling_infestation
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":["Kicked"]}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Saproling","colors":["Green"],"types":["Creature"],"subtypes":["Saproling"],"power":1,"toughness":1}}}}]}}"#,
        r"Whenever a player kicks a spell, you create a 1/1 green Saproling creature token.",
    ),
    // storm_elemental
    (
        r#"{"Activated":{"cost":[{"Mana":"{U}"},{"ExileTop":1}],"targets":[{"what":{"Object":[{"Type":"Creature"},{"HasKeyword":"Flying"}]}}],"effects":[{"Tap":{"what":{"Target":0}}}]}}"#,
        r"{U}, Exile the top card of your library: Tap target creature with flying.",
    ),
    // storm_elemental
    (
        r#"{"Activated":{"cost":[{"Mana":"{U}"},{"ExileTop":1}],"effects":[{"If":{"condition":{"Matches":{"subject":"Paid","selector":[{"Supertype":"Snow"},{"Type":"Land"}]}},"then":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
        r"{U}, Exile the top card of your library: If the exiled card is a snow land, this creature gets +1/+1 until end of turn.",
    ),
    // words_of_waste
    (
        r#"{"Activated":{"cost":"{1}","effects":[{"Shield":{"when":{"Draw":{}},"by":{"InsteadNext":[{"Discard":{"who":"Opponents"}}]},"duration":"UntilEndOfTurn"}}]}}"#,
        r"{1}: The next time you would draw a card this turn, each opponent discards a card instead.",
    ),
    // words_of_wilding
    (
        r#"{"Activated":{"cost":"{1}","effects":[{"Shield":{"when":{"Draw":{}},"by":{"InsteadNext":[{"CreateToken":{"token":{"Custom":{"name":"Bear","colors":["Green"],"types":["Creature"],"subtypes":["Bear"],"power":2,"toughness":2}}}}]},"duration":"UntilEndOfTurn"}}]}}"#,
        r"{1}: The next time you would draw a card this turn, create a 2/2 green Bear creature token instead.",
    ),
    // words_of_wind
    (
        r#"{"Activated":{"cost":"{1}","effects":[{"Shield":{"when":{"Draw":{}},"by":{"InsteadNext":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Choose":{"who":{"Bound":"player"},"from":{"Each":[{"ControlledBy":{"Bound":"player"}}]},"as":"picked"}},{"Move":{"what":{"Bound":"picked"},"to":"Hand"}}]}}]},"duration":"UntilEndOfTurn"}}]}}"#,
        r"{1}: The next time you would draw a card this turn, each player returns a permanent they control to its owner's hand instead.",
    ),
    // words_of_worship
    (
        r#"{"Activated":{"cost":"{1}","effects":[{"Shield":{"when":{"Draw":{}},"by":{"InsteadNext":[{"GainLife":{"amount":5}}]},"duration":"UntilEndOfTurn"}}]}}"#,
        r"{1}: The next time you would draw a card this turn, you gain 5 life instead.",
    ),
    // brace_for_impact
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"Colors","op":"Ge","to":2}}]}}],"effects":[{"Shield":{"when":{"Damage":{"to":{"Target":0}}},"by":{"PreventAnd":[{"PutCounters":{"on":"Triggering","counter":"+1/+1","count":"Triggering"}}]},"duration":"UntilEndOfTurn"}}]}}"#,
        r"Prevent all damage that would be dealt to target multicolored creature this turn. For each 1 damage prevented this way, put a +1/+1 counter on that creature.",
    ),
    // chandra_spark_hunter
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"targets":[{"what":{"Object":[{"Subtype":"Vehicle"},{"ControlledBy":"You"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddType":"Artifact"},{"AddType":"Creature"},{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}}"#,
        r"At the beginning of combat on your turn, choose up to one target Vehicle you control. Until end of turn, it becomes an artifact creature and gains haste.",
    ),
    // crumbling_sanctuary
    (
        r#"{"Replacement":{"when":{"Damage":{"to":"EachPlayer"}},"by":{"Instead":[{"ExileTop":{"who":"Triggering","count":"Triggering","as":"exiled"}}]}}}"#,
        r"If damage would be dealt to a player, that player exiles that many cards from the top of their library instead.",
    ),
    // gloom_surgeon
    (
        r#"{"Replacement":{"when":{"Damage":{"to":"This","combat":true}},"by":{"PreventAnd":[{"ExileTop":{"count":"Triggering","as":"exiled"}}]}}}"#,
        r"If combat damage would be dealt to this creature, prevent that damage and exile that many cards from the top of your library.",
    ),
    // jared_carthalion
    (
        r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Kavu","colors":["White","Blue","Black","Red","Green"],"types":["Creature"],"subtypes":["Kavu"],"power":3,"toughness":3,"abilities":[{"Keyword":"Trample"}]}}}}]}}"#,
        r"+1: Create a 3/3 Kavu creature token with trample that's all colors.",
    ),
    // jared_carthalion
    (
        r#"{"Loyalty":{"cost":{"Minus":3},"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":2}}],"effects":[{"RepeatEach":{"over":{"Target":0},"as":"each","effects":[{"PutCounters":{"on":{"Bound":"each"},"counter":"+1/+1","count":{"Stat":{"of":{"Bound":"each"},"stat":"Colors"}}}}]}}]}}"#,
        r"−3: Choose up to two target creatures. For each of them, put a number of +1/+1 counters on it equal to the number of colors it is.",
    ),
    // jared_carthalion
    (
        r#"{"Loyalty":{"cost":{"Minus":6},"targets":[{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"You"},{"Compare":{"stat":"Colors","op":"Ge","to":2}}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}},{"If":{"condition":{"Matches":{"subject":{"Target":0},"selector":[{"Compare":{"stat":"Colors","op":"Eq","to":5}}]}},"then":[{"Draw":{}},{"CreateToken":{"token":{"Named":"treasure"},"count":2}}]}}]}}"#,
        r"−6: Return target multicolored card from your graveyard to your hand. If that card was all colors, draw a card and create two Treasure tokens.",
    ),
    // joshua_phoenixs_dominant_phoenix_warden_of_fire
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Discard":{"count":2,"optional":true,"as":"discarded"}},{"Draw":{"count":{"Count":[{"Is":{"Bound":"discarded"}}]}}}]}}"#,
        r"When ~ enters, discard up to two cards, then draw that many cards.",
    ),
    // joshua_phoenixs_dominant_phoenix_warden_of_fire
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"targets":[{"what":{"Together":{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"rule":{"TotalManaValueAtMost":6}}},"bounds":{"min":0,"max":null}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}},{"Exile":{"what":"This","as":"exiled"}},{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{}}}}]}}"#,
        r"III — Flames of Rebirth — Return any number of target creature cards with total mana value 6 or less from your graveyard to the battlefield. Exile ~, then return it to the battlefield.",
    ),
    // phyrexian_vindicator
    (
        r#"{"Replacement":{"when":{"Damage":{"to":"This"}},"by":{"PreventAnd":[{"Reflexive":{"targets":[{"what":{"Either":["Player",{"Object":[{"Not":"This"},{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"},{"Type":"Battle"}]}]}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Triggering"}}]}}]}}}"#,
        r"If damage would be dealt to this creature, prevent that damage. When damage is prevented this way, this creature deals that much damage to any other target.",
    ),
    // seers_vision
    (
        r#"{"Activated":{"cost":["SacrificeSelf"],"timing":"Sorcery","targets":[{"what":"Player"}],"effects":[{"Discard":{"who":{"Target":0},"by":"You"}}]}}"#,
        r"Sacrifice this enchantment: Look at target player's hand and choose a card from it. That player discards that card. Activate only as a sorcery.",
    ),
    // sheoldred_the_true_scriptures
    (
        r#"{"Activated":{"cost":"{4}{B}","timing":"Sorcery","condition":{"Compare":{"left":{"Players":{"PlayersWhere":{"who":"Opponents","test":{"Compare":{"left":{"Count":[{"In":"Graveyard"},{"OwnedBy":"You"}]},"op":"Ge","right":8}}}}},"op":"Ge","right":1}},"effects":[{"Exile":{"what":"This","as":"exiled"}},{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{"transformed":true}}}}]}}"#,
        r"{4}{B}: Exile ~, then return it to the battlefield transformed under its owner's control. Activate only as a sorcery and only if an opponent has eight or more cards in their graveyard.",
    ),
    // sheoldred_the_true_scriptures
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":2}},"effects":[{"Discard":{"who":"Opponents","count":3}},{"Mill":{"who":"Opponents","count":3}}]}}"#,
        r"II — Each opponent discards three cards, then mills three cards.",
    ),
    // sheoldred_the_true_scriptures
    (
        r#"{"Triggered":{"when":{"Chapter":{"number":3}},"effects":[{"Move":{"what":{"Each":[{"Type":"Creature"},{"In":"Graveyard"}]},"to":{"Battlefield":{"under":"You"}}}},{"Exile":{"what":"This","as":"exiled"}},{"Move":{"what":{"Bound":"exiled"},"to":{"Battlefield":{}}}}]}}"#,
        r"III — Put all creature cards from all graveyards onto the battlefield under your control. Exile this Saga, then return it to the battlefield.",
    ),
    // sorin_of_house_markov_sorin_ravenous_neonate
    (
        r#"{"Loyalty":{"cost":{"Minus":1},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Tallied":{"who":"You","what":"LifeGainedTotal"}}}}]}}"#,
        r"−1: ~ deals damage equal to the amount of life you gained this turn to any target.",
    ),
    // sorin_of_house_markov_sorin_ravenous_neonate
    (
        r#"{"Loyalty":{"cost":{"Minus":6},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetController":"You"},{"AddSubtype":"Vampire"}],"duration":"Forever"}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Color":"White"},{"ControlledBy":"You"},"Other",{"Not":{"Is":{"Target":0}}}]},"op":"Ge","right":1}},"then":[{"PutCounters":{"on":{"Target":0},"counter":{"Other":"lifelink"}}}]}}]}}"#,
        r"−6: Gain control of target creature. It becomes a Vampire in addition to its other types. Put a lifelink counter on it if you control a white permanent other than that creature or ~.",
    ),
    // tezzeret_the_schemer
    (
        r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Etherium Cell","types":["Artifact"],"abilities":[{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}]}}}}]}}"#,
        r#"+1: Create a colorless artifact token named Etherium Cell with "{T}, Sacrifice this token: Add one mana of any color.""#,
    ),
    // tezzeret_the_schemer
    (
        r#"{"Loyalty":{"cost":{"Minus":7},"effects":[{"CreateEmblem":{"abilities":[{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Artifact"},{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddType":"Creature"},{"SetPT":{"power":5,"toughness":5}}],"duration":"Forever"}}]}}]}}]}}"#,
        r#"−7: You get an emblem with "At the beginning of combat on your turn, target artifact you control becomes an artifact creature with base power and toughness 5/5.""#,
    ),
    // the_mindskinner
    (
        r#"{"Replacement":{"when":{"Damage":{"to":"Opponents","source":[{"ControlledBy":"You"}]}},"by":{"PreventAnd":[{"Mill":{"who":"Opponents","count":"Triggering"}}]}}}"#,
        r"If a source you control would deal damage to an opponent, prevent that damage and each opponent mills that many cards.",
    ),
    // tor_wauki_the_younger
    (
        r#"{"Replacement":{"when":{"Damage":{"source":["Other",{"ControlledBy":"You"}],"combat":false}},"by":{"Add":1}}}"#,
        r"If another source you control would deal noncombat damage to a permanent or player, it deals that much damage plus 1 to that permanent or player instead.",
    ),
    // tor_wauki_the_younger
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]}],"by":"You"}},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":2}}]}}"#,
        r"Whenever you cast an instant or sorcery spell, Tor Wauki deals 2 damage to any target.",
    ),
    // ancient_cellarspawn
    (
        r#"{"CostModifier":{"applies_to":["Spell",{"AnyOf":[{"Subtype":"Demon"},{"Subtype":"Horror"},{"Subtype":"Nightmare"}]},{"ControlledBy":"You"}],"change":"Reduce","mana":"{1}"}}"#,
        r"Each spell you cast that's a Demon, Horror, or Nightmare costs {1} less to cast.",
    ),
    // ancient_cellarspawn
    (
        r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"if":{"Compare":{"left":{"Stat":{"of":"Triggering","stat":"ManaSpent"}},"op":"Lt","right":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}},"targets":[{"what":"Opponent"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":{"Difference":[{"Stat":{"of":"Triggering","stat":"ManaValue"}},{"Stat":{"of":"Triggering","stat":"ManaSpent"}}]}}}]}}"#,
        r"Whenever you cast a spell, if the amount of mana spent to cast it was less than its mana value, target opponent loses life equal to the difference.",
    ),
    // cartographers_hawk
    (
        r#"{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":{"PlayersWhere":{"who":"EachPlayer","test":{"Compare":{"left":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]},"op":"Gt","right":{"Count":[{"Type":"Land"},{"ControlledBy":"Asker"}]}}}}},"combat":true}},"effects":[{"Move":{"what":"This","to":"Hand","as":"returned"}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Is":{"Bound":"returned"}},{"In":"Hand"}]},"op":"Ge","right":1}},"then":[{"May":{"effects":[{"Search":{"filter":[{"Subtype":"Plains"}],"as":"found"}},{"Move":{"what":{"Bound":"found"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}]}}]}}"#,
        r"When this creature deals combat damage to a player who controls more lands than you, return it to its owner's hand. If you do, you may search your library for a Plains card, put it onto the battlefield tapped, then shuffle.",
    ),
    // disciple_of_deceit
    (
        r#"{"Triggered":{"when":{"Untapped":{"what":["This"]}},"effects":[{"Discard":{"filter":[{"Not":{"Type":"Land"}}],"optional":true,"as":"discarded"}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Is":{"Bound":"discarded"}}]},"op":"Ge","right":1}},"then":[{"Search":{"filter":[{"Compare":{"stat":"ManaValue","op":"Eq","to":{"Stat":{"of":{"Bound":"discarded"},"stat":"ManaValue"}}}}],"as":"found"}},{"Reveal":{"what":{"Bound":"found"}}},{"Move":{"what":{"Bound":"found"},"to":"Hand"}},{"Shuffle":{}}]}}]}}"#,
        r"Inspired — Whenever this creature becomes untapped, you may discard a nonland card. If you do, search your library for a card with the same mana value as that card, reveal it, put it into your hand, then shuffle.",
    ),
    // dovin_baan
    (
        r#"{"Loyalty":{"cost":{"Plus":1},"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":-3,"toughness":0}}],"duration":"UntilYourNextTurn"}},{"Rule":{"rule":{"Cant":{"who":{"Target":0},"action":{"Activate":[]}}},"duration":"UntilYourNextTurn"}}]}}"#,
        r"+1: Until your next turn, up to one target creature gets -3/-0 and its activated abilities can't be activated.",
    ),
    // fire_giants_fury
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Subtype":"Giant"},{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}},{"DelayedTrigger":{"when":{"DealsDamage":{"source":[{"Is":{"Target":0}}],"to":"EachPlayer","combat":true}},"once":false,"this_turn":true,"effects":[{"ExileTop":{"count":"Triggering","as":"exiled"}},{"Rule":{"rule":{"MayCast":{"what":[{"Is":{"Bound":"exiled"}}],"from":"Exile","play":true}},"duration":"UntilEndOfYourNextTurn"}}]}}]}}"#,
        r"Target Giant you control gets +2/+2 and gains trample until end of turn. Whenever it deals combat damage to a player this turn, exile that many cards from the top of your library. Until the end of your next turn, you may play those cards.",
    ),
    // gideon_of_the_trials
    (
        r#"{"Loyalty":{"cost":{"Plus":1},"targets":[{"what":{"Object":[]}}],"effects":[{"Shield":{"when":{"Damage":{"source":[{"Is":{"Target":0}}]}},"by":"Prevent","duration":"UntilYourNextTurn"}}]}}"#,
        r"+1: Until your next turn, prevent all damage target permanent would deal.",
    ),
    // gideon_of_the_trials
    (
        r#"{"Loyalty":{"cost":"Zero","effects":[{"Continuous":{"affected":"This","mods":[{"AddType":"Creature"},{"SetCreatureTypes":["Human","Soldier"]},{"SetPT":{"power":4,"toughness":4}},{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}},{"Shield":{"when":{"Damage":{"to":"This"}},"by":"Prevent","duration":"UntilEndOfTurn"}}]}}"#,
        r"0: Until end of turn, ~ becomes a 4/4 Human Soldier creature with indestructible that's still a planeswalker. Prevent all damage that would be dealt to him this turn.",
    ),
    // gideon_of_the_trials
    (
        r#"{"Loyalty":{"cost":"Zero","effects":[{"CreateEmblem":{"abilities":[{"Rule":{"rule":{"CantLoseGame":{"who":"You","opponents_cant_win":true}},"condition":{"Compare":{"left":{"Count":[{"Type":"Planeswalker"},{"Subtype":"Gideon"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}}}]}}]}}"#,
        r#"0: You get an emblem with "As long as you control a ~ planeswalker, you can't lose the game and your opponents can't win the game.""#,
    ),
    // hazezon_tamar
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"DelayedTrigger":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Sand Warrior","colors":["Red","Green","White"],"types":["Creature"],"subtypes":["Sand","Warrior"],"power":1,"toughness":1}},"count":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}}]}}]}}"#,
        r"When ~ enters, create X 1/1 Sand Warrior creature tokens that are red, green, and white at the beginning of your next upkeep, where X is the number of lands you control at that time.",
    ),
    // hazezon_tamar
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield"}},"effects":[{"Exile":{"what":{"Each":[{"Subtype":"Sand"},{"Subtype":"Warrior"}]}}}]}}"#,
        r"When ~ leaves the battlefield, exile all Sand Warriors.",
    ),
    // hit_run
    (
        r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"Sacrifice":{"who":{"Target":0},"filter":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]}],"as":"sacrificed"}},{"DealDamage":{"to":{"Target":0},"amount":{"Total":{"of":[{"Is":{"Bound":"sacrificed"}}],"stat":"ManaValue"}}}}]}}"#,
        r"Target player sacrifices an artifact or creature of their choice. ~ deals damage to that player equal to that permanent's mana value.",
    ),
    // invasion_of_regatha_disciples_of_the_inferno
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Either":["Opponent",{"Object":[{"Type":"Battle"},{"Not":"This"}]}]}},{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":1}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":4}},{"DealDamage":{"to":{"Target":1},"amount":1}}]}}"#,
        r"When this Siege enters, it deals 4 damage to another target battle or opponent and 1 damage to up to one target creature.",
    ),
    // kaya_geist_hunter
    (
        r#"{"Loyalty":{"cost":{"Minus":6},"effects":[{"Exile":{"what":{"Each":[{"In":"Graveyard"}]},"as":"exiled"}},{"CreateToken":{"token":{"Custom":{"name":"Spirit","colors":["White"],"types":["Creature"],"subtypes":["Spirit"],"power":1,"toughness":1,"abilities":[{"Keyword":"Flying"}]}},"count":{"Count":[{"Is":{"Bound":"exiled"}}]}}}]}}"#,
        r"−6: Exile all cards from all graveyards, then create a 1/1 white Spirit creature token with flying for each card exiled this way.",
    ),
    // stunning_reversal
    (
        r#"{"Spell":{"effects":[{"Shield":{"when":{"LoseGame":{}},"by":{"InsteadNext":[{"Draw":{"count":7}},{"SetLife":{"amount":1}}]},"duration":"UntilEndOfTurn"}},{"Exile":{"what":"This"}}]}}"#,
        r"The next time you would lose the game this turn, instead draw seven cards and your life total becomes 1.
Exile ~.",
    ),
    // tam_mindful_first_year
    (
        r#"{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"SetColors":["White","Blue","Black","Red","Green"]}],"duration":"UntilEndOfTurn"}}]}}"#,
        r"{T}: Target creature you control becomes all colors until end of turn.",
    ),
    // toshiro_umezawa
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"Opponents"}],"from":"Battlefield","to":"Graveyard"}},"targets":[{"what":{"Object":[{"Type":"Instant"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"CastFromZone":{"what":{"Target":0},"as":"cast"}},{"Shield":{"when":{"ZoneChange":{"what":[{"In":"Stack"},{"Is":{"Bound":"cast"}}],"from":"Stack","to":"Graveyard"}},"by":{"MoveTo":"Exile"},"duration":"Forever"}}]}}"#,
        r"Whenever a creature an opponent controls dies, you may cast target instant card from your graveyard. If that spell would be put into a graveyard, exile it instead.",
    ),
    // transmutation_font
    (
        r#"{"Activated":{"cost":["Tap"],"effects":[{"OneOf":{"options":[[{"CreateToken":{"token":{"Named":"blood"}}}],[{"CreateToken":{"token":{"Named":"clue"}}}],[{"CreateToken":{"token":{"Named":"food"}}}]]}}]}}"#,
        r"{T}: Create your choice of a Blood token, a Clue token, or a Food token.",
    ),
    // walking_desecration
    (
        r#"{"Activated":{"cost":[{"Mana":"{B}"},"Tap"],"effects":[{"ChooseValue":{"kind":"CreatureType","as":"type"}},{"Rule":{"rule":{"MustAttack":{"Each":[{"Type":"Creature"},{"ChosenAs":"type"}]}},"duration":"UntilEndOfTurn"}}]}}"#,
        r"{B}, {T}: Creatures of the creature type of your choice attack this turn if able.",
    ),
    // weftwalking
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Matches":{"subject":"This","selector":[{"WasCast":{"by":"You"}}]}},"effects":[{"Move":{"what":{"Each":[{"AnyOf":[{"In":"Hand"},{"In":"Graveyard"}]},{"OwnedBy":"You"}]},"to":{"Library":{"place":"Shuffled"}}}},{"Draw":{"count":7}}]}}"#,
        r"When this enchantment enters, if you cast it, shuffle your hand and graveyard into your library, then draw seven cards.",
    ),
    // sephiroth_fabled_soldier_sephiroth_one_winged_angel
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other"],"from":"Battlefield","to":"Graveyard"}},"targets":[{"what":"Opponent"}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":1}},{"GainLife":{"amount":1}},{"If":{"condition":{"Compare":{"left":"ResolvedThisTurn","op":"Eq","right":4}},"then":[{"Transform":{"what":"This"}}]}}]}}"#,
        r"Whenever another creature dies, target opponent loses 1 life and you gain 1 life. If this is the fourth time this ability has resolved this turn, transform ~.",
    ),
    // sephiroth_fabled_soldier_sephiroth_one_winged_angel
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"SacrificeUpTo":{"filter":[{"Type":"Creature"},"Other"],"count":{"Count":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"as":"sacrificed"}},{"Draw":{"count":{"Count":[{"Is":{"Bound":"sacrificed"}}]}}}]}}"#,
        r"Whenever ~ attacks, you may sacrifice any number of other creatures. If you do, draw that many cards.",
    ),
    // curse_of_leeches_leeching_lurker
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"AttachedPlayer"}},"effects":[{"LoseLife":{"who":"AttachedPlayer","amount":1}},{"GainLife":{"amount":1}}]}}"#,
        r"At the beginning of enchanted player's upkeep, they lose 1 life and you gain 1 life.",
    ),
    // airtight_alibi
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Untap":{"what":"Attached"}},{"Continuous":{"affected":"Attached","mods":[{"AddKeyword":"Hexproof"}],"duration":"UntilEndOfTurn"}},{"Unsuspect":{"what":"Attached"}}]}}"#,
        r"When this Aura enters, untap enchanted creature. It gains hexproof until end of turn. If it's suspected, it's no longer suspected.",
    ),
    // charitable_levy
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Not":{"Type":"Creature"}}]}},"effects":[{"PutCounters":{"on":"This","counter":{"Other":"collection"}}},{"If":{"condition":{"Matches":{"subject":"This","selector":[{"CounterCount":{"kind":{"Other":"collection"},"op":"Ge","to":3}}]}},"then":[{"Sacrifice":{"filter":["This"],"as":"sacrificed"}},{"If":{"condition":{"Compare":{"left":{"Count":[{"Is":{"Bound":"sacrificed"}}]},"op":"Ge","right":1}},"then":[{"Draw":{}},{"May":{"effects":[{"Search":{"filter":[{"Subtype":"Plains"}],"as":"found"}},{"Move":{"what":{"Bound":"found"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}}]}}]}}]}}"#,
        r"Whenever a player casts a noncreature spell, put a collection counter on this enchantment. Then if there are three or more collection counters on it, sacrifice it. If you do, draw a card, then you may search your library for a Plains card, put it onto the battlefield tapped, then shuffle.",
    ),
    // altar_of_the_pantheon
    (
        r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}},{"If":{"condition":{"Any":[{"Compare":{"left":{"Count":[{"Subtype":"God"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},{"Compare":{"left":{"Count":[{"Subtype":"Demigod"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},{"Compare":{"left":{"Count":[{"Supertype":"Legendary"},{"Type":"Enchantment"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}]},"then":[{"GainLife":{"amount":1}}]}}]}}"#,
        r"{T}: Add one mana of any color. If you control a God, a Demigod, or a legendary enchantment, you gain 1 life.",
    ),
    // ashiok_wicked_manipulator
    (
        r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"LookAtTop":{"count":2,"as":"top"}},{"Choose":{"from":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}}]},"as":"exiled"}},{"Exile":{"what":{"Bound":"exiled"}}},{"Move":{"what":{"Each":[{"In":"Library"},{"Is":{"Bound":"top"}}]},"to":"Hand"}}]}}"#,
        r"+1: Look at the top two cards of your library. Exile one of them and put the other into your hand.",
    ),
    // ashiok_wicked_manipulator
    (
        r#"{"Loyalty":{"cost":{"Minus":7},"targets":[{"what":"Player"}],"effects":[{"ExileTop":{"who":{"Target":0},"count":{"Total":{"of":[{"In":"Exile"},{"OwnedBy":"You"}],"stat":"ManaValue"}},"as":"exiled"}}]}}"#,
        r"−7: Target player exiles the top X cards of their library, where X is the total mana value of cards you own in exile.",
    ),
    // evidence_examiner
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"effects":[{"Pay":{"cost":[{"CollectEvidence":4}]}}]}}"#,
        r"At the beginning of combat on your turn, you may collect evidence 4.",
    ),
    // fear_of_sleep_paralysis
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"to":"Battlefield"}},{"ZoneChange":{"what":[{"Type":"Enchantment"},"Other",{"ControlledBy":"You"}],"to":"Battlefield"}},{"DoorUnlocked":{"who":"You","fully":true}}]},"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Tap":{"what":{"Target":0}}},{"PutCounters":{"on":{"Target":0},"counter":{"Other":"stun"}}}]}}"#,
        r"Eerie — Whenever this creature or another enchantment you control enters and whenever you fully unlock a Room, tap up to one target creature and put a stun counter on it.",
    ),
    // needle_drop
    (
        r#"{"Spell":{"targets":[{"what":{"Either":[{"PlayerAmong":{"PlayersWhere":{"who":"EachPlayer","test":{"Compare":{"left":{"Tallied":{"who":"You","what":"DamageTaken"}},"op":"Ge","right":1}}}}},{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"},{"Type":"Battle"}]},"DamagedThisTurn"]}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}},{"Draw":{}}]}}"#,
        r"~ deals 1 damage to any target that was dealt damage this turn.
Draw a card.",
    ),
    // neyith_of_the_dire_hunt
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Pay":{"cost":"{2}{R/G}","paid":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":{"Stat":{"of":{"Target":0},"stat":"Power"}},"toughness":0}}],"duration":"UntilEndOfTurn"}},{"Rule":{"rule":{"Combat":{"who":{"Target":0},"quirk":{"MustBeBlocked":"AtLeastOne"}}},"duration":"UntilEndOfCombat"}}]}}]}}"#,
        r"At the beginning of combat on your turn, you may pay {2}{R/G}. If you do, double target creature's power until end of turn. That creature must be blocked this combat if able.",
    ),
    // oko_lorwyn_liege_oko_shadowmoor_scion
    (
        r#"{"Loyalty":{"cost":{"Minus":1},"effects":[{"Mill":{"count":3,"as":"milled"}},{"Choose":{"from":{"Each":[{"Is":{"Bound":"milled"}},{"In":"Graveyard"},{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"},{"Type":"Battle"}]}]},"count":{"min":0,"max":1},"as":"kept"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}}]}}"#,
        r"−1: Mill three cards. You may put a permanent card from among them into your hand.",
    ),
    // soul_transfer
    (
        r#"{"Spell":{"effects":[{"Modal":{"all_if":{"All":[{"Compare":{"left":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},{"Compare":{"left":{"Count":[{"Type":"Enchantment"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}]},"modes":[{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]}]}}],"effects":[{"Exile":{"what":{"Target":0}}}]},{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}]}}]}}"#,
        r"Choose one. If you control an artifact and an enchantment as you cast this spell, you may choose both instead.
• Exile target creature or planeswalker.
• Return target creature or planeswalker card from your graveyard to your hand.",
    ),
    // tapestry_warden
    (
        r#"{"Rule":{"rule":{"Combat":{"who":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"Test":{"left":{"Stat":{"of":"Candidate","stat":"Toughness"}},"op":"Gt","right":{"Stat":{"of":"Candidate","stat":"Power"}}}}]},"quirk":"AssignsByToughness"}}}}"#,
        r"Each creature you control with toughness greater than its power assigns combat damage equal to its toughness rather than its power.",
    ),
    // tergrid_god_of_fright_tergrids_lantern
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"Sacrificed":{"what":[{"Not":"Token"}],"by":"Opponents"}},{"Discarded":{"what":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"},{"Type":"Battle"}]}],"by":"Opponents"}}]},"effects":[{"May":{"effects":[{"Move":{"what":{"Arrived":{"of":"Triggering","zone":"Graveyard"}},"to":{"Battlefield":{"under":"You"}}}}]}}]}}"#,
        r"Whenever an opponent sacrifices a nontoken permanent or discards a permanent card, you may put that card from a graveyard onto the battlefield under your control.",
    ),
    // thought_lash
    (
        r#"{"Keyword":{"CumulativeUpkeep":[{"ExileTop":1}]}}"#,
        r"Cumulative upkeep—Exile the top card of your library.",
    ),
    // thought_lash
    (
        r#"{"Activated":{"cost":[{"ExileTop":1}],"effects":[{"Shield":{"when":{"Damage":{"to":"You"}},"by":{"PreventUpTo":1},"duration":"UntilEndOfTurn"}}]}}"#,
        r"Exile the top card of your library: Prevent the next 1 damage that would be dealt to you this turn.",
    ),
    // tom_bombadil
    (
        r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Hexproof"},{"AddKeyword":"Indestructible"}],"condition":{"Compare":{"left":{"Counters":{"on":{"Each":[{"Subtype":"Saga"},{"ControlledBy":"You"}]},"counter":{"Other":"lore"}}},"op":"Ge","right":4}}}}"#,
        r"As long as there are four or more lore counters among Sagas you control, ~ has hexproof and indestructible.",
    ),
    // weeping_angel
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Type":"Creature"}],"by":"Opponents"}},"effects":[{"Continuous":{"affected":"This","mods":[{"RemoveType":"Creature"}],"duration":"UntilEndOfTurn"}}]}}"#,
        r"Whenever an opponent casts a creature spell, this creature isn't a creature until end of turn.",
    ),
    // weeping_angel
    (
        r#"{"Replacement":{"when":{"Damage":{"to":{"Each":[{"Type":"Creature"}]},"source":["This"],"combat":true}},"by":{"PreventAnd":[{"Move":{"what":"Triggering","to":{"Library":{"place":"Shuffled"}}}}]}}}"#,
        r"If this creature would deal combat damage to a creature, prevent that damage and that creature's owner shuffles it into their library.",
    ),
    // wicked_akuba
    (
        r#"{"Activated":{"cost":"{B}","targets":[{"what":{"PlayerAmong":{"PlayersWhere":{"who":"EachPlayer","test":{"Matches":{"subject":"This","selector":[{"DidThisTurn":{"event":"Damaged","to":"You"}}]}}}}}}],"effects":[{"LoseLife":{"who":{"Target":0},"amount":1}}]}}"#,
        r"{B}: Target player dealt damage by this creature this turn loses 1 life.",
    ),
    // rewrite_history
    (
        r#"{"Triggered":{"when":{"Batch":{"Tapped":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}},"effects":[{"Draw":{}},{"Discard":{}},{"PutCounters":{"on":"This","counter":{"Other":"plan"}}}]}}"#,
        r"Whenever one or more creatures you control become tapped, draw a card, then discard a card and put a plan counter on this enchantment.",
    ),
];

/// The entries parsed once.
static RMT5_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    RMT5_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn rmt5_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in RMT5_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the rmt5 table parses as an ability, so none is silently dropped.
    use super::{RMT5_SHAPES, RMT5_WHOLES};

    #[test]
    fn rmt5_every_whole_parses() {
        assert_eq!(RMT5_SHAPES.len(), RMT5_WHOLES.len());
    }
}
