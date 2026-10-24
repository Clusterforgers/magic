//! Three colors in a row: "create a 1/1 red, green, and white Sand Warrior creature token." The renderers join the
//! colors of a token with "and" between each pair.

/// The five colors, as a token's name list words them.
const COLORS: [&str; 5] = ["white", "blue", "black", "red", "green"];

/// The text with each run of three colors joined as a list, if it has one.
fn listed(text: &str) -> Option<String> {
    let mut out = text.to_owned();
    for (a, first) in COLORS.iter().enumerate() {
        for (b, second) in COLORS.iter().enumerate().filter(|(b, _)| *b != a) {
            for (_, third) in COLORS
                .iter()
                .enumerate()
                .filter(|(c, _)| *c != a && *c != b)
            {
                out = out.replace(
                    &format!("{first} and {second} and {third}"),
                    &format!("{first}, {second}, and {third}"),
                );
            }
        }
    }
    (out != text).then_some(out)
}

/// Adds the wordings that list three colors with commas.
pub(super) fn add(texts: &mut Vec<String>) {
    let said: Vec<String> = texts.iter().filter_map(|text| listed(text)).collect();
    texts.extend(said);
}
