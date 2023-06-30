use brainfuck_analyzer::{flat_parse, parse, Range, Token, TokenGroup, TokenType};

use crate::interpreter;
use crate::jit::IBrainfuckMemory;
use core::time;
use simplelog::*;
use std::borrow::BorrowMut;
use std::io::Read;
use std::marker::PhantomData;
use std::mem::{self, transmute};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::{default, fs, vec};

pub use brainfuck_analyzer::Position;

pub struct BrainfuckMemory {
    pub index: usize,
    pub memory: Vec<u8>,
}

#[derive(Clone)]
pub struct BrainfuckBreakpoint {
    pub position: Position,
    pub id: usize,
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
    source_file: String,
    source_content: String,
    breakpoints: Vec<BrainfuckBreakpoint>,
    interpreter_debug_command_tx: Option<Sender<InterpreterCommand>>,
    interpreter_debug_user_input_tx: Option<Sender<char>>,
    interpreter_debug_response_rx: Option<Receiver<InterpreterResponse>>,
    thread: Option<JoinHandle<()>>,
    breakpoint_id: usize,
    _phantom: PhantomData<&'a ()>,
}

pub struct BrainfuckDebugThreadData<'a> {
    state: InterpreterState,
    breakpoints: Vec<BrainfuckBreakpoint>,
    interpreter_debug_command_rx: Receiver<InterpreterCommand>,
    interpreter_debug_response_tx: Sender<InterpreterResponse>,
    interpreter_debug_user_input_rx: Receiver<char>,
    breakpoint_callback:
        Option<Box<dyn FnMut(StoppedReasonEnum, Option<Position>, Option<usize>) + 'a + Send>>, // stop_reason, stop_position, breakpoint_id
    output_callback: Option<Box<dyn FnMut(OutputCategoryEnum, String) + 'a + Send>>,
}

impl<'a> BrainfuckDebugInterpreter<'a> {
    pub fn new(source_content: String) -> Self {
        BrainfuckDebugInterpreter {
            source_file: "unknown".to_string(),
            source_content,
            breakpoints: Vec::new(),
            interpreter_debug_command_tx: None,
            interpreter_debug_user_input_tx: None,
            interpreter_debug_response_rx: None,
            thread: None,
            breakpoint_id: 0,
            _phantom: Default::default(),
        }
    }

    pub fn from_file(source_file: &str) -> Result<Self, String> {
        let source_content =
            fs::read_to_string(source_file).map_err(|x| format!("read file failed: {}", x))?;
        Ok(BrainfuckDebugInterpreter {
            source_file: source_file.to_string(),
            source_content,
            breakpoints: Vec::new(),
            interpreter_debug_command_tx: None,
            interpreter_debug_user_input_tx: None,
            interpreter_debug_response_rx: None,
            thread: None,
            breakpoint_id: 0,
            _phantom: Default::default(),
        })
    }

    pub fn get_filename(&self) -> String {
        self.source_file.clone()
    }

    // pub fn set_breakpoint_callback(&mut self, fn_handler: Box<dyn FnMut(StoppedReasonEnum) + 'a>) {
    //     self.breakpoint_callback = Some(fn_handler);
    // }

    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    pub fn update_runtime_breakpoints(&mut self) {
        if let Some(interpreter_debug_tx) = &self.interpreter_debug_command_tx {
            interpreter_debug_tx
                .send(InterpreterCommand::UpdateBreakpoints(
                    self.breakpoints.clone(),
                ))
                .ok();
        }
    }

    pub fn add_and_validate_breakpoint(
        &mut self,
        row: u32,
        col: Option<u32>,
    ) -> Option<BrainfuckBreakpoint> {
        let parse_result = flat_parse(&self.source_content).unwrap();
        let mut last_token: Option<&Token> = None;
        for t in parse_result.parse_token_group.tokens() {
            if let Some(col) = col {
                if row == t.range.start.line && col == t.range.start.character {
                    debug!("breakpoint validated @ {},{}", row, col);
                    let breakpoint = BrainfuckBreakpoint {
                        position: Position::new(row, col),
                        id: self.breakpoint_id,
                    };
                    self.breakpoint_id += 1;
                    self.breakpoints.push(breakpoint.clone());
                    return Some(breakpoint);
                }
            } else {
                if last_token.is_some()
                    && last_token.unwrap().range.end.line != row
                    && t.range.start.line == row
                {
                    debug!(
                        "breakpoint validated @ {},{}",
                        t.range.start.line, t.range.start.character
                    );
                    let breakpoint = BrainfuckBreakpoint {
                        position: t.range.start,
                        id: self.breakpoint_id,
                    };
                    self.breakpoint_id += 1;
                    self.breakpoints.push(breakpoint.clone());
                    return Some(breakpoint);
                }
            }
            last_token = Some(t);
        }
        debug!("breakpoint invalid @ {},{:?}", row, col);
        None
    }

    fn handle_command(locals: &mut BrainfuckDebugThreadData, token: &Token) {
        match locals.interpreter_debug_command_rx.try_recv() {
            Ok(command) => match command {
                InterpreterCommand::Continue => locals.state = InterpreterState::Running,
                InterpreterCommand::Step => locals.state = InterpreterState::Step,
                InterpreterCommand::GetPosition => match &locals.state {
                    InterpreterState::Paused(token) => locals
                        .interpreter_debug_response_tx
                        .send(InterpreterResponse::Position(token.range.start))
                        .unwrap(),
                    _ => locals
                        .interpreter_debug_response_tx
                        .send(InterpreterResponse::Error)
                        .unwrap(),
                },
                InterpreterCommand::Pause => locals.state = InterpreterState::Paused(token.clone()),
                InterpreterCommand::Terminate => locals.state = InterpreterState::Terminated,
                InterpreterCommand::UpdateBreakpoints(breakpoints) => {
                    locals.breakpoints = breakpoints
                }
            },
            Err(TryRecvError::Disconnected) => locals.state = InterpreterState::Terminated,
            Err(TryRecvError::Empty) => (),
        }
    }

    pub fn interpret_token(
        locals: &mut BrainfuckDebugThreadData,
        brainfuck_runtime: &mut BrainfuckMemory,
        token: &Token,
    ) -> bool {
        loop {
            if locals.state == InterpreterState::Running {
                for breakpoint in &locals.breakpoints {
                    if breakpoint.position == token.range.start {
                        if let Some(bc) = &mut locals.breakpoint_callback {
                            (*bc)(
                                StoppedReasonEnum::Breakpoint,
                                Some(breakpoint.position),
                                Some(breakpoint.id),
                            );
                        };
                        locals.state = InterpreterState::Paused(token.clone());
                    }
                }
            } else if locals.state == InterpreterState::Step {
                if let Some(bc) = &mut locals.breakpoint_callback {
                    (*bc)(StoppedReasonEnum::Step, Some(token.range.start), None);
                }
                locals.state = InterpreterState::Paused(token.clone());
            }

            BrainfuckDebugInterpreter::handle_command(locals, token);

            match &locals.state {
                InterpreterState::Paused(t) => continue,
                InterpreterState::Running => break,
                InterpreterState::Step => break,
                InterpreterState::Terminated => return false,
            }
        }

        match &token.token_type {
            TokenType::PointerIncrement => {
                if brainfuck_runtime.memory.len() - brainfuck_runtime.index == 1 {
                    brainfuck_runtime
                        .memory
                        .resize(brainfuck_runtime.memory.len() * 2, 0);
                }
                brainfuck_runtime.index += 1;

                // scope.yield_(StoppedReasonEnum::Step);
            }
            TokenType::PointerDecrement => {
                if brainfuck_runtime.index == 0 {
                    panic!("Cannot decrease pointer when pointer index = 0.");
                }
                brainfuck_runtime.index -= 1;
                // scope.yield_(StoppedReasonEnum::Step);
            }
            TokenType::Increment => {
                if brainfuck_runtime.memory[brainfuck_runtime.index] == u8::MAX {
                    brainfuck_runtime.memory[brainfuck_runtime.index] = u8::MIN;
                } else {
                    brainfuck_runtime.memory[brainfuck_runtime.index] += 1;
                }
                // scope.yield_(StoppedReasonEnum::Step);
            }
            TokenType::Decrement => {
                if brainfuck_runtime.memory[brainfuck_runtime.index] == u8::MIN {
                    brainfuck_runtime.memory[brainfuck_runtime.index] = u8::MAX;
                } else {
                    brainfuck_runtime.memory[brainfuck_runtime.index] -= 1;
                }
                // scope.yield_(StoppedReasonEnum::Step);
            }
            TokenType::Output => {
                let c: char = brainfuck_runtime.memory[brainfuck_runtime.index].into();
                if let Some(oc) = &mut locals.output_callback {
                    (*oc)(OutputCategoryEnum::StdOut, c.to_string());
                }
                // scope.yield_(StoppedReasonEnum::Step);
            }
            TokenType::Input => {
                debug!("interpret_token: meet TokenType::Input");
                // user input -> vsc client -> dap request -> dap -> buffer. Instead of stdin
                let mut user_input_noticed = false;
                loop {
                    if let Ok(input_char) = locals.interpreter_debug_user_input_rx.try_recv() {
                        debug!("Get `{}` from user input", input_char);
                        brainfuck_runtime.memory[brainfuck_runtime.index] = input_char as u8;
                        break;
                    } else {
                        debug!("Cannot get user input");
                        if user_input_noticed == false {
                            if let Some(oc) = &mut locals.output_callback {
                                (*oc)(
                                    OutputCategoryEnum::Console,
                                    "Waiting for user input".to_string(),
                                );
                                user_input_noticed = true;
                            }
                        }

                        thread::sleep(time::Duration::from_millis(500));
                    }
                }
            }
            TokenType::SubGroup(sg) => {
                while brainfuck_runtime.memory[brainfuck_runtime.index] != 0 {
                    for token in sg.tokens().into_iter() {
                        if Self::interpret_token(locals, brainfuck_runtime, token) == false {
                            return false;
                        }
                    }
                }
            }
            TokenType::Breakpoint => {
                // scope.yield_(StoppedReasonEnum::Breakpoint);
                // if let Some(bc) = &mut debug_thread_data.breakpoint_callback {
                //     (*bc)(StoppedReasonEnum::Breakpoint);
                // };
                // if let Ok(start_reason) = debug_thread_data.interpreter_debug_start_rx.recv() {
                //     match start_reason {
                //         StartReasonEnum::Continue => (),
                //         StartReasonEnum::Step => todo!(),
                //     }
                // }
            }
            _ => (),
        };
        true
    }

    // fn _insert_breakpoints(vec_token: &mut Vec<Token>, breakpoint_lines: &mut Vec<Position>) {
    //     let mut indexs = vec![];
    //     for i in 0..vec_token.len() {
    //         match &mut vec_token[i].token_type {
    //             TokenType::SubGroup(ref mut sg) => {
    //                 Self::_insert_breakpoints(sg.as_mut().tokens_mut(), breakpoint_lines)
    //             }
    //             TokenType::Comment(_) => (),
    //             _ => {
    //                 if breakpoint_lines
    //                     .iter()
    //                     .any(|bpl| *bpl as u32 <= vec_token[i].range.start.line)
    //                 {
    //                     indexs.push((i, vec_token[i].range.start.line));
    //                     breakpoint_lines.retain(|bpl| *bpl as u32 > vec_token[i].range.start.line);
    //                 }
    //             }
    //         }
    //     }

    //     for i in (0..indexs.len()).rev() {
    //         let index = indexs[i].0;
    //         let line = indexs[i].1;
    //         vec_token.insert(
    //             index,
    //             Token {
    //                 range: Range {
    //                     start: Position { line, character: 0 },
    //                     end: Position { line, character: 0 },
    //                 },
    //                 token_type: TokenType::Breakpoint,
    //             },
    //         );
    //     }
    // }

    pub fn launch(
        &mut self,
        breakback_callback: Option<
            Box<dyn FnMut(StoppedReasonEnum, Option<Position>, Option<usize>) + 'a + Send>,
        >,
        output_callback: Option<Box<dyn FnMut(OutputCategoryEnum, String) + 'a + Send>>,
    ) {
        info!(">> debug_interpreter launch function");

        let mut parse_result = parse(&self.source_content).unwrap();
        let vec_token = parse_result.parse_token_group.tokens_mut();

        /*
         * Why exception:
         * default lifetime of callback handler is 'a, equal to `self`.
         * default lifetime of thread is static.
         *
         * Avoid unsafe side effect:
         * impl drop() for self to make sure thread will no longer alive after `self` drop.
         * lifetime of thread: static => 'a
         */
        let bc: Option<
            Box<dyn FnMut(StoppedReasonEnum, Option<Position>, Option<usize>) + 'static + Send>,
        > = unsafe { transmute(breakback_callback) };
        let oc: Option<Box<dyn FnMut(OutputCategoryEnum, String) + 'static + Send>> =
            unsafe { transmute(output_callback) };
        let token_group = parse_result.parse_token_group;
        let breakpoints = self.breakpoints.clone();
        let (interpreter_debug_start_tx, interpreter_debug_start_rx) = mpsc::channel();
        let (interpreter_debug_user_tx, interpreter_debug_user_rx) = mpsc::channel();
        let (interpreter_debug_response_tx, interpreter_debug_response_rx) = mpsc::channel();
        self.interpreter_debug_command_tx = Some(interpreter_debug_start_tx);
        self.interpreter_debug_user_input_tx = Some(interpreter_debug_user_tx);
        self.interpreter_debug_response_rx = Some(interpreter_debug_response_rx);
        self.thread = Some(thread::spawn(move || {
            let debug_data = BrainfuckDebugThreadData {
                state: InterpreterState::Running,
                breakpoints,
                interpreter_debug_command_rx: interpreter_debug_start_rx,
                interpreter_debug_user_input_rx: interpreter_debug_user_rx,
                interpreter_debug_response_tx: interpreter_debug_response_tx,
                breakpoint_callback: bc,
                output_callback: oc,
            };
            Self::debug_thread(debug_data, token_group);
        }));
        info!("<< debug_interpreter launch function");
    }

    fn debug_thread(mut debug_data: BrainfuckDebugThreadData, token_group: TokenGroup) {
        info!(">> debug_interpreter debug_thread function");
        let mut memory = BrainfuckMemory::new();

        for token in token_group.tokens().into_iter() {
            if Self::interpret_token(&mut debug_data, &mut memory, token) == false {
                if let Some(bc) = &mut debug_data.breakpoint_callback {
                    (*bc)(StoppedReasonEnum::Terminated, Some(token.range.start), None);
                    return;
                };
            }
        }
        info!("debug_thread execute token completed.");

        if let Some(bc) = &mut debug_data.breakpoint_callback {
            (*bc)(StoppedReasonEnum::Complete, None, None);
            info!("debug_thread send complete to breakpoint_callback.");
        };
        info!("<< debug_interpreter debug_thread function");
    }

    // // run means user click "continue" and only stopped when breakpoint/complete
    pub fn run(&mut self) -> Result<(), String> {
        if let Some(interpreter_debug_tx) = &self.interpreter_debug_command_tx {
            if let Err(_) = interpreter_debug_tx.send(InterpreterCommand::Continue) {
                return Err("Debug program already finished.".to_string());
            }
        }
        Ok(())
    }

    pub fn get_position(&mut self) -> Result<Position, String> {
        let tx = &self
            .interpreter_debug_command_tx
            .as_ref()
            .ok_or("Debug program already finished.".to_string())?;
        let rx = &self
            .interpreter_debug_response_rx
            .as_ref()
            .ok_or("Debug program already finished.".to_string())?;

        tx.send(InterpreterCommand::GetPosition)
            .map_err(|_| "Debug program already finished.")?;

        if let InterpreterResponse::Position(p) =
            rx.recv().map_err(|_| "Debug program already finished.")?
        {
            return Ok(p);
        } else {
            return Err("Invalid response from get_position".to_string());
        }
    }

    // // next means user want to run only one step
    pub fn next(&mut self) {
        if let Some(interpreter_debug_tx) = &self.interpreter_debug_command_tx {
            interpreter_debug_tx.send(InterpreterCommand::Step).ok();
        }
    }

    pub fn evaluate(&mut self, input: String) {
        if let Some(interpreter_debug_user_tx) = &self.interpreter_debug_user_input_tx {
            for c in input.chars() {
                interpreter_debug_user_tx.send(c).ok();
            }
        }
    }

    pub fn terminate(&mut self) {
        if let Some(interpreter_debug_tx) = &self.interpreter_debug_command_tx {
            interpreter_debug_tx
                .send(InterpreterCommand::Terminate)
                .ok();
        }
    }
}

impl<'a> Drop for BrainfuckDebugInterpreter<'a> {
    fn drop(&mut self) {
        debug!(">> BrainfuckDebugInterpreter.drop()");
        self.terminate();
        debug!("BrainfuckDebugInterpreter notice debug_thread to stop.");

        let thread = mem::replace(&mut self.thread, None);
        if let Some(thread) = thread {
            thread.join().ok();
            debug!("BrainfuckDebugInterpreter debug_thread drop.");
        }
        debug!("<< BrainfuckDebugInterpreter.drop()");
    }
}

#[derive(Debug, PartialEq)]
pub enum StoppedReasonEnum {
    Breakpoint,
    Step,
    Complete,
    Terminated,
}

pub enum OutputCategoryEnum {
    Console,
    StdOut,
}

#[derive(PartialEq)]
pub enum InterpreterState {
    Running,
    Paused(Token),
    Step,
    Terminated,
}

pub enum InterpreterCommand {
    Step,
    Continue,
    GetPosition,
    Pause,
    Terminate,
    UpdateBreakpoints(Vec<BrainfuckBreakpoint>),
}

pub enum InterpreterResponse {
    Error,
    Position(Position),
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
    let breakpoint_lines: Vec<Position> = vec![Position::new(0, 0), Position::new(6, 9)];
    brainfuck_debug_interpreter.add_and_validate_breakpoint(0, None);
    brainfuck_debug_interpreter.add_and_validate_breakpoint(6, Some(9));

    let callback = |reason: StoppedReasonEnum, loc, id| {
        callback_hit += 1;
    };
    brainfuck_debug_interpreter.launch(Some(Box::new(callback)), None);
    for i in 0..(255 * 255 * 255 + 1) {
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
