//! Goal: Wheel of Misfortune's `ChooseNumber` of every player with `Highest$` and `NotLowest$` translates as the secret
//! numbers and the three effects that read them, exactly as the compiler reads the Oracle text, and a near miss of the
//! script stays a gap.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

const WHEEL: &str = "Name:Wheel of Misfortune\nManaCost:2 R R\nTypes:Sorcery\nA:SP$ ChooseNumber | Defined$ Player | Secretly$ True | Highest$ DBLoseLife | NotLowest$ DBDiscard | AILogic$ HighestLoseLife | StackDescription$ SpellDescription | SpellDescription$ Each player secretly chooses a number.\nSVar:DBLoseLife:DB$ DealDamage | Defined$ Remembered | NumDmg$ Count$ChosenNumber\nSVar:DBDiscard:DB$ Discard | Defined$ Remembered | Mode$ Hand | SubAbility$ DBDraw\nSVar:DBDraw:DB$ Draw | Defined$ Remembered | NumCards$ 7\n";

#[test]
fn dk27_wheel_of_misfortune_translates_as_the_secret_numbers() {
    assert_eq!(gaps(WHEEL), Vec::<String>::new());
    let not_lowest = json!({"PlayersWhere": {"who": "EachPlayer", "test": {"Not": {"Among": {"Bound": "lowest"}}}}});
    let hands = json!({"Count": [{"In": "Hand"}, {"OwnedBy": "EachPlayer"}]});
    assert_eq!(
        abilities(WHEEL)[0],
        json!({"Spell": {"effects": [
            {"ChooseNumbers": {"secret": true, "as": "number", "highest": "highest", "lowest": "lowest"}},
            {"DealDamage": {"to": {"Bound": "highest"}, "amount": {"ChosenAs": "number"}}},
            {"Discard": {"who": not_lowest, "count": hands}},
            {"Draw": {"who": not_lowest, "count": 7}},
        ]}})
    );
}

#[rstest]
#[case::one_player("Defined$ Player |", "Defined$ You |")]
#[case::secretly_false("Secretly$ True", "Secretly$ False")]
#[case::life_lost_instead(
    "DB$ DealDamage | Defined$ Remembered",
    "DB$ LoseLife | Defined$ Remembered"
)]
#[case::a_fixed_damage("NumDmg$ Count$ChosenNumber", "NumDmg$ 3")]
#[case::discard_a_card("Mode$ Hand", "NumCards$ 1")]
#[case::draw_x("NumCards$ 7", "NumCards$ X")]
#[case::another_key("Mode$ Hand |", "Mode$ Hand | Optional$ True |")]
fn dk27_a_near_miss_of_the_wheel_stays_a_gap(#[case] from: &str, #[case] to: &str) {
    let script = WHEEL.replacen(from, to, 1);
    assert_ne!(script, WHEEL, "{from}");
    assert!(!gaps(&script).is_empty(), "{script}");
}
