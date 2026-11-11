//! Whole abilities of the r28-3 card pack whose steps the renderers do not join: each is matched whole as data and
//! worded as the card prints it, so another count, target, condition or duration in the same ability never reads back.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (the card's name as `~`; the lines of one spell joined by a newline).
const R283_WHOLES: [(&str, &str); 32] = [
    // avacyn_s_judgment#0.1
    (
        r#"{"Spell":{"targets":[{"what":"AnyTarget","bounds":{"min":0,"max":{"When":{"condition":{"CastWith":"Madness"},"then":"X","otherwise":2}}},"divide":{"When":{"condition":{"CastWith":"Madness"},"then":"X","otherwise":2}}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"Divided"}}]}}"#,
        r"~ deals 2 damage divided as you choose among any number of targets. If this spell's madness cost was paid, it deals X damage divided as you choose among those permanents and/or players instead.",
    ),
    // beacon_of_immortality#0.0
    (
        r#"{"Spell":{"targets":[{"what":"Player"}],"effects":[{"SetLife":{"who":{"Target":0},"amount":{"Times":{"by":2,"of":{"Life":{"Target":0}}}}}},{"Move":{"what":"This","to":{"Library":{"place":"Shuffled"}}}}]}}"#,
        r"Double target player's life total. Shuffle ~ into its owner's library.",
    ),
    // calamity_of_the_titans#0.1
    (
        r#"{"Spell":{"effects":[{"Exile":{"what":{"Each":[{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]},{"Compare":{"stat":"ManaValue","op":"Lt","to":{"Stat":{"of":"Paid","stat":"ManaValue"}}}}]}}}]}}"#,
        r"Exile each creature and planeswalker with mana value less than the revealed card's mana value.",
    ),
    // crush_of_tentacles#0.1
    (
        r#"{"Spell":{"effects":[{"Move":{"what":{"Each":[{"Not":{"Type":"Land"}}]},"to":"Hand"}},{"If":{"condition":{"CastWith":"Surge"},"then":[{"CreateToken":{"token":{"Custom":{"name":"Octopus","colors":["Blue"],"types":["Creature"],"subtypes":["Octopus"],"power":8,"toughness":8}}}}]}}]}}"#,
        r"Return all nonland permanents to their owners' hands. If this spell's surge cost was paid, create an 8/8 blue Octopus creature token.",
    ),
    // enter_the_infinite#0.0
    (
        r#"{"Spell":{"effects":[{"Draw":{"count":{"Count":[{"In":"Library"},{"OwnedBy":"You"}]}}},{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"}]},"as":"back"}},{"Move":{"what":{"Bound":"back"},"to":{"Library":{"place":"Top"}}}},{"Rule":{"rule":{"NoMaxHandSize":"You"},"duration":"UntilYourNextTurn"}}]}}"#,
        r"Draw cards equal to the number of cards in your library, then put a card from your hand on top of your library. You have no maximum hand size until your next turn.",
    ),
    // errand_of_duty#0.0
    (
        r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Knight","colors":["White"],"types":["Creature"],"subtypes":["Knight"],"power":1,"toughness":1,"abilities":[{"Keyword":"Banding"}]}}}}]}}"#,
        r"Create a 1/1 white Knight creature token with banding.",
    ),
    // feast_of_dreams#0.0
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"AnyOf":[{"HasAttached":[{"Subtype":"Aura"}]},{"Type":"Enchantment"}]}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
        r"Destroy target enchanted creature or enchantment creature.",
    ),
    // fell_beast_s_shriek#0.0
    (
        r#"{"Spell":{"effects":[{"RepeatEach":{"over":"Opponents","as":"opponent","effects":[{"Choose":{"who":{"Bound":"opponent"},"from":{"Each":[{"Type":"Creature"},{"ControlledBy":{"Bound":"opponent"}}]},"as":"chosen"}},{"Tap":{"what":{"Bound":"chosen"}}},{"Goad":{"what":{"Bound":"chosen"}}}]}}]}}"#,
        r"Each opponent chooses a creature they control. Tap and goad the chosen creatures.",
    ),
    // from_under_the_floorboards#0.1
    (
        r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Zombie","colors":["Black"],"types":["Creature"],"subtypes":["Zombie"],"power":2,"toughness":2}},"count":{"When":{"condition":{"CastWith":"Madness"},"then":"X","otherwise":3}},"tapped":true}},{"GainLife":{"amount":{"When":{"condition":{"CastWith":"Madness"},"then":"X","otherwise":3}}}}]}}"#,
        r"Create three tapped 2/2 black Zombie creature tokens and you gain 3 life. If this spell's madness cost was paid, instead create X of those tokens and you gain X life.",
    ),
    // grip_of_amnesia#0.0
    (
        r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":[{"ExileFromGraveyard":{"filter":[],"count":{"Count":[{"In":"Graveyard"},{"OwnedBy":{"ControllerOf":{"Target":0}}}]}}}],"unpaid":[{"Counter":{"what":{"Target":0}}}]}},{"Draw":{}}]}}"#,
        r"Counter target spell unless its controller exiles all cards from their graveyard.
Draw a card.",
    ),
    // hezrou_demonic_stench#0.0
    (
        r#"{"Triggered":{"when":{"Batch":{"BecomesBlocked":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}}},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},"Blocking"]},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}}],"duration":"UntilEndOfTurn"}}]}}"#,
        r"Whenever one or more creatures you control become blocked, each blocking creature gets -1/-1 until end of turn.",
    ),
    // hezrou_demonic_stench#1.0
    (
        r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"DidThisTurn":{"event":"Blocked"}}]},"mods":[{"ModifyPT":{"power":-1,"toughness":-1}}],"duration":"UntilEndOfTurn"}}]}}"#,
        r"Each creature that blocked this turn gets -1/-1 until end of turn.",
    ),
    // hustle_bustle#0.0
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Rule":{"rule":{"MustAttack":{"Target":0}},"duration":"UntilEndOfTurn"}},{"Rule":{"rule":{"MustBlock":{"Target":0}},"duration":"UntilEndOfTurn"}}]}}"#,
        r"Target creature attacks or blocks this turn if able.",
    ),
    // hustle_bustle#1.0
    (
        r#"{"Spell":{"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}},{"May":{"effects":[{"Choose":{"from":{"Each":[{"Type":"Creature"},"FaceDown",{"ControlledBy":"You"}]},"as":"turned"}},{"TurnFaceUp":{"what":{"Bound":"turned"}}}]}}]}}"#,
        r"Creatures you control get +2/+2 and gain trample until end of turn. You may turn a creature you control face up.",
    ),
    // illuminate#0.2
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":"X"}},{"If":{"condition":{"KickedWith":"{2}{R}"},"then":[{"DealDamage":{"to":{"ControllerOf":{"Target":0}},"amount":"X"}}]}},{"If":{"condition":{"KickedWith":"{3}{U}"},"then":[{"Draw":{"count":"X"}}]}}]}}"#,
        r"~ deals X damage to target creature. If this spell was kicked with its {2}{R} kicker, it deals X damage to that creature's controller. If this spell was kicked with its {3}{U} kicker, you draw X cards.",
    ),
    // lava_blister#0.0
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Land"},{"Not":{"Supertype":"Basic"}}]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":[{"DamageToYou":6}],"unpaid":[{"Destroy":{"what":{"Target":0}}}]}}]}}"#,
        r"Destroy target nonbasic land unless its controller has ~ deal 6 damage to them.",
    ),
    // misinformation#0.0
    (
        r#"{"Spell":{"targets":[{"what":{"Together":{"what":{"Object":[{"In":"Graveyard"},{"OwnedBy":"Opponents"}]},"rule":"SameController"}},"bounds":{"min":0,"max":3}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Library":{"place":"Top","order":"Chosen"}}}}]}}"#,
        r"Put up to three target cards from an opponent's graveyard on top of their library in any order.",
    ),
    // monastery_raid#0.1
    (
        r#"{"Spell":{"effects":[{"ExileTop":{"count":{"When":{"condition":{"CastWith":"Freerunning"},"then":"X","otherwise":2}},"as":"exiled"}},{"Rule":{"rule":{"MayCast":{"from":"Exile","only":{"Bound":"exiled"},"play":true}},"duration":"UntilEndOfYourNextTurn"}}]}}"#,
        r"Exile the top two cards of your library. If this spell's freerunning cost was paid, exile the top X cards of your library instead. You may play the exiled cards until the end of your next turn.",
    ),
    // perplex#0.0
    (
        r#"{"Spell":{"targets":[{"what":{"Object":["Spell"]}}],"effects":[{"Pay":{"who":{"ControllerOf":{"Target":0}},"cost":[{"Discard":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":{"ControllerOf":{"Target":0}}}]}}}],"unpaid":[{"Counter":{"what":{"Target":0}}}]}}]}}"#,
        r"Counter target spell unless its controller discards their hand.",
    ),
    // pistus_strike#0.0
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"HasKeyword":"Flying"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}},{"PutCounters":{"on":{"ControllerOf":{"Target":0}},"counter":"poison"}}]}}"#,
        r"Destroy target creature with flying. Its controller gets a poison counter.",
    ),
    // pull_from_eternity#0.0
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"In":"Exile"},{"Not":"FaceDown"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Graveyard"}}]}}"#,
        r"Put target face-up exiled card into its owner's graveyard.",
    ),
    // raphael_s_technique#0.2
    (
        r#"{"Spell":{"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"May":{"who":{"Bound":"player"},"effects":[{"Discard":{"who":{"Bound":"player"},"count":{"Count":[{"In":"Hand"},{"OwnedBy":{"Bound":"player"}}]}}},{"Draw":{"who":{"Bound":"player"},"count":7}}]}}]}}]}}"#,
        r"Each player may discard their hand and draw seven cards.",
    ),
    // rise_fall#0.0
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}},{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}},{"Move":{"what":{"Target":1},"to":"Hand"}}]}}"#,
        r"Return target creature card from a graveyard and target creature on the battlefield to their owners' hands.",
    ),
    // rise_from_the_wreck#0.0
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":1}},{"what":{"Object":[{"Subtype":"Mount"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":1}},{"what":{"Object":[{"Subtype":"Vehicle"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":1}},{"what":{"Object":[{"Type":"Creature"},{"Not":{"HasAbility":"Any"}},{"In":"Graveyard"},{"OwnedBy":"You"}]},"bounds":{"min":0,"max":1}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}},{"Move":{"what":{"Target":1},"to":"Hand"}},{"Move":{"what":{"Target":2},"to":"Hand"}},{"Move":{"what":{"Target":3},"to":"Hand"}}]}}"#,
        r"Return up to one target creature card, up to one target Mount card, up to one target Vehicle card, and up to one target creature card with no abilities from your graveyard to your hand.",
    ),
    // runic_repetition#0.0
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"In":"Exile"},{"HasKeywordNamed":"Flashback"},{"OwnedBy":"You"}]}}],"effects":[{"Move":{"what":{"Target":0},"to":"Hand"}}]}}"#,
        r"Return target exiled card with flashback you own to your hand.",
    ),
    // scout_for_survivors#0.0
    (
        r#"{"Spell":{"targets":[{"what":{"Together":{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"rule":{"TotalManaValueAtMost":3}}},"bounds":{"min":0,"max":3}}],"effects":[{"Move":{"what":{"Target":0},"to":{"Battlefield":{}},"as":"returned"}},{"PutCounters":{"on":{"Bound":"returned"},"counter":"+1/+1"}}]}}"#,
        r"Return up to three target creature cards with total mana value 3 or less from your graveyard to the battlefield. Put a +1/+1 counter on each of them.",
    ),
    // spider_sense#0.1
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"AnyOf":["Spell",{"Ability":"Triggered"}]},{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"},{"Ability":"Triggered"}]}]}}],"effects":[{"Counter":{"what":{"Target":0}}}]}}"#,
        r"Counter target instant spell, sorcery spell, or triggered ability.",
    ),
    // starnheim_unleashed#0.0
    (
        r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Custom":{"name":"Angel Warrior","colors":["White"],"types":["Creature"],"subtypes":["Angel","Warrior"],"power":4,"toughness":4,"abilities":[{"Keyword":"Flying"},{"Keyword":"Vigilance"}]}},"count":{"When":{"condition":{"CastWith":"Foretold"},"then":"X","otherwise":1}}}}]}}"#,
        r"Create a 4/4 white Angel Warrior creature token with flying and vigilance. If this spell was foretold, create X of those tokens instead.",
    ),
    // the_crowd_goes_wild#0.1
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]},"bounds":{"min":0,"max":"X"}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1"}},{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"HasCounter":"+1/+1"}]},"mods":[{"AddKeyword":"Trample"}],"duration":"UntilEndOfTurn"}}]}}"#,
        r"Support X.
Each creature with a +1/+1 counter on it gains trample until end of turn.",
    ),
    // the_last_ronin_s_technique#0.2
    (
        r#"{"Spell":{"effects":[{"If":{"condition":{"CastWith":"Sneak"},"then":[{"CreateToken":{"token":{"Custom":{"name":"Ninja Turtle Spirit","colors":["White"],"types":["Creature"],"subtypes":["Ninja","Turtle","Spirit"],"power":1,"toughness":1}},"count":3,"tapped":true,"attacking":true}}],"otherwise":[{"CreateToken":{"token":{"Custom":{"name":"Ninja Turtle Spirit","colors":["White"],"types":["Creature"],"subtypes":["Ninja","Turtle","Spirit"],"power":1,"toughness":1}},"count":3}}]}}]}}"#,
        r"Create three 1/1 white Ninja Turtle Spirit creature tokens. If this spell's sneak cost was paid, they enter tapped and attacking.",
    ),
    // titan_s_presence#0.1
    (
        r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"If":{"condition":{"Compare":{"left":{"Stat":{"of":{"Target":0},"stat":"Power"}},"op":"Le","right":{"Stat":{"of":"Paid","stat":"Power"}}}},"then":[{"Exile":{"what":{"Target":0}}}]}}]}}"#,
        r"Exile target creature if its power is less than or equal to the revealed card's power.",
    ),
    // winds_of_change#0.0
    (
        r#"{"Spell":{"effects":[{"RepeatEach":{"over":"EachPlayer","as":"player","effects":[{"Let":{"as":"n","amount":{"Count":[{"In":"Hand"},{"OwnedBy":{"Bound":"player"}}]}}},{"Move":{"what":{"Each":[{"In":"Hand"},{"OwnedBy":{"Bound":"player"}}]},"to":{"Library":{"place":"Shuffled"}}}},{"Draw":{"who":{"Bound":"player"},"count":{"Bound":"n"}}}]}}]}}"#,
        r"Each player shuffles the cards from their hand into their library, then draws that many cards.",
    ),
];

/// The entries parsed.
static R283_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    R283_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r283_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R283_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r28-3 table parses as an ability, so none is silently dropped.
    use super::{R283_SHAPES, R283_WHOLES};

    #[test]
    fn r283_every_whole_parses() {
        assert_eq!(R283_SHAPES.len(), R283_WHOLES.len());
    }
}
