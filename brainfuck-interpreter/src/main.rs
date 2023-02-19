use crate::{autojit::interpret_auto_jit, jit::interpret_jit};
use brainfuck_interpreter::BrainfuckInterpreter;
use clap::Parser;
use std::fs;

mod autojit;
mod interpreter;
mod jit;

fn main() {
    let args = Args::parse();

    let contents = fs::read_to_string(args.file).expect("Should have been able to read the file");

    match args.mode.as_str() {
        "interpret" => {
            let mut brainfuck_interpreter = BrainfuckInterpreter::new(contents, false);
            brainfuck_interpreter.launch();
        }
        "jit" => {
            interpret_jit(&contents);
        }
        "autojit" => {
            interpret_auto_jit(&contents);
        }
        _ => panic!("Invalid mode value."),
    }
}

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    // Valid value = interprete / jit / autojit. Default value = interprete.
    #[arg(short, long, default_value_t = String::from("interpret"))]
    mode: String,

    // brainfuck file path
    #[arg(short, long)]
    file: String,
}
