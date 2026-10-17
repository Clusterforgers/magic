//! Goal: mana value ranges, color match modes and format legality filter correctly over the recorded card fixture, including empty color sets and open or inverted ranges.

mod common;

use mtg_db::ColorMatch;
use sqlx::PgPool;

use common::search::{ALL_DECK_CARDS, COLORLESS, Case, colors_are, found, legal, mana, run};
use common::{repo, seeded};

fn mana_value_cases() -> Vec<Case> {
    vec![
        found(
            "at least four",
            mana(Some(4.0), None),
            &[
                "Atraxa, Praetors' Voice",
                "Elspeth, Sun's Champion // Elspeth, Sun's Champion",
                "Fire // Ice",
                "Jace, the Mind Sculptor",
            ],
        ),
        found(
            "at most zero",
            mana(None, Some(0.0)),
            &["Forest", "Island", "Mountain", "Mystery Card"],
        ),
        found("exactly half", mana(Some(0.5), Some(0.5)), &["Little Girl"]),
        found(
            "exactly one",
            mana(Some(1.0), Some(1.0)),
            &[
                "Æther Vial",
                "Delver of Secrets // Insectile Aberration",
                "Lightning Bolt",
                "Pyroblast",
                "Shock",
                "Sol Ring",
            ],
        ),
        found(
            "two to three inclusive",
            mana(Some(2.0), Some(3.0)),
            &[
                "Arcane Signet",
                "Bonecrusher Giant // Stomp",
                "Hazmat Suit (Used)",
                "Lim-Dûl's Vault",
                "Valakut Awakening // Valakut Stoneforge",
            ],
        ),
        found(
            "fractions below the bound",
            mana(None, Some(0.4)),
            &["Forest", "Island", "Mountain", "Mystery Card"],
        ),
        found("above everything", mana(Some(7.0), None), &[]),
        found(
            "inverted range matches nothing",
            mana(Some(5.0), Some(3.0)),
            &[],
        ),
        found(
            "not-a-number bounds are ignored",
            mana(Some(f64::NAN), Some(f64::NAN)),
            &ALL_DECK_CARDS,
        ),
        found(
            "infinite bounds are open",
            mana(Some(f64::NEG_INFINITY), Some(f64::INFINITY)),
            &ALL_DECK_CARDS,
        ),
    ]
}

fn color_cases() -> Vec<Case> {
    use ColorMatch::{Exactly, Includes, Overlaps, Within};
    let red = &[
        "Bonecrusher Giant // Stomp",
        "Lightning Bolt",
        "Pyroblast",
        "Shock",
        "Valakut Awakening // Valakut Stoneforge",
    ];
    vec![
        found("exactly red", colors_are("R", Exactly), red),
        found("exactly colorless", colors_are("", Exactly), &COLORLESS),
        found(
            "exactly two colors",
            colors_are("UR", Exactly),
            &["Fire // Ice"],
        ),
        found(
            "exactly blue",
            colors_are("U", Exactly),
            &[
                "Delver of Secrets // Insectile Aberration",
                "Jace, the Mind Sculptor",
            ],
        ),
        found(
            "exactly four colors",
            colors_are("WUBG", Exactly),
            &["Atraxa, Praetors' Voice"],
        ),
        found("exactly all five", colors_are("WUBRG", Exactly), &[]),
        found(
            "includes blue",
            colors_are("U", Includes),
            &[
                "Atraxa, Praetors' Voice",
                "Delver of Secrets // Insectile Aberration",
                "Fire // Ice",
                "Jace, the Mind Sculptor",
                "Lim-Dûl's Vault",
            ],
        ),
        found(
            "includes blue and black",
            colors_are("UB", Includes),
            &["Atraxa, Praetors' Voice", "Lim-Dûl's Vault"],
        ),
        found(
            "includes nothing is everything",
            colors_are("", Includes),
            &ALL_DECK_CARDS,
        ),
        found("includes all five", colors_are("WUBRG", Includes), &[]),
        found(
            "within blue-red",
            colors_are("UR", Within),
            &[
                &COLORLESS[..],
                &[
                    "Delver of Secrets // Insectile Aberration",
                    "Jace, the Mind Sculptor",
                    "Fire // Ice",
                ],
                red,
            ]
            .concat(),
        ),
        found(
            "within nothing is colorless",
            colors_are("", Within),
            &COLORLESS,
        ),
        found(
            "within all five is everything",
            colors_are("WUBRG", Within),
            &ALL_DECK_CARDS,
        ),
        found(
            "overlaps white-black",
            colors_are("WB", Overlaps),
            &[
                "Atraxa, Praetors' Voice",
                "Elspeth, Sun's Champion // Elspeth, Sun's Champion",
                "Lim-Dûl's Vault",
                "Little Girl",
            ],
        ),
        found(
            "overlaps red",
            colors_are("R", Overlaps),
            &[&red[..], &["Fire // Ice"]].concat(),
        ),
        found(
            "overlaps nothing matches nothing",
            colors_are("", Overlaps),
            &[],
        ),
    ]
}

fn legality_cases() -> Vec<Case> {
    vec![
        found(
            "commander",
            legal("commander"),
            &[
                "Æther Vial",
                "Arcane Signet",
                "Atraxa, Praetors' Voice",
                "Bonecrusher Giant // Stomp",
                "Delver of Secrets // Insectile Aberration",
                "Fire // Ice",
                "Forest",
                "Island",
                "Jace, the Mind Sculptor",
                "Lightning Bolt",
                "Lim-Dûl's Vault",
                "Mountain",
                "Pyroblast",
                "Shock",
                "Sol Ring",
                "Valakut Awakening // Valakut Stoneforge",
            ],
        ),
        found(
            "format names ignore case and padding",
            legal("  CoMmAnDeR "),
            &[
                "Æther Vial",
                "Arcane Signet",
                "Atraxa, Praetors' Voice",
                "Bonecrusher Giant // Stomp",
                "Delver of Secrets // Insectile Aberration",
                "Fire // Ice",
                "Forest",
                "Island",
                "Jace, the Mind Sculptor",
                "Lightning Bolt",
                "Lim-Dûl's Vault",
                "Mountain",
                "Pyroblast",
                "Shock",
                "Sol Ring",
                "Valakut Awakening // Valakut Stoneforge",
            ],
        ),
        found(
            "modern",
            legal("modern"),
            &[
                "Æther Vial",
                "Bonecrusher Giant // Stomp",
                "Delver of Secrets // Insectile Aberration",
                "Fire // Ice",
                "Forest",
                "Island",
                "Lightning Bolt",
                "Mountain",
                "Pyroblast",
                "Shock",
                "Valakut Awakening // Valakut Stoneforge",
            ],
        ),
        found(
            "standard",
            legal("standard"),
            &["Forest", "Island", "Mountain"],
        ),
        found(
            "restricted counts as legal",
            legal("vintage"),
            &["Jace, the Mind Sculptor", "Lightning Bolt", "Sol Ring"],
        ),
        found(
            "banned does not",
            legal("legacy"),
            &[
                "Æther Vial",
                "Fire // Ice",
                "Lightning Bolt",
                "Lim-Dûl's Vault",
            ],
        ),
        found("unknown format", legal("no-such-format"), &[]),
        found("blank format is ignored", legal(" "), &ALL_DECK_CARDS),
    ]
}

#[sqlx::test]
async fn mana_value_filters(pool: PgPool) {
    seeded(pool.clone()).await;
    run(&repo(&pool), mana_value_cases()).await;
}

#[sqlx::test]
async fn color_filters(pool: PgPool) {
    seeded(pool.clone()).await;
    run(&repo(&pool), color_cases()).await;
}

#[sqlx::test]
async fn legality_filters(pool: PgPool) {
    seeded(pool.clone()).await;
    run(&repo(&pool), legality_cases()).await;
}
