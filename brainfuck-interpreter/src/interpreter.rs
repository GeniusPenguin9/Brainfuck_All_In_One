use brainfuck_analyzer::{parse, Position, Range, Token, TokenType};

use std::io::Read;

use crate::jit::IBrainfuckMemory;

pub struct BrainfuckMemory {
    pub index: usize,
    pub memory: Vec<u8>,
}

impl IBrainfuckMemory for BrainfuckMemory {
    fn get_memory_vec_ptr(&self) -> *const u8 {
        &self.memory[0] as *const u8
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_index(&mut self, new_index: usize) {
        self.index = new_index;
    }
}

impl BrainfuckMemory {
    pub fn new() -> BrainfuckMemory {
        BrainfuckMemory {
            index: 0,
            memory: vec![0; 1000],
        }
    }
}

pub struct BrainfuckInterpreter {
    source_content: String,
    debug_mode: bool,
    breakpoint_lines: Vec<usize>,
    breakpoint_callback: Option<Box<dyn Fn(StoppedReasonEnum) + Send>>,
}

impl BrainfuckInterpreter {
    pub fn new(source_content: String, debug_mode: bool) -> Self {
        BrainfuckInterpreter {
            source_content,
            debug_mode,
            breakpoint_lines: Vec::new(),
            breakpoint_callback: None,
        }
    }

    pub fn set_breakpoint_callback(&mut self, fn_handler: Box<dyn Fn(StoppedReasonEnum) + Send>) {
        self.breakpoint_callback = Some(fn_handler);
    }

    pub fn set_breakpoints(&mut self, breakpoint_lines: &Vec<usize>) {
        self.breakpoint_lines = breakpoint_lines.clone();
    }

    #[allow(dead_code)]
    pub fn add_breakpoints(&mut self, breakpoint_lines: &mut Vec<usize>) {
        self.breakpoint_lines.append(breakpoint_lines);
    }

    pub fn interpret_token(&self, brainfuck_memory: &mut BrainfuckMemory, token: &Token) {
        match &token.token_type {
            TokenType::PointerIncrement => {
                if brainfuck_memory.memory.len() - brainfuck_memory.index == 1 {
                    brainfuck_memory
                        .memory
                        .resize(brainfuck_memory.memory.len() * 2, 0);
                }
                brainfuck_memory.index += 1;
            }
            TokenType::PointerDecrement => {
                if brainfuck_memory.index == 0 {
                    panic!("Cannot decrease pointer when pointer index = 0.");
                }
                brainfuck_memory.index -= 1;
            }
            TokenType::Increment => {
                if brainfuck_memory.memory[brainfuck_memory.index] == u8::MAX {
                    brainfuck_memory.memory[brainfuck_memory.index] = u8::MIN;
                } else {
                    brainfuck_memory.memory[brainfuck_memory.index] += 1;
                }
            }
            TokenType::Decrement => {
                if brainfuck_memory.memory[brainfuck_memory.index] == u8::MIN {
                    brainfuck_memory.memory[brainfuck_memory.index] = u8::MAX;
                } else {
                    brainfuck_memory.memory[brainfuck_memory.index] -= 1;
                }
            }
            TokenType::Output => {
                let c: char = brainfuck_memory.memory[brainfuck_memory.index].into();
                print!("{}", c);
            }
            TokenType::Input => {
                brainfuck_memory.memory[brainfuck_memory.index] =
                    std::io::stdin().bytes().next().unwrap().unwrap();
            }
            TokenType::SubGroup(sg) => {
                while brainfuck_memory.memory[brainfuck_memory.index] != 0 {
                    for token in sg.tokens().into_iter() {
                        self.interpret_token(brainfuck_memory, token);
                    }
                }
            }
            TokenType::Breakpoint => {
                if let Some(callback) = &self.breakpoint_callback {
                    (callback)(StoppedReasonEnum::Breakpoint);
                }
            }
            _ => (),
        }
    }

    fn _insert_breakpoints(vec_token: &mut Vec<Token>, breakpoint_lines: &mut Vec<usize>) {
        let mut indexs = vec![];
        for i in 0..vec_token.len() {
            match &mut vec_token[i].token_type {
                TokenType::SubGroup(ref mut sg) => {
                    Self::_insert_breakpoints(sg.as_mut().tokens_mut(), breakpoint_lines)
                }
                TokenType::Comment(_) => (),
                _ => {
                    if breakpoint_lines
                        .iter()
                        .any(|bpl| *bpl as u32 <= vec_token[i].range.start.line)
                    {
                        indexs.push((i, vec_token[i].range.start.line));
                        breakpoint_lines.retain(|bpl| *bpl as u32 > vec_token[i].range.start.line);
                    }
                }
            }
        }

        for i in (0..indexs.len()).rev() {
            let index = indexs[i].0;
            let line = indexs[i].1;
            vec_token.insert(
                index,
                Token {
                    range: Range {
                        start: Position { line, character: 0 },
                        end: Position { line, character: 0 },
                    },
                    token_type: TokenType::Breakpoint,
                },
            );
        }
    }

    pub fn launch(&mut self) {
        let mut parse_result = parse(&self.source_content).unwrap();

        let vec_token = parse_result.parse_token_group.tokens_mut();
        let mut breakpoint_lines = self.breakpoint_lines.clone();
        Self::_insert_breakpoints(vec_token, &mut breakpoint_lines);

        let mut memory = BrainfuckMemory::new();
        for token in vec_token.into_iter() {
            self.interpret_token(&mut memory, token);
        }
    }
}

#[derive(Debug)]
pub enum StoppedReasonEnum {
    Breakpoint,
    Complete,
}

#[test]
pub fn test_breakpoint() {
    use std::fs;
    
    let source_content =  fs::read_to_string("C:/Users/cauli/source/repos/rust/Brainfuck_All_In_One/brainfuck-interpreter/benches/jit_benchmark_test_calculation.bf".to_string())
                    .expect("Should have been able to read the file");
    let mut brainfuck_interpreter = BrainfuckInterpreter::new(source_content, true);
    let breakpoint_lines: Vec<usize> = vec![0];
    brainfuck_interpreter.set_breakpoints(&breakpoint_lines);
    // let mut callback_hit = 0;
    let callback = |reason: StoppedReasonEnum| {
        println!("Reason:{:?}", reason);
    };
    brainfuck_interpreter.set_breakpoint_callback(Box::new(callback));
    brainfuck_interpreter.launch();
    // assert_eq!(2, callback_hit);
    // TODO: add assert
}
