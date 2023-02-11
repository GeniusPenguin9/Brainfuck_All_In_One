use brainfuck_interpreter::BrainfuckInterpreter;
use dap::{DapService, EventPoster};
use serde::{Deserialize, Serialize};
mod dap;

struct UserData {
    event_poster: EventPoster,
    brainfuck_interpreter: BrainfuckInterpreter,
}

impl UserData {
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

        self.brainfuck_interpreter.set_breakpoints(breakpoint_lines);

        Ok(vec![Breakpoint { verified: true }; breakpoint_len])
    }

    fn launch(&mut self, launch_request_args: LaunchRequestArguments) {
        let event_poster = self.event_poster.clone();
        let breakpoint_callback = move || {
            event_poster.send_event(&StoppedEvent::new_breakpoint());
        };
        self.brainfuck_interpreter
            .set_breakpoint_callback(Box::new(breakpoint_callback));
        self.brainfuck_interpreter.launch(
            launch_request_args.source.path.unwrap(),
            launch_request_args.debug_mode,
        );
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
struct StoppedEvent {
    #[serde(rename(serialize = "type"))]
    event_type: String,
    event: String,
    body: StoppedEventBody,
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

impl StoppedEvent {
    fn new_breakpoint() -> Self {
        StoppedEvent {
            event_type: "event".to_string(),
            event: "stopped".to_string(),
            body: StoppedEventBody {
                reason: StoppedEventBodyEnum::Breakpoint,
            },
        }
    }
}

/* ----------------- launch ----------------- */
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LaunchRequestArguments {
    debug_mode: bool,
    source: Source,
}

/* ----------------- main ----------------- */

fn main() {
    let mut dap_service = DapService::new_with_poster(|event_poster| UserData {
        event_poster,
        brainfuck_interpreter: BrainfuckInterpreter::new(),
    })
    .register("initialize".to_string(), Box::new(UserData::initialize))
    .register(
        "setBreakpoints".to_string(),
        Box::new(UserData::set_breakpoints),
    )
    .build();
    dap_service.start();
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
