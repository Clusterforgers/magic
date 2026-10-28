//! Sentences of the sixth sample of the names S to Z: the targets and effects of an ability are matched whole as data
//! and worded as one sentence after the head the renderers already write ("When ~ enters, if it was kicked, return all
//! creatures to their owners' hands except for Merfolk, Krakens, Leviathans, Octopuses, and Serpents.").

use super::standin::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::TargetSpec;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// The targets and effects of an ability, the part a sentence words.
#[derive(Serialize, Deserialize)]
struct C6SzBody {
    /// The target specifications.
    #[serde(default)]
    targets: Vec<TargetSpec>,
    /// The effects.
    effects: Vec<Effect>,
}

/// The bodies, as JSON, each with its sentence in lower case and without the period at its end.
const C6_SZ_BODIES: [(&str, &str); 22] = [
    (
        r#"{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Battle"},{"Type":"Creature"},{"Type":"Enchantment"},{"Type":"Land"},{"Type":"Planeswalker"}]},{"In":"Graveyard"},{"OwnedBy":"You"},{"Compare":{"stat":"ManaValue","op":"Le","to":3}}]}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}}}},{"If":{"condition":{"Matches":{"subject":"This","selector":[{"CastFrom":"Graveyard"}]}},"then":[{"May":{"effects":[{"CopySpell":{"what":"This","new_targets":true}}]}}]}}]}"#,
        "return target permanent card with mana value 3 or less from your graveyard to the battlefield. If this spell was cast from a graveyard, you may copy this spell and may choose a new target for the copy",
    ),
    (
        r#"{"effects":[{"RevealUntil":{"filter":[{"Subtype":"Aura"}],"as":"aura","revealed_as":"rest"}},{"May":{"effects":[{"Move":{"what":{"Bound":"aura"},"to":{"Battlefield":{}}}}]}},{"If":{"condition":{"Not":"Performed"},"then":[{"Move":{"what":{"Bound":"aura"},"to":"Hand"}}]}},{"Move":{"what":{"Bound":"rest"},"to":{"Library":{"place":"Bottom","order":"Random"}}}}]}"#,
        "reveal cards from the top of your library until you reveal an Aura card. You may put that card onto the battlefield. If you don't, put it into your hand. Put the rest on the bottom of your library in a random order",
    ),
    (
        r#"{"effects":[{"Move":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"},{"Compare":{"stat":"Toughness","op":"Le","to":{"Count":[{"Subtype":"Island"},{"ControlledBy":"You"}]}}}]},"to":"Hand"}}]}"#,
        "return each creature your opponents control with toughness X or less to its owner's hand, where X is the number of Islands you control",
    ),
    (
        r#"{"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Type":"Land"}],"count":{"min":0,"max":"X"},"as":"lands"}},{"Move":{"what":{"Bound":"lands"},"to":{"Battlefield":{"tapped":true}}}},{"Shuffle":{}}]}"#,
        "search your library for up to X basic land cards, put them onto the battlefield tapped, then shuffle",
    ),
    (
        r#"{"effects":[{"Search":{"filter":[{"Supertype":"Basic"},{"Subtype":"Plains"}],"reveal":true,"as":"land"}},{"If":{"condition":{"Compare":{"left":{"Largest":{"who":"Opponents","of":[{"Type":"Land"}]}},"op":"Gt","right":{"Count":[{"Type":"Land"},{"ControlledBy":"You"}]}}},"then":[{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}}],"otherwise":[{"Move":{"what":{"Bound":"land"},"to":"Hand"}}]}},{"Shuffle":{}}]}"#,
        "search your library for a basic Plains card and reveal it. If an opponent controls more lands than you, put it onto the battlefield tapped. Otherwise, put it into your hand. Then shuffle",
    ),
    (
        r#"{"effects":[{"GainLife":{"amount":2}},{"If":{"condition":{"Compare":{"left":{"Life":"You"},"op":"Gt","right":{"LowestLife":"Opponents"}}},"then":[{"Draw":{}}]}}]}"#,
        "you gain 2 life. Then if you have more life than an opponent, draw a card",
    ),
    (
        r#"{"targets":[{"what":{"Object":[{"Type":"Artifact"},{"Not":{"Type":"Creature"}}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddType":"Creature"},{"SetPT":{"power":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}},"toughness":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}}}}],"duration":"UntilEndOfTurn"}}]}"#,
        "target noncreature artifact becomes an artifact creature with power and toughness each equal to its mana value until end of turn",
    ),
    (
        r#"{"effects":[{"Mill":{"count":3}},{"May":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Land"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"as":"land"}},{"Move":{"what":{"Bound":"land"},"to":{"Battlefield":{"tapped":true}}}}]}}]}"#,
        "mill three cards. Then you may return a land card from your graveyard to the battlefield tapped",
    ),
    (
        r#"{"effects":[{"AddMana":{"mana":{"Fixed":["Red","Red","Red","Red","Red","Red"]}}}]}"#,
        "add six {R}",
    ),
    (
        r#"{"effects":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"},{"HasKeyword":"Flying"},{"ControlledBy":"Opponents"}]},"amount":1}},{"Tap":{"what":{"Each":[{"Type":"Creature"},{"HasKeyword":"Flying"},{"ControlledBy":"Opponents"}]}}}]}"#,
        "it deals 1 damage to each creature with flying your opponents control. Tap those creatures",
    ),
    (
        r#"{"effects":[{"Modal":{"choose":{"min":2,"max":2},"modes":[{"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]},{"targets":[{"what":"Player"}],"effects":[{"GainLife":{"who":{"Target":0},"amount":5}}]},{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Rhino Warrior","colors":["Green"],"types":["Creature"],"subtypes":["Rhino","Warrior"],"power":4,"toughness":4}}}}]},{"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"as":"creature"}},{"PutCounters":{"on":{"Bound":"creature"},"counter":{"Other":"shield"}}}]}]}}]}"#,
        "choose two \u{2014}\n\u{2022} Destroy target artifact or enchantment.\n\u{2022} Target player gains 5 life.\n\u{2022} Create a 4/4 green Rhino Warrior creature token.\n\u{2022} Put a shield counter on a creature you control",
    ),
    (
        r#"{"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"OwnedBy":{"Bound":"player"}}]},"mods":[{"SetController":{"Bound":"player"}}],"duration":"Forever"}}]}}]}"#,
        "each player gains control of all creatures they own",
    ),
    (
        r#"{"effects":[{"Sacrifice":{"who":"Opponents","filter":[{"Type":"Enchantment"},{"Not":"Token"}]}},{"Sacrifice":{"who":"Opponents","filter":[{"Type":"Creature"},{"Not":"Token"}]}}]}"#,
        "each opponent sacrifices a nontoken enchantment of their choice, then sacrifices a nontoken creature of their choice",
    ),
    (
        r#"{"targets":[{"what":{"Object":[{"Type":"Artifact"},{"ControlledBy":"Triggering"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"PutCounters":{"on":"This","counter":"+1/+1"}}]}"#,
        "destroy up to one target artifact that player controls and put a +1/+1 counter on ~",
    ),
    (
        r#"{"effects":[{"Continuous":{"affected":"Attached","mods":[{"AddKeyword":"Deathtouch"}],"duration":"UntilEndOfTurn"}},{"Continuous":{"affected":{"Each":[{"Subtype":"Zombie"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Deathtouch"}],"duration":"UntilEndOfTurn"}}]}"#,
        "it and Zombies you control gain deathtouch until end of turn",
    ),
    (
        r#"{"effects":[{"Sacrifice":{"who":"Triggering","filter":[{"Type":"Creature"}]}},{"If":{"condition":{"Not":"Performed"},"then":[{"CreateToken":{"token":{"Custom":{"name":"Cat Dragon","colors":["Black","Red","Green"],"types":["Creature"],"subtypes":["Cat","Dragon"],"power":3,"toughness":3,"abilities":[{"Keyword":"Flying"}]}}}}]}}]}"#,
        "that player sacrifices a creature of their choice. If the player can't, you create a 3/3 black, red, and green Cat Dragon creature token with flying",
    ),
    (
        r#"{"targets":[{"what":{"Object":[{"Type":"Artifact"},{"Not":{"Type":"Creature"}},{"ControlledBy":"You"}]}}],"effects":[{"May":{"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddType":"Creature"},{"SetPT":{"power":0,"toughness":0}}],"duration":"Forever"}}]}},{"If":{"condition":"Performed","then":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":4}}]}}]}"#,
        "you may have target noncreature artifact you control become a 0/0 artifact creature. If you do, put four +1/+1 counters on it",
    ),
    (
        r#"{"effects":[{"Modal":{"modes":[{"effects":[{"CreateToken":{"token":{"Named":"food"}}}]},{"effects":[{"CreateToken":{"token":{"Named":"treasure"}}}]}]}}]}"#,
        "create a Food token or a Treasure token",
    ),
    (
        r#"{"effects":[{"Draw":{}},{"Choose":{"from":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"as":"blighted"}},{"PutCounters":{"on":{"Bound":"blighted"},"counter":"-1/-1"}}]}"#,
        "draw a card and blight 1",
    ),
    (
        r#"{"effects":[{"If":{"condition":{"All":[{"Compare":{"left":{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},{"Compare":{"left":{"Greatest":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"Power"}},"op":"Ge","right":{"Greatest":{"of":[{"Type":"Creature"}],"stat":"Power"}}}}]},"then":[{"Draw":{"count":2}}]}}]}"#,
        "draw two cards if you control the creature with the greatest power or tied for the greatest power",
    ),
    (
        r#"{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Flying"},{"GrantAbility":{"Triggered":{"when":{"DealsDamage":{"source":["This"],"to":"EachPlayer","combat":true}},"effects":[{"Draw":{"count":"Triggering"}}]}}}],"duration":"UntilEndOfTurn"}}]}"#,
        "target creature gains flying and \"Whenever ~ deals combat damage to a player, draw that many cards\" until end of turn",
    ),
    (
        r#"{"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddSubtype":"Mutant"}],"duration":"Forever"}}]}"#,
        "put a +1/+1 counter on target creature you control. It becomes a Mutant in addition to its other types",
    ),
];

/// The sentences with their bodies as the data writes them, read once.
static C6_SZ_SHAPES: LazyLock<Vec<(serde_json::Value, &str)>> = LazyLock::new(|| {
    C6_SZ_BODIES
        .iter()
        .filter_map(|(shape, sentence)| Some((c6_sz_shape(shape)?, *sentence)))
        .collect()
});

/// Adds the wordings of an ability whose targets and effects one of the sentences words.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some(body) = c6_sz_body(ability.targets(), ability.effects()) else {
        return;
    };
    for (shape, sentence) in C6_SZ_SHAPES.iter() {
        if *shape == body {
            texts.extend(with_effects(ability, sentence));
        }
    }
}

/// The JSON of a body.
fn c6_sz_body(targets: &[TargetSpec], effects: &[Effect]) -> Option<serde_json::Value> {
    serde_json::to_value(C6SzBody {
        targets: targets.to_vec(),
        effects: effects.to_vec(),
    })
    .ok()
}

/// The JSON of a body as the data writes it, defaults filled in the way the rules files hold them.
fn c6_sz_shape(shape: &str) -> Option<serde_json::Value> {
    let body: C6SzBody = serde_json::from_str(shape).ok()?;
    c6_sz_body(&body.targets, &body.effects)
}
