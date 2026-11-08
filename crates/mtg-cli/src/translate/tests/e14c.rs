//! Goal: choices made by the game or by other players translate to their nodes (modes at random or chosen by an opponent, modes that each target a different player, votes on objects and for the most-voted choices, players at random, type choices with limits, total power and toughness, an additional vote), and a shape the nodes cannot say stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// A sorcery whose spell line is `line`, with parts the lines name.
fn spell(line: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {line}\n\
         SVar:DBA:DB$ GainLife | LifeAmount$ 2 | SpellDescription$ Alpha\n\
         SVar:DBB:DB$ Draw | SpellDescription$ Beta\n\
         SVar:DBC:DB$ Draw | NumCards$ 2 | SpellDescription$ Gamma\n\
         SVar:DBT:DB$ Draw | SubAbility$ DBA | SpellDescription$ Tee\n\
         SVar:DBP:DB$ LoseLife | ValidTgts$ Opponent | TargetUnique$ True | LifeAmount$ 2 | SpellDescription$ P\n\
         SVar:DBQ:DB$ Draw | ValidTgts$ Player | TargetUnique$ True | SpellDescription$ Q\n\
         SVar:DBR:DB$ Draw | ValidTgts$ Creature | TargetUnique$ True | SpellDescription$ R\n\
         SVar:DBX:DB$ ChangeZone | Defined$ Remembered | Origin$ Battlefield | Destination$ Exile\n\
         SVar:DBRed:DB$ GainLife | LifeAmount$ 1 | SpellDescription$ Redhorn Pass\n\
         SVar:DBLong:DB$ GainLife | LifeAmount$ 1 | SpellDescription$ If this gets more votes, gain life\n\
         SVar:Many:PlayerCountOpponents$Amount\nSVar:X:Count$xPaid\n"
    )
}

/// The effects of the spell.
fn effects(script: &str) -> Value {
    assert_eq!(gaps(script), Vec::<String>::new(), "{script}");
    abilities(script)[0]["Spell"]["effects"].clone()
}

#[rstest]
#[case::at_random("Charm | Choices$ DBA,DBB | Random$ True", "at_random", json!(true))]
#[case::an_opponent_chooses("Charm | Choices$ DBA,DBB | Chooser$ Opponent", "who", json!("Opponents"))]
#[case::x_announced("Charm | Choices$ DBA,DBC | CharmNum$ 2 | Announce$ X", "choose", json!({"min": 2, "max": 2}))]
#[case::different_players(
    "Charm | Choices$ DBP,DBQ | CharmNum$ 2 | MinCharmNum$ 1 | AdditionalDescription$ . Each mode must target a different player.",
    "different_players",
    json!(true)
)]
#[case::one_mode_per_opponent(
    "Charm | Choices$ DBP,DBQ | CharmNum$ Many | MinCharmNum$ 1 | AdditionalDescription$ or more. Each mode must target a different player.",
    "choose",
    json!({"min": 1, "max": {"Players": "Opponents"}})
)]
fn a_charm_keeps_who_picks_its_modes(
    #[case] line: &str,
    #[case] field: &str,
    #[case] value: Value,
) {
    let found = effects(&spell(line));
    assert_eq!(found[0]["Modal"][field], value, "{found}");
}

#[rstest]
#[case::several_at_random(
    "Charm | Choices$ DBA,DBB,DBC | CharmNum$ 2 | Random$ True",
    "key:Charm.Random-count"
)]
#[case::another_chooser("Charm | Choices$ DBA,DBB | Chooser$ You", "key:Charm.Chooser=You")]
#[case::x_modes(
    "Charm | Choices$ DBA,DBB | CharmNum$ X | Announce$ X",
    "key:Charm.Announce"
)]
#[case::unique_without_the_rule(
    "Charm | Choices$ DBP,DBQ | CharmNum$ 2",
    "key:Charm.DifferentPlayerPerMode"
)]
#[case::different_creatures(
    "Charm | Choices$ DBP,DBR | CharmNum$ 2 | AdditionalDescription$ Each mode must target a different player.",
    "key:Charm.DifferentPlayerPerMode"
)]
fn a_charm_beyond_the_nodes_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert!(
        gaps(&spell(line)).contains(&gap.to_owned()),
        "{:?}",
        gaps(&spell(line))
    );
}

#[rstest]
#[case::battlefield("", json!({"In": "Battlefield"}))]
#[case::graveyard(" | Zone$ Graveyard", json!({"In": "Graveyard"}))]
fn a_vote_on_objects_binds_the_most_voted(#[case] zone: &str, #[case] place: Value) {
    let found = effects(&spell(&format!(
        "Vote | Defined$ Player | VoteCard$ Creature.YouDontCtrl | VoteMessage$ for a creature | VoteSubAbility$ DBX{zone}"
    )));
    assert_eq!(
        found[0]["Vote"],
        json!({"options": {"Of": {"Each": [{"Type": "Creature"}, {"Not": {"ControlledBy": "You"}}, place]}}, "as": "voted"})
    );
    assert_eq!(found[1]["Exile"]["what"], json!({"Bound": "voted"}));
}

#[rstest]
#[case::up_to(
    "Vote | Defined$ Player | VoteCard$ Creature | UpTo$ True | VoteSubAbility$ DBX",
    "key:Vote.UpTo"
)]
#[case::other_zone(
    "Vote | Defined$ Player | VoteCard$ Card | Zone$ Library | VoteSubAbility$ DBX",
    "key:Vote.Zone=Library"
)]
#[case::no_follow_up(
    "Vote | Defined$ Player | VoteCard$ Creature",
    "key:Vote.VoteCard-shape"
)]
#[case::one_choice("Vote | Defined$ Player | Choices$ DBA", "key:Vote.shape")]
#[case::a_choice_runs_another(
    "Vote | Defined$ Player | Choices$ DBT,DBA | StoreVoteNum$ True",
    "key:Vote.Choices-chained"
)]
#[case::sentence_name(
    "Vote | Defined$ Player | Choices$ DBA,DBLong",
    "key:Vote.Choices-name"
)]
fn a_vote_beyond_the_nodes_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert!(
        gaps(&spell(line)).contains(&gap.to_owned()),
        "{:?}",
        gaps(&spell(line))
    );
}

#[test]
fn each_choice_with_the_most_votes_runs() {
    let found = effects(&spell("Vote | Defined$ Player | Choices$ DBA,DBB,DBRed"));
    assert_eq!(
        found[0]["Vote"]["options"],
        json!({"Named": ["alpha", "beta", "redhorn_pass"]})
    );
    let most = |name: &str, other: &str| json!({"Compare": {"left": {"Players": {"Bound": name}}, "op": "Ge", "right": {"Players": {"Bound": other}}}});
    assert_eq!(
        found[1]["If"]["condition"],
        json!({"All": [most("alpha", "beta"), most("alpha", "redhorn_pass")]})
    );
    assert_eq!(
        found[3]["If"]["condition"],
        json!({"All": [most("redhorn_pass", "alpha"), most("redhorn_pass", "beta")]})
    );
    assert!(found[2]["If"]["then"][0].get("Draw").is_some());
}

#[rstest]
#[case::an_opponent("Choices$ Player.Opponent", "Opponents")]
#[case::a_player("Choices$ Player", "EachPlayer")]
fn a_player_at_random_is_picked_by_the_game(#[case] choices: &str, #[case] from: &str) {
    let found = effects(&spell(&format!(
        "ChoosePlayer | Defined$ You | {choices} | Random$ True | SubAbility$ Hit\nSVar:Hit:DB$ DealDamage | Defined$ ChosenPlayer | NumDmg$ 2"
    )));
    assert_eq!(
        found[0],
        json!({"ChooseAtRandom": {"from": from, "as": "player"}})
    );
    assert_eq!(found[1]["DealDamage"]["to"], json!({"Bound": "player"}));
}

#[rstest]
#[case::permanent_type(
    "Type$ Card | InvalidTypes$ Instant,Sorcery,Kindred",
    json!({"CardTypeOf": ["Artifact", "Battle", "Creature", "Enchantment", "Land", "Planeswalker"]})
)]
#[case::creature_or_land("Type$ Card | ValidTypes$ Creature,Land", json!({"CardTypeOf": ["Creature", "Land"]}))]
#[case::not_a_wall("Type$ Creature | InvalidTypes$ Wall", json!({"CreatureTypeExcept": ["Wall"]}))]
fn a_type_choice_keeps_its_limits(#[case] keys: &str, #[case] kind: Value) {
    let found = effects(&spell(&format!("ChooseType | Defined$ You | {keys}")));
    assert_eq!(found[0]["ChooseValue"]["kind"], kind);
}

#[rstest]
#[case::not_a_type("Type$ Card | ValidTypes$ Land,Nonland", "key:ChooseType.ValidTypes")]
#[case::one_type("Type$ Card | ValidTypes$ Land", "key:ChooseType.ValidTypes")]
#[case::creature_valid("Type$ Creature | ValidTypes$ Elf,Goblin", "key:ChooseType.ValidTypes")]
#[case::land_invalid(
    "Type$ Basic Land | InvalidTypes$ Plains",
    "key:ChooseType.InvalidTypes"
)]
fn a_type_choice_beyond_the_kinds_stays_a_gap(#[case] keys: &str, #[case] gap: &str) {
    let script = spell(&format!("ChooseType | Defined$ You | {keys}"));
    assert!(
        gaps(&script).contains(&gap.to_owned()),
        "{:?}",
        gaps(&script)
    );
}

#[rstest]
#[case::at_most("totalPT_LE5", "Le", json!(5))]
#[case::exactly("totalPT_EQ4", "Eq", json!(4))]
fn total_power_and_toughness_adds_the_two(
    #[case] word: &str,
    #[case] op: &str,
    #[case] right: Value,
) {
    let found = abilities(&format!(
        "Name:T\nTypes:Instant\nA:SP$ Destroy | ValidTgts$ Creature.{word}\n"
    ));
    let stat = |kind: &str| json!({"Stat": {"of": "Candidate", "stat": kind}});
    assert_eq!(
        found[0]["Spell"]["targets"][0]["what"]["Object"][1],
        json!({"Test": {"left": {"Sum": [stat("Power"), stat("Toughness")]}, "op": op, "right": right}})
    );
}

#[rstest]
#[case::may_vote_again("Affected$ You | AdditionalOptionalVote$ 1", None)]
#[case::two_more(
    "Affected$ You | AdditionalOptionalVote$ 2",
    Some("key:Static.Continuous.AdditionalOptionalVote")
)]
#[case::for_opponents(
    "Affected$ Opponent | AdditionalOptionalVote$ 1",
    Some("key:Static.Continuous.AdditionalOptionalVote")
)]
fn an_additional_vote_is_the_vote_again_rule(#[case] keys: &str, #[case] gap: Option<&str>) {
    let script =
        format!("Name:T\nTypes:Creature\nPT:1/1\nS:Mode$ Continuous | {keys} | Description$ D\n");
    match gap {
        None => assert_eq!(
            abilities(&script)[0],
            json!({"Rule": {"rule": {"VoteAgain": "You"}}})
        ),
        Some(gap) => assert!(
            gaps(&script).contains(&gap.to_owned()),
            "{:?}",
            gaps(&script)
        ),
    }
}

#[test]
fn the_source_attacks_the_player_picked_at_random() {
    let script = "Name:T\nTypes:Creature\nPT:1/1\n\
        T:Mode$ Phase | Phase$ BeginCombat | ValidPlayer$ You | Execute$ Pick | TriggerZones$ Battlefield | TriggerDescription$ D\n\
        SVar:Pick:DB$ ChoosePlayer | Defined$ You | Choices$ Player.Opponent | Random$ True | SubAbility$ Must\n\
        SVar:Must:DB$ Effect | RememberObjects$ Self | ExileOnMoved$ Battlefield | StaticAbilities$ Attack | Duration$ UntilEndOfCombat\n\
        SVar:Attack:Mode$ MustAttack | ValidCreature$ Card.IsRemembered | MustAttack$ ChosenPlayer | Description$ D\n";
    assert_eq!(gaps(script), Vec::<String>::new());
    let found = abilities(script);
    assert_eq!(
        found[0]["Triggered"]["effects"][1]["Rule"]["rule"],
        json!({"MustAttackPlayers": {"who": "This", "players": {"Bound": "player"}}})
    );
}
