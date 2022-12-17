use std::fs;

use interpreter::interpret;

mod interpreter;
mod jit;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let contents = fs::read_to_string(&args[1]).expect("Should have been able to read the file");

    // TODO: support is_jit by user args
    interpret(&contents, true);
}
