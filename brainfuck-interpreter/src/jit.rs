use std::mem::transmute;

use crate::interpreter::BrainfuckMemory;
use assembler::mnemonic_parameter_types::memory::{Memory, MemoryOperand};
use assembler::mnemonic_parameter_types::registers::Register64Bit::*;
use assembler::mnemonic_parameter_types::registers::Register8Bit::*;
use assembler::ExecutableAnonymousMemoryMap::ExecutableAnonymousMemoryMap;
use assembler::InstructionStream::InstructionStream;
use assembler::InstructionStreamHints::InstructionStreamHints;
use brainfuck_analyzer::{parse, TokenGroup, TokenType};

pub struct JITCache {
    #[allow(unused_variables, dead_code)]
    function_pointer:
        unsafe extern "sysv64" fn(mem: *const u8, offset: u64, struct_ptr: *const u8) -> u64,
    #[allow(unused_variables, dead_code)]
    memory_map: ExecutableAnonymousMemoryMap,
}
unsafe impl Send for JITCache {}

pub trait IBrainfuckRuntime {
    fn get_memory_vec_ptr(&self) -> *const u8;
    fn get_index(&self) -> usize;
    fn set_index(&mut self, new_index: usize);
}

pub fn compile(input: &TokenGroup) -> JITCache {
    // TODO: should support memory allocation increasement
    let mut memory_map =
        ExecutableAnonymousMemoryMap::new(4096, false, true).expect("Could not anonymously mmap");

    let mut instruction_stream = memory_map.instruction_stream(&InstructionStreamHints::default());

    instruction_stream.emit_alignment(64);

    // use current position as fuction_pointer_head
    let function_pointer_head: unsafe extern "sysv64" fn(
        mem: *const u8,
        offset: u64,
        struct_ptr: *const u8,
    ) -> u64 = unsafe {
        transmute(instruction_stream.ternary_function_pointer::<u64, *const u8, u64, *const u8>())
    };

    _compile(input, &mut instruction_stream);

    // copy offset into return value
    instruction_stream.mov_Register64Bit_Register64Bit_r64_rm64(RAX, RSI);

    // Caller should clean up stack. So just pop rip and jump the this address.
    // ref data: https://en.wikipedia.org/wiki/X86_calling_conventions
    // #List of x86 calling conventions #System V AMD64 ABI
    instruction_stream.ret();

    instruction_stream.finish();

    JITCache {
        function_pointer: function_pointer_head,
        memory_map,
    }
}

fn _compile(input: &TokenGroup, instruction_stream: &mut InstructionStream) {
    // RDI pointer to the head of brainfuck memory(vec part)
    // RSI = current offset in brainfuck memory
    // RDX = pointer to the head of BrainfuckMemory struct
    // ref data: https://github.com/phip1611/rust-different-calling-conventions-example
    for t in input.tokens().into_iter() {
        match &t.token_type {
            TokenType::PointerDecrement => instruction_stream.dec_Register64Bit(RSI),
            TokenType::PointerIncrement => {
                // jump to rust function runtime_resize, rust function will resize runtime memory if needed
                // push stack: RSI, RDX
                // here we don't really push RDI, because the head of brainfuck memory(vec part) may change after resize. Always use fn return value as RDI.
                instruction_stream.push_Register64Bit_r64(RSI);
                instruction_stream.push_Register64Bit_r64(RDX);

                // move runtime(RDX) to RDI(the first param of function runtime_resize)
                instruction_stream.mov_Register64Bit_Register64Bit_r64_rm64(RDI, RDX);
                // [NO CODE] move RSI to RSI

                // call function
                let fn_ptr: u64 = unsafe {
                    transmute::<
                        unsafe extern "sysv64" fn(
                            runtime: &mut BrainfuckMemory,
                            current_index: u8,
                        ) -> *const u8,
                        u64,
                    >(runtime_resize)
                };
                instruction_stream.mov_Register64Bit_Immediate64Bit(RAX, fn_ptr.into());
                instruction_stream.call_Register64Bit(RAX);

                //pop stack: RSI, RDI
                instruction_stream.pop_Register64Bit_r64(RDX);
                instruction_stream.pop_Register64Bit_r64(RSI);
                // here we don't really pop RDI. Always use fn return value as RDI.
                instruction_stream.mov_Register64Bit_Register64Bit_r64_rm64(RDI, RAX);

                instruction_stream.inc_Register64Bit(RSI);
            }
            TokenType::Decrement => instruction_stream.sub_Any8BitMemory_Immediate8Bit(
                MemoryOperand::base_64_index_64(RDI, RSI).into(),
                1u8.into(),
            ),
            TokenType::Increment => instruction_stream.add_Any8BitMemory_Immediate8Bit(
                MemoryOperand::base_64_index_64(RDI, RSI).into(),
                1u8.into(),
            ),
            TokenType::Output => {
                // push RDI, RSI, RDX
                instruction_stream.push_Register64Bit_r64(RDI);
                instruction_stream.push_Register64Bit_r64(RSI);
                instruction_stream.push_Register64Bit_r64(RDX);

                // move RDI+RSI value (the char for print) to RDI
                // pub fn mov_Register64Bit_Any64BitMemory(&mut self, dist: Register64Bit, src: Any64BitMemory) // function name format <behavior>_<dist>_<src>
                instruction_stream.mov_Register64Bit_Any64BitMemory(
                    RDI,
                    MemoryOperand::base_64_index_64(RDI, RSI).into(),
                );
                // call function, input use RDI number
                let fn_ptr: u64 = unsafe {
                    transmute::<unsafe extern "sysv64" fn(c: u8) -> u8, u64>(output_char)
                };
                instruction_stream.mov_Register64Bit_Immediate64Bit(RAX, fn_ptr.into());
                instruction_stream.call_Register64Bit(RAX);

                //pop RDX, RSI, RDI
                instruction_stream.pop_Register64Bit_r64(RDX);
                instruction_stream.pop_Register64Bit_r64(RSI);
                instruction_stream.pop_Register64Bit_r64(RDI);
            }
            TokenType::Input => {
                // push RDI, RSI, RDX
                instruction_stream.push_Register64Bit_r64(RDI);
                instruction_stream.push_Register64Bit_r64(RSI);
                instruction_stream.push_Register64Bit_r64(RDX);

                // call function, return value will be saved into RAX
                // Integer return values up to 64 bits in size are stored in RAX, ref data: https://en.wikipedia.org/wiki/X86_calling_conventions
                let fn_ptr: u64 =
                    unsafe { transmute::<unsafe extern "sysv64" fn() -> u8, u64>(input_char) };
                instruction_stream.mov_Register64Bit_Immediate64Bit(RAX, fn_ptr.into());
                instruction_stream.call_Register64Bit(RAX);

                //pop RDX, RSI, RDI
                instruction_stream.pop_Register64Bit_r64(RDX);
                instruction_stream.pop_Register64Bit_r64(RSI);
                instruction_stream.pop_Register64Bit_r64(RDI);

                // move input char (RAX) into RDI+RSI
                instruction_stream.mov_Any64BitMemory_Register64Bit(
                    MemoryOperand::base_64_index_64(RDI, RSI).into(),
                    RAX,
                );
            }
            TokenType::SubGroup(sg) => {
                let loop_start_label = instruction_stream.create_and_attach_label();
                let loop_end_label = instruction_stream.create_label();

                // If the byte at the data pointer != zero, start loop
                instruction_stream.mov_Register8Bit_Any8BitMemory(
                    AL,
                    MemoryOperand::base_64_index_64(RDI, RSI).into(),
                );
                instruction_stream.cmp_Register8Bit_Immediate8Bit(AL, 0u8.into());
                instruction_stream.jz_Label_1(loop_end_label);

                // loop part
                _compile(&sg, instruction_stream);

                // jump to "["
                instruction_stream.jmp_Label_1(loop_start_label);
                instruction_stream.attach_label(loop_end_label);
            }
            _ => (),
        }
    }
}

pub fn run<T: IBrainfuckRuntime>(jit_cache: &JITCache, runtime: &mut T) {
    let new_index = unsafe {
        let runtime_memory_vec_ptr = runtime.get_memory_vec_ptr();
        let runtime_struct_ptr = transmute::<&mut T, *const u8>(runtime);
        (jit_cache.function_pointer)(
            runtime_memory_vec_ptr,
            runtime.get_index() as u64,
            runtime_struct_ptr,
        )
    };

    runtime.set_index(new_index as usize);
}

pub fn interpret_jit(input: &str) {
    let parse_result = parse(input).unwrap();
    let mut memory = BrainfuckMemory::new();
    let jit_cache = compile(&parse_result.parse_token_group);
    run(&jit_cache, &mut memory);
}

#[allow(unused_variables, dead_code)]
unsafe extern "sysv64" fn input_char() -> u8 {
    libc::getchar() as u8
}

#[allow(unused_variables, dead_code)]
unsafe extern "sysv64" fn output_char(c: u8) -> u8 {
    libc::putchar(c as i32);
    c
}

#[allow(unused_variables, dead_code)]
unsafe extern "sysv64" fn runtime_resize(
    runtime: &mut BrainfuckMemory,
    current_index: u8,
) -> *const u8 {
    if runtime.memory.len() - current_index as usize == 1 {
        // may re-alloc new part of memory and copy the original data. should return memory head pointer
        runtime.memory.resize(runtime.memory.len() * 2, 0);
    }

    &runtime.memory[0] as *const u8
}

#[test]
#[cfg(windows)]
pub fn test_jit_simple() {
    let input = ">>++<-";
    let parse_result = parse(input).unwrap();

    let mut memory = BrainfuckMemory::new();
    let jit_cache = compile(&parse_result.parse_token_group);
    run(&jit_cache, &mut memory);
    assert_eq!(2, memory.memory[2]);
    assert_eq!(u8::MAX, memory.memory[1]);
    assert_eq!(1, memory.index);
}

#[test]
#[cfg(windows)]
pub fn test_jit_with_io() {
    let input = "+++++++++++++++++++++++++++++++++.";
    let parse_result = parse(input).unwrap();

    let mut memory = BrainfuckMemory::new();
    let jit_cache = compile(&parse_result.parse_token_group);
    run(&jit_cache, &mut memory);

    // should find "!" in test terminal
}

// This test case need manual input, disable by default for auto testing
// #[test]
// pub fn test_jit_with_io2() {
//     let input = ",+.";
//     let parse_result = parse(input).unwrap();

//     let mut memory = BrainfuckMemory::new();
//     let jit_cache = compile(&parse_result.parse_token_group);
//     run(&jit_cache, &mut memory);

//     // manual input "A", should find a "B" as output
// }

#[test]
#[cfg(windows)]
pub fn test_jit_with_loop() {
    let input = "++[>+<-]";
    let parse_result = parse(input).unwrap();

    let mut memory = BrainfuckMemory::new();
    let jit_cache = compile(&parse_result.parse_token_group);
    run(&jit_cache, &mut memory);
    assert_eq!(2, memory.memory[1]);
    assert_eq!(0, memory.memory[0]);
    assert_eq!(0, memory.index);
}

#[test]
#[cfg(windows)]
pub fn test_jit_memory_extension() {
    let input = ">>>>++";
    let parse_result = parse(input).unwrap();

    let mut memory = BrainfuckMemory::new();
    memory.memory = vec![0; 3];

    let jit_cache = compile(&parse_result.parse_token_group);
    run(&jit_cache, &mut memory);

    assert_eq!(6, memory.memory.len());
    assert_eq!(4, memory.index);
    assert_eq!(2, memory.memory[4]);
}
