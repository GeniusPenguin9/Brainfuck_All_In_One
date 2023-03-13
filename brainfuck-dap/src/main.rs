use std::{
    env, fs,
    mem::transmute,
    sync::{Arc, Mutex},
};

use brainfuck_interpreter::{BrainfuckDebugInterpreter, OutputCategoryEnum, StoppedReasonEnum};
use dap::{DapService, EventPoster};
use serde::{Deserialize, Serialize};
use simplelog::*;
use std::fs::File;
mod dap;

struct UserData<'a> {
    event_poster: EventPoster,
    runtime: Arc<Mutex<RunningState<'a>>>,
    breakpoint_lines: Vec<usize>,
}

enum RunningState<'a> {
    Idle,
    Running(BrainfuckDebugInterpreter<'a>),
}

impl<'a> UserData<'a> {
    fn initialize(
        &mut self,
        _initialize_requst_args: InitializeRequestArguments,
    ) -> Result<Capabilities, String> {
        self.event_poster.queue_event(&InitializeEvent::new());

        Ok(Capabilities {
            supports_single_thread_execution_requests: Some(true),
        })
    }

    fn set_breakpoints(
        &mut self,
        set_breakpoints_request_args: SetBreakpointsArguments,
    ) -> Result<Vec<Breakpoint>, String> {
        let breakpoint_lines = match set_breakpoints_request_args.breakpoints {
            Some(source_breakpoint_vec) => source_breakpoint_vec.iter().map(|b| b.line).collect(),
            None => Vec::new(),
        };
        let breakpoint_len = breakpoint_lines.len();
        self.breakpoint_lines = breakpoint_lines;
        Ok(vec![Breakpoint { verified: true }; breakpoint_len])
    }

    fn launch(&mut self, launch_request_args: LaunchRequestArguments) -> Result<(), String> {
        info!(">> brainfuck-dap/main launch function");
        let mut event_poster = self.event_poster.clone();
        let callback_runtime = self.runtime.clone();
        let breakpoint_callback = move |reason: StoppedReasonEnum| match reason {
            StoppedReasonEnum::Breakpoint => event_poster.queue_event(
                &Event::<StoppedEventBody>::new(StoppedEventBodyEnum::Breakpoint),
            ),
            StoppedReasonEnum::Complete => {
                if let Ok(mut runtime_lock) = callback_runtime.lock() {
                    *runtime_lock = RunningState::Idle;
                };
                event_poster.queue_event(&Event::<TerminatedEventBody>::new());
                event_poster.queue_event(&Event::<ExitedEventBody>::new(0));
            }
            StoppedReasonEnum::Terminated => {
                if let Ok(mut runtime_lock) = callback_runtime.lock() {
                    *runtime_lock = RunningState::Idle;
                };
                event_poster.queue_event(&Event::<TerminatedEventBody>::new());
                event_poster.queue_event(&Event::<ExitedEventBody>::new(-1));
            }
            StoppedReasonEnum::Step => event_poster
                .queue_event(&Event::<StoppedEventBody>::new(StoppedEventBodyEnum::Step)),
            _ => (),
        };

        let mut event_poster = self.event_poster.clone();
        let output_callback =
            move |output_category: OutputCategoryEnum, output: String| match output_category {
                OutputCategoryEnum::Console => event_poster.queue_event(
                    &Event::<OutputEventBody>::new("console".to_string(), output),
                ),
                OutputCategoryEnum::StdOut => event_poster
                    .queue_event(&Event::<OutputEventBody>::new("stdout".to_string(), output)),
            };

        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match *current_runtime_lock {
                RunningState::Idle => {
                    let source_content = fs::read_to_string(launch_request_args.program)
                        .expect("Should have been able to read the file");
                    let mut brainfuck_debug_interpreter =
                        BrainfuckDebugInterpreter::new(source_content);

                    brainfuck_debug_interpreter.set_breakpoints(&self.breakpoint_lines);
                    info!("brainfuck_debug_interpreter init completed.");
                    brainfuck_debug_interpreter.launch(
                        Some(Box::new(breakpoint_callback)),
                        Some(Box::new(output_callback)),
                    );
                    info!("brainfuck_debug_interpreter launch completed.");

                    *current_runtime_lock = RunningState::Running(brainfuck_debug_interpreter);
                }
                RunningState::Running(_) => todo!(), //panic??
            }
        };
        info!("<< brainfuck-dap/main launch function. Successful.");
        Ok(())
    }

    fn run(&mut self, _continue_request_args: ContinueRequestArguments) -> Result<(), String> {
        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match &mut *current_runtime_lock {
                RunningState::Idle => todo!(),
                RunningState::Running(brainfuck_interpreter) => {
                    brainfuck_interpreter.run()?;
                }
            }
        };
        Ok(())
    }

    fn next(&mut self, _next_request_args: NextRequestArguments) -> Result<(), String> {
        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match &mut *current_runtime_lock {
                RunningState::Idle => todo!(),
                RunningState::Running(brainfuck_interpreter) => {
                    brainfuck_interpreter.next();
                }
            }
        };
        Ok(())
    }

    fn disconnect(
        &mut self,
        _disconnect_request_args: DisconnectRequestArguments,
    ) -> Result<(), String> {
        if let Ok(mut current_runtime_lock) = self.runtime.lock() {
            match &mut *current_runtime_lock {
                RunningState::Idle => todo!(),
                RunningState::Running(_) => {
                    *current_runtime_lock = RunningState::Idle;
                }
            }
        };
        Ok(())
    }
}

/* ----------------- initialize ----------------- */
#[derive(Deserialize)]
struct InitializeRequestArguments {
    #[serde(rename(deserialize = "adapterID"))]
    adapter_id: String,
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
    supports_single_thread_execution_requests: Option<bool>,
}

/* ----------------- set_breakpoints ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetBreakpointsArguments {
    source: Source,
    breakpoints: Option<Vec<SourceBreakpoint>>,
    source_modified: Option<bool>,
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
    condition: Option<String>,
    hit_condition: Option<String>,
    log_message: Option<String>,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Breakpoint {
    verified: bool,
}

#[derive(Serialize)]
struct Event<T> {
    #[serde(rename(serialize = "type"))]
    event_type: String,
    event: String,
    body: T,
}
#[derive(Serialize)]
struct StoppedEventBody {
    reason: StoppedEventBodyEnum,
}
#[derive(Serialize)]
enum StoppedEventBodyEnum {
    #[serde(rename(serialize = "step"))]
    Step,
    #[serde(rename(serialize = "breakpoint"))]
    Breakpoint,
    #[serde(rename(serialize = "exception"))]
    Exception,
    #[serde(rename(serialize = "pause"))]
    Pause,
    #[serde(rename(serialize = "entry"))]
    Entry,
    #[serde(rename(serialize = "goto"))]
    GoTo,
    #[serde(rename(serialize = "function breakpoint"))]
    FunctionBreakpoint,
    #[serde(rename(serialize = "data breakpoint"))]
    DataBreakpoint,
    #[serde(rename(serialize = "instruction breakpoint"))]
    InstructionBreakpoint,
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
struct TerminatedEventBody {}

impl Event<StoppedEventBody> {
    fn new(reason: StoppedEventBodyEnum) -> Event<StoppedEventBody> {
        Event {
            event_type: "event".to_string(),
            event: "stopped".to_string(),
            body: StoppedEventBody { reason },
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

/* ----------------- launch ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LaunchRequestArguments {
    program: String,
}

/* ----------------- continue ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContinueRequestArguments {
    thread_id: usize,
}

/* ----------------- next ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NextRequestArguments {
    thread_id: usize,
}
/* ----------------- disconnect ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DisconnectRequestArguments {
    restart: Option<bool>,
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
        breakpoint_lines: vec![],
    })
    .register("initialize".to_string(), Box::new(UserData::initialize))
    .register(
        "setBreakpoints".to_string(),
        Box::new(UserData::set_breakpoints),
    )
    .register("launch".to_string(), Box::new(UserData::launch))
    .register("continue".to_string(), Box::new(UserData::run))
    .register("next".to_string(), Box::new(UserData::next))
    .register("disconnect".to_string(), Box::new(UserData::disconnect))
    .build();
    dap_service.start();
    info!("<< brainfuck-dap main");
}

/* ----------------- test ----------------- */

#[test]
fn test_initialization_request() {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    use std::{thread, time};

    let mut child = Command::new("cargo")
        .args(["run"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed during cargo run");

    let child_stdin = child.stdin.as_mut().unwrap();
    let child_stdout = child.stdout.as_mut().unwrap();
    let initialization_request = "Content-Length: 128\r\n\r\n{\r\n    \"seq\": 153,\r\n    \"type\": \"request\",\r\n    \"command\": \"initialize\",\r\n    \"arguments\": {\r\n        \"adapterID\": \"a\"\r\n    }\r\n}\r\n";
    child_stdin
        .write_all(initialization_request.as_bytes())
        .unwrap();
    // Close stdin to finish and avoid indefinite blocking
    drop(child_stdin);
    thread::sleep(time::Duration::from_secs(5));

    let mut read_buf: [u8; 300] = [0; 300];
    child_stdout.read(&mut read_buf).unwrap();
    child.kill().unwrap();

    let actual = String::from_utf8(read_buf.to_vec()).unwrap();
    assert!(actual.contains("Content-Length: 129\r\n\r\n{\"type\":\"response\",\"request_seq\":153,\"success\":true,\"command\":\"initialize\",\"body\":{\"supportsSingleThreadExecutionRequests\":true}}\r\nContent-Length: 38\r\n\r\n{\"type\":\"event\",\"event\":\"initialized\"}"));
}

#[test]
fn test_launch_request() {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    use std::{thread, time};

    let mut child = Command::new("cargo")
        .args(["run"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed during cargo run");

    let child_stdin = child.stdin.as_mut().unwrap();
    let child_stdout = child.stdout.as_mut().unwrap();
    let launch_request = "Content-Length: 233\r\n\r\n{\"command\": \"launch\",\"arguments\": {\"name\": \"Brainfuck-Debug\",\"type\": \"brainfuck\",\"request\": \"launch\",\"program\":\"../test.bf\",\"__configurationTarget\": 6,\"__sessionId\": \"36201e43-539a-4fd6-beb8-5e0bc2b18abe\"},\"type\": \"request\",\"seq\": 2}\r\n";
    child_stdin.write_all(launch_request.as_bytes()).unwrap();
    // Close stdin to finish and avoid indefinite blocking
    drop(child_stdin);
    thread::sleep(time::Duration::from_secs(5));

    let mut read_buf: [u8; 300] = [0; 300];
    child_stdout.read(&mut read_buf).unwrap();
    child.kill().unwrap();

    let actual = String::from_utf8(read_buf.to_vec()).unwrap();
    assert!(actual.contains("Content-Length: 81\r\n\r\n{\"type\":\"response\",\"request_seq\":2,\"success\":true,\"command\":\"launch\",\"body\":null}"));
}

#[test]
fn test_unknown_request() {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    use std::{thread, time};

    let mut child = Command::new("cargo")
        .args(["run"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed during cargo run");

    let child_stdin = child.stdin.as_mut().unwrap();
    let child_stdout = child.stdout.as_mut().unwrap();
    let unknown_request = "Content-Length: 85\r\n\r\n{\"command\": \"abcdefghij\",\"arguments\": {\"restart\": false},\"type\": \"request\",\"seq\": 1}\r\n";
    child_stdin.write_all(unknown_request.as_bytes()).unwrap();
    // Close stdin to finish and avoid indefinite blocking
    drop(child_stdin);
    thread::sleep(time::Duration::from_secs(5));

    let mut read_buf: [u8; 300] = [0; 300];
    child_stdout.read(&mut read_buf).unwrap();
    child.kill().unwrap();

    let actual = String::from_utf8(read_buf.to_vec()).unwrap();
    assert!(actual.contains("Content-Length: 74\r\n\r\n{\"type\":\"response\",\"request_seq\":1,\"success\":false,\"command\":\"abcdefghij\"}"));
}
