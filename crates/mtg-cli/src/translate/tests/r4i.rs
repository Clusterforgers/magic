//! Goal: the words of an ability a static ability gives (`OriginalHost`) name the permanent that gave it, in a defined line, a count, a filter and a cost, and read as a gap anywhere else.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn equipment(granted: &str, svar: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact Equipment\nK:Equip:1\nS:Mode$ Continuous | AffectedDefined$ Equipped | AddAbility$ Gift | Description$ x\nSVar:Gift:{granted}\n{svar}"
    )
}

#[rstest]
#[case::defined(
    "AB$ PutCounter | Cost$ T | Defined$ OriginalHost | CounterType$ BLOOD | CounterNum$ 1",
    ""
)]
#[case::amount(
    "AB$ DealDamage | Cost$ T | ValidTgts$ Any | NumDmg$ X",
    "SVar:X:OriginalHost$CardCounters.ARROW\n"
)]
#[case::sacrificed("AB$ Draw | Cost$ 1 T Sac<1/OriginalHost/T> | NumCards$ 1", "")]
fn the_original_host_of_a_given_ability_is_the_granter(#[case] line: &str, #[case] svar: &str) {
    let text = equipment(line, svar);
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    assert!(
        serde_json::to_string(&abilities(&text))
            .unwrap()
            .contains("Granter"),
        "{:?}",
        abilities(&text)
    );
}

#[test]
fn a_sacrifice_of_the_host_picks_the_granter_alone() {
    let text = equipment("AB$ Draw | Cost$ Sac<1/OriginalHost/T> | NumCards$ 1", "");
    let found = abilities(&text);
    assert!(
        serde_json::to_string(&found)
            .unwrap()
            .contains(&json!({"Sacrifice": {"filter": [{"Is": "Granter"}]}}).to_string()),
        "{found:?}"
    );
}

#[rstest]
#[case::counters_removed("AB$ Draw | Cost$ T SubCounter<All/AIM/OriginalHost/T> | NumCards$ 1")]
#[case::outside_a_given_ability("AB$ Draw | Cost$ Sac<1/OriginalHost/T> | NumCards$ 1")]
fn other_uses_of_the_host_stay_gaps(#[case] line: &str) {
    let text = if line.contains("SubCounter") {
        equipment(line, "")
    } else {
        format!("Name:T\nTypes:Artifact\nA:{line}\n")
    };
    assert!(!gaps(&text).is_empty(), "{:?}", abilities(&text));
}

#[rstest]
#[case::plain("{1}{R}", "2", true)]
#[case::with_x("{X}{R}", "2", false)]
#[case::not_mana(
    "{1}{R}",
    "tapXType<3/Creature.withFlying/creatures with flying>",
    false
)]
fn may_flash_cost_is_a_way_to_cast_for_the_cost_and_the_extra(
    #[case] printed: &str,
    #[case] extra: &str,
    #[case] mana: bool,
) {
    let words = printed.replace("}{", " ").replace(['{', '}'], "");
    let text = format!(
        "Name:T\nManaCost:{words}\nTypes:Sorcery\nK:MayFlashCost:{extra}\nA:SP$ GainLife | LifeAmount$ 2\n"
    );
    if mana {
        assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
        let found = serde_json::to_string(&abilities(&text)).unwrap();
        assert!(
            found.contains("AltCast") && found.contains("\"flash\":true"),
            "{found}"
        );
    } else {
        assert!(!gaps(&text).is_empty());
    }
}

#[test]
fn an_aura_that_could_enchant_the_source_is_one_predicate() {
    let text = "Name:T\nTypes:Creature Human\nPT:1/1\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Go | TriggerDescription$ x\nSVar:Go:DB$ ChangeZone | Origin$ Hand | Destination$ Battlefield | ChangeType$ Aura.CanEnchantSource | AttachedTo$ Self\n";
    assert!(gaps(text).is_empty(), "{:?}", gaps(text));
    assert!(
        serde_json::to_string(&abilities(text))
            .unwrap()
            .contains("CanEnchantSource")
    );
}

#[rstest]
#[case::defined(
    "A:SP$ DealDamage | ValidTgts$ Player,Planeswalker | NumDmg$ 2 | SubAbility$ D\nSVar:D:DB$ Draw | Defined$ TargetedOrController | NumCards$ 1"
)]
#[case::valid(
    "A:SP$ DamageAll | ValidTgts$ Player,Planeswalker | NumDmg$ 2 | ValidCards$ Creature.ControlledBy TargetedOrController"
)]
fn a_player_or_the_controller_of_the_target_is_one_reference(#[case] lines: &str) {
    let text = format!("Name:T\nTypes:Sorcery\n{lines}\n");
    assert!(gaps(&text).is_empty(), "{:?}", gaps(&text));
    assert!(
        serde_json::to_string(&abilities(&text))
            .unwrap()
            .contains(&json!({"OrController": {"Target": 0}}).to_string()),
        "{:?}",
        abilities(&text)
    );
}
