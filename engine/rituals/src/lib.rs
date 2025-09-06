use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ritual {
    pub name: String,
    pub steps: Vec<String>,
}

pub fn run_ritual(ritual: &Ritual) {
    println!("Running ritual: {}", ritual.name);
    for step in &ritual.steps {
        println!("  - {}", step);
    }
}
