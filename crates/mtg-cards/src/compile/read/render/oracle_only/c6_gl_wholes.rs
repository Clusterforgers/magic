//! Whole abilities of the sixth sample of the names G to L whose steps the renderers do not join: the ability is
//! matched whole as data and worded as the card prints it ("If this spell was cast from a graveyard, copy that spell
//! twice instead.").

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording.
const C6_GL_WHOLES: [(&str, &str); 18] = [
    (
        r#"{"Spell":{"effects":[{"ExileTop":{"count":3,"as":"impulse"}},{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"impulse"},"play":true}},"duration":"UntilEndOfTurn"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Move":{"what":{"Each":[{"Is":{"Bound":"impulse"}},{"In":"Exile"}]},"to":"Graveyard"}},{"CreateToken":{"token":{"Custom":{"name":"Eldrazi Spawn","types":["Creature"],"subtypes":["Eldrazi","Spawn"],"power":0,"toughness":1,"abilities":[{"Activated":{"cost":["SacrificeSelf"],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]}}}]}}]}},"count":{"Count":[{"Is":{"Bound":"impulse"}},{"In":"Graveyard"}]}}}]}}]}}"#,
        "Exile the top three cards of your library. You may play those cards this turn. At the beginning of the next end step, if any of those cards remain exiled, put them into your graveyard, then create a 0/1 colorless Eldrazi Spawn creature token for each card put into your graveyard this way. Those tokens have \"Sacrifice this token: Add {C}.\"",
    ),
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},{"DealsDamage":{"source":["This"],"to":{"Each":[{"Type":"Planeswalker"}]},"combat":true}}]},"effects":[{"Proliferate":{}}]}}"#,
        "Whenever this creature deals combat damage to a player or planeswalker, proliferate.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Not":{"Matches":{"subject":"This","selector":["Token"]}}},"effects":[{"CreateToken":{"token":{"CopyOf":{"of":"This"}},"count":2}}]}}"#,
        "When this creature enters, if it isn't a token, create two tokens that are copies of it.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"effects":[{"PutCounters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"},{"Named":"Gruff Triplets"}]},"counter":"+1/+1","count":{"Stat":{"of":"This","stat":"Power"}}}}]}}"#,
        "When this creature dies, put a number of +1/+1 counters equal to its power on each creature you control named ~.",
    ),
    (
        r#"{"Rule":{"rule":{"MayCast":{"what":[{"Subtype":"Desert"},{"Type":"Land"}],"from":"Graveyard","play":true}}}}"#,
        "You may play Desert lands from your graveyard.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"AnyTarget"}],"effects":[{"May":{"effects":[{"Choose":{"from":{"Each":["Other",{"Type":"Creature"},{"ControlledBy":"You"}]},"as":"fodder"}},{"Sacrifice":{"filter":[{"Is":{"Bound":"fodder"}}]}},{"DealDamage":{"to":{"Target":0},"amount":{"Stat":{"of":{"Bound":"fodder"},"stat":"Power"}}}}]}}]}}"#,
        "When this creature enters, you may sacrifice another creature. When you do, this creature deals damage equal to that creature's power to any target.",
    ),
    (
        r#"{"CostModifier":{"applies_to":[{"Type":"Creature"},"ChosenSubtype"],"change":"Reduce","mana":"{1}"}}"#,
        "Creature spells you cast of the chosen type cost {1} less to cast.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"LookAtTop":{"count":1,"as":"top"}},{"Choose":{"from":{"Each":[{"Is":{"Bound":"top"}},{"In":"Library"},{"Type":"Creature"},"ChosenSubtype"]},"count":{"min":0,"max":1},"as":"kept"}},{"Reveal":{"what":{"Bound":"kept"}}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}}]}}"#,
        "At the beginning of your upkeep, look at the top card of your library. If it's a creature card of the chosen type, you may reveal it and put it into your hand.",
    ),
    (
        r#"{"Spell":{"effects":[{"ExileTop":{"count":3,"as":"impulse"}},{"If":{"condition":{"Matches":{"subject":"This","selector":[{"CastFrom":"Graveyard"}]}},"then":[{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"impulse"},"play":true,"free":true}},"duration":"UntilEndOfYourNextTurn"}}],"otherwise":[{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"impulse"},"play":true}},"duration":"UntilEndOfYourNextTurn"}}]}}]}}"#,
        "Exile the top three cards of your library. Until the end of your next turn, you may play those cards. If this spell was cast from a graveyard, you may play cards this way without paying their mana costs.",
    ),
    (
        r#"{"Spell":{"targets":[{"what":{"Object":["Spell",{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"ControlledBy":"You"}]}}],"effects":[{"If":{"condition":{"Matches":{"subject":"This","selector":[{"CastFrom":"Graveyard"}]}},"then":[{"CopySpell":{"what":{"Target":0},"count":2,"new_targets":true}}],"otherwise":[{"CopySpell":{"what":{"Target":0},"new_targets":true}}]}}]}}"#,
        "Copy target instant or sorcery spell you control. If this spell was cast from a graveyard, copy that spell twice instead. You may choose new targets for the copies.",
    ),
    (
        r#"{"Spell":{"effects":[{"Modal":{"all_if":"Kicked","modes":[{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":2}}]},{"targets":[{"what":"Player"}],"effects":[{"GainLife":{"who":{"Target":0},"amount":{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":{"Target":0}}],"stat":"Power"}}}}]},{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}},{"what":{"Object":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}}],"effects":[{"Fight":{"what":{"Target":0},"with":{"Target":1}}}]}]}}]}}"#,
        "Choose one. If this spell was kicked, choose any number instead.\n• Put two +1/+1 counters on target creature.\n• Target player gains X life, where X is the greatest power among creatures they control.\n• Target creature you control fights target creature you don't control.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":{"Object":[{"Subtype":"Dinosaur"},{"ControlledBy":"You"}]}},{"what":{"Object":[{"Type":"Creature"},{"Not":{"Is":{"Target":0}}}]}}],"effects":[{"Pay":{"cost":"{2}","paid":[{"DealDamage":{"source":{"Target":0},"to":{"Target":1},"amount":{"Stat":{"of":{"Target":0},"stat":"Power"}}}}]}}]}}"#,
        "When ~ enters, you may pay {2}. When you do, target Dinosaur you control deals damage equal to its power to another target creature.",
    ),
    (
        r#"{"Activated":{"cost":["Tap",{"Sacrifice":{"filter":["Other",{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]}],"count":3}}],"effects":[{"LookAtTop":{"count":3,"as":"top"}},{"Choose":{"from":{"Bound":"top"},"count":{"min":2,"max":2},"as":"kept","rest_as":"rest"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}},{"Move":{"what":{"Bound":"rest"},"to":"Graveyard"}}]}}"#,
        "{T}, Sacrifice three other artifacts and/or creatures: Look at the top three cards of your library. Put two of them into your hand and the other into your graveyard.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"targets":[{"what":"Opponent"}],"effects":[{"May":{"effects":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}},{"Draw":{"count":{"Tallied":{"who":{"Target":0},"what":"DamageTaken"}}}}]}}]}}"#,
        "When this creature enters, you may discard your hand and draw cards equal to the damage dealt to target opponent this turn.",
    ),
    (
        r#"{"Activated":{"cost":"{X}{G}","effects":[{"Continuous":{"affected":"This","mods":[{"AddType":"Creature"},{"SetPT":{"power":"X","toughness":"X"}},{"SetColors":["Green"]},{"AddSubtype":"Hydra"}],"duration":"UntilEndOfTurn"}}]}}"#,
        "{X}{G}: Until end of turn, this land becomes an X/X green Hydra creature. It's still a land. X can't be 0.",
    ),
    (
        r#"{"Triggered":{"when":{"StepBegins":{"step":"End"}},"if":{"Compare":{"left":{"Tallied":{"who":"You","what":"LifeGainedTotal"}},"op":"Ge","right":1}},"targets":[{"what":{"Object":["Other",{"Type":"Creature"}]},"bounds":{"min":0,"max":{"Tallied":{"who":"You","what":"LifeGainedTotal"}}},"divide":{"Tallied":{"who":"You","what":"LifeGainedTotal"}}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":"Divided"}}]}}"#,
        "At the beginning of each end step, if you gained life this turn, distribute up to that many +1/+1 counters among any number of other target creatures.",
    ),
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Not":{"Matches":{"subject":"This","selector":["Token"]}}},"effects":[{"CreateToken":{"who":"Opponents","token":{"CopyOf":{"of":"This"}},"as":"copies"}},{"Continuous":{"affected":{"Bound":"copies"},"mods":["Goaded"],"duration":"Forever"}}]}}"#,
        "When this creature enters, if it's not a token, each opponent creates a token that's a copy of it. The tokens are goaded for the rest of the game.",
    ),
    (
        r#"{"Loyalty":{"cost":{"Plus":2},"effects":[{"Discard":{"as":"discarded"}},{"Draw":{}},{"If":{"condition":{"Matches":{"subject":{"Bound":"discarded"},"selector":[{"Type":"Land"}]}},"then":[{"Draw":{}}]}}]}}"#,
        "+2: Discard a card, then draw a card. If a land card is discarded this way, draw an additional card.",
    ),
];

/// The abilities of the table read once, each with its wording.
static C6_GL_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    C6_GL_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn c6_gl_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in C6_GL_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}
