//! Scores the decision-quality suite (`brain::suite`) per level: `bot_suite [levels...] [--timed] [--seed N]
//! [--only name]`, levels among easy, normal, hard, veryhard (all by default), or `old-<level>` for main's frozen
//! brains (`legacy_brain`).
#![allow(clippy::print_stdout, clippy::unwrap_used)]

#[allow(dead_code, clippy::all, clippy::pedantic)]
#[path = "legacy_brain/mod.rs"]
mod legacy;

use std::collections::BTreeMap;

use mtg_core::ids::PlayerId;
use mtg_testkit::brain::suite::{Theme, cases, run};
use mtg_testkit::brain::{Level, brain};

/// A brain of this branch or of main's frozen copy.
#[derive(Clone, Copy)]
enum Who {
    New(Level),
    Old(legacy::Level),
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let timed = args.iter().any(|a| a == "--timed");
    let seed = args
        .iter()
        .position(|a| a == "--seed")
        .and_then(|at| args.get(at + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(7);
    let only = args
        .iter()
        .position(|a| a == "--only")
        .and_then(|at| args.get(at + 1))
        .cloned();
    let verbose = args.iter().any(|a| a == "-v");
    let pick = |name: &str| match name {
        "easy" => Some(Who::New(Level::Easy)),
        "normal" => Some(Who::New(Level::Normal)),
        "hard" => Some(Who::New(Level::Hard)),
        "veryhard" => Some(Who::New(Level::VeryHard)),
        "old-normal" => Some(Who::Old(legacy::Level::Normal)),
        "old-hard" => Some(Who::Old(legacy::Level::Hard)),
        "old-veryhard" => Some(Who::Old(legacy::Level::VeryHard)),
        _ => None,
    };
    let mut levels: Vec<(String, Who)> = args
        .iter()
        .filter_map(|a| pick(a).map(|who| (a.clone(), who)))
        .collect();
    if levels.is_empty() {
        levels = ["easy", "normal", "hard", "veryhard"]
            .into_iter()
            .filter_map(|name| pick(name).map(|who| (name.to_owned(), who)))
            .collect();
    }
    let all = cases();
    for (level, who) in levels {
        let mut passed = 0;
        let mut total = 0;
        let mut themes: BTreeMap<Theme, (u32, u32)> = BTreeMap::new();
        let mut failed: Vec<&str> = Vec::new();
        let mut longest = std::time::Duration::ZERO;
        for case in &all {
            if only
                .as_ref()
                .is_some_and(|o| !case.name.contains(o.as_str()))
            {
                continue;
            }
            let seat = PlayerId::from_index(0);
            let mut bot = match who {
                Who::New(level) => brain(level, seed, seat, timed),
                Who::Old(level) => legacy::brain(level, seed, seat, timed),
            };
            match run(case, bot.as_mut()) {
                Ok(verdict) => {
                    total += 1;
                    longest = longest.max(verdict.longest);
                    let entry = themes.entry(case.theme).or_default();
                    entry.1 += 1;
                    if verdict.passed {
                        passed += 1;
                        entry.0 += 1;
                    } else {
                        failed.push(case.name);
                    }
                    if verbose {
                        println!(
                            "  {} {} ({} ms)",
                            if verdict.passed { "ok  " } else { "FAIL" },
                            case.name,
                            verdict.longest.as_millis()
                        );
                    }
                }
                Err(error) => println!("  SETUP {}: {error}", case.name),
            }
        }
        let themes: Vec<String> = themes
            .iter()
            .map(|(theme, (p, t))| format!("{theme:?} {p}/{t}"))
            .collect();
        println!(
            "{level}: {passed}/{total} ({:.0}%), longest answer {} ms; {}",
            100.0 * f64::from(passed) / f64::from(total.max(1)),
            longest.as_millis(),
            themes.join(", ")
        );
        println!("  failed: {}", failed.join(" "));
    }
}
