//! Scores the decision-quality suite (`brain::suite`) per level: `bot_suite [levels...] [--timed] [--seed N]
//! [--only name]`, levels among easy, normal, hard, veryhard (all by default).
#![allow(clippy::print_stdout, clippy::unwrap_used)]

use std::collections::BTreeMap;

use mtg_core::ids::PlayerId;
use mtg_testkit::brain::suite::{Theme, cases, run};
use mtg_testkit::brain::{Level, brain};

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
    let mut levels: Vec<Level> = args
        .iter()
        .filter_map(|a| match a.as_str() {
            "easy" => Some(Level::Easy),
            "normal" => Some(Level::Normal),
            "hard" => Some(Level::Hard),
            "veryhard" => Some(Level::VeryHard),
            _ => None,
        })
        .collect();
    if levels.is_empty() {
        levels = vec![Level::Easy, Level::Normal, Level::Hard, Level::VeryHard];
    }
    let all = cases();
    for level in levels {
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
            let mut bot = brain(level, seed, PlayerId::from_index(0), timed);
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
            "{level:?}: {passed}/{total} ({:.0}%), longest answer {} ms; {}",
            100.0 * f64::from(passed) / f64::from(total.max(1)),
            longest.as_millis(),
            themes.join(", ")
        );
        println!("  failed: {}", failed.join(" "));
    }
}
