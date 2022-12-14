use std::fs;

use interpreter::interpret;

mod interpreter;

fn main() {
    let file_path: Vec<String> = std::env::args().collect();

    let contents =
        fs::read_to_string(&file_path[1]).expect("Should have been able to read the file");

    interpret(&contents);
}
