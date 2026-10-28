//! Whole abilities of the stored proposals of the names M to Z whose steps the renderers do not join: the ability is
//! matched whole as data and worded as the card prints it ("When you cycle this card, counter target activated or
//! triggered ability you don't control.").

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording.
const C11G_WHOLES: [(&str, &str); 39] = [
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Not":{"CastFrom":"Hand"}}],"by":"You"}},"targets":[{"what":"Player"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":2}},{"Discard":{"who":{"Target":0}}},{"Draw":{"who":{"Target":0}}},{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#,
        "Paradox — Whenever you cast a spell from anywhere other than your hand, this creature deals 2 damage to target player. That player discards a card, then draws a card. Put a +1/+1 counter on this creature.",
    ),
    (
        r#"{"Triggered":{"when":{"Batch":{"ZoneChange":{"what":[{"Type":"Artifact"},{"ControlledBy":"You"}],"from":"Battlefield"}}},"if":"YourTurn","limit":"OncePerTurn","effects":[{"CreateToken":{"token":{"Custom":{"name":"Construct","types":["Artifact","Creature"],"subtypes":["Construct"],"power":1,"toughness":1}}}}]}}"#,
        "Whenever one or more artifacts you control leave the battlefield during your turn, create a 1/1 colorless Construct artifact creature token. This ability triggers only once each turn.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Elemental","colors":["Red"],"types":["Creature"],"subtypes":["Elemental"],"power":0,"toughness":1,"abilities":[{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"targets":[{"what":{"Object":[{"Named":"Rekindling Phoenix"},{"In":"Graveyard"},{"OwnedBy":"You"}]}}],"effects":[{"Sacrifice":{"filter":["This"]}},{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}}]}}}}]}}"#,
        "When this creature dies, create a 0/1 red Elemental creature token with \"At the beginning of your upkeep, sacrifice this token and return target card named ~ from your graveyard to the battlefield. It gains haste until end of turn.\"",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]},"bounds":{"min":0,"max":1}}],"effects":[{"Goad":{"what":{"Target":0}}},{"PutCounters":{"on":"This","counter":"+1/+1"}},{"PhaseOut":{"what":"This"}}]}}"#,
        "At the beginning of your end step, goad up to one target creature you don't control and put a +1/+1 counter on this creature. This creature phases out.",
    ),
    (
        r#"{"Triggered":{"when":{"TurnedFaceUp":{"what":["Other",{"ControlledBy":"You"}]}},"if":{"Matches":{"subject":"Triggering","selector":[{"Type":"Creature"}]}},"effects":[{"PutCounters":{"on":"Triggering","counter":"+1/+1","count":2}}]}}"#,
        "Whenever another permanent you control is turned face up, if it's a creature, put two +1/+1 counters on it.",
    ),
    (
        r#"{"Triggered":{"when":{"Attacks":{"what":[{"Is":"Attached"}]}},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"Not":{"Is":"Triggering"}},{"AttackingPlayer":"Opponents"}]},"mods":[{"ModifyPT":{"power":1,"toughness":1}}],"duration":"UntilEndOfTurn"}}]}}"#,
        "Whenever enchanted creature attacks, each other creature that's attacking one of your opponents gets +1/+1 until end of turn.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"OwnedBy":"You"}],"from":"Battlefield","to":"Graveyard"}},"from":"Graveyard","effects":[{"Pay":{"cost":"{B}","paid":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}]}}"#,
        "Whenever another creature is put into your graveyard from the battlefield, you may pay {B}. If you do, return this card from your graveyard to the battlefield.",
    ),
    (
        r#"{"Activated":{"cost":"{1}","targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"},"ChosenSubtype"]}}],"effects":[{"BecomeCopy":{"what":"This","of":{"Target":0},"except":[{"AddType":"Artifact"}],"duration":"UntilEndOfTurn"}}]}}"#,
        "{1}: Until end of turn, this artifact becomes a copy of target creature you control of the chosen type, except it's an artifact in addition to its other types.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Land"},{"ControlledBy":"You"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddType":"Creature"},{"SetCreatureTypes":["Elemental"]},{"SetPT":{"power":{"Stat":{"of":"This","stat":"Power"}},"toughness":{"Stat":{"of":"This","stat":"Power"}}}},{"AddKeyword":"Trample"},{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}}"#,
        "At the beginning of combat on your turn, up to one target land you control becomes an X/X Elemental creature with trample and haste until end of turn, where X is ~'s power. It's still a land.",
    ),
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},{"ZoneChange":{"what":[{"Subtype":"Goblin"},"Other",{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}}]},"effects":[{"ExileTop":{"as":"card"}},{"If":{"condition":{"Matches":{"subject":{"Bound":"card"},"selector":[{"Subtype":"Goblin"},{"Type":"Creature"}]}},"then":[{"Rule":{"rule":{"MayCast":{"what":[{"Is":{"Bound":"card"}}],"from":"Exile"}},"duration":"UntilEndOfYourNextTurn"}}]}}]}}"#,
        "Whenever this creature or another Goblin you control dies, exile the top card of your library. If it's a Goblin creature card, you may cast that card until the end of your next turn.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"BeginCombat","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}},{"DelayedTrigger":{"when":{"AttacksPlayer":{"what":[{"Is":{"Target":0}}],"at":"Monarch"}},"once":false,"effects":[{"Continuous":{"affected":"Triggering","mods":[{"AddKeyword":"DoubleStrike"},{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}}]}}]}}"#,
        "At the beginning of combat on your turn, put a +1/+1 counter on target creature. Whenever that creature attacks the monarch this turn, it gains double strike and trample until end of turn.",
    ),
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}}]},"if":{"Matches":{"subject":"Triggering","selector":[{"HasCounter":"+1/+1"}]}},"effects":[{"PutCounters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"HasCounter":"+1/+1"}]},"counter":"+1/+1"}}]}}"#,
        "Whenever ~ or another creature you control dies, if it had a +1/+1 counter on it, put a +1/+1 counter on each creature you control that has a +1/+1 counter on it.",
    ),
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":["This"]}},"if":{"Compare":{"left":{"Tallied":{"who":"EachPlayer","what":"CreaturesDied"}},"op":"Ge","right":1}},"from":"Stack","effects":[{"May":{"effects":[{"CopySpell":{"what":"This","new_targets":true}}]}}]}}"#,
        "Morbid — When you cast this spell, if a creature died this turn, you may copy ~ and may choose a new target for the copy.",
    ),
    (
        r#"{"Spell":{"effects":[{"Modal":{"modes":[{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":4}}]},"mods":[{"SetController":"You"}],"duration":"UntilEndOfTurn"}},{"Untap":{"what":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":4}}]}}},{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Ge","to":4}}]},"mods":[{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]},{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":3}}]},"mods":[{"SetController":"You"}],"duration":"UntilEndOfTurn"}},{"Untap":{"what":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":3}}]}}},{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Le","to":3}}]},"mods":[{"AddKeyword":"Haste"}],"duration":"UntilEndOfTurn"}}]}]}}]}}"#,
        "Choose one —\n• Gain control of all creatures with power 4 or greater until end of turn. Untap those creatures. They gain haste until end of turn.\n• Gain control of all creatures with power 3 or less until end of turn. Untap those creatures. They gain haste until end of turn.",
    ),
    (
        r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":2,"toughness":2}}],"duration":"UntilEndOfTurn"}},{"DelayedTrigger":{"when":{"Blocks":{"what":[{"Type":"Creature"}]}},"once":false,"effects":[{"PutCounters":{"on":{"ControllerOf":"Triggering"},"counter":"poison"}}]}}]}}"#,
        "Creatures you control get +2/+2 until end of turn. Whenever a creature blocks this turn, its controller gets a poison counter.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player"}],"effects":[{"Exile":{"what":{"Each":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]},"as":"exiled"}},{"CreateToken":{"token":{"Custom":{"name":"Zombie","colors":["Black"],"types":["Creature"],"subtypes":["Zombie"],"power":2,"toughness":2}},"count":{"Count":[{"Is":{"Bound":"exiled"}}]}}}]}}"#,
        "When this enchantment enters, exile all creature cards from target player's graveyard, then create a 2/2 black Zombie creature token for each card exiled this way.",
    ),
    (
        r#"{"Spell":{"effects":[{"Modal":{"modes":[{"effects":[{"Surveil":{"count":2}},{"Draw":{}}]},{"targets":[{"what":"AnyTarget","bounds":{"min":1,"max":2}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":1}}]},{"targets":[{"what":{"Object":[{"Not":{"Type":"Land"}}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}]}}]}}"#,
        "Choose one —\n• Surveil 2, then draw a card.\n• ~ deals 1 damage to each of one or two targets.\n• Return target nonland permanent to its owner's hand.",
    ),
    (
        r#"{"Spell":{"effects":[{"Modal":{"modes":[{"targets":[{"what":"Player"}],"effects":[{"Exile":{"what":{"Each":[{"In":"Graveyard"},{"OwnedBy":{"Target":0}}]}}}]},{"targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]},{"effects":[{"RepeatEach":{"over":{"Each":[{"Type":"Creature"}]},"as":"c","effects":[{"DealDamage":{"source":{"Bound":"c"},"to":{"ControllerOf":{"Bound":"c"}},"amount":1}}]}}]}]}}]}}"#,
        "Choose one —\n• Exile target player's graveyard.\n• Destroy target artifact.\n• Each creature deals 1 damage to its controller.",
    ),
    (
        r#"{"Activated":{"cost":[{"Mana":"{R}{G}{W}"},"Tap"],"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Count":[{"Subtype":"Dog"},{"ControlledBy":"You"}]}}},{"GainLife":{"amount":{"Count":[{"Subtype":"Cat"},{"ControlledBy":"You"}]}}}]}}"#,
        "{R}{G}{W}, {T}: ~ deals damage to any target equal to the number of Dogs you control. You gain life equal to the number of Cats you control.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":1,"max":2}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Haste"},{"CantBeBlockedBy":[{"Not":{"HasKeyword":"Haste"}}]}],"duration":"UntilEndOfTurn"}}]}}"#,
        "One or two target creatures each gain haste until end of turn. They can't be blocked this turn except by creatures with haste.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]},"bounds":{"min":0,"max":null},"divide":4}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":"Divided"}}]}}"#,
        "When this creature enters, distribute four +1/+1 counters among any number of target creatures you control.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"May":{"effects":[{"GainLife":{"amount":{"Stat":{"of":{"Target":0},"stat":"Power"}}}}]}}]}}"#,
        "At the beginning of your end step, you may gain life equal to the power of target creature you control.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Player"}],"effects":[{"May":{"effects":[{"Search":{"filter":[{"Subtype":"Forest"}],"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":{"Battlefield":{"tapped":true,"under":{"Target":0}}}}},{"Shuffle":{}}]}}]}}"#,
        "When this creature enters, you may search your library for a Forest card, put it onto the battlefield tapped under target player's control, then shuffle.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"GainLife":{"amount":{"Count":[{"OwnedBy":"You"},{"ControlledBy":"Opponents"}]}}},{"Draw":{"count":{"Count":[{"OwnedBy":"You"},{"ControlledBy":"Opponents"}]}}}]}}"#,
        "At the beginning of your upkeep, you gain X life and draw X cards, where X is the number of permanents you own that your opponents control.",
    ),
    (
        r#"{"Activated":{"cost":"{U}{R}{W}","targets":[{"what":"Opponent"},{"what":{"Object":[{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":{"Target":1},"mods":[{"SetController":{"Target":0}}],"duration":"Forever"}}]}}"#,
        "{U}{R}{W}: Target opponent gains control of target permanent you control.",
    ),
    (
        r#"{"Triggered":{"when":{"SpellCast":{"by":"EachPlayer"}},"effects":[{"FlipCoin":{"who":{"ControllerOf":"Triggering"},"lose":[{"Counter":{"what":"Triggering"}}]}}]}}"#,
        "Whenever a player casts a spell, that player flips a coin. If they lose the flip, counter that spell.",
    ),
    (
        r#"{"Activated":{"cost":[{"Sacrifice":{"filter":[{"Subtype":"Treasure"}],"count":5}}],"effects":[{"Search":{"filter":[{"AnyOf":[{"Type":"Artifact"},{"Subtype":"Dragon"}]}],"as":"card"}},{"Move":{"what":{"Bound":"card"},"to":{"Battlefield":{}}}},{"Shuffle":{}}]}}"#,
        "Sacrifice five Treasures: Search your library for an artifact or Dragon card, put that card onto the battlefield, then shuffle.",
    ),
    (
        r#"{"Activated":{"cost":[{"Mana":"{3}{U}{U}"},"ExileSelf"],"condition":{"Compare":{"left":{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]},"op":"Ge","right":6}},"effects":[{"Move":{"what":{"Each":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"In":"Graveyard"},{"OwnedBy":"You"}]},"to":"Hand"}}]}}"#,
        "{3}{U}{U}, Exile this enchantment: Return all instant and sorcery cards from your graveyard to your hand. Activate only if you control six or more artifacts.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"DealDamage":{"to":{"ControllerOf":{"Target":0}},"amount":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}]}}"#,
        "~ deals damage to target spell's controller equal to that spell's mana value.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Counter":{"what":{"Target":0}}},{"CastFromZone":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Compare":{"stat":"ManaValue","op":"Le","to":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}]},"free":true}}]}}"#,
        "Counter target spell. You may cast a spell with equal or lesser mana value from your hand without paying its mana cost.",
    ),
    (
        r#"{"Rule":{"rule":{"MayCast":{"what":[{"Subtype":"Zombie"},{"Type":"Creature"}],"free":true}}}}"#,
        "You may pay {0} rather than pay the mana cost for Zombie creature spells you cast.",
    ),
    (
        r#"{"Triggered":{"when":{"Cycled":{"what":["This"],"by":"You"}},"from":"Graveyard","targets":[{"what":{"Object":[{"AnyOf":[{"Ability":"Activated"},{"Ability":"Triggered"}]},{"Not":{"ControlledBy":"You"}}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#,
        "When you cycle this card, counter target activated or triggered ability you don't control.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Type":"Creature"}]}},{"what":"Opponent"}],"effects":[{"Rule":{"rule":{"MustAttackPlayers":{"who":{"Target":0},"players":{"Target":1}}},"duration":"UntilEndOfTurn"}}]}}"#,
        "When this creature enters, target creature attacks target opponent this turn if able.",
    ),
    (
        r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"AnyOf":[{"Compare":{"stat":"Power","op":"Le","to":1}},{"Compare":{"stat":"Toughness","op":"Le","to":1}}]}]},"mods":["CantBeBlocked"]}}"#,
        "Creatures you control with power or toughness 1 or less can't be blocked.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Sacrifice":{"filter":["This"]}},{"If":{"condition":"Performed","then":[{"Search":{"filter":[{"Supertype":"Basic"},{"AnyOf":[{"Subtype":"Plains"},{"Subtype":"Island"},{"Subtype":"Swamp"}]}],"count":{"min":0,"max":1},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}},{"GainLife":{"amount":1}}]}}]}}"#,
        "When this land enters, sacrifice it. When you do, search your library for a basic Plains, Island, or Swamp card, put it onto the battlefield tapped, then shuffle and you gain 1 life.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Sacrifice":{"filter":["This"]}},{"If":{"condition":"Performed","then":[{"Search":{"filter":[{"Supertype":"Basic"},{"AnyOf":[{"Subtype":"Island"},{"Subtype":"Swamp"},{"Subtype":"Mountain"}]}],"count":{"min":0,"max":1},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}},{"GainLife":{"amount":1}}]}}]}}"#,
        "When this land enters, sacrifice it. When you do, search your library for a basic Island, Swamp, or Mountain card, put it onto the battlefield tapped, then shuffle and you gain 1 life.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Fish","colors":["Blue"],"types":["Creature"],"subtypes":["Fish"],"power":3,"toughness":3,"abilities":[{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Whale","colors":["Blue"],"types":["Creature"],"subtypes":["Whale"],"power":6,"toughness":6,"abilities":[{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Kraken","colors":["Blue"],"types":["Creature"],"subtypes":["Kraken"],"power":9,"toughness":9}}}}]}}]}}}}]}}]}}}}]}}"#,
        "When this creature dies, create a 3/3 blue Fish creature token with \"When this token dies, create a 6/6 blue Whale creature token with 'When this token dies, create a 9/9 blue Kraken creature token.'\"",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Land"},{"ControlledBy":"You"}],"to":"Battlefield"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Dragon Egg","colors":["Red"],"types":["Creature"],"subtypes":["Dragon","Egg"],"power":0,"toughness":2,"abilities":[{"Keyword":"Defender"},{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Dragon","colors":["Red"],"types":["Creature"],"subtypes":["Dragon"],"power":2,"toughness":2,"abilities":[{"Keyword":"Flying"},{"Activated":{"cost":"{R}","effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}]}}}}]}}]}}}}]}}"#,
        "Landfall — Whenever a land you control enters, create a 0/2 red Dragon Egg creature token with defender and \"When this token dies, create a 2/2 red Dragon creature token with flying and '{R}: This token gets +1/+0 until end of turn.'\"",
    ),
    (
        r#"{"Spell":{"effects":[{"Modal":{"modes":[{"label":"Disarm","targets":[{"what":{"Object":[{"Type":"Artifact"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"CreateToken":{"token":{"Named":"treasure"},"count":4}}]},{"label":"Detonate","effects":[{"DealDamage":{"source":"This","to":{"Each":[{"Type":"Creature"}]},"amount":8}},{"PutCounters":{"on":"EachPlayer","counter":{"Other":"rad"},"count":4}}]}]}}]}}"#,
        "Choose one —\n• Disarm — Destroy target artifact. Create four Treasure tokens.\n• Detonate — ~ deals 8 damage to each creature. Each player gets four rad counters.",
    ),
];

/// The entries parsed once.
static C11G_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    C11G_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn c11g_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in C11G_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses as an ability, so none is silently dropped.
    use super::{C11G_SHAPES, C11G_WHOLES};

    #[test]
    fn c11g_every_whole_parses() {
        assert_eq!(C11G_SHAPES.len(), C11G_WHOLES.len());
    }
}
