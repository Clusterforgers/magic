//! Goal: the writers of continuous effects and static grants give the oracle wording of a static ability under a condition of the card, a keyword the list does not read, protection from each of your opponents, "can't be blocked except by" a subtype, a pump of the card and a subtype or by the life gained, a land that becomes a creature with its changes in any order, an effect without end or until your next turn, permanents kept tapped and a card given to an opponent, and a wrong number, kind, player, keyword, subtype, duration or condition never reads back.
use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const FRANK: &str = r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"Indestructible"}],"condition":{"Matches":{"subject":"This","selector":["AttackedThisTurn"]}}}}"#;
const GAVEL: &str = r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"DoubleStrike"}],"condition":{"Compare":{"left":{"AnyCounters":"This"},"op":"Ge","right":4}}}}"#;
const LYSE: &str = r#"{"Static":{"affected":"This","mods":[{"AddKeyword":"DoubleStrike"}],"condition":{"Compare":{"left":{"Tallied":{"who":"You","what":"NoncreatureSpellsCast"}},"op":"Ge","right":2}}}}"#;
const POISON: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Lifelink"}],"condition":{"Compare":{"left":{"Counters":{"on":"Opponents","counter":"poison"}},"op":"Ge","right":3}}}}"#;
const MARCHESA: &str = r#"{"Static":{"affected":{"Each":["Other",{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Dethrone"}]}}"#;
const RESCUER: &str = r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":{"Protection":[{"ControlledBy":"Opponents"}]}}],"duration":"UntilEndOfTurn"}}]}}"#;
const DECKHAND: &str = r#"{"Activated":{"cost":"{3}{U}","targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"},"Other"]}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"CantBeBlockedBy":[{"Not":{"Subtype":"Spirit"}}]}],"duration":"UntilEndOfTurn"}}]}}"#;
const ALANDRA: &str = r#"{"Triggered":{"when":{"Nth":{"event":{"Drew":{"who":"You"}},"nth":5}},"effects":[{"Continuous":{"affected":{"Each":[{"AnyOf":["This",{"Subtype":"Drake"}]},{"ControlledBy":"You"}]},"mods":[{"ModifyPT":{"power":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]},"toughness":{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}}}],"duration":"UntilEndOfTurn"}}]}}"#;
const BOGBEAST: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"GainLife":{"amount":2}},{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Trample"},{"ModifyPT":{"power":{"Tallied":{"who":"You","what":"LifeGainedTotal"}},"toughness":{"Tallied":{"who":"You","what":"LifeGainedTotal"}}}}],"duration":"UntilEndOfTurn"}}]}}"#;
const DISTRICT: &str = r#"{"Activated":{"cost":[{"Mana":"{4}"},{"Changed":{"change":"Reduce","mana":"{1}","per":{"Count":[{"Supertype":"Legendary"},{"AnyOf":[{"Type":"Creature"},{"Type":"Planeswalker"}]},{"ControlledBy":"You"}]}}}],"effects":[{"Continuous":{"affected":"This","mods":[{"SetPT":{"power":3,"toughness":3}},{"AddType":"Creature"},{"AddSubtype":"Citizen"},{"AddKeyword":"Vigilance"}],"duration":"UntilEndOfTurn"}}]}}"#;
const WAKER: &str = r#"{"Activated":{"cost":"{X}{G}{G}","targets":[{"what":{"Object":[{"Type":"Land"},{"ControlledBy":"You"}]}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":"X"}},{"Continuous":{"affected":{"Target":0},"mods":[{"AddType":"Creature"},{"SetPT":{"power":0,"toughness":0}},{"AddSubtype":"Elemental"},{"AddKeyword":"Haste"}],"duration":"Forever"}}]}}"#;
const TAUNT: &str = r#"{"Spell":{"effects":[{"Goad":{"what":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]}}},{"Continuous":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"Opponents"}]},"mods":["CantBlock"],"duration":"UntilYourNextTurn"}}]}}"#;
const VORINCLEX: &str = r#"{"Triggered":{"when":{"TappedForMana":{"what":[{"Type":"Land"}],"by":"Opponents"}},"effects":[{"Continuous":{"affected":"Triggering","mods":["DoesntUntap"],"duration":"NextUntapStep"}}]}}"#;
const LEVIATHAN: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Matches":{"subject":"This","selector":[{"CastFrom":"Hand"}]}},"effects":[{"Tap":{"what":{"Each":[{"Not":{"Color":"Blue"}},{"Type":"Creature"}]}}},{"Continuous":{"affected":{"Each":[{"Not":{"Color":"Blue"}},{"Type":"Creature"}]},"mods":["DoesntUntap"],"duration":"NextUntapStep"}}]}}"#;
const AKROAN: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Choose":{"from":"Opponents","as":"opponent"}},{"Continuous":{"affected":"This","mods":[{"SetController":{"Bound":"opponent"}}],"duration":"Forever"}}]}}"#;

#[rstest]
#[case::frank(FRANK, "~ has indestructible as long as it attacked this turn.")]
#[case::gavel(
    GAVEL,
    "As long as this Equipment has four or more counters on it, equipped creature has double strike."
)]
#[case::lyse(
    LYSE,
    "As long as you've cast two or more noncreature spells this turn, ~ has double strike."
)]
#[case::poison(
    POISON,
    "As long as an opponent has three or more poison counters, creatures you control have lifelink."
)]
#[case::marchesa(MARCHESA, "Other creatures you control have dethrone.")]
#[case::rescuer(
    RESCUER,
    "{T}, Sacrifice this creature: Target creature you control gains protection from each of your opponents until end of turn."
)]
#[case::deckhand(
    DECKHAND,
    "{3}{U}: Another target creature you control can't be blocked this turn except by Spirits."
)]
#[case::alandra(
    ALANDRA,
    "Whenever you draw your fifth card each turn, ~ and Drakes you control each get +X/+X until end of turn, where X is the number of cards in your hand."
)]
#[case::bogbeast(
    BOGBEAST,
    "Whenever this creature attacks, you gain 2 life. Then creatures you control gain trample and get +X/+X until end of turn, where X is the amount of life you gained this turn."
)]
#[case::district(
    DISTRICT,
    "{4}: This land becomes a 3/3 Citizen creature with vigilance until end of turn. It's still a land. This ability costs {1} less to activate for each legendary creature and planeswalker you control."
)]
#[case::waker(
    WAKER,
    "{X}{G}{G}: Put X +1/+1 counters on target land you control. That land becomes a 0/0 Elemental creature with haste. It's still a land."
)]
#[case::taunt(
    TAUNT,
    "Goad all creatures your opponents control. Until your next turn, those creatures can't block."
)]
#[case::vorinclex(
    VORINCLEX,
    "Whenever an opponent taps a land for mana, that land doesn't untap during its controller's next untap step."
)]
#[case::leviathan(
    LEVIATHAN,
    "When this creature enters, if you cast it from your hand, tap all nonblue creatures. Those creatures don't untap during their controllers' next untap steps."
)]
#[case::akroan(AKROAN, "When this creature enters, an opponent gains control of it.")]
fn writes_the_oracle_wording(#[case] json: &str, #[case] clause: &str) {
    assert!(
        written(json, clause),
        "{clause}: {:?}",
        written_variants(&ability(json))
    );
}

#[rstest]
#[case::attacked_by_another(&FRANK.replace(r#""subject":"This""#, r#""subject":"Attached""#), "~ has indestructible as long as it attacked this turn.")]
#[case::fewer_counters(
    GAVEL,
    "As long as this Equipment has three or more counters on it, equipped creature has double strike."
)]
#[case::creature_spells(
    LYSE,
    "As long as you've cast two or more creature spells this turn, ~ has double strike."
)]
#[case::at_most(&LYSE.replace(r#""op":"Ge""#, r#""op":"Le""#), "As long as you've cast two or more noncreature spells this turn, ~ has double strike.")]
#[case::other_counter(&POISON.replace("poison", "charge"), "As long as an opponent has three or more poison counters, creatures you control have lifelink.")]
#[case::other_keyword(MARCHESA, "Other creatures you control have exalted.")]
#[case::read_keyword(&MARCHESA.replace("Dethrone", "Flying"), "Other creatures you control have dethrone.")]
#[case::from_you(
    RESCUER,
    "{T}, Sacrifice this creature: Target creature you control gains protection from you until end of turn."
)]
#[case::other_subtype(
    DECKHAND,
    "{3}{U}: Another target creature you control can't be blocked this turn except by Zombies."
)]
#[case::other_subtype_pump(
    ALANDRA,
    "Whenever you draw your fifth card each turn, ~ and Dragons you control each get +X/+X until end of turn, where X is the number of cards in your hand."
)]
#[case::life_lost(
    BOGBEAST,
    "Whenever this creature attacks, you gain 2 life. Then creatures you control gain trample and get +X/+X until end of turn, where X is the amount of life you lost this turn."
)]
#[case::forever_district(&DISTRICT.replace("UntilEndOfTurn", "Forever"), "{4}: This land becomes a 3/3 Citizen creature with vigilance until end of turn. It's still a land. This ability costs {1} less to activate for each legendary creature and planeswalker you control.")]
#[case::waker_until_end(&WAKER.replace("Forever", "UntilEndOfTurn"), "{X}{G}{G}: Put X +1/+1 counters on target land you control. That land becomes a 0/0 Elemental creature with haste. It's still a land.")]
#[case::waker_creature(&WAKER.replace(r#"{"Type":"Land"},"#, r#"{"Type":"Creature"},"#), "{X}{G}{G}: Put X +1/+1 counters on target creature you control. That land becomes a 0/0 Elemental creature with haste. It's still a land.")]
#[case::taunt_this_turn(&TAUNT.replace("UntilYourNextTurn", "UntilEndOfTurn"), "Goad all creatures your opponents control. Until your next turn, those creatures can't block.")]
#[case::taunt_other_group(&TAUNT.replacen("Opponents", "You", 1), "Goad all creatures you control. Until your next turn, those creatures can't block.")]
#[case::vorinclex_this_turn(&VORINCLEX.replace("NextUntapStep", "UntilEndOfTurn"), "Whenever an opponent taps a land for mana, that land doesn't untap during its controller's next untap step.")]
#[case::vorinclex_artifact(
    VORINCLEX,
    "Whenever an opponent taps a land for mana, that artifact doesn't untap during its controller's next untap step."
)]
#[case::leviathan_untapped(&LEVIATHAN.replacen(r#"{"Tap":{"what":{"Each":[{"Not":{"Color":"Blue"}},{"Type":"Creature"}]}}},"#, "", 1), "When this creature enters, if you cast it from your hand, tap all nonblue creatures. Those creatures don't untap during their controllers' next untap steps.")]
#[case::leviathan_other_group(&LEVIATHAN.replacen("Blue", "Red", 1), "When this creature enters, if you cast it from your hand, tap all nonblue creatures. Those creatures don't untap during their controllers' next untap steps.")]
#[case::akroan_you(&AKROAN.replace(r#""from":"Opponents""#, r#""from":"You""#), "When this creature enters, an opponent gains control of it.")]
#[case::akroan_until_end(&AKROAN.replace("Forever", "UntilEndOfTurn"), "When this creature enters, an opponent gains control of it.")]
fn near_misses_do_not_read_back(#[case] json: &str, #[case] clause: &str) {
    assert!(!written(json, clause), "{clause}");
}
