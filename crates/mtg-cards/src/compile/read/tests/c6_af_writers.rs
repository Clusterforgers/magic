//! Goal: the writers the sixth sample of the names A to F needed give the oracle wording of each twin with the card's
//! words put back, each phrase swap and two steps on two targets worded as two sentences; a changed number, kind,
//! player, zone, step or group never reads back.
use super::*;

/// The wordings the checker accepts for the rules.
fn c6_af_wordings(json: &str) -> Vec<String> {
    written_variants(&ability(json))
}

/// Whether some wording the checker accepts for the rules is the clause.
fn c6_af_written(json: &str, clause: &str) -> bool {
    c6_af_wordings(json)
        .iter()
        .any(|wording| same_text(wording, clause))
}

const C6_AF_PHOENIX: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":[{"HasKeyword":"Cascade"}],"by":"You"}},"from":"Graveyard","effects":[{"Move":{"what":"This","to":"Hand"}}]}}"#;
const C6_AF_BREATH: &str = r#"{"Spell":{"effects":[{"If":{"condition":"Kicked","then":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"},{"Not":{"HasKeyword":"Flying"}}]},"amount":4}},{"DealDamage":{"to":"EachPlayer","amount":4}}],"otherwise":[{"DealDamage":{"to":{"Each":[{"Type":"Creature"},{"Not":{"HasKeyword":"Flying"}}]},"amount":1}},{"DealDamage":{"to":"EachPlayer","amount":1}}]}}]}}"#;
const C6_AF_DRAKE: &str = r#"{"Static":{"affected":"This","mods":[{"SetPT":{"power":{"Count":[{"AnyOf":[{"Type":"Instant"},{"Type":"Sorcery"}]},{"AnyOf":[{"In":"Exile"},{"In":"Graveyard"}]},{"OwnedBy":"You"}]},"toughness":4}}],"defining":true}}"#;
const C6_AF_ARCHAIC: &str = r#"{"Activated":{"cost":[{"Mana":"{1}"},"Tap"],"targets":[{"what":{"Object":[{"AnyOf":[{"Ability":"Activated"},{"Ability":"Triggered"}]},{"ControlledBy":"You"},"Colorless"]}}],"effects":[{"CopyAbility":{"what":{"Target":0},"new_targets":true}}]}}"#;
const C6_AF_AUTHORITY: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Enchantment"}]}]}}],"effects":[{"May":{"effects":[{"Exile":{"what":{"Target":0}}}]}},{"If":{"condition":"Performed","then":[{"Continuous":{"affected":"This","mods":[{"SetController":{"ControllerOf":{"Target":0}}}],"duration":"Forever"}}]}}]}}"#;
const C6_AF_AGILITY: &str = r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]},"bounds":{"min":0,"max":{"Count":[{"Subtype":"Bobblehead"},{"ControlledBy":"You"}]}}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"AddKeyword":"Haste"},{"CantBeBlockedBy":[{"Not":{"HasKeyword":"Haste"}}]}],"duration":"UntilEndOfTurn"}}]}}"#;
const C6_AF_ENDURANCE: &str = r#"{"Activated":{"cost":[{"Mana":"{3}"},"Tap"],"timing":"Sorcery","targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]},"bounds":{"min":0,"max":{"Count":[{"Subtype":"Bobblehead"},{"ControlledBy":"You"}]}}}],"effects":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":1,"toughness":0}},{"AddKeyword":"Indestructible"}],"duration":"UntilEndOfTurn"}}]}}"#;
const C6_AF_AWAKENER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Continuous":{"affected":{"Each":[{"Type":"Artifact"},{"Not":{"Type":"Creature"}},{"ControlledBy":"You"}]},"mods":[{"AddType":"Artifact"},{"AddType":"Creature"},{"SetPT":{"power":4,"toughness":4}}],"duration":"UntilEndOfTurn"}}]}}"#;
const C6_AF_DOMRI: &str = r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"AddMana":{"mana":{"OneOf":["Red","Green"]}}},{"Rule":{"rule":{"CantBeCountered":{"Each":["Spell",{"Type":"Creature"},{"ControlledBy":"You"}]}},"duration":"UntilEndOfTurn"}}]}}"#;
const C6_AF_ALTAR: &str = r#"{"Triggered":{"when":{"AttacksAlone":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}]}},"effects":[{"Continuous":{"affected":"Triggering","mods":[{"ModifyPT":{"power":{"CardTypes":[{"In":"Graveyard"}]},"toughness":{"CardTypes":[{"In":"Graveyard"}]}}}],"duration":"UntilEndOfTurn"}}]}}"#;
const C6_AF_DINOSAURS: &str = r#"{"Triggered":{"when":{"CountersRemoved":{"what":["This"],"counter":{"Other":"time"}}},"from":"Exile","effects":[{"CreateToken":{"token":{"Custom":{"name":"Dinosaur","colors":["Red","White"],"types":["Creature"],"subtypes":["Dinosaur"],"power":2,"toughness":2,"abilities":[{"Keyword":"Flying"},{"Keyword":"Haste"}]}}}}]}}"#;
const C6_AF_DOOR: &str = r#"{"Triggered":{"when":{"SpellCast":{"what":["ChosenSubtype"],"by":"You"}},"effects":[{"PutCounters":{"on":"This","counter":"charge"}}]}}"#;
const C6_AF_CRAWLER: &str = r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"WithCounters":{"counter":"+1/+1","count":"ColorsSpent"}}}}"#;
const C6_AF_FLAYER: &str = r#"{"Triggered":{"when":{"AnyOf":[{"ZoneChange":{"what":["This"],"from":"Graveyard","to":"Battlefield"}},{"ZoneChange":{"what":[{"Type":"Creature"},"Other",{"OwnedBy":"You"}],"from":"Graveyard","to":"Battlefield"}}]},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"source":"Triggering","to":{"Target":0},"amount":{"Stat":{"of":"Triggering","stat":"Power"}}}}]}}"#;
const C6_AF_ALIBOU: &str = r#"{"Triggered":{"when":{"GroupAttacks":{"what":[{"Type":"Artifact"},{"Type":"Creature"},{"ControlledBy":"You"}]}},"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":{"Count":[{"Type":"Artifact"},"Tapped",{"ControlledBy":"You"}]}}},{"Scry":{"count":{"Count":[{"Type":"Artifact"},"Tapped",{"ControlledBy":"You"}]}}}]}}"#;
const C6_AF_CORNUCOPIA: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"AnyColor":{"Counters":{"on":"This","counter":"charge"}}}}}]}}"#;
const C6_AF_FIRJA: &str = r#"{"Triggered":{"when":{"Chapter":{"number":2}},"effects":[{"Continuous":{"affected":{"Each":[{"Subtype":"Angel"},{"ControlledBy":"You"}]},"mods":[{"GrantAbility":{"Activated":{"cost":["Tap"],"targets":[{"what":{"Object":[{"Type":"Creature"},{"Compare":{"stat":"Power","op":"Lt","to":{"Stat":{"of":"This","stat":"Power"}}}}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}}],"duration":"UntilEndOfTurn"}}]}}"#;
const C6_AF_ARMIX: &str = r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"Defending"}]}}],"effects":[{"May":{"effects":[{"Discard":{}}]}},{"If":{"condition":"Performed","then":[{"Continuous":{"affected":{"Target":0},"mods":[{"ModifyPT":{"power":{"Times":{"by":-1,"of":{"Sum":[{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]},{"Count":[{"Type":"Artifact"},{"In":"Graveyard"},{"OwnedBy":"You"}]}]}}},"toughness":{"Times":{"by":-1,"of":{"Sum":[{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]},{"Count":[{"Type":"Artifact"},{"In":"Graveyard"},{"OwnedBy":"You"}]}]}}}}}],"duration":"UntilEndOfTurn"}}]}}]}}"#;
const C6_AF_BETOR: &str = r#"{"Triggered":{"when":{"StepBegins":{"step":"End","whose":"You"}},"targets":[{"what":{"Object":[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]},"bounds":{"min":0,"max":1}},{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"You"},{"Compare":{"stat":"ManaValue","op":"Le","to":{"Tallied":{"who":"You","what":"LifeLost"}}}}]},"bounds":{"min":0,"max":1}}],"effects":[{"PutCounters":{"on":{"Target":0},"counter":"+1/+1","count":{"Tallied":{"who":"You","what":"LifeGained"}}}},{"Move":{"what":{"Target":1},"to":{"Battlefield":{}}}}]}}"#;

#[rstest]
#[case::cascade(
    C6_AF_PHOENIX,
    "Whenever you cast a spell with cascade, return ~ from your graveyard to your hand."
)]
#[case::each_player(
    C6_AF_BREATH,
    "~ deals 1 damage to each creature without flying and each player. If this spell was kicked, it deals 4 damage to each creature without flying and each player instead."
)]
#[case::exile_and_graveyard(
    C6_AF_DRAKE,
    "~'s power is equal to the total number of instant and sorcery cards you own in exile and in your graveyard."
)]
#[case::colorless_source(
    C6_AF_ARCHAIC,
    "{1}, {T}: Copy target activated or triggered ability you control from a colorless source. You may choose new targets for the copy."
)]
#[case::controller_gains_control(
    C6_AF_AUTHORITY,
    "At the beginning of your upkeep, you may exile target artifact or enchantment. If you do, its controller gains control of ~."
)]
#[case::except_by_haste(
    C6_AF_AGILITY,
    "{3}, {T}: Up to X target creatures you control each gain haste until end of turn and can't be blocked this turn except by creatures with haste, where X is the number of Bobbleheads you control as you activate this ability."
)]
#[case::as_you_activate(
    C6_AF_ENDURANCE,
    "{3}, {T}: Up to X target creatures you control get +1/+0 and gain indestructible until end of turn, where X is the number of Bobbleheads you control as you activate this ability. Activate only as a sorcery."
)]
#[case::becomes_artifact_creature(
    C6_AF_AWAKENER,
    "When ~ enters, each noncreature artifact you control becomes a 4/4 artifact creature until end of turn."
)]
#[case::cant_be_countered(
    C6_AF_DOMRI,
    "+1: Add {R} or {G}. Creature spells you cast this turn can't be countered."
)]
#[case::attacks_alone(
    C6_AF_ALTAR,
    "Whenever a creature you control attacks alone, it gets +X/+X until end of turn, where X is the number of card types among cards in all graveyards."
)]
#[case::while_exiled(
    C6_AF_DINOSAURS,
    "Whenever a time counter is removed from ~ while it's exiled, create a 2/2 red and white Dinosaur creature token with flying and haste."
)]
#[case::chosen_type(
    C6_AF_DOOR,
    "Whenever you cast a spell of the chosen type, put a charge counter on ~."
)]
#[case::converge(
    C6_AF_CRAWLER,
    "~ enters with a +1/+1 counter on it for each color of mana spent to cast it."
)]
#[case::from_your_graveyard(
    C6_AF_FLAYER,
    "Whenever ~ or another creature enters from your graveyard, that creature deals damage equal to its power to any target."
)]
#[case::and_you_scry(
    C6_AF_ALIBOU,
    "Whenever one or more artifact creatures you control attack, ~ deals X damage to any target and you scry X, where X is the number of tapped artifacts you control."
)]
#[case::choose_a_color(
    C6_AF_CORNUCOPIA,
    "{T}: Choose a color. Add one mana of that color for each charge counter on ~."
)]
#[case::granted_until_end_of_turn(
    C6_AF_FIRJA,
    "II — Until end of turn, Angels you control gain \"{T}: Destroy target creature with power less than ~'s power.\""
)]
#[case::when_you_do(
    C6_AF_ARMIX,
    "Whenever ~ attacks, you may discard a card. When you do, target creature defending player controls gets -X/-X until end of turn, where X is the number of artifacts you control plus the number of artifact cards in your graveyard."
)]
#[case::two_targets(
    C6_AF_BETOR,
    "At the beginning of your end step, put a number of +1/+1 counters on up to one other target creature you control equal to the amount of life you gained this turn. Return up to one target creature card with mana value less than or equal to the amount of life you lost this turn from your graveyard to the battlefield."
)]
fn c6_af_writes_the_card_wording(#[case] json: &str, #[case] clause: &str) {
    assert!(
        c6_af_written(json, clause),
        "{clause}\nwordings: {:#?}",
        c6_af_wordings(json)
    );
}

#[rstest]
#[case::other_keyword(
    C6_AF_PHOENIX,
    "Whenever you cast a spell with flying, return ~ from your graveyard to your hand."
)]
#[case::other_amount(
    C6_AF_BREATH,
    "~ deals 2 damage to each creature without flying and each player. If this spell was kicked, it deals 4 damage to each creature without flying and each player instead."
)]
#[case::other_zone(
    C6_AF_DRAKE,
    "~'s power is equal to the total number of instant and sorcery cards you own in exile and in your hand."
)]
#[case::other_color(
    C6_AF_ARCHAIC,
    "{1}, {T}: Copy target activated or triggered ability you control from a blue source. You may choose new targets for the copy."
)]
#[case::other_player(
    C6_AF_AUTHORITY,
    "At the beginning of your upkeep, you may exile target artifact or enchantment. If you do, its owner gains control of ~."
)]
#[case::other_blockers(
    C6_AF_AGILITY,
    "{3}, {T}: Up to X target creatures you control each gain haste until end of turn and can't be blocked this turn except by creatures with flying, where X is the number of Bobbleheads you control as you activate this ability."
)]
#[case::other_timing(
    C6_AF_ENDURANCE,
    "{3}, {T}: Up to X target creatures you control get +1/+0 and gain indestructible until end of turn, where X is the number of Bobbleheads you control as you activate this ability. Activate only during your turn."
)]
#[case::other_size(
    C6_AF_AWAKENER,
    "When ~ enters, each noncreature artifact you control becomes a 3/3 artifact creature until end of turn."
)]
#[case::other_kind(
    C6_AF_DOMRI,
    "+1: Add {R} or {G}. Instant spells you cast this turn can't be countered."
)]
#[case::other_group(
    C6_AF_ALTAR,
    "Whenever a creature an opponent controls attacks alone, it gets +X/+X until end of turn, where X is the number of card types among cards in all graveyards."
)]
#[case::other_place(
    C6_AF_DINOSAURS,
    "Whenever a time counter is removed from ~ while it's in your graveyard, create a 2/2 red and white Dinosaur creature token with flying and haste."
)]
#[case::other_choice(
    C6_AF_DOOR,
    "Whenever you cast a spell of the chosen color, put a charge counter on ~."
)]
#[case::other_counter(
    C6_AF_CRAWLER,
    "~ enters with a -1/-1 counter on it for each color of mana spent to cast it."
)]
#[case::other_origin(
    C6_AF_FLAYER,
    "Whenever ~ or another creature enters from exile, that creature deals damage equal to its power to any target."
)]
#[case::other_count(
    C6_AF_ALIBOU,
    "Whenever one or more artifact creatures you control attack, ~ deals X damage to any target and you scry X, where X is the number of untapped artifacts you control."
)]
#[case::other_counters(
    C6_AF_CORNUCOPIA,
    "{T}: Choose a color. Add one mana of that color for each time counter on ~."
)]
#[case::other_creatures(
    C6_AF_FIRJA,
    "II — Until end of turn, Humans you control gain \"{T}: Destroy target creature with power less than ~'s power.\""
)]
#[case::other_cost(
    C6_AF_ARMIX,
    "Whenever ~ attacks, you may sacrifice a creature. When you do, target creature defending player controls gets -X/-X until end of turn, where X is the number of artifacts you control plus the number of artifact cards in your graveyard."
)]
#[case::other_life(
    C6_AF_BETOR,
    "At the beginning of your end step, put a number of +1/+1 counters on up to one other target creature you control equal to the amount of life you gained this turn. Return up to one target creature card with mana value less than or equal to the amount of life you gained this turn from your graveyard to the battlefield."
)]
fn c6_af_misses_another_wording(#[case] json: &str, #[case] clause: &str) {
    assert!(!c6_af_written(json, clause), "{clause}");
}
