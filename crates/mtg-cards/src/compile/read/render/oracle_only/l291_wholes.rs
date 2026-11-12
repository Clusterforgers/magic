//! Whole abilities of the l291 card pack whose steps the renderers do not join: each is matched whole as data and worded
//! as the card prints it, so another count, target, condition or duration in the same ability never reads back.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (the card's name as `~`).
const L291_WHOLES: [(&str, &str); 8] = [
    // kros_defense_contractor: one trigger for each creature that gets counters (CR 603.2c).
    (
        r#"{"Triggered":{"when":{"Caused":{"inner":{"Batch":{"CountersPut":{"what":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}],"each":true}}},"by":"You"}},"effects":[{"Tap":{"what":"Triggering"}},{"Goad":{"what":"Triggering"}},{"Continuous":{"affected":"Triggering","mods":[{"AddKeyword":"Trample"}],"duration":"UntilYourNextTurn"}}]}}"#,
        r"Whenever you put one or more counters on a creature you don't control, tap that creature and goad it. It gains trample until your next turn.",
    ),
    // lifetime_pass_holder: the number rolled is the amount of the roll to visit.
    (
        r#"{"Triggered":{"when":{"RolledToVisit":{"who":"You"}},"if":{"Compare":{"left":"Triggering","op":"Eq","right":6}},"from":"Graveyard","effects":[{"May":{"effects":[{"Move":{"what":"This","to":{"Battlefield":{}}}}]}}]}}"#,
        r"Whenever you roll to visit your Attractions, if you roll a 6, you may return ~ from your graveyard to the battlefield.",
    ),
    // ravager_wurm
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Modal":{"choose":{"min":0,"max":1},"modes":[{"targets":[{"what":{"Object":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}}],"effects":[{"Fight":{"what":"This","with":{"Target":0}}}]},{"targets":[{"what":{"Object":[{"Type":"Land"},{"HasAbility":"NonManaActivated"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}]}}]}}"#,
        "When ~ enters, choose up to one —\n• ~ fights target creature you don't control.\n• Destroy target land with an activated ability that isn't a mana ability.",
    ),
    // arcbound_tracker: the turn's count of spells includes the one just cast, so two or more is not the first.
    (
        r#"{"Triggered":{"when":{"SpellCast":{"by":"You"}},"if":{"Compare":{"left":{"Tallied":{"who":"You","what":"SpellsCast"}},"op":"Ge","right":2}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#,
        r"Whenever you cast a spell other than your first spell each turn, put a +1/+1 counter on ~.",
    ),
    // lichs_tomb
    (
        r#"{"Triggered":{"when":{"LifeLost":{"who":"You"}},"effects":[{"Sacrifice":{"filter":[],"count":"Triggering"}}]}}"#,
        r"Whenever you lose life, sacrifice a permanent for each 1 life you lost.",
    ),
    // neerdiv_devious_diver: the source of an activation is judged as it was before the activation's costs.
    (
        r#"{"Triggered":{"when":{"AnyOf":[{"SpellCast":{"what":[{"WasCast":{"from":"Graveyard"}},{"OwnedBy":"You"}],"by":"You"}},{"Did":{"action":"Activated","who":"You","what":[{"In":"Graveyard"},{"OwnedBy":"You"}]}}]},"effects":[{"Draw":{}},{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#,
        r"Whenever you cast a spell from your graveyard or activate an ability of a card in your graveyard, draw a card and put a +1/+1 counter on ~.",
    ),
    // imminent_doom: "that much" is the mana value of the spell cast.
    (
        r#"{"Triggered":{"when":{"SpellCast":{"what":[{"Compare":{"stat":"ManaValue","op":"Eq","to":{"Counters":{"on":"This","counter":{"Other":"doom"}}}}}],"by":"You"}},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Stat":{"of":"Triggering","stat":"ManaValue"}}}},{"PutCounters":{"on":"This","counter":{"Other":"doom"}}}]}}"#,
        r"Whenever you cast a spell with mana value equal to the number of doom counters on ~, ~ deals that much damage to any target. Then put a doom counter on ~.",
    ),
    // amalia_benavides_aguirre
    (
        r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"effects":[{"Explore":{"what":"This"}},{"If":{"condition":{"Compare":{"left":{"Stat":{"of":"This","stat":"Power"}},"op":"Eq","right":20}},"then":[{"Destroy":{"what":{"Each":[{"Type":"Creature"},"Other"]}}}]}}]}}"#,
        r"Whenever you gain life, ~ explores. Then destroy all other creatures if its power is exactly 20.",
    ),
];

/// The entries parsed once.
static L291_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    L291_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in L291_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the l291 table parses as an ability, so none is silently dropped.
    use super::{L291_SHAPES, L291_WHOLES};

    #[test]
    fn l291_every_whole_parses() {
        assert_eq!(L291_SHAPES.len(), L291_WHOLES.len());
    }
}
