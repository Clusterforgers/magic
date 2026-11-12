//! Goal: the writers r30-5b adds write the card's words and nothing near them: a creature held back by its attack during its controller's last turn, power and toughness from the +1/+1 counters on a group, power or toughness defined alone, a cost counted from every counter of players and permanents, the legend rule lifted for a group, an attack as though with haste unless it entered this turn, an anthem over Saprolings and other Treefolk, "must be blocked by exactly one creature if able", spells cast from graveyards or exile costing more, and the turn's draws and noncombat damage of opponents as a cost reduction.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::compile::{same_written_text, static_pair_texts, written_variants};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn writes(json: &str, text: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|written| same_written_text(written, text))
}

const TURTLE: &str = "This creature can't attack if it attacked during your last turn.";
const SLED: &str =
    "This creature doesn't untap during your untap step if it attacked during your last turn.";

fn held(mods: &str, atom: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":{mods},"condition":{{"Matches":{{"subject":"This","selector":["{atom}"]}}}}}}}}"#
    )
}

#[rstest]
#[case::turtle(held(r#"["CantAttack"]"#, "AttackedLastTurn"), TURTLE, true)]
#[case::turtle_this_turn(held(r#"["CantAttack"]"#, "AttackedThisTurn"), TURTLE, false)]
#[case::turtle_is_no_untap(held(r#"["DoesntUntap"]"#, "AttackedLastTurn"), TURTLE, false)]
#[case::sled(held(r#"["DoesntUntap"]"#, "AttackedLastTurn"), SLED, true)]
#[case::sled_always(r#"{"Static":{"affected":"This","mods":["DoesntUntap"]}}"#.to_owned(), SLED, false)]
fn r305b_held_back_by_last_turns_attack(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    assert_eq!(writes(&json, text), reads);
}

const BEHEMOTH: &str = "Fungal Behemoth's power and toughness are each equal to the number of +1/+1 counters on creatures you control.";

fn counted(power: &str, toughness: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"SetPT":{{"power":{power},"toughness":{toughness}}}}}],"defining":true}}}}"#
    )
}

const MINE: &str = r#"{"Counters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"+1/+1"}}"#;
const ALL: &str = r#"{"Counters":{"on":{"Each":[{"Type":"Creature"}]},"counter":"+1/+1"}}"#;
const MINE_MINUS: &str = r#"{"Counters":{"on":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"counter":"-1/-1"}}"#;

#[rstest]
#[case::mine(counted(MINE, MINE), true)]
#[case::every_creature(counted(ALL, ALL), false)]
#[case::another_kind(counted(MINE_MINUS, MINE_MINUS), false)]
#[case::toughness_differs(counted(MINE, ALL), false)]
fn r305b_power_and_toughness_from_counters_on_a_group(#[case] json: String, #[case] reads: bool) {
    let text = BEHEMOTH.replace("Fungal Behemoth", "~");
    assert_eq!(writes(&json, &text), reads);
}

fn alone(stat: &str, color: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"{stat}":{{"Count":[{{"Color":"{color}"}},{{"Type":"Creature"}}]}}}}],"defining":true}}}}"#
    )
}

const KAVU_POWER: &str = "~'s power is equal to the number of red creatures on the battlefield.";
const KAVU_TOUGHNESS: &str =
    "~'s toughness is equal to the number of green creatures on the battlefield.";

#[rstest]
#[case::power(alone("SetPower", "Red"), KAVU_POWER, true)]
#[case::power_of_green(alone("SetPower", "Green"), KAVU_POWER, false)]
#[case::toughness_is_no_power(alone("SetToughness", "Red"), KAVU_POWER, false)]
#[case::toughness(alone("SetToughness", "Green"), KAVU_TOUGHNESS, true)]
#[case::toughness_of_red(alone("SetToughness", "Red"), KAVU_TOUGHNESS, false)]
fn r305b_power_or_toughness_defined_alone(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    assert_eq!(writes(&json, text), reads);
}

const SLOTH: &str =
    "This spell costs {1} less to cast for each counter among players and permanents.";

fn sloth(on: &str, mana: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":["This"],"change":"Reduce","mana":"{mana}","per":{{"AnyCounters":{on}}}}}}}"#
    )
}

#[rstest]
#[case::players_and_permanents(sloth(r#"{"Union":["EachPlayer",{"Each":[]}]}"#, "{1}"), true)]
#[case::permanents_only(sloth(r#"{"Each":[]}"#, "{1}"), false)]
#[case::players_only(sloth(r#""EachPlayer""#, "{1}"), false)]
#[case::two_less(sloth(r#"{"Union":["EachPlayer",{"Each":[]}]}"#, "{2}"), false)]
fn r305b_a_cost_counted_from_every_counter(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, SLOTH), reads);
}

const MASTER: &str = "The \"legend rule\" doesn't apply to creature tokens you control.";

#[rstest]
#[case::creature_tokens(
    r#"{"Rule":{"rule":{"NoLegendRuleFor":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]}}}"#,
    true
)]
#[case::every_token(
    r#"{"Rule":{"rule":{"NoLegendRuleFor":["Token",{"ControlledBy":"You"}]}}}"#,
    false
)]
#[case::anyones(
    r#"{"Rule":{"rule":{"NoLegendRuleFor":[{"Type":"Creature"},"Token"]}}}"#,
    false
)]
#[case::under_a_condition(r#"{"Rule":{"rule":{"NoLegendRuleFor":[{"Type":"Creature"},"Token",{"ControlledBy":"You"}]},"condition":"YourTurn"}}"#, false)]
fn r305b_the_legend_rule_lifted_for_a_group(#[case] json: &str, #[case] reads: bool) {
    assert_eq!(writes(json, MASTER), reads);
}

const CHAOS: &str = "This creature can attack as though it had haste unless it entered this turn.";

#[rstest]
#[case::unless_entered(r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":"AttackAsThoughHaste"}},"condition":{"Not":{"Matches":{"subject":"This","selector":["EnteredThisTurn"]}}}}}"#, true)]
#[case::always(
    r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":"AttackAsThoughHaste"}}}}"#,
    false
)]
#[case::only_if_entered(r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":"AttackAsThoughHaste"}},"condition":{"Matches":{"subject":"This","selector":["EnteredThisTurn"]}}}}"#, false)]
fn r305b_haste_unless_it_entered_this_turn(#[case] json: &str, #[case] reads: bool) {
    assert_eq!(writes(json, CHAOS), reads);
}

const VERDELOTH: &str = "Saproling creatures and other Treefolk creatures get +1/+1.";

fn anthem(group: &str, bonus: i32) -> String {
    format!(
        r#"{{"Static":{{"affected":{{"Each":[{{"Type":"Creature"}},{group}]}},"mods":[{{"ModifyPT":{{"power":{bonus},"toughness":{bonus}}}}}]}}}}"#
    )
}

const OTHER_TREEFOLK: &str = r#"{"AnyOf":[{"Subtype":"Saproling"},{"Not":{"AnyOf":[{"Not":{"Subtype":"Treefolk"}},"This"]}}]}"#;
const ANY_TREEFOLK: &str = r#"{"AnyOf":[{"Subtype":"Saproling"},{"Subtype":"Treefolk"}]}"#;

#[rstest]
#[case::other_treefolk(anthem(OTHER_TREEFOLK, 1), true)]
#[case::every_treefolk(anthem(ANY_TREEFOLK, 1), false)]
#[case::bigger(anthem(OTHER_TREEFOLK, 2), false)]
fn r305b_saprolings_and_other_treefolk(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, VERDELOTH), reads);
}

const MUST: &str =
    r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":{"MustBeBlocked":"AtLeastOne"}}}}}"#;

#[rstest]
#[case::exactly_one(
    MUST,
    r#"{"Static":{"affected":"This","mods":[{"MaxBlockers":1}]}}"#,
    true
)]
#[case::at_most_two(
    MUST,
    r#"{"Static":{"affected":"This","mods":[{"MaxBlockers":2}]}}"#,
    false
)]
#[case::by_every(
    r#"{"Rule":{"rule":{"Combat":{"who":"This","quirk":{"MustBeBlocked":"Every"}}}}}"#,
    r#"{"Static":{"affected":"This","mods":[{"MaxBlockers":1}]}}"#,
    false
)]
fn r305b_must_be_blocked_by_exactly_one(
    #[case] first: &str,
    #[case] second: &str,
    #[case] reads: bool,
) {
    let texts = static_pair_texts(&ability(first), &ability(second));
    assert_eq!(
        texts.iter().any(|text| same_written_text(
            text,
            "~ must be blocked by exactly one creature if able."
        )),
        reads
    );
}

const AVEN: &str =
    "Spells your opponents cast from graveyards or from exile cost {2} more to cast.";

fn from_zones(zones: &str, who: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":["Spell",{{"ControlledBy":"{who}"}},{{"AnyOf":[{zones}]}}],"change":"Increase","mana":"{{2}}"}}}}"#
    )
}

#[rstest]
#[case::graveyards_and_exile(
    from_zones(r#"{"CastFrom":"Graveyard"},{"CastFrom":"Exile"}"#, "Opponents"),
    true
)]
#[case::graveyards_only(
    from_zones(r#"{"CastFrom":"Graveyard"},{"CastFrom":"Hand"}"#, "Opponents"),
    false
)]
#[case::yours(
    from_zones(r#"{"CastFrom":"Graveyard"},{"CastFrom":"Exile"}"#, "You"),
    false
)]
fn r305b_spells_cast_from_graveyards_or_exile(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, AVEN), reads);
}

const HELIOD: &str =
    "Spells you cast cost {1} less to cast for each card your opponents have drawn this turn.";
const INCINERATOR: &str = "This spell costs {X} less to cast, where X is the total amount of noncombat damage dealt to your opponents this turn.";

fn tallied(applies_to: &str, who: &str, what: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":{applies_to},"change":"Reduce","mana":"{{1}}","per":{{"Tallied":{{"who":"{who}","what":"{what}"}}}}}}}}"#
    )
}

const YOURS: &str = r#"["Spell",{"ControlledBy":"You"}]"#;

#[rstest]
#[case::drawn(tallied(YOURS, "Opponents", "CardsDrawnTotal"), HELIOD, true)]
#[case::the_most_drawn(tallied(YOURS, "Opponents", "Drawn"), HELIOD, false)]
#[case::your_draws(tallied(YOURS, "You", "CardsDrawnTotal"), HELIOD, false)]
#[case::noncombat(
    tallied(r#"["This"]"#, "Opponents", "NoncombatDamageTakenTotal"),
    INCINERATOR,
    true
)]
#[case::the_most_noncombat(
    tallied(r#"["This"]"#, "Opponents", "NoncombatDamageTaken"),
    INCINERATOR,
    false
)]
#[case::all_damage(tallied(r#"["This"]"#, "Opponents", "DamageTaken"), INCINERATOR, false)]
fn r305b_the_turns_draws_and_noncombat_damage_as_a_cost(
    #[case] json: String,
    #[case] text: &str,
    #[case] reads: bool,
) {
    assert_eq!(writes(&json, text), reads);
}

const HINATA_YOURS: &str = "Spells you cast cost {1} less to cast for each target.";
const HINATA_THEIRS: &str = "Spells your opponents cast cost {1} more to cast for each target.";

fn per_target(who: &str, change: &str, per: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":["Spell",{{"ControlledBy":"{who}"}}],"change":"{change}","mana":"{{1}}","per":{per}}}}}"#
    )
}

#[rstest]
#[case::yours(per_target("You", "Reduce", r#""CastTargets""#), HINATA_YOURS, true)]
#[case::yours_more(per_target("You", "Increase", r#""CastTargets""#), HINATA_YOURS, false)]
#[case::triggering_targets(
    per_target("You", "Reduce", r#""TriggeringTargets""#),
    HINATA_YOURS,
    false
)]
#[case::theirs(
    per_target("Opponents", "Increase", r#""CastTargets""#),
    HINATA_THEIRS,
    true
)]
#[case::theirs_less(
    per_target("Opponents", "Reduce", r#""CastTargets""#),
    HINATA_THEIRS,
    false
)]
fn r305b_a_cost_for_each_target(#[case] json: String, #[case] text: &str, #[case] reads: bool) {
    assert_eq!(writes(&json, text), reads);
}

const FROGANTUA: &str = "This creature gets +10/+10 for each player who has lost the game.";

fn grows(by: i32, of: &str) -> String {
    format!(
        r#"{{"Static":{{"affected":"This","mods":[{{"ModifyPT":{{"power":{{"Times":{{"by":{by},"of":{of}}}}},"toughness":{{"Times":{{"by":{by},"of":{of}}}}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::lost(grows(10, r#""PlayersLost""#), true)]
#[case::by_one(grows(1, r#""PlayersLost""#), false)]
#[case::opponents(grows(10, r#"{"Players":"Opponents"}"#), false)]
fn r305b_a_bonus_for_each_player_who_lost(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, FROGANTUA), reads);
}

const MILLED: &str = "Whenever this creature deals combat damage to a player, you may mill that many cards. Put any number of land cards from among them onto the battlefield tapped.";

fn milled(kind: &str, tapped: bool) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"DealsDamage":{{"source":["This"],"to":"EachPlayer","combat":true}}}},"effects":[{{"May":{{"effects":[{{"Mill":{{"count":"Triggering","as":"milled"}}}},{{"Choose":{{"from":{{"Each":[{{"In":"Graveyard"}},{{"Is":{{"Bound":"milled"}}}},{{"Type":"{kind}"}}]}},"count":{{"min":0,"max":null}},"as":"lands"}}}},{{"Move":{{"what":{{"Bound":"lands"}},"to":{{"Battlefield":{{"tapped":{tapped}}}}}}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::lands_tapped(milled("Land", true), true)]
#[case::untapped(milled("Land", false), false)]
#[case::creatures(milled("Creature", true), false)]
fn r305b_lands_from_among_the_milled_cards(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, MILLED), reads);
}

const AVENGE: &str =
    "This spell costs {2} less to cast if a player attacked you during their last turn.";

fn avenge(mana: &str, condition: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":["This"],"change":"Reduce","mana":"{mana}","condition":{condition}}}}}"#
    )
}

#[rstest]
#[case::attacked_you(avenge("{2}", r#"{"AttackedYouLastTurn":"EachPlayer"}"#), true)]
#[case::three_less(avenge("{3}", r#"{"AttackedYouLastTurn":"EachPlayer"}"#), false)]
#[case::an_opponent_only(avenge("{2}", r#"{"AttackedYouLastTurn":"Opponents"}"#), false)]
#[case::attacked_this_combat(avenge("{2}", r#"{"AttackedThisCombat":"EachPlayer"}"#), false)]
fn r305b_cheaper_after_an_attack_on_you(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, AVENGE), reads);
}

const SPAWN: &str = "This creature escapes with two +1/+1 counters on it. When it enters this way, each opponent sacrifices a non-Gorgon creature of their choice.";

fn escapes(count: u32) -> String {
    format!(
        r#"{{"Replacement":{{"when":{{"Enters":{{"what":["This"]}}}},"by":{{"WithCounters":{{"counter":"+1/+1","count":{count}}}}},"condition":{{"CastWith":"Escape"}}}}}}"#
    )
}

fn sacrifices(subtype: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"if":{{"CastWith":"Escape"}},"effects":[{{"Sacrifice":{{"who":"Opponents","filter":[{{"Type":"Creature"}},{{"Not":{{"Subtype":"{subtype}"}}}}]}}}}]}}}}"#
    )
}

#[rstest]
#[case::spawn(escapes(2), sacrifices("Gorgon"), true)]
#[case::three_counters(escapes(3), sacrifices("Gorgon"), false)]
#[case::non_zombie(escapes(2), sacrifices("Zombie"), false)]
fn r305b_escapes_with_and_enters_this_way(
    #[case] first: String,
    #[case] second: String,
    #[case] reads: bool,
) {
    let texts = static_pair_texts(&ability(&first), &ability(&second));
    let text = SPAWN.replace("This creature", "~");
    assert_eq!(
        texts
            .iter()
            .any(|written| same_written_text(written, &text)),
        reads
    );
}

const TOOLKIT: &str = "Whenever a creature you control enters, you may move a counter from this artifact onto that creature.";

fn moves(whose: &str, may: bool) -> String {
    let step = r#"{"MoveCounters":{"from":"This","to":"Triggering"}}"#;
    let effects = if may {
        format!(r#"[{{"May":{{"effects":[{step}]}}}}]"#)
    } else {
        format!("[{step}]")
    };
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":[{{"Type":"Creature"}},{{"ControlledBy":"{whose}"}}],"to":"Battlefield"}}}},"effects":{effects}}}}}"#
    )
}

#[rstest]
#[case::yours_may(moves("You", true), true)]
#[case::must(moves("You", false), false)]
#[case::theirs(moves("Opponents", true), false)]
fn r305b_a_counter_moved_onto_that_creature(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, TOOLKIT), reads);
}

const CURSE: &str = "Spells you cast that target enchanted player cost {1} less to cast.";

fn at_player(who: &str, change: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":["Spell",{{"ControlledBy":"You"}},{{"TargetsPlayer":"{who}"}}],"change":"{change}","mana":"{{1}}"}}}}"#
    )
}

#[rstest]
#[case::enchanted(at_player("AttachedPlayer", "Reduce"), true)]
#[case::more(at_player("AttachedPlayer", "Increase"), false)]
#[case::an_opponent(at_player("Opponents", "Reduce"), false)]
fn r305b_spells_that_target_the_enchanted_player(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, CURSE), reads);
}

const FEAR: &str = "Stun counters can't be removed from permanents your opponents control.";

fn kept(who: &str, action: &str) -> String {
    format!(
        r#"{{"Rule":{{"rule":{{"Cant":{{"who":{{"Each":[{{"ControlledBy":"{who}"}}]}},"action":{action}}}}}}}}}"#
    )
}

#[rstest]
#[case::stun_on_theirs(kept("Opponents", r#"{"RemoveCounterOf":{"Other":"stun"}}"#), true)]
#[case::stun_on_yours(kept("You", r#"{"RemoveCounterOf":{"Other":"stun"}}"#), false)]
#[case::another_kind(kept("Opponents", r#"{"RemoveCounterOf":"+1/+1"}"#), false)]
#[case::put_is_not_removed(kept("Opponents", r#"{"PutCounterOf":{"Other":"stun"}}"#), false)]
fn r305b_counters_that_cant_be_removed(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, FEAR), reads);
}

const CHANGELING: &str = "This spell costs {1} less to cast for each creature type among creatures you control. This effect can't reduce the amount of mana this spell costs by more than {5}.";

fn capped(per: &str) -> String {
    format!(
        r#"{{"CostModifier":{{"applies_to":["This"],"change":"Reduce","mana":"{{1}}","per":{per}}}}}"#
    )
}

const TYPES: &str =
    r#"{"Distinct":{"of":[{"Type":"Creature"},{"ControlledBy":"You"}],"stat":"CreatureType"}}"#;

#[rstest]
#[case::at_most_five(capped(&format!(r#"{{"AtMost":{{"of":{TYPES},"most":5}}}}"#)), true)]
#[case::at_most_four(capped(&format!(r#"{{"AtMost":{{"of":{TYPES},"most":4}}}}"#)), false)]
#[case::uncapped(capped(TYPES), false)]
fn r305b_a_reduction_that_cant_pass_five(#[case] json: String, #[case] reads: bool) {
    assert_eq!(writes(&json, CHANGELING), reads);
}
