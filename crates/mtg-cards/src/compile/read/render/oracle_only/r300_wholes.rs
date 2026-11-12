//! Whole abilities of the r30-0 card pack that the renderers do not write: each is matched whole as data and worded as
//! the card prints it, so another count, target, cost, condition or duration in the same ability never reads back.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (the card's name as `~`).
const R300_WHOLES: [(&str, &str); 20] = [
    // ashen_ghoul
    (
        r#"{"Activated":{"cost":"{B}","from":"Graveyard","condition":{"All":["YourTurn",{"Step":"Upkeep"},{"Compare":{"left":{"CardsAbove":{"filter":[{"Type":"Creature"}],"directly":false}},"op":"Ge","right":3}}]},"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}"#,
        r"{B}: Return this card from your graveyard to the battlefield. Activate only during your upkeep and only if three or more creature cards are above this card.",
    ),
    // chromatic_orrery
    (
        r#"{"Activated":{"cost":[{"Mana":"{5}"},"Tap"],"effects":[{"Draw":{"count":{"Distinct":{"of":[{"ControlledBy":"You"}],"stat":"Colors"}}}}]}}"#,
        r"{5}, {T}: Draw a card for each color among permanents you control.",
    ),
    // clockwork_avian
    (
        r#"{"Activated":{"cost":[{"Mana":"{X}"},"Tap"],"condition":{"All":["YourTurn",{"Step":"Upkeep"}]},"effects":[{"ChooseValue":{"kind":{"Number":[0,"X"]},"as":"n"}},{"If":{"condition":{"Compare":{"left":{"Sum":[{"ChosenAs":"n"},{"Counters":{"on":"This","counter":{"Other":"+1/+0"}}}]},"op":"Le","right":4}},"then":[{"PutCounters":{"on":"This","counter":{"Other":"+1/+0"},"count":{"ChosenAs":"n"}}}],"otherwise":[{"If":{"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"+1/+0"}}},"op":"Lt","right":4}},"then":[{"PutCounters":{"on":"This","counter":{"Other":"+1/+0"},"count":{"Difference":[4,{"Counters":{"on":"This","counter":{"Other":"+1/+0"}}}]}}}]}}]}}]}}"#,
        r"{X}, {T}: Put up to X +1/+0 counters on this creature. This ability can't cause the total number of +1/+0 counters on this creature to be greater than four. Activate only during your upkeep.",
    ),
    // clockwork_beast
    (
        r#"{"Activated":{"cost":[{"Mana":"{X}"},"Tap"],"condition":{"All":["YourTurn",{"Step":"Upkeep"}]},"effects":[{"ChooseValue":{"kind":{"Number":[0,"X"]},"as":"n"}},{"If":{"condition":{"Compare":{"left":{"Sum":[{"ChosenAs":"n"},{"Counters":{"on":"This","counter":{"Other":"+1/+0"}}}]},"op":"Le","right":7}},"then":[{"PutCounters":{"on":"This","counter":{"Other":"+1/+0"},"count":{"ChosenAs":"n"}}}],"otherwise":[{"If":{"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"+1/+0"}}},"op":"Lt","right":7}},"then":[{"PutCounters":{"on":"This","counter":{"Other":"+1/+0"},"count":{"Difference":[7,{"Counters":{"on":"This","counter":{"Other":"+1/+0"}}}]}}}]}}]}}]}}"#,
        r"{X}, {T}: Put up to X +1/+0 counters on this creature. This ability can't cause the total number of +1/+0 counters on this creature to be greater than seven. Activate only during your upkeep.",
    ),
    // clockwork_steed
    (
        r#"{"Activated":{"cost":[{"Mana":"{X}"},"Tap"],"condition":{"All":["YourTurn",{"Step":"Upkeep"}]},"effects":[{"ChooseValue":{"kind":{"Number":[0,"X"]},"as":"n"}},{"If":{"condition":{"Compare":{"left":{"Sum":[{"ChosenAs":"n"},{"Counters":{"on":"This","counter":{"Other":"+1/+0"}}}]},"op":"Le","right":4}},"then":[{"PutCounters":{"on":"This","counter":{"Other":"+1/+0"},"count":{"ChosenAs":"n"}}}],"otherwise":[{"If":{"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"+1/+0"}}},"op":"Lt","right":4}},"then":[{"PutCounters":{"on":"This","counter":{"Other":"+1/+0"},"count":{"Difference":[4,{"Counters":{"on":"This","counter":{"Other":"+1/+0"}}}]}}}]}}]}}]}}"#,
        r"{X}, {T}: Put up to X +1/+0 counters on this creature. This ability can't cause the total number of +1/+0 counters on this creature to be greater than four. Activate only during your upkeep.",
    ),
    // clockwork_swarm
    (
        r#"{"Activated":{"cost":[{"Mana":"{X}"},"Tap"],"condition":{"All":["YourTurn",{"Step":"Upkeep"}]},"effects":[{"ChooseValue":{"kind":{"Number":[0,"X"]},"as":"n"}},{"If":{"condition":{"Compare":{"left":{"Sum":[{"ChosenAs":"n"},{"Counters":{"on":"This","counter":{"Other":"+1/+0"}}}]},"op":"Le","right":4}},"then":[{"PutCounters":{"on":"This","counter":{"Other":"+1/+0"},"count":{"ChosenAs":"n"}}}],"otherwise":[{"If":{"condition":{"Compare":{"left":{"Counters":{"on":"This","counter":{"Other":"+1/+0"}}},"op":"Lt","right":4}},"then":[{"PutCounters":{"on":"This","counter":{"Other":"+1/+0"},"count":{"Difference":[4,{"Counters":{"on":"This","counter":{"Other":"+1/+0"}}}]}}}]}}]}}]}}"#,
        r"{X}, {T}: Put up to X +1/+0 counters on this creature. This ability can't cause the total number of +1/+0 counters on this creature to be greater than four. Activate only during your upkeep.",
    ),
    // elvish_spirit_guide
    (
        r#"{"Activated":{"cost":["ExileSelf"],"from":"Hand","effects":[{"AddMana":{"mana":{"Fixed":["Green"]}}}]}}"#,
        r"Exile this card from your hand: Add {G}.",
    ),
    // genju_of_the_realm
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Is":"Attached"}],"to":"Graveyard"}},"effects":[{"May":{"effects":[{"Move":{"what":{"Arrived":{"of":"This","zone":"Graveyard"}},"to":"Hand"}}]}}]}}"#,
        r"When enchanted land is put into a graveyard, you may return ~ from your graveyard to your hand.",
    ),
    // godsire
    (
        r#"{"Activated":{"cost":["Tap"],"effects":[{"CreateToken":{"token":{"Custom":{"name":"Beast","colors":["Red","Green","White"],"types":["Creature"],"subtypes":["Beast"],"power":8,"toughness":8}}}}]}}"#,
        r"{T}: Create an 8/8 Beast creature token that's red, green, and white.",
    ),
    // natures_blessing
    (
        r#"{"Activated":{"cost":[{"Mana":"{G}{W}"},{"Discard":{}}],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"OneOf":{"options":[[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}}],[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Banding"}],"duration":"Forever"}}],[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"FirstStrike"}],"duration":"Forever"}}],[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Trample"}],"duration":"Forever"}}]]}}]}}"#,
        r"{G}{W}, Discard a card: Put a +1/+1 counter on target creature or that creature gains banding, first strike, or trample.",
    ),
    // pendant_of_prosperity
    (
        r#"{"Activated":{"cost":[{"Mana":"{2}"},"Tap"],"effects":[{"Draw":{}},{"May":{"effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},{"Type":"Land"}]},"as":"a"}},{"Move":{"what":{"Bound":"a"},"to":{"Battlefield":{}}}}]}},{"Draw":{"who":{"OwnerOf":"This"}}},{"May":{"who":{"OwnerOf":"This"},"effects":[{"Choose":{"who":{"OwnerOf":"This"},"from":{"Each":[{"In":"Hand"},{"OwnedBy":{"OwnerOf":"This"}},{"Type":"Land"}]},"as":"b"}},{"Move":{"what":{"Bound":"b"},"to":{"Battlefield":{}}}}]}}]}}"#,
        r"{2}, {T}: Draw a card, then you may put a land card from your hand onto the battlefield. This artifact's owner draws a card, then that player may put a land card from their hand onto the battlefield.",
    ),
    // pyromancy
    (
        r#"{"Activated":{"cost":[{"Mana":"{3}"},{"DiscardRandom":1}],"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Stat":{"of":"Paid","stat":"ManaValue"}}}}]}}"#,
        r"{3}, Discard a card at random: This enchantment deals damage to any target equal to the mana value of the discarded card.",
    ),
    // rhys_the_evermore
    (
        r#"{"Activated":{"cost":[{"Mana":"{W}"},"Tap"],"timing":"Sorcery","targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"RemoveCounters":{"from":{"Target":0},"count":{"AnyCounters":{"Target":0}},"up_to":true}}]}}"#,
        r"{W}, {T}: Remove any number of counters from target creature you control. Activate only as a sorcery.",
    ),
    // scrying_sheets
    (
        r#"{"Activated":{"cost":[{"Mana":"{1}{S}"},"Tap"],"effects":[{"LookAtTop":{"as":"top"}},{"If":{"condition":{"Matches":{"subject":{"Bound":"top"},"selector":[{"Supertype":"Snow"}]}},"then":[{"May":{"effects":[{"Reveal":{"what":{"Bound":"top"}}},{"Move":{"what":{"Bound":"top"},"to":"Hand"}}]}}]}}]}}"#,
        r"{1}{S}, {T}: Look at the top card of your library. If that card is snow, you may reveal it and put it into your hand.",
    ),
    // simian_spirit_guide
    (
        r#"{"Activated":{"cost":["ExileSelf"],"from":"Hand","effects":[{"AddMana":{"mana":{"Fixed":["Red"]}}}]}}"#,
        r"Exile this card from your hand: Add {R}.",
    ),
    // stormscale_anarch
    (
        r#"{"Activated":{"cost":[{"Mana":"{2}{R}"},{"DiscardRandom":1}],"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"When":{"condition":{"Matches":{"subject":"Paid","selector":[{"Compare":{"stat":"Colors","op":"Ge","to":2}}]}},"then":4,"otherwise":2}}}}]}}"#,
        r"{2}{R}, Discard a card at random: This creature deals 2 damage to any target. If the discarded card was multicolored, this creature deals 4 damage instead.",
    ),
    // the_jolly_balloon_man
    (
        r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"timing":"Sorcery","targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"},"Other"]}}],"effects":[{"CreateToken":{"token":{"CopyOf":{"of":{"Target":0},"except":[{"SetPT":{"power":1,"toughness":1}},{"AddColors":["Red"]},{"AddType":"Creature"},{"AddSubtype":"Balloon"},{"AddKeyword":"Flying"},{"AddKeyword":"Haste"}]}},"as":"copy"}},{"DelayedTrigger":{"when":{"StepBegins":{"step":"End"}},"effects":[{"Sacrifice":{"filter":[{"Is":{"Bound":"copy"}}]}}]}}]}}"#,
        r"{1}, {T}: Create a token that's a copy of another target creature you control, except it's a 1/1 red Balloon creature in addition to its other colors and types and it has flying and haste. Sacrifice it at the beginning of the next end step. Activate only as a sorcery.",
    ),
    // thrull_wizard
    (
        r#"{"Activated":{"cost":"{1}{B}","targets":[{"what":{"Object":["Spell",{"Color":"Black"}]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":[{"OneOf":["{B}","{3}"]}],"unpaid":[{"Counter":{"what":{"Target":0}}}]}}]}}"#,
        r"{1}{B}: Counter target black spell unless that spell's controller pays {B} or {3}.",
    ),
    // urzas_avenger
    (
        r#"{"Activated":{"cost":"{0}","effects":[{"OneOf":{"options":[[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":-1,"toughness":-1}},{"AddKeyword":"Banding"}],"duration":"UntilEndOfTurn"}}],[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":-1,"toughness":-1}},{"AddKeyword":"Flying"}],"duration":"UntilEndOfTurn"}}],[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":-1,"toughness":-1}},{"AddKeyword":"FirstStrike"}],"duration":"UntilEndOfTurn"}}],[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":-1,"toughness":-1}},{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}}]]}}]}}"#,
        r"{0}: This creature gets -1/-1 and gains your choice of banding, flying, first strike, or trample until end of turn.",
    ),
    // wall_of_vipers
    (
        r#"{"Activated":{"cost":"{3}","by":"Anyone","targets":[{"what":{"Object":[{"Type":"Creature"},{"BlockedBy":"This"}]}}],"effects":[{"Together":{"effects":[{"Destroy":{"what":"This"}},{"Destroy":{"what":{"Target":0}}}]}}]}}"#,
        r"{3}: Destroy this creature and target creature it's blocking. Any player may activate this ability.",
    ),
];

/// The entries parsed once.
static R300_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    R300_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r300_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R300_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r300 table parses as an ability, so none is silently dropped.
    use super::{R300_SHAPES, R300_WHOLES};

    #[test]
    fn r300_every_whole_parses() {
        assert_eq!(R300_SHAPES.len(), R300_WHOLES.len());
    }
}
