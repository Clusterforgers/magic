//! Goal: a copy that gains a triggered, an activated or a static ability as an exception, a static that adds the
//! creature type or sets the colour chosen as the card entered, a keyword lost for good ("can't have or gain"), every
//! basic land type and the combat restrictions of an animation translate to the nodes that say them; a shape the
//! nodes cannot say stays a named gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

const CHOOSE_TYPE: &str =
    "K:ETBReplacement:Other:ChooseCT\nSVar:ChooseCT:DB$ ChooseType | Type$ Creature\n";
const CHOOSE_COLOR: &str =
    "K:ETBReplacement:Other:ChooseColor\nSVar:ChooseColor:DB$ ChooseColor | Defined$ You\n";

fn statics(script: &str) -> Vec<Value> {
    abilities(script)
        .into_iter()
        .filter_map(|ability| ability.get("Static").cloned())
        .collect()
}

#[rstest]
#[case::chosen_type(
    CHOOSE_TYPE,
    "AffectedDefined$ Self | AddType$ ChosenType",
    json!("This"),
    json!(["AddChosenSubtype"])
)]
#[case::chosen_type_of_a_group(
    CHOOSE_TYPE,
    "Affected$ Creature.YouCtrl | AddType$ ChosenType",
    json!({"Each": [{"Type": "Creature"}, {"ControlledBy": "You"}]}),
    json!(["AddChosenSubtype"])
)]
#[case::chosen_color(
    CHOOSE_COLOR,
    "Affected$ Permanent.nonLand | SetColor$ ChosenColor",
    json!({"Each": [{"Not": {"Type": "Land"}}]}),
    json!(["SetChosenColor"])
)]
#[case::every_basic_land_type(
    "",
    "Affected$ Land.YouCtrl | AddType$ AllBasicLandType",
    json!({"Each": [{"Type": "Land"}, {"ControlledBy": "You"}]}),
    json!([{"AddSubtype": "Plains"}, {"AddSubtype": "Island"}, {"AddSubtype": "Swamp"}, {"AddSubtype": "Mountain"}, {"AddSubtype": "Forest"}])
)]
#[case::locked_keyword(
    "",
    "Affected$ Creature.OppCtrl | RemoveKeyword$ Trample | CantHaveKeyword$ Trample",
    json!({"Each": [{"Type": "Creature"}, {"ControlledBy": "Opponents"}]}),
    json!([{"LockKeyword": "Trample"}])
)]
fn a_continuous_line_is_its_modification(
    #[case] choice: &str,
    #[case] line: &str,
    #[case] affected: Value,
    #[case] mods: Value,
) {
    let script = format!("Name:T\nTypes:Artifact\n{choice}S:Mode$ Continuous | {line}\n");
    let found = statics(&script);
    assert_eq!(found.len(), 1, "{:?}", gaps(&script));
    assert_eq!(found[0]["affected"], affected);
    assert_eq!(found[0]["mods"], mods);
}

#[rstest]
#[case::type_without_a_choice(
    "",
    "AffectedDefined$ Self | AddType$ ChosenType",
    "key:Static.Continuous.AddType=ChosenType"
)]
#[case::colour_without_a_choice(
    "",
    "AffectedDefined$ Self | SetColor$ ChosenColor",
    "key:Static.Continuous.SetColor=ChosenColor"
)]
#[case::chosen_type_replacing(
    CHOOSE_TYPE,
    "AffectedDefined$ Self | AddType$ ChosenType | RemoveCreatureTypes$ True",
    "key:Static.Continuous.AddType=ChosenType"
)]
#[case::locked_but_not_lost(
    "",
    "Affected$ Creature.OppCtrl | CantHaveKeyword$ Trample",
    "key:Static.Continuous.CantHaveKeyword-without-RemoveKeyword"
)]
#[case::locked_another_keyword(
    "",
    "Affected$ Creature.OppCtrl | RemoveKeyword$ Flying | CantHaveKeyword$ Trample",
    "key:Static.Continuous.CantHaveKeyword-without-RemoveKeyword"
)]
fn a_continuous_shape_the_nodes_cannot_say_is_a_gap(
    #[case] choice: &str,
    #[case] line: &str,
    #[case] gap: &str,
) {
    let script = format!("Name:T\nTypes:Artifact\n{choice}S:Mode$ Continuous | {line}\n");
    assert!(
        gaps(&script).iter().any(|found| found == gap),
        "{:?}",
        gaps(&script)
    );
}

const SAC: &str = "SVar:Sac:Mode$ BecomesTarget | ValidTarget$ Card.Self | Execute$ TrigSac | TriggerDescription$ When this creature becomes the target of a spell or ability, sacrifice it.\nSVar:TrigSac:DB$ Sacrifice | Defined$ Self\n";
const BOUNCE: &str = "SVar:Bounce:AB$ ChangeZone | Cost$ 2 U U | Defined$ Self | Origin$ Battlefield | Destination$ Hand | SpellDescription$ Return this creature to its owner's hand.\n";

#[rstest]
#[case::a_trigger("AddTriggers$ Sac | AddSVars$ TrigSac", SAC, "Triggered")]
#[case::an_activated_ability("AddAbilities$ Bounce", BOUNCE, "Activated")]
fn a_copy_gains_the_named_ability(#[case] keys: &str, #[case] parts: &str, #[case] kind: &str) {
    let script = format!(
        "Name:T\nTypes:Creature Shapeshifter\nPT:0/0\nK:ETBReplacement:Copy:DBCopy:Optional\nSVar:DBCopy:DB$ Clone | Choices$ Creature.Other | {keys}\n{parts}"
    );
    let found = abilities(&script);
    let except = &found[0]["Replacement"]["by"]["AsCopy"]["except"];
    assert!(
        except[0]["GrantAbility"][kind].is_object(),
        "{found:?} {:?}",
        gaps(&script)
    );
}

#[test]
fn a_copy_that_becomes_one_keeps_its_ability_and_needs_no_parts() {
    let script = "Name:T\nTypes:Creature Shapeshifter\nPT:0/0\nT:Mode$ Phase | Phase$ Upkeep | ValidPlayer$ You | TriggerZones$ Battlefield | Execute$ TrigCopy | TriggerDescription$ At the beginning of your upkeep, you may have CARDNAME become a copy of another target creature, except it has this ability.\nSVar:TrigCopy:DB$ Clone | ValidTgts$ Creature.Other | Optional$ True | GainThisAbility$ True | AddSVars$ TrigCopy\n";
    let found = abilities(script);
    let copy = &found[0]["Triggered"]["effects"][0]["May"]["effects"][0]["BecomeCopy"];
    assert_eq!(copy["except"], json!(["ThisAbility"]), "{:?}", gaps(script));
}

#[rstest]
#[case::cant_block("CARDNAME can't block.", Some(json!(["CantBlock"])))]
#[case::cant_attack_or_block("CARDNAME can't attack or block.", Some(json!(["CantAttack", "CantBlock"])))]
#[case::must_be_blocked("CARDNAME must be blocked if able.", None)]
fn an_animation_restricts_combat(#[case] word: &str, #[case] mods: Option<Value>) {
    let script = format!(
        "Name:T\nTypes:Instant\nA:SP$ Animate | ValidTgts$ Creature | HiddenKeywords$ {word}\n"
    );
    match mods {
        Some(mods) => {
            let found = abilities(&script);
            assert_eq!(
                found[0]["Spell"]["effects"][0]["Continuous"]["mods"],
                mods,
                "{:?}",
                gaps(&script)
            );
        }
        None => assert!(
            gaps(&script)
                .iter()
                .any(|gap| gap == "key:Animate.HiddenKeywords")
        ),
    }
}

const FIGHT: &str = "SVar:TrigFight:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ TrigFightBis | TriggerDescription$ When CARDNAME enters, it fights up to one target creature you don't control.\nSVar:TrigFightBis:DB$ Fight | Defined$ TriggeredCardLKICopy | ValidTgts$ Creature.YouDontCtrl | TargetMin$ 0 | TargetMax$ 1\n";

#[test]
fn the_ability_a_token_copy_gains_keeps_its_targets_apart() {
    let script = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ CopyPermanent | ValidTgts$ Creature.YouCtrl | AddTriggers$ TrigFight | AddSVars$ TrigFight,TrigFightBis\n{FIGHT}"
    );
    let spell = &abilities(&script)[0]["Spell"];
    assert_eq!(
        spell["targets"],
        json!([{"what": {"Object": [{"Type": "Creature"}, {"ControlledBy": "You"}]}}]),
        "{:?}",
        gaps(&script)
    );
    let granted =
        &spell["effects"][0]["CreateToken"]["token"]["CopyOf"]["except"][0]["GrantAbility"];
    assert_eq!(
        granted["Triggered"]["targets"][0]["bounds"],
        json!({"min": 0, "max": 1})
    );
}

#[rstest]
#[case::chosen_permanent("Choices$ Enchantment.nonLegendary+YouCtrl", None)]
#[case::chosen_among_a_subject(
    "Defined$ Remembered | Choices$ Card",
    Some("key:CopyPermanent.Choices-of-a-subject")
)]
fn a_token_copies_a_chosen_permanent(#[case] keys: &str, #[case] gap: Option<&str>) {
    let script = format!("Name:T\nTypes:Sorcery\nA:SP$ CopyPermanent | {keys}\n");
    match gap {
        None => {
            let effects = &abilities(&script)[0]["Spell"]["effects"];
            let name = effects[0]["Choose"]["as"].clone();
            assert_eq!(
                effects[0]["Choose"]["from"],
                json!({"Each": [{"Type": "Enchantment"}, {"Not": {"Supertype": "Legendary"}}, {"ControlledBy": "You"}]}),
                "{:?}",
                gaps(&script)
            );
            assert_eq!(
                effects[1]["CreateToken"]["token"]["CopyOf"]["of"],
                json!({ "Bound": name })
            );
        }
        Some(gap) => assert!(
            gaps(&script).iter().any(|found| found == gap),
            "{:?}",
            gaps(&script)
        ),
    }
}

#[rstest]
#[case::while_you_control(
    "That creature can't attack or block for as long as you control T.",
    Some("WhileYouControlSource")
)]
#[case::while_it_remains(
    "For as long as T remains on the battlefield, that creature can't attack or block.",
    Some("WhileSourceOnBattlefield")
)]
#[case::both(
    "For as long as T remains on the battlefield, that creature can't block. That creature can't attack for as long as you control T.",
    None
)]
fn an_animation_until_the_host_leaves_follows_the_oracle(
    #[case] oracle: &str,
    #[case] duration: Option<&str>,
) {
    let script = format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Animate | ValidTgts$ Creature | HiddenKeywords$ CARDNAME can't attack or block. | Duration$ UntilHostLeavesPlay\nOracle:{oracle}\n"
    );
    match duration {
        Some(duration) => assert_eq!(
            abilities(&script)[0]["Spell"]["effects"][0]["Continuous"]["duration"],
            json!(duration),
            "{:?}",
            gaps(&script)
        ),
        None => assert!(
            gaps(&script)
                .iter()
                .any(|gap| gap == "key:Animate.Duration-while-you-control-or-remains")
        ),
    }
}
