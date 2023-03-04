use brainfuck_analyzer::{parse, Position, Range, Token, TokenGroup, TokenType};

use core::time;
use std::io::Read;
use std::marker::PhantomData;
use std::mem::transmute;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::vec;

use crate::interpreter;
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

pub struct BrainfuckDebugInterpreter<'a> {
    source_content: String,
    breakpoint_lines: Vec<usize>,
    interpreter_debug_tx: Option<Sender<StartReasonEnum>>,
    thread: Option<JoinHandle<()>>,
    phantom_data: PhantomData<&'a ()>,
}

pub struct BrainfuckDebugThreadData<'a> {
    interpreter_debug_rx: Receiver<StartReasonEnum>,
    breakpoint_callback: Option<Box<dyn FnMut(StoppedReasonEnum) + 'a + Send>>,
    output_callback: Option<Box<dyn FnMut(OutputCategoryEnum, String) + 'a + Send>>,
}

impl<'a> BrainfuckDebugInterpreter<'a> {
    pub fn new(source_content: String) -> Self {
        BrainfuckDebugInterpreter {
            source_content,
            breakpoint_lines: Vec::new(),
            interpreter_debug_tx: None,
            thread: None,
            phantom_data: Default::default(),
        }
    }

    // pub fn set_breakpoint_callback(&mut self, fn_handler: Box<dyn FnMut(StoppedReasonEnum) + 'a>) {
    //     self.breakpoint_callback = Some(fn_handler);
    // }

    pub fn set_breakpoints(&mut self, breakpoint_lines: &Vec<usize>) {
        self.breakpoint_lines = breakpoint_lines.clone();
    }

    #[allow(dead_code)]
    pub fn add_breakpoints(&mut self, breakpoint_lines: &mut Vec<usize>) {
        self.breakpoint_lines.append(breakpoint_lines);
    }

    pub fn interpret_token(
        debug_thread_data: &mut BrainfuckDebugThreadData,
        brainfuck_memory: &mut BrainfuckMemory,
        token: &Token,
    ) {
        match &token.token_type {
            TokenType::PointerIncrement => {
                if brainfuck_memory.memory.len() - brainfuck_memory.index == 1 {
                    brainfuck_memory
                        .memory
                        .resize(brainfuck_memory.memory.len() * 2, 0);
                }
                brainfuck_memory.index += 1;

                // scope.yield_(StoppedReasonEnum::Step);
            }
            TokenType::PointerDecrement => {
                if brainfuck_memory.index == 0 {
                    panic!("Cannot decrease pointer when pointer index = 0.");
                }
                brainfuck_memory.index -= 1;
                // scope.yield_(StoppedReasonEnum::Step);
            }
            TokenType::Increment => {
                if brainfuck_memory.memory[brainfuck_memory.index] == u8::MAX {
                    brainfuck_memory.memory[brainfuck_memory.index] = u8::MIN;
                } else {
                    brainfuck_memory.memory[brainfuck_memory.index] += 1;
                }
                // scope.yield_(StoppedReasonEnum::Step);
            }
            TokenType::Decrement => {
                if brainfuck_memory.memory[brainfuck_memory.index] == u8::MIN {
                    brainfuck_memory.memory[brainfuck_memory.index] = u8::MAX;
                } else {
                    brainfuck_memory.memory[brainfuck_memory.index] -= 1;
                }
                // scope.yield_(StoppedReasonEnum::Step);
            }
            TokenType::Output => {
                let c: char = brainfuck_memory.memory[brainfuck_memory.index].into();
                if let Some(oc) = &mut debug_thread_data.output_callback {
                    (*oc)(OutputCategoryEnum::StdOut, c.to_string());
                }
                // scope.yield_(StoppedReasonEnum::Step);
            }
            TokenType::Input => {
                brainfuck_memory.memory[brainfuck_memory.index] =
                    std::io::stdin().bytes().next().unwrap().unwrap();
                // scope.yield_(StoppedReasonEnum::Step);
            }
            TokenType::SubGroup(sg) => {
                while brainfuck_memory.memory[brainfuck_memory.index] != 0 {
                    for token in sg.tokens().into_iter() {
                        Self::interpret_token(debug_thread_data, brainfuck_memory, token);
                    }
                }
            }
            TokenType::Breakpoint => {
                // scope.yield_(StoppedReasonEnum::Breakpoint);
                if let Some(bc) = &mut debug_thread_data.breakpoint_callback {
                    (*bc)(StoppedReasonEnum::Breakpoint);
                };
                if let Ok(start_reason) = debug_thread_data.interpreter_debug_rx.recv() {
                    match start_reason {
                        StartReasonEnum::Continue => (),
                        StartReasonEnum::Step => todo!(),
                    }
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

    pub fn launch(
        &mut self,
        breakback_callback: Option<Box<dyn FnMut(StoppedReasonEnum) + 'a + Send>>,
        output_callback: Option<Box<dyn FnMut(OutputCategoryEnum, String) + 'a + Send>>,
    ) {
        let mut parse_result = parse(&self.source_content).unwrap();

        let vec_token = parse_result.parse_token_group.tokens_mut();
        let mut breakpoint_lines = self.breakpoint_lines.clone();
        Self::_insert_breakpoints(vec_token, &mut breakpoint_lines);

        //drop(vec_token);

        // TODO: solve unsafe
        let bc: Option<Box<dyn FnMut(StoppedReasonEnum) + 'static + Send>> =
            unsafe { transmute(breakback_callback) };
        let oc: Option<Box<dyn FnMut(OutputCategoryEnum, String) + 'static + Send>> =
            unsafe { transmute(output_callback) };
        let token_group = parse_result.parse_token_group;
        let (interpreter_debug_tx, interpreter_debug_rx) = mpsc::channel();
        self.interpreter_debug_tx = Some(interpreter_debug_tx);
        self.thread = Some(thread::spawn(move || {
            let mut debug_data = BrainfuckDebugThreadData {
                interpreter_debug_rx,
                breakpoint_callback: bc,
                output_callback: oc,
            };
            Self::debug_thread(debug_data, token_group);
        }));
    }

    fn debug_thread(mut debug_data: BrainfuckDebugThreadData, token_group: TokenGroup) {
        let mut memory = BrainfuckMemory::new();

        for token in token_group.tokens().into_iter() {
            Self::interpret_token(&mut debug_data, &mut memory, token);
        }
        if let Some(bc) = &mut debug_data.breakpoint_callback {
            (*bc)(StoppedReasonEnum::Complete);
        };
    }

    // // run means user click "continue" and only stopped when breakpoint/complete
    pub fn run(&mut self) -> Result<(), String> {
        if let Some(interpreter_debug_tx) = &self.interpreter_debug_tx {
            if let Err(_) = interpreter_debug_tx.send(StartReasonEnum::Continue) {
                return Err("Debug program already finished.".to_string());
            }
        }
        Ok(())
    }

    // // next means user want to run only one step
    pub fn next(&mut self) {
        //     while let Some(reason) = self.generator.as_mut().unwrap().next() {
        //         match reason {
        //             StoppedReasonEnum::Step | StoppedReasonEnum::Complete => {
        //                 if let Some(callback) = &mut self.breakpoint_callback {
        //                     (callback)(reason);
        //                 };
        //                 break;
        //             }
        //             _ => (),
        //         }
        //     }
    }
}

#[derive(Debug, PartialEq)]
pub enum StoppedReasonEnum {
    Breakpoint,
    Step,
    Complete,
}

pub enum OutputCategoryEnum {
    Console,
    StdOut,
}

pub enum StartReasonEnum {
    Step,
    Continue,
}

// #[test]
// pub fn test_breakpoint_debug_mode() {
//     use std::fs;
//     let mut callback_hit = 0;
//     let source_content = include_str!("../benches/jit_benchmark_test_calculation.bf").to_string();
//     let mut brainfuck_debug_interpreter = BrainfuckDebugInterpreter::new(source_content);
//     let breakpoint_lines: Vec<usize> = vec![0, 6];
//     brainfuck_debug_interpreter.set_breakpoints(&breakpoint_lines);

//     let callback = |reason: StoppedReasonEnum| {
//         assert_eq!(StoppedReasonEnum::Breakpoint, reason);
//         callback_hit += 1;
//     };

//     brainfuck_debug_interpreter.launch(Some(Box::new(callback)), None);
//     drop(brainfuck_debug_interpreter);
//     assert_eq!(1, callback_hit);
// }

#[test]
pub fn test_breakpoint_continue_debug_mode() {
    use std::fs;
    let mut callback_hit = 0;
    let source_content = include_str!("../benches/jit_benchmark_test_calculation.bf").to_string();
    let mut brainfuck_debug_interpreter = BrainfuckDebugInterpreter::new(source_content);
    let breakpoint_lines: Vec<usize> = vec![0, 6];
    brainfuck_debug_interpreter.set_breakpoints(&breakpoint_lines);

    let callback = |reason: StoppedReasonEnum| {
        callback_hit += 1;
    };
    brainfuck_debug_interpreter.launch(Some(Box::new(callback)), None);
    for _ in 0..(255 * 255 * 255 + 1) {
        brainfuck_debug_interpreter.run();
    }

    // wait until interpreter thread complete
    thread::sleep(time::Duration::from_secs(10));
    drop(brainfuck_debug_interpreter);

    // line 0, breakpoint 1 time
    // line 6, breakpoint 255 * 255 * 255 times
    // complete 1 time
    assert_eq!(1 + 255 * 255 * 255 + 1, callback_hit);
}

// TODO:
// #[test]
// pub fn test_breakpoint_disable_debug_mode() {
//     use std::fs;
//     let mut callback_hit = 0;
//     let source_content = include_str!("../benches/jit_benchmark_test_calculation.bf").to_string();
//     let mut brainfuck_debug_interpreter = BrainfuckDebugInterpreter::new(source_content, false);
//     let breakpoint_lines: Vec<usize> = vec![0, 6];
//     brainfuck_debug_interpreter.set_breakpoints(&breakpoint_lines);

//     let callback = |reason: StoppedReasonEnum| {
//         assert_eq!(StoppedReasonEnum::Complete, reason);
//         callback_hit += 1;
//     };
//     brainfuck_debug_interpreter.set_breakpoint_callback(Box::new(callback));
//     brainfuck_debug_interpreter.launch();

//     drop(brainfuck_debug_interpreter);
//     assert_eq!(1, callback_hit);
// }
