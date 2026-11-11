//! Goal: Wheel of Misfortune's `ChooseNumber` of every player with `Highest$` and `NotLowest$` translates as the secret
//! numbers and the three effects that read them, exactly as the compiler reads the Oracle text, and a near miss of the
//! script stays a gap; Pain's Reward's `BidLife` translates as the bidding and the loss and draw of the high bidder.
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

const PAINS_REWARD: &str = "Name:Pain's Reward\nManaCost:2 B\nTypes:Sorcery\nA:SP$ BidLife | StartBidding$ Any | AILogic$ Min | AIBidMax$ 10 | BidSubAbility$ DBLoseLife | SpellDescription$ Each player may bid life.\nSVar:DBLoseLife:DB$ LoseLife | Defined$ Remembered | LifeAmount$ X | SubAbility$ DBDraw\nSVar:DBDraw:DB$ Draw | Defined$ Remembered | NumCards$ 4\nSVar:X:Count$ChosenNumber\n";

#[test]
fn dk27_pains_reward_translates_as_the_bidding() {
    assert_eq!(gaps(PAINS_REWARD), Vec::<String>::new());
    let bidder = json!({"Bound": "bidder"});
    assert_eq!(
        abilities(PAINS_REWARD)[0],
        json!({"Spell": {"effects": [
            {"BidLife": {"as": "bid", "winner": "bidder"}},
            {"LoseLife": {"who": bidder, "amount": {"ChosenAs": "bid"}}},
            {"Draw": {"who": bidder, "count": 4}},
        ]}})
    );
}

#[rstest]
#[case::a_fixed_opening("StartBidding$ Any", "StartBidding$ 0")]
#[case::for_a_target("StartBidding$ Any |", "StartBidding$ Any | ValidTgts$ Creature |")]
#[case::another_amount("SVar:X:Count$ChosenNumber", "SVar:X:Count$YourLifeTotal")]
#[case::gain_instead("DB$ LoseLife", "DB$ GainLife")]
#[case::the_controller_draws("DB$ Draw | Defined$ Remembered", "DB$ Draw | Defined$ You")]
fn dk27_a_near_miss_of_the_bidding_stays_a_gap(#[case] from: &str, #[case] to: &str) {
    let script = PAINS_REWARD.replacen(from, to, 1);
    assert_ne!(script, PAINS_REWARD, "{from}");
    assert!(!gaps(&script).is_empty(), "{script}");
}
