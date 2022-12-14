use std::io::Read;

use brainfuck_analyzer::{parse, Token, TokenType};

struct Memory {
    index: usize,
    memory: Vec<u8>,
}

impl Memory {
    pub fn new() -> Memory {
        Memory {
            index: 0,
            memory: vec![0; 1000],
        }
    }

    fn interpret_token(&mut self, token: &Token) {
        match &token.token_type {
            TokenType::PointerIncrement => {
                if self.memory.len() - self.index == 1 {
                    self.memory.resize(self.memory.len() * 2, 0);
                }
                self.index += 1;
            }
            TokenType::PointerDecrement => {
                if self.index == 0 {
                    panic!("Cannot decrease pointer when pointer index = 0.");
                }
                self.index -= 1;
            }
            TokenType::Increment => {
                if self.memory[self.index] == u8::MAX {
                    self.memory[self.index] = u8::MIN;
                } else {
                    self.memory[self.index] += 1;
                }
            }
            TokenType::Decrement => {
                if self.memory[self.index] == u8::MIN {
                    self.memory[self.index] = u8::MAX;
                } else {
                    self.memory[self.index] -= 1;
                }
            }
            TokenType::Output => {
                let c: char = self.memory[self.index].into();
                print!("{}", c);
            }
            TokenType::Input => {
                self.memory[self.index] = std::io::stdin().bytes().next().unwrap().unwrap();
            }
            TokenType::SubGroup(sg) => {
                while self.memory[self.index] != 0 {
                    for token in sg.tokens().into_iter() {
                        self.interpret_token(token);
                    }
                }
            }
            _ => (),
        }
    }
}

pub fn interpret(input: &str) {
    let parse_result = parse(input).unwrap();
    let iter = parse_result.parse_token_group.tokens().into_iter();

    let mut memory = Memory::new();
    for token in iter {
        memory.interpret_token(token);
    }
}
