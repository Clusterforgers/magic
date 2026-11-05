//! Goal: votes on named choices and clashes translate to their nodes, and a vote shape or key the nodes cannot say stays a named gap.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn spell(line: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {line}\nSVar:DBA:DB$ GainLife | LifeAmount$ 2 | SpellDescription$ Alpha\nSVar:DBB:DB$ Draw | SpellDescription$ Beta\nSVar:DBC:DB$ Draw | NumCards$ VoteNum | SpellDescription$ Gamma\nSVar:DBD:DB$ GainLife | LifeAmount$ VoteNum | SpellDescription$ Delta\n"
    )
}

#[rstest]
#[case::tied(
    "Vote | Defined$ Player | Choices$ DBA,DBB | VoteTiedAbility$ DBB",
    "If"
)]
#[case::counted(
    "Vote | Defined$ Player | Choices$ DBD,DBC | StoreVoteNum$ True",
    "Draw"
)]
#[case::each(
    "Vote | Defined$ Player | Choices$ DBA,DBB | EachVote$ True",
    "RepeatEach"
)]
fn a_vote_maps_to_the_node_of_its_shape(#[case] line: &str, #[case] node: &str) {
    let script = spell(line);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let found = abilities(&script);
    let effects = &found[0]["Spell"]["effects"];
    let names = match node {
        "Draw" => json!(["delta", "gamma"]),
        _ => json!(["alpha", "beta"]),
    };
    assert_eq!(effects[0]["Vote"]["options"], json!({"Named": names}));
    assert!(
        effects
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e.get(node).is_some())
    );
}

#[test]
fn a_tie_goes_to_the_named_choice() {
    let found = abilities(&spell(
        "Vote | Defined$ Player | Choices$ DBA,DBB | VoteTiedAbility$ DBA",
    ));
    let branch = &found[0]["Spell"]["effects"][1]["If"];
    assert_eq!(
        branch["condition"]["Compare"]["left"],
        json!({"Players": {"Bound": "beta"}})
    );
    assert!(branch["otherwise"][0].get("GainLife").is_some());
}

#[rstest]
#[case::no_rule("Vote | Defined$ Player | Choices$ DBA,DBB", "key:Vote.shape")]
#[case::count_unread(
    "Vote | Defined$ Player | Choices$ DBA,DBB | StoreVoteNum$ True",
    "key:Vote.StoreVoteNum-unread"
)]
#[case::player_votes(
    "Vote | Defined$ Player | VotePlayer$ Other | StoreVoteNum$ True",
    "key:Vote.VotePlayer"
)]
#[case::tied_of_three(
    "Vote | Defined$ Player | Choices$ DBA,DBB,DBC | VoteTiedAbility$ DBB",
    "key:Vote.shape"
)]
#[case::tied_unknown(
    "Vote | Defined$ Player | Choices$ DBA,DBB | VoteTiedAbility$ DBC",
    "key:Vote.VoteTiedAbility"
)]
#[case::voters(
    "Vote | Defined$ You | Choices$ DBA,DBB | EachVote$ True",
    "key:Vote.Defined"
)]
fn a_vote_beyond_the_nodes_stays_a_gap(#[case] line: &str, #[case] gap: &str) {
    assert_eq!(gaps(&spell(line)), [gap]);
}

#[test]
fn a_clash_keeps_its_win_and_otherwise_parts() {
    let found = abilities(&spell(
        "Clash | WinSubAbility$ DBA | OtherwiseSubAbility$ DBB",
    ));
    let clash = &found[0]["Spell"]["effects"][0]["Clash"];
    assert!(clash["win"][0].get("GainLife").is_some());
    assert!(clash["lose"][0].get("Draw").is_some());
}

#[test]
fn a_clash_with_a_remembered_opponent_is_a_gap() {
    assert_eq!(
        gaps(&spell("Clash | WinSubAbility$ DBA | RememberClasher$ True")),
        ["key:Clash.RememberClasher"]
    );
}

fn blocker(line: &str, trigger: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Bear\nPT:2/2\n{trigger}A:AB$ MustBlock | Cost$ G | {line}\nSVar:Trig:DB$ MustBlock | {line}\n"
    )
}

#[rstest]
#[case::ability("ValidTgts$ Creature", "", "UntilEndOfTurn")]
#[case::until_combat(
    "ValidTgts$ Creature | Duration$ UntilEndOfCombat",
    "",
    "UntilEndOfCombat"
)]
fn a_must_block_ability_forces_a_block_of_the_source(
    #[case] line: &str,
    #[case] trigger: &str,
    #[case] duration: &str,
) {
    let script = blocker(line, trigger);
    assert_eq!(gaps(&script), Vec::<String>::new());
    let found = abilities(&script);
    let rule = &found[0]["Activated"]["effects"][0]["Rule"];
    assert_eq!(
        rule["rule"],
        json!({"MustBlockAttacker": {"who": {"Target": 0}, "attacker": "This"}})
    );
    assert_eq!(rule["duration"], json!(duration));
}

#[rstest]
#[case::attack_of_self(
    "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ Trig | TriggerZones$ Battlefield\n",
    true
)]
#[case::attack_of_another(
    "T:Mode$ Attacks | ValidCard$ Creature.Other+YouCtrl | Execute$ Trig | TriggerZones$ Battlefield\n",
    false
)]
#[case::damage(
    "T:Mode$ DamageDone | ValidSource$ Card.Self | Execute$ Trig | TriggerZones$ Battlefield\n",
    false
)]
fn a_must_block_trigger_names_the_source_only_when_it_attacks(
    #[case] trigger: &str,
    #[case] reads: bool,
) {
    let script = blocker("ValidTgts$ Creature", trigger)
        .replace("A:AB$ MustBlock | Cost$ G | ValidTgts$ Creature\n", "");
    let failed = gaps(&script)
        .iter()
        .any(|gap| gap == "key:MustBlock.attacker");
    assert_eq!(failed, !reads, "{:?}", gaps(&script));
}

#[rstest]
#[case::attacker_named(
    "DefinedAttacker$ ParentTarget | ValidTgts$ Creature",
    "key:MustBlock.DefinedAttacker"
)]
#[case::all(
    "ValidTgts$ Creature | BlockAllDefined$ True",
    "key:MustBlock.BlockAllDefined"
)]
#[case::no_target("Duration$ UntilEndOfCombat", "key:MustBlock.no-target")]
fn a_must_block_beyond_the_node_is_a_gap(#[case] line: &str, #[case] gap: &str) {
    let script = blocker(line, "");
    assert!(
        gaps(&script).iter().any(|g| g == gap),
        "{:?}",
        gaps(&script)
    );
}
