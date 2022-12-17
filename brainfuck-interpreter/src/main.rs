use clap::Parser;
use interpreter::interpret;
use std::fs;

mod interpreter;
mod jit;

fn main() {
    let args = Args::parse();

    let contents = fs::read_to_string(args.file).expect("Should have been able to read the file");
    interpret(&contents, args.aot);
}

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// enable ahead of time, if true, use jit. Default = false.
    #[arg(short, long, default_value_t = false)]
    aot: bool,

    // brainfuck file path
    #[arg(short, long)]
    file: String,
}
