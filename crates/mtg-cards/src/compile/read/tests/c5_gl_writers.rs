//! Goal: the writers the fifth sample of the names G to L needed.
use super::*;

#[test]
fn c5_gl_debug() {
    let Ok(path) = std::env::var("C5_GL_DEBUG") else {
        return;
    };
    let text = std::fs::read_to_string(&path).unwrap();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let parsed = ability(line);
        println!("JSON {}", serde_json::to_string(&parsed).unwrap());
        for wording in render::variants(&parsed) {
            println!("  V {wording}");
        }
        for wording in written_variants(&parsed) {
            println!("  W {wording}");
        }
    }
}
