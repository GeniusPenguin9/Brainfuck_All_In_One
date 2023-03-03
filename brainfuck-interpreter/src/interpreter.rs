use brainfuck_analyzer::{parse, Position, Range, Token, TokenType};
use generator::Generator;
use generator::{done, Gn, Scope};
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

pub struct BrainfuckInterpreter<'a> {
    source_content: String,
    debug_mode: bool,
    breakpoint_lines: Vec<usize>,
    breakpoint_callback: Option<Box<dyn FnMut(StoppedReasonEnum) + 'a>>,
    generator: Option<Generator<'a, (), StoppedReasonEnum>>,
}

impl<'a> BrainfuckInterpreter<'a> {
    pub fn new(source_content: String, debug_mode: bool) -> Self {
        BrainfuckInterpreter {
            source_content,
            debug_mode,
            breakpoint_lines: Vec::new(),
            breakpoint_callback: None,
            generator: None,
        }
    }

    pub fn set_breakpoint_callback(&mut self, fn_handler: Box<dyn FnMut(StoppedReasonEnum) + 'a>) {
        self.breakpoint_callback = Some(fn_handler);
    }

    pub fn set_breakpoints(&mut self, breakpoint_lines: &Vec<usize>) {
        self.breakpoint_lines = breakpoint_lines.clone();
    }

    #[allow(dead_code)]
    pub fn add_breakpoints(&mut self, breakpoint_lines: &mut Vec<usize>) {
        self.breakpoint_lines.append(breakpoint_lines);
    }

    pub fn interpret_token(
        brainfuck_memory: &mut BrainfuckMemory,
        token: &Token,
        scope: &mut Scope<(), StoppedReasonEnum>,
        debug_mode: bool,
    ) {
        match &token.token_type {
            TokenType::PointerIncrement => {
                if brainfuck_memory.memory.len() - brainfuck_memory.index == 1 {
                    brainfuck_memory
                        .memory
                        .resize(brainfuck_memory.memory.len() * 2, 0);
                }
                brainfuck_memory.index += 1;
                if debug_mode {
                    scope.yield_(StoppedReasonEnum::Step);
                }
            }
            TokenType::PointerDecrement => {
                if brainfuck_memory.index == 0 {
                    panic!("Cannot decrease pointer when pointer index = 0.");
                }
                brainfuck_memory.index -= 1;
                if debug_mode {
                    scope.yield_(StoppedReasonEnum::Step);
                }
            }
            TokenType::Increment => {
                if brainfuck_memory.memory[brainfuck_memory.index] == u8::MAX {
                    brainfuck_memory.memory[brainfuck_memory.index] = u8::MIN;
                } else {
                    brainfuck_memory.memory[brainfuck_memory.index] += 1;
                }
                if debug_mode {
                    scope.yield_(StoppedReasonEnum::Step);
                }
            }
            TokenType::Decrement => {
                if brainfuck_memory.memory[brainfuck_memory.index] == u8::MIN {
                    brainfuck_memory.memory[brainfuck_memory.index] = u8::MAX;
                } else {
                    brainfuck_memory.memory[brainfuck_memory.index] -= 1;
                }
                if debug_mode {
                    scope.yield_(StoppedReasonEnum::Step);
                }
            }
            TokenType::Output => {
                let c: char = brainfuck_memory.memory[brainfuck_memory.index].into();
                print!("{}", c);
                if debug_mode {
                    scope.yield_(StoppedReasonEnum::Step);
                }
            }
            TokenType::Input => {
                brainfuck_memory.memory[brainfuck_memory.index] =
                    std::io::stdin().bytes().next().unwrap().unwrap();
                if debug_mode {
                    scope.yield_(StoppedReasonEnum::Step);
                }
            }
            TokenType::SubGroup(sg) => {
                while brainfuck_memory.memory[brainfuck_memory.index] != 0 {
                    for token in sg.tokens().into_iter() {
                        Self::interpret_token(brainfuck_memory, token, scope, debug_mode);
                    }
                }
            }
            TokenType::Breakpoint => {
                if debug_mode {
                    scope.yield_(StoppedReasonEnum::Breakpoint);
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
        drop(vec_token);

        let mut memory = BrainfuckMemory::new();
        let debug_mode = self.debug_mode;
        self.generator = Some(Gn::new_scoped(
            move |mut scope: Scope<(), StoppedReasonEnum>| {
                for token in parse_result.parse_token_group.tokens_mut().into_iter() {
                    Self::interpret_token(&mut memory, token, &mut scope, debug_mode);
                }
                scope.yield_(StoppedReasonEnum::Complete);
                done!();
            },
        ));

        self.run();
    }

    // run means user click "continue" and only stopped when breakpoint/complete
    pub fn run(&mut self) {
        while let Some(reason) = self.generator.as_mut().unwrap().next() {
            match reason {
                StoppedReasonEnum::Breakpoint | StoppedReasonEnum::Complete => {
                    if let Some(callback) = &mut self.breakpoint_callback {
                        (callback)(reason);
                    };
                    break;
                }
                _ => (),
            }
        }
    }

    // next means user want to run only one step
    pub fn next(&mut self) {
        while let Some(reason) = self.generator.as_mut().unwrap().next() {
            match reason {
                StoppedReasonEnum::Step | StoppedReasonEnum::Complete => {
                    if let Some(callback) = &mut self.breakpoint_callback {
                        (callback)(reason);
                    };
                    break;
                }
                _ => (),
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum StoppedReasonEnum {
    Breakpoint,
    Step,
    Complete,
}

#[test]
pub fn test_breakpoint_debug_mode() {
    use std::fs;
    let mut callback_hit = 0;
    let source_content = include_str!("../benches/jit_benchmark_test_calculation.bf").to_string();
    let mut brainfuck_interpreter = BrainfuckInterpreter::new(source_content, true);
    let breakpoint_lines: Vec<usize> = vec![0, 6];
    brainfuck_interpreter.set_breakpoints(&breakpoint_lines);

    let callback = |reason: StoppedReasonEnum| {
        assert_eq!(StoppedReasonEnum::Breakpoint, reason);
        callback_hit += 1;
    };
    brainfuck_interpreter.set_breakpoint_callback(Box::new(callback));
    brainfuck_interpreter.launch();

    drop(brainfuck_interpreter);
    assert_eq!(1, callback_hit);
}

#[test]
pub fn test_breakpoint_continue_debug_mode() {
    use std::fs;
    let mut callback_hit = 0;
    let source_content = include_str!("../benches/jit_benchmark_test_calculation.bf").to_string();
    let mut brainfuck_interpreter = BrainfuckInterpreter::new(source_content, true);
    let breakpoint_lines: Vec<usize> = vec![0, 6];
    brainfuck_interpreter.set_breakpoints(&breakpoint_lines);

    let callback = |reason: StoppedReasonEnum| {
        callback_hit += 1;
    };
    brainfuck_interpreter.set_breakpoint_callback(Box::new(callback));
    brainfuck_interpreter.launch();
    for _ in 0..(255 * 255 * 255 + 1) {
        brainfuck_interpreter.run();
    }

    drop(brainfuck_interpreter);

    // line 0, breakpoint 1 time
    // line 6, breakpoint 255 * 255 * 255 times
    // complete 1 time
    assert_eq!(1 + 255 * 255 * 255 + 1, callback_hit);
}

#[test]
pub fn test_breakpoint_disable_debug_mode() {
    use std::fs;
    let mut callback_hit = 0;
    let source_content = include_str!("../benches/jit_benchmark_test_calculation.bf").to_string();
    let mut brainfuck_interpreter = BrainfuckInterpreter::new(source_content, false);
    let breakpoint_lines: Vec<usize> = vec![0, 6];
    brainfuck_interpreter.set_breakpoints(&breakpoint_lines);

    let callback = |reason: StoppedReasonEnum| {
        assert_eq!(StoppedReasonEnum::Complete, reason);
        callback_hit += 1;
    };
    brainfuck_interpreter.set_breakpoint_callback(Box::new(callback));
    brainfuck_interpreter.launch();

    drop(brainfuck_interpreter);
    assert_eq!(1, callback_hit);
}
