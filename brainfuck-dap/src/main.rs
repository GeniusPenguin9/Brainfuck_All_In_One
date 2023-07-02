use std::{
    env::{self, current_dir},
    fs,
    mem::{self},
    path::Path,
    sync::{Arc, Mutex},
};

use base64::engine::general_purpose::STANDARD_NO_PAD as base64_encoder;
use base64::Engine as _;
use brainfuck_interpreter::{
    BrainfuckDebugInterpreter, OutputCategoryEnum, Position, StoppedReasonEnum,
};
use dap::{DapService, EventPoster};
use serde::{Deserialize, Serialize};
use simplelog::*;
use std::fs::File;
mod dap;

struct UserData<'a> {
    event_poster: EventPoster,
    runtime: Arc<Mutex<RunningState<'a>>>,
}

enum RunningState<'a> {
    Idle,
    LaunchReady(Option<BrainfuckDebugInterpreter<'a>>),
    Running(Option<BrainfuckDebugInterpreter<'a>>),
    Terminated(BrainfuckDebugInterpreter<'a>),
}

impl<'a> UserData<'a> {
    fn state_error(&self, event_poster: &mut EventPoster, state: &str, command: &str) {
        error!("not expecting {} in state {}!", command, state);
        event_poster.queue_event(&Event::<OutputEventBody>::new(
            "console".to_string(),
            format!("not expecting {} in state {}!", command, state),
        ));
        panic!("not expecting {} in state {}!", command, state);
    }

    fn initialize(
        &mut self,
        _initialize_requst_args: Option<InitializeRequestArguments>,
    ) -> Result<Capabilities, String> {
        self.event_poster.queue_event(&InitializeEvent::new());

        let mut event_poster = self.event_poster.clone();
        let initialize_message = format!(
            "Current working dictionary = {:?}\n",
            current_dir().unwrap()
        );
        event_poster.queue_event(&Event::<OutputEventBody>::new(
            "console".to_string(),
            initialize_message,
        ));

        Ok(Capabilities {
            supports_configuration_done_request: Some(true),
            supports_single_thread_execution_requests: Some(true),
            supports_read_memory_request: Some(true),
        })
    }

    fn set_breakpoints(
        &mut self,
        set_breakpoints_request_args: Option<SetBreakpointsArguments>,
    ) -> Result<Breakpoints, String> {
        info!(">> brainfuck-dap/main set_breakpoints function");
        let mut result = vec![];
        let mut event_poster = self.event_poster.clone();

        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match &mut *current_runtime_lock {
                RunningState::Idle => {
                    self.state_error(&mut event_poster, "Idle", "set_breakpoints")
                }
                RunningState::LaunchReady(brainfuck_interpreter) => {
                    // interpreter.clear_breakpoints();
                    result = set_breakpoints_impl(
                        set_breakpoints_request_args,
                        brainfuck_interpreter.as_mut().unwrap(),
                    );
                }
                RunningState::Running(brainfuck_interpreter) => {
                    if let Some(interpreter) = brainfuck_interpreter.as_mut() {
                        interpreter.clear_breakpoints();
                        result = set_breakpoints_impl(set_breakpoints_request_args, interpreter);
                        interpreter.update_runtime_breakpoints();
                    }
                }
                RunningState::Terminated(_) => {
                    self.state_error(&mut event_poster, "Terminated", "set_breakpoints")
                }
            }
        };
        info!("<< brainfuck-dap/main set_breakpoints function. Successful.");
        Ok(Breakpoints {
            breakpoints: result,
        })
    }

    fn launch(
        &mut self,
        launch_request_args: Option<LaunchRequestArguments>,
    ) -> Result<(), String> {
        info!(">> brainfuck-dap/main launch function");
        let mut event_poster = self.event_poster.clone();

        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match *current_runtime_lock {
                RunningState::Idle => {
                    let mut brainfuck_debug_interpreter = BrainfuckDebugInterpreter::from_file(
                        &launch_request_args.unwrap().program,
                    )?;

                    brainfuck_debug_interpreter.clear_breakpoints();
                    info!("brainfuck_debug_interpreter init completed.");

                    *current_runtime_lock =
                        RunningState::LaunchReady(Some(brainfuck_debug_interpreter));
                }
                RunningState::LaunchReady(_) => {
                    self.state_error(&mut event_poster, "LaunchReady", "launch")
                }
                RunningState::Running(_) => {
                    self.state_error(&mut event_poster, "Running", "launch")
                }
                RunningState::Terminated(_) => {
                    let source_content = fs::read_to_string(launch_request_args.unwrap().program)
                        .expect("Should have been able to read the file");
                    let mut brainfuck_debug_interpreter =
                        BrainfuckDebugInterpreter::new(source_content);

                    brainfuck_debug_interpreter.clear_breakpoints();
                    info!("brainfuck_debug_interpreter init completed.");

                    *current_runtime_lock =
                        RunningState::LaunchReady(Some(brainfuck_debug_interpreter));
                }
            }
        };
        info!("<< brainfuck-dap/main launch function. Successful.");
        Ok(())
    }

    fn configuration_done(
        &mut self,
        _configuration_done_request_args: Option<ConfigurationDoneRequestArguments>,
    ) -> Result<(), String> {
        info!(">> brainfuck-dap/main configuration_done function");
        let mut event_poster = self.event_poster.clone();
        let callback_runtime = self.runtime.clone();
        let breakpoint_callback =
            move |reason: StoppedReasonEnum, pos: Option<Position>, bpid: Option<usize>| {
                info!(">> Breakpoint callback with reasone = {:?}", reason);
                match reason {
                    StoppedReasonEnum::Breakpoint => {
                        event_poster.queue_event(&Event::<StoppedEventBody>::new(
                            StoppedEventBodyEnum::Breakpoint,
                            "Paused on breakpoint".to_string(),
                            bpid,
                        ));
                        event_poster.queue_event(&Event::<OutputEventBody>::new(
                            "console".to_string(),
                            format!(
                                "paused on breakpoint line {}, col {}\n",
                                pos.unwrap_or_default().line,
                                pos.unwrap_or_default().character
                            ),
                        ));
                    }
                    StoppedReasonEnum::Complete => {
                        if let Ok(mut runtime_lock) = callback_runtime.lock() {
                            match &mut *runtime_lock {
                                RunningState::Running(brainfuck_interpreter) => {
                                    let interpreter = mem::replace(brainfuck_interpreter, None);
                                    *runtime_lock = RunningState::Terminated(interpreter.unwrap());
                                }
                                _ => (),
                            };
                        };
                        event_poster.queue_event(&Event::<TerminatedEventBody>::new());
                        event_poster.queue_event(&Event::<ExitedEventBody>::new(0));
                    }
                    StoppedReasonEnum::Terminated => {
                        if let Ok(mut runtime_lock) = callback_runtime.lock() {
                            match &mut *runtime_lock {
                                RunningState::Running(brainfuck_interpreter) => {
                                    let interpreter = mem::replace(brainfuck_interpreter, None);
                                    *runtime_lock = RunningState::Terminated(interpreter.unwrap());
                                }
                                _ => (),
                            };
                        };
                        event_poster.queue_event(&Event::<TerminatedEventBody>::new());
                        event_poster.queue_event(&Event::<ExitedEventBody>::new(-1));
                    }
                    StoppedReasonEnum::Step => {
                        event_poster.queue_event(&Event::<StoppedEventBody>::new(
                            StoppedEventBodyEnum::Step,
                            "Paused On Step".to_string(),
                            None,
                        ))
                    }
                };
            };

        let mut event_poster = self.event_poster.clone();
        let mut event_poster2 = self.event_poster.clone();
        let output_callback =
            move |output_category: OutputCategoryEnum, output: String| match output_category {
                OutputCategoryEnum::Console => event_poster.queue_event(
                    &Event::<OutputEventBody>::new("console".to_string(), output),
                ),
                OutputCategoryEnum::StdOut => event_poster
                    .queue_event(&Event::<OutputEventBody>::new("stdout".to_string(), output)),
                OutputCategoryEnum::MemoryEvent((offset, length)) => {
                    event_poster.queue_event(&Event::<MemoryEventBody>::new(offset, length))
                }
            };

        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match &mut *current_runtime_lock {
                RunningState::Idle => {
                    self.state_error(&mut event_poster2, "Idle", "configuration_done")
                }
                RunningState::LaunchReady(brainfuck_interpreter) => {
                    let interpreter = mem::replace(brainfuck_interpreter, None);
                    if let Some(mut interpreter) = interpreter {
                        interpreter.launch(
                            Some(Box::new(breakpoint_callback)),
                            Some(Box::new(output_callback)),
                        );
                        info!("brainfuck_debug_interpreter launch completed.");

                        *current_runtime_lock = RunningState::Running(Some(interpreter));
                    }
                }
                RunningState::Running(_) => {
                    self.state_error(&mut event_poster2, "Running", "configuration_done")
                }
                RunningState::Terminated(_) => {
                    self.state_error(&mut event_poster2, "Terminated", "configuration_done")
                }
            }
        };
        Ok(())
    }

    fn run(
        &mut self,
        _continue_request_args: Option<ContinueRequestArguments>,
    ) -> Result<(), String> {
        let mut event_poster = self.event_poster.clone();
        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match &mut *current_runtime_lock {
                RunningState::Idle => self.state_error(&mut event_poster, "Idle", "run"),
                RunningState::LaunchReady(_) => {
                    self.state_error(&mut event_poster, "LaunchReady", "run")
                }
                RunningState::Running(brainfuck_interpreter) => {
                    brainfuck_interpreter.as_mut().unwrap().run()?;
                }
                RunningState::Terminated(_) => {
                    self.state_error(&mut event_poster, "Terminated", "run")
                }
            }
        };
        Ok(())
    }

    fn next(&mut self, _next_request_args: Option<NextRequestArguments>) -> Result<(), String> {
        let mut event_poster = self.event_poster.clone();
        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match &mut *current_runtime_lock {
                RunningState::Idle => self.state_error(&mut event_poster, "Idle", "next"),
                RunningState::LaunchReady(_) => {
                    self.state_error(&mut event_poster, "LaunchReady", "next")
                }
                RunningState::Running(brainfuck_interpreter) => {
                    brainfuck_interpreter.as_mut().unwrap().next();
                }
                RunningState::Terminated(_) => {
                    self.state_error(&mut event_poster, "Terminated", "next")
                }
            }
        };
        Ok(())
    }

    fn disconnect(
        &mut self,
        _disconnect_request_args: Option<DisconnectRequestArguments>,
    ) -> Result<(), String> {
        // Precondition: not support `attach` request in brainfuck-dap
        // 1. The disconnect request asks the debug adapter to disconnect from the debuggee (thus ending the debug session)
        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match &mut *current_runtime_lock {
                RunningState::Idle => (),
                RunningState::LaunchReady(brainfuck_interpreter) => {
                    let interpreter = mem::replace(brainfuck_interpreter, None);
                    *current_runtime_lock = RunningState::Terminated(interpreter.unwrap());
                }
                RunningState::Running(brainfuck_interpreter) => {
                    let interpreter = mem::replace(brainfuck_interpreter, None);
                    *current_runtime_lock = RunningState::Terminated(interpreter.unwrap());
                }
                RunningState::Terminated(_) => (),
            }
        };

        // 2. The disconnect request asks the debug adapter to shut down itself (the debug adapter).
        Err("Disconnect".to_string())
    }

    fn terminate(
        &mut self,
        _terminate_request_args: Option<TerminateRequestArguments>,
    ) -> Result<(), String> {
        let mut event_poster = self.event_poster.clone();
        // The terminate request is sent from the client to the debug adapter in order to shut down the debuggee gracefully.
        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match &mut *current_runtime_lock {
                RunningState::Idle => self.state_error(&mut event_poster, "Idle", "terminate"),
                RunningState::LaunchReady(brainfuck_interpreter) => {
                    let interpreter = mem::replace(brainfuck_interpreter, None);
                    *current_runtime_lock = RunningState::Terminated(interpreter.unwrap());
                }
                RunningState::Running(brainfuck_interpreter) => {
                    let interpreter = mem::replace(brainfuck_interpreter, None);
                    *current_runtime_lock = RunningState::Terminated(interpreter.unwrap());
                }
                RunningState::Terminated(_) => (),
            }
        };
        Ok(())
    }

    fn evaluate(
        &mut self,
        evaluate_request_args: Option<EvaluateRequestArguments>,
    ) -> Result<(), String> {
        info!(">> receive user input via evaluate request");
        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match &mut *current_runtime_lock {
                RunningState::Idle => (),
                RunningState::LaunchReady(_) => (),
                RunningState::Running(brainfuck_interpreter) => {
                    brainfuck_interpreter
                        .as_mut()
                        .unwrap()
                        .evaluate(evaluate_request_args.unwrap().expression);
                }
                RunningState::Terminated(_) => (),
            }
        };
        Ok(())
    }

    fn scopes(
        &mut self,
        _threads_request_args: Option<ScopesRequestArguments>,
    ) -> Result<ScopesResponse, String> {
        Ok(ScopesResponse {
            scopes: vec![Scope {
                name: "default".to_string(),
                variables_reference: 1,
                expensive: false,
            }],
        })
    }

    fn variables(
        &mut self,
        _variables_request_args: Option<VariablesRequestArguments>,
    ) -> Result<VariablesResponse, String> {
        info!(">> receive variables request");

        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match &mut *current_runtime_lock {
                RunningState::Idle => (),
                RunningState::LaunchReady(_) => (),
                RunningState::Running(brainfuck_interpreter) => {
                    let interpreter = brainfuck_interpreter.as_mut().unwrap();
                    let pos = interpreter.get_variables()?;
                    let variables: Vec<Variable> = pos
                        .into_iter()
                        .map(|(name, value)| Variable {
                            name,
                            value,
                            variables_reference: 0,
                            memory_reference: "1".to_string(),
                        })
                        .collect();
                    return Ok(VariablesResponse { variables });
                }
                RunningState::Terminated(_) => (),
            }
        };
        let r = VariablesResponse { variables: vec![] };
        Ok(r)
    }

    fn read_memory(
        &mut self,
        read_memory_args: Option<ReadMemoryRequestArguments>,
    ) -> Result<ReadMemoryResponse, String> {
        info!(">> receive read_memory request");

        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match &mut *current_runtime_lock {
                RunningState::Idle => (),
                RunningState::LaunchReady(_) => (),
                RunningState::Running(brainfuck_interpreter) => {
                    let interpreter = brainfuck_interpreter.as_mut().unwrap();
                    let start = read_memory_args
                        .as_ref()
                        .unwrap()
                        .offset
                        .unwrap_or(0)
                        .max(0) as usize;
                    let mem = interpreter.read_memory(start, read_memory_args.unwrap().count)?;
                    let mem_str = base64_encoder.encode(mem);
                    return Ok(ReadMemoryResponse {
                        address: start.to_string(),
                        data: mem_str,
                    });
                }
                RunningState::Terminated(_) => (),
            }
        };
        Err("invalid state".to_string())
    }

    fn threads(
        &mut self,
        _threads_request_args: Option<ThreadsRequestArguments>,
    ) -> Result<ThreadsResponse, String> {
        Ok(ThreadsResponse {
            threads: vec![Thread {
                id: 0,
                name: "default".to_string(),
            }],
        })
    }

    fn stack_trace(
        &mut self,
        _args: Option<StackTraceRequestArguments>,
    ) -> Result<StackTraceResponse, String> {
        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match &mut *current_runtime_lock {
                RunningState::Idle => (),
                RunningState::LaunchReady(_) => (),
                RunningState::Running(brainfuck_interpreter) => {
                    let interpreter = brainfuck_interpreter.as_mut().unwrap();
                    let pos = interpreter.get_position()?;
                    let frame = StackFrame {
                        id: 0,
                        name: Path::new(&interpreter.get_filename())
                            .file_name()
                            .unwrap_or_default()
                            .to_str()
                            .unwrap_or_default()
                            .to_string(),
                        source: StackFrameSource {
                            name: Path::new(&interpreter.get_filename())
                                .file_name()
                                .unwrap_or_default()
                                .to_str()
                                .unwrap_or_default()
                                .to_string(),
                            path: interpreter.get_filename(),
                        },
                        line: pos.line + 1,
                        column: pos.character + 1,
                    };
                    return Ok(StackTraceResponse {
                        stack_frames: vec![frame],
                        total_frames: 1,
                    });
                }
                RunningState::Terminated(_) => (),
            }
        };
        Err("unknown error".to_string())
    }
}

fn set_breakpoints_impl(
    set_breakpoints_request_args: Option<SetBreakpointsArguments>,
    interpreter: &mut BrainfuckDebugInterpreter<'_>,
) -> Vec<Breakpoint> {
    let mut result = vec![];
    for bp in &set_breakpoints_request_args
        .unwrap()
        .breakpoints
        .unwrap_or_default()
    {
        let breakpoint_validate_result = interpreter
            .add_and_validate_breakpoint(bp.line as u32 - 1, bp.column.map(|x| x as u32 - 1));
        if let Some(breakpoint) = breakpoint_validate_result {
            let verify_result = Breakpoint {
                id: Some(breakpoint.id),
                verified: true,
                line: breakpoint.position.line + 1,
                column: breakpoint.position.character + 1,
            };
            result.push(verify_result);
        } else {
            result.push(Breakpoint {
                id: None,
                verified: false,
                line: 0,
                column: 0,
            })
        }
    }
    result
}

/* ----------------- initialize ----------------- */
#[derive(Deserialize)]
struct InitializeRequestArguments {
    // #[serde(rename(deserialize = "adapterID"))]
    // adapter_id: String,
}

#[derive(Serialize)]
struct InitializeEvent {
    #[serde(rename(serialize = "type"))]
    event_type: String,
    event: String,
}

impl InitializeEvent {
    pub fn new() -> Self {
        InitializeEvent {
            event_type: "event".to_string(),
            event: "initialized".to_string(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Capabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    supports_configuration_done_request: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    supports_single_thread_execution_requests: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    supports_read_memory_request: Option<bool>,
}

/* ----------------- set_breakpoints ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetBreakpointsArguments {
    // source: Source,
    breakpoints: Option<Vec<SourceBreakpoint>>,
    // source_modified: Option<bool>,
}

/**
 * Ignore fields:
 * adapterData?: any;
 * checksums?: Checksum[];
 */
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    name: Option<String>,
    path: Option<String>,
    source_reference: Option<usize>,
    presentation_hint: Option<PresentationHintEnum>,
    origin: Option<String>,
    sources: Option<Vec<Source>>,
}
#[derive(Deserialize, Serialize)]
enum PresentationHintEnum {
    #[serde(rename(serialize = "normal"))]
    Normal,
    #[serde(rename(serialize = "emphasize"))]
    Emphasize,
    #[serde(rename(serialize = "deemphasize"))]
    Deemphasize,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceBreakpoint {
    line: usize,
    column: Option<usize>,
    // condition: Option<String>,
    // hit_condition: Option<String>,
    // log_message: Option<String>,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Breakpoint {
    id: Option<usize>,
    verified: bool,
    line: u32,
    column: u32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Breakpoints {
    breakpoints: Vec<Breakpoint>,
}

#[derive(Serialize)]
struct Event<T> {
    #[serde(rename(serialize = "type"))]
    event_type: String,
    event: String,
    body: T,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StoppedEventBody {
    reason: StoppedEventBodyEnum,
    description: String,
    thread_id: u32,
    preserve_focus_hint: bool,
    text: String,
    all_threads_stopped: bool,
    hit_breakpoint_ids: Vec<usize>,
}
#[derive(Serialize)]
enum StoppedEventBodyEnum {
    #[serde(rename(serialize = "step"))]
    Step,
    #[serde(rename(serialize = "breakpoint"))]
    Breakpoint,
    // #[serde(rename(serialize = "exception"))]
    // Exception,
    // #[serde(rename(serialize = "pause"))]
    // Pause,
    // #[serde(rename(serialize = "entry"))]
    // Entry,
    // #[serde(rename(serialize = "goto"))]
    // GoTo,
    // #[serde(rename(serialize = "function breakpoint"))]
    // FunctionBreakpoint,
    // #[serde(rename(serialize = "data breakpoint"))]
    // DataBreakpoint,
    // #[serde(rename(serialize = "instruction breakpoint"))]
    // InstructionBreakpoint,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExitedEventBody {
    exit_code: i32,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputEventBody {
    category: String,
    output: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryEventBody {
    memory_reference: String,
    offset: usize,
    count: usize,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminatedEventBody {}

impl Event<StoppedEventBody> {
    fn new(
        reason: StoppedEventBodyEnum,
        text: String,
        breakpoint_id: Option<usize>,
    ) -> Event<StoppedEventBody> {
        Event {
            event_type: "event".to_string(),
            event: "stopped".to_string(),
            body: StoppedEventBody {
                reason,
                description: text.clone(),
                thread_id: 0,
                preserve_focus_hint: false,
                text: text,
                all_threads_stopped: true,
                hit_breakpoint_ids: if breakpoint_id.is_some() {
                    vec![breakpoint_id.unwrap()]
                } else {
                    vec![]
                },
            },
        }
    }
}

impl Event<ExitedEventBody> {
    fn new(exit_code: i32) -> Event<ExitedEventBody> {
        Event {
            event_type: "event".to_string(),
            event: "exited".to_string(),
            body: ExitedEventBody { exit_code },
        }
    }
}

impl Event<OutputEventBody> {
    fn new(category: String, output: String) -> Event<OutputEventBody> {
        Event {
            event_type: "event".to_string(),
            event: "output".to_string(),
            body: OutputEventBody { category, output },
        }
    }
}

impl Event<TerminatedEventBody> {
    fn new() -> Event<TerminatedEventBody> {
        Event {
            event_type: "event".to_string(),
            event: "terminated".to_string(),
            body: TerminatedEventBody {},
        }
    }
}

impl Event<MemoryEventBody> {
    fn new(offset: usize, count: usize) -> Event<MemoryEventBody> {
        Event {
            event_type: "event".to_string(),
            event: "memory".to_string(),
            body: MemoryEventBody {
                memory_reference: "1".to_string(),
                offset,
                count,
            },
        }
    }
}

/* ----------------- launch ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LaunchRequestArguments {
    program: String,
}
/* ----------------- configuration_done ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigurationDoneRequestArguments {}
/* ----------------- continue ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContinueRequestArguments {
    // thread_id: usize,
}

/* ----------------- next ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NextRequestArguments {
    // thread_id: usize,
}
/* ----------------- disconnect ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DisconnectRequestArguments {
    // restart: Option<bool>,
}
/* ----------------- terminate ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminateRequestArguments {
    // restart: Option<bool>,
}
/* ----------------- evaluate ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EvaluateRequestArguments {
    expression: String,
}
/* ----------------- variables ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VariablesRequestArguments {
    // variables_reference: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VariablesResponse {
    variables: Vec<Variable>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Variable {
    name: String,
    value: String,
    variables_reference: usize,
    memory_reference: String,
}

/* ----------------- read_memory ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadMemoryRequestArguments {
    // memory_reference: String,
    offset: Option<i64>,
    count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadMemoryResponse {
    address: String,
    data: String,
}

/* ----------------- scopes ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScopesRequestArguments {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScopesResponse {
    scopes: Vec<Scope>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Scope {
    name: String,
    variables_reference: usize,
    expensive: bool,
}

/* ----------------- threads ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadsRequestArguments {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ThreadsResponse {
    threads: Vec<Thread>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Thread {
    id: usize,
    name: String,
}
/* ----------------- stackTrace ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StackTraceRequestArguments {
    // thread_id: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StackTraceResponse {
    stack_frames: Vec<StackFrame>,
    total_frames: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StackFrameSource {
    name: String,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StackFrame {
    id: usize,
    name: String,
    source: StackFrameSource,
    line: u32,
    column: u32,
}

/* ----------------- main ----------------- */

fn main() {
    let log_level = match env::var("DAP_LOG_LEVEL") {
        Ok(l) => match l.as_str() {
            "OFF" => LevelFilter::Off,
            "ERROR" => LevelFilter::Error,
            "WARN" => LevelFilter::Warn,
            "INFO" => LevelFilter::Info,
            "DEBUG" => LevelFilter::Debug,
            "TRACE" => LevelFilter::Trace,
            _ => LevelFilter::Debug,
        },
        Err(_) => LevelFilter::Debug,
    };
    CombinedLogger::init(vec![WriteLogger::new(
        log_level,
        Config::default(),
        File::create("brainfuck_interpreter.log").unwrap(),
    )])
    .unwrap();
    info!(">> brainfuck-dap main");
    let mut dap_service = DapService::new_with_poster(|event_poster| UserData {
        event_poster,
        runtime: Arc::new(Mutex::new(RunningState::Idle)),
    })
    .register("initialize".to_string(), Box::new(UserData::initialize))
    .register(
        "setBreakpoints".to_string(),
        Box::new(UserData::set_breakpoints),
    )
    .register("launch".to_string(), Box::new(UserData::launch))
    .register(
        "configurationDone".to_string(),
        Box::new(UserData::configuration_done),
    )
    .register("continue".to_string(), Box::new(UserData::run))
    .register("next".to_string(), Box::new(UserData::next))
    .register("stepIn".to_string(), Box::new(UserData::next))
    .register("stepOut".to_string(), Box::new(UserData::next))
    .register("disconnect".to_string(), Box::new(UserData::disconnect))
    .register("terminate".to_string(), Box::new(UserData::terminate))
    .register("evaluate".to_string(), Box::new(UserData::evaluate))
    .register("variables".to_string(), Box::new(UserData::variables))
    .register("readMemory".to_string(), Box::new(UserData::read_memory))
    .register("scopes".to_string(), Box::new(UserData::scopes))
    .register("threads".to_string(), Box::new(UserData::threads))
    .register("stackTrace".to_string(), Box::new(UserData::stack_trace))
    .build();
    dap_service.start();
    info!("<< brainfuck-dap main");
}

/* ----------------- test ----------------- */

// #[test]
// fn test_initialization_request() {
//     use std::io::{Read, Write};
//     use std::process::{Command, Stdio};
//     use std::{thread, time};

//     let mut child = Command::new("cargo")
//         .args(["run"])
//         .stdin(Stdio::piped())
//         .stdout(Stdio::piped())
//         .spawn()
//         .expect("Failed during cargo run");

//     let child_stdin = child.stdin.as_mut().unwrap();
//     let child_stdout = child.stdout.as_mut().unwrap();
//     let initialization_request = "Content-Length: 128\r\n\r\n{\r\n    \"seq\": 153,\r\n    \"type\": \"request\",\r\n    \"command\": \"initialize\",\r\n    \"arguments\": {\r\n        \"adapterID\": \"a\"\r\n    }\r\n}\r\n";
//     child_stdin
//         .write_all(initialization_request.as_bytes())
//         .unwrap();
//     // Close stdin to finish and avoid indefinite blocking
//     drop(child_stdin);
//     thread::sleep(time::Duration::from_secs(5));

//     let mut read_buf: [u8; 300] = [0; 300];
//     child_stdout.read(&mut read_buf).unwrap();
//     child.kill().unwrap();

//     let actual = String::from_utf8(read_buf.to_vec()).unwrap();
//     println!("Get actual response = {}", actual);
//     assert!(actual.contains("Content-Length: 129\r\n\r\n{\"type\":\"response\",\"request_seq\":153,\"success\":true,\"command\":\"initialize\",\"body\":{\"supportsSingleThreadExecutionRequests\":true}}\r\nContent-Length: 38\r\n\r\n{\"type\":\"event\",\"event\":\"initialized\"}"));
// }

// #[test]
// fn test_launch_request() {
//     use std::io::{Read, Write};
//     use std::process::{Command, Stdio};
//     use std::{thread, time};

//     let mut child = Command::new("cargo")
//         .args(["run"])
//         .stdin(Stdio::piped())
//         .stdout(Stdio::piped())
//         .spawn()
//         .expect("Failed during cargo run");

//     let child_stdin = child.stdin.as_mut().unwrap();
//     let child_stdout = child.stdout.as_mut().unwrap();
//     let launch_request = "Content-Length: 233\r\n\r\n{\"command\": \"launch\",\"arguments\": {\"name\": \"Brainfuck-Debug\",\"type\": \"brainfuck\",\"request\": \"launch\",\"program\":\"../test.bf\",\"__configurationTarget\": 6,\"__sessionId\": \"36201e43-539a-4fd6-beb8-5e0bc2b18abe\"},\"type\": \"request\",\"seq\": 2}\r\n";
//     child_stdin.write_all(launch_request.as_bytes()).unwrap();
//     // Close stdin to finish and avoid indefinite blocking
//     drop(child_stdin);
//     thread::sleep(time::Duration::from_secs(5));

//     let mut read_buf: [u8; 300] = [0; 300];
//     child_stdout.read(&mut read_buf).unwrap();
//     child.kill().unwrap();

//     let actual = String::from_utf8(read_buf.to_vec()).unwrap();
//     println!("Get actual response = {}", actual);
//     assert!(actual.contains("Content-Length: 81\r\n\r\n{\"type\":\"response\",\"request_seq\":2,\"success\":true,\"command\":\"launch\",\"body\":null}"));
// }

// #[test]
// fn test_unknown_request() {
//     use std::io::{Read, Write};
//     use std::process::{Command, Stdio};
//     use std::{thread, time};

//     let mut child = Command::new("cargo")
//         .args(["run"])
//         .stdin(Stdio::piped())
//         .stdout(Stdio::piped())
//         .spawn()
//         .expect("Failed during cargo run");

//     let child_stdin = child.stdin.as_mut().unwrap();
//     let child_stdout = child.stdout.as_mut().unwrap();
//     let unknown_request = "Content-Length: 85\r\n\r\n{\"command\": \"abcdefghij\",\"arguments\": {\"restart\": false},\"type\": \"request\",\"seq\": 1}\r\n";
//     child_stdin.write_all(unknown_request.as_bytes()).unwrap();
//     // Close stdin to finish and avoid indefinite blocking
//     drop(child_stdin);
//     thread::sleep(time::Duration::from_secs(5));

//     let mut read_buf: [u8; 300] = [0; 300];
//     child_stdout.read(&mut read_buf).unwrap();
//     child.kill().unwrap();

//     let actual = String::from_utf8(read_buf.to_vec()).unwrap();
//     println!("Get actual response = {}", actual);
//     assert!(actual.contains("Content-Length: 74\r\n\r\n{\"type\":\"response\",\"request_seq\":1,\"success\":false,\"command\":\"abcdefghij\"}"));
// }
