use std::{
    io::Read,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use crate::jit::{compile, run, IBrainfuckMemory, JITCache};
use brainfuck_analyzer::{parse, Range, Token, TokenGroup, TokenType};

struct SubGroupCache {
    range: Range,
    jit_cache: Option<JITCache>,
    hit_count: usize,
}

pub struct AutoJITBrainfuckMemory {
    pub index: usize,
    pub memory: Vec<u8>,
}

impl IBrainfuckMemory for AutoJITBrainfuckMemory {
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

impl AutoJITBrainfuckMemory {
    pub fn new() -> AutoJITBrainfuckMemory {
        AutoJITBrainfuckMemory {
            index: 0,
            memory: vec![0; 1000],
        }
    }

    fn interpret_token(
        &mut self,
        token: &Token,
        sub_group_cache_stack: &mut Vec<SubGroupCache>,
        m2j_tx: &Sender<(Range, TokenGroup)>,
        j2m_tx: &Receiver<(Range, JITCache)>,
    ) {
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
                let mut need_pop = false;
                while self.memory[self.index] != 0 {
                    need_pop = true;

                    // try to receive jit analysis result and update cache
                    while let Result::Ok((jit_range, jit_cache)) = j2m_tx.try_recv() {
                        for current_item in sub_group_cache_stack.iter_mut() {
                            if (current_item.range).eq(&jit_range) {
                                current_item.jit_cache = Some(jit_cache);
                                break;
                            }
                        }
                    }

                    // update stack
                    if sub_group_cache_stack.is_empty()
                        || (sub_group_cache_stack.last().unwrap().range != token.range)
                    {
                        sub_group_cache_stack.push(SubGroupCache {
                            range: token.range,
                            jit_cache: None,
                            hit_count: 1,
                        });
                    } else {
                        sub_group_cache_stack.last_mut().unwrap().hit_count += 1;
                    }

                    if let Some(jit_cache) = &sub_group_cache_stack.last().unwrap().jit_cache {
                        run(jit_cache, self);
                    } else {
                        if sub_group_cache_stack.last().unwrap().hit_count == 3 {
                            m2j_tx.send((token.range, *sg.clone())).unwrap();
                        }

                        for token in sg.tokens().into_iter() {
                            self.interpret_token(token, sub_group_cache_stack, m2j_tx, j2m_tx);
                        }
                    }
                }

                // clear sub group from stack
                if need_pop {
                    sub_group_cache_stack.truncate(sub_group_cache_stack.len() - 1);
                }
            }
            _ => (),
        }
    }
}

pub fn interpret_auto_jit(input: &str) {
    let parse_result = parse(input).unwrap();
    let token_group = parse_result.parse_token_group;

    let mut memory = AutoJITBrainfuckMemory::new();
    main_thread(&mut memory, &token_group);
}

fn main_thread(memory: &mut AutoJITBrainfuckMemory, token_group: &TokenGroup) {
    let (m2j_tx, m2j_rx) = mpsc::channel();
    let (j2m_tx, j2m_rx) = mpsc::channel();

    thread::spawn(move || {
        jit_thread(m2j_rx, j2m_tx);
    });

    let mut sub_group_cache_stack = vec![];
    for token in token_group.tokens().into_iter() {
        memory.interpret_token(token, &mut sub_group_cache_stack, &m2j_tx, &j2m_rx);
    }
}

fn jit_thread(m2j_rx: Receiver<(Range, TokenGroup)>, j2m_tx: Sender<(Range, JITCache)>) {
    loop {
        match m2j_rx.recv() {
            Result::Err(_) => {
                break;
            }
            Result::Ok(received) => {
                let jit_cache = compile(&received.1);
                if let Err(_) = j2m_tx.send((received.0, jit_cache)) {
                    break;
                };
            }
        }
    }
}

#[test]
pub fn test_auto_jit_simple() {
    let input = ">>++<-";
    let parse_result = parse(input).unwrap();
    let token_group = parse_result.parse_token_group;

    let mut memory = AutoJITBrainfuckMemory::new();
    main_thread(&mut memory, &token_group);
    assert_eq!(2, memory.memory[2]);
    assert_eq!(u8::MAX, memory.memory[1]);
    assert_eq!(1, memory.index);
}

#[test]
pub fn test_auto_jit_with_loop() {
    let input = ">>+++++++++++++++++++++++++++++++++<+++++[>.<-]";
    interpret_auto_jit(input);
    // should find five "!" in test terminal
}

#[test]
pub fn test_auto_jit_with_memory_extension() {
    let input = ">>>>++";
    let parse_result = parse(input).unwrap();
    let token_group = parse_result.parse_token_group;

    let mut memory = AutoJITBrainfuckMemory::new();
    memory.memory = vec![0; 3];

    main_thread(&mut memory, &token_group);
    assert_eq!(6, memory.memory.len());
    assert_eq!(4, memory.index);
    assert_eq!(2, memory.memory[4]);
}
