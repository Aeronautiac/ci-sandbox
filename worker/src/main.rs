// stands in for yagami-runtime: a sibling binary the server spawns and talks to over stdio.
use std::io::{Read, Write};

fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    std::io::stdout()
        .write_all(input.trim().to_uppercase().as_bytes())
        .unwrap();
}
