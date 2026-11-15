//! Prints the layout for the workload model's base case (#15) and for the
//! Satchel lab cluster as JSON.
//!
//!   cargo run -p satchel-cost --example layout

use satchel_cost::layout::{layout, LayoutInput};

fn main() {
    let cases = [
        ("dropbox_base_case", LayoutInput::dropbox_base_case()),
        ("satchel_lab", LayoutInput::satchel_lab()),
    ];
    let out: serde_json::Map<String, serde_json::Value> = cases
        .into_iter()
        .map(|(name, input)| {
            let l = layout(&input).expect("presets are valid");
            (
                name.to_owned(),
                serde_json::to_value(l).expect("layout is JSON"),
            )
        })
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&out).expect("layout is JSON")
    );
}
