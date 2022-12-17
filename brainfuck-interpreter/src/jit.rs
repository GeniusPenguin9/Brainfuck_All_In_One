use std::mem::transmute;

use assembler::mnemonic_parameter_types::memory::{Memory, MemoryOperand};
use assembler::mnemonic_parameter_types::registers::Register64Bit::*;
use assembler::ExecutableAnonymousMemoryMap::ExecutableAnonymousMemoryMap;
use assembler::InstructionStreamHints::InstructionStreamHints;
use brainfuck_analyzer::{ TokenGroup, TokenType, parse};

use crate::interpreter::BrainfuckMemory;

pub struct JITCache {
    function_pointer: unsafe extern "sysv64" fn(mem: *const u8, offset: u64) -> u64,
    #[allow(unused_variables, dead_code)]
    memory_map: ExecutableAnonymousMemoryMap,
}

pub fn compile(input: &TokenGroup) -> JITCache {
    let mut memory_map =
        ExecutableAnonymousMemoryMap::new(4096, false, false).expect("Could not anonymously mmap");

    let mut instruction_stream = memory_map.instruction_stream(&InstructionStreamHints::default());

    instruction_stream.emit_alignment(64);

    // use current position as fuction_pointer_head
    let function_pointer_head: unsafe extern "sysv64" fn(mem: *const u8, offset: u64) -> u64 =
        unsafe { transmute(instruction_stream.binary_function_pointer::<u64, *const u8, u64>()) };

    // RDI pointer to the head of brainfuck memory
    // RSI = current offset in brainfuck memory
    // ref data: https://github.com/phip1611/rust-different-calling-conventions-example
    for t in input.tokens().into_iter() {
        match t.token_type {
            TokenType::PointerDecrement => instruction_stream.dec_Register64Bit(RSI),
            TokenType::PointerIncrement => instruction_stream.inc_Register64Bit(RSI),
            TokenType::Decrement => instruction_stream.sub_Any8BitMemory_Immediate8Bit(
                MemoryOperand::base_64_index_64(RDI, RSI).into(),
                1u8.into(),
            ),
            TokenType::Increment => instruction_stream.add_Any8BitMemory_Immediate8Bit(
                MemoryOperand::base_64_index_64(RDI, RSI).into(),
                1u8.into(),
            ),
            TokenType::Output => {
                // push RDI, RSI
                instruction_stream.push_Register64Bit_r64(RDI);
                instruction_stream.push_Register64Bit_r64(RSI);

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

                //pop RSI,RDI
                instruction_stream.pop_Register64Bit_r64(RSI);
                instruction_stream.pop_Register64Bit_r64(RDI);
            }
            TokenType::Input => {
                // push RDI, RSI
                instruction_stream.push_Register64Bit_r64(RDI);
                instruction_stream.push_Register64Bit_r64(RSI);

                // call function, return value will be saved into RAX
                // Integer return values up to 64 bits in size are stored in RAX, ref data: https://en.wikipedia.org/wiki/X86_calling_conventions
                let fn_ptr: u64 =
                    unsafe { transmute::<unsafe extern "sysv64" fn() -> u8, u64>(input_char) };
                instruction_stream.mov_Register64Bit_Immediate64Bit(RAX, fn_ptr.into());
                instruction_stream.call_Register64Bit(RAX);

                //pop RSI,RDI
                instruction_stream.pop_Register64Bit_r64(RSI);
                instruction_stream.pop_Register64Bit_r64(RDI);

                // move input char (RAX) into RDI+RSI
                instruction_stream.mov_Any64BitMemory_Register64Bit(
                    MemoryOperand::base_64_index_64(RDI, RSI).into(),
                    RAX,
                );
            }
            _ => (),
        }
    }

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

pub fn run(jit_cache: JITCache, runtime: &mut BrainfuckMemory) {
    let new_index = unsafe {
        let ptr = &runtime.memory[0] as *const u8;
        (jit_cache.function_pointer)(ptr, runtime.index as u64)
    };
    runtime.index = new_index as usize;
}

unsafe extern "sysv64" fn input_char() -> u8 {
    libc::getchar() as u8
}

unsafe extern "sysv64" fn output_char(c: u8) -> u8 {
    libc::putchar(c as i32);
    c
}

#[test]
pub fn test_jit_simple() {
    let input = ">>++<-";
    let parse_result = parse(input).unwrap();

    let mut memory = BrainfuckMemory::new();
    let jit_cache = compile(&parse_result.parse_token_group);
    run(jit_cache, &mut memory);
    assert_eq!(2, memory.memory[2]);
    assert_eq!(u8::MAX, memory.memory[1]);
    assert_eq!(1, memory.index);
}

#[test]
pub fn test_jit_with_io() {
    let input = "+++++++++++++++++++++++++++++++++.";
    let parse_result = parse(input).unwrap();

    let mut memory = BrainfuckMemory::new();
    let jit_cache = compile(&parse_result.parse_token_group);
    run(jit_cache, &mut memory);

    // should find "!" in test terminal
}

#[test]
pub fn test_jit_with_io2() {
    let input = ",+.";
    let parse_result = parse(input).unwrap();

    let mut memory = BrainfuckMemory::new();
    let jit_cache = compile(&parse_result.parse_token_group);
    run(jit_cache, &mut memory);

    // manual input "A", should find a "B" as output
}
