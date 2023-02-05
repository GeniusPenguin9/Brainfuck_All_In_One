use core::time;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::BufRead;
use std::sync::mpsc::{self, Receiver, Sender};
use std::{io, thread, vec};

type Message = String;

/* ----------------- START: DAP Service for user ----------------- */

pub struct DapService<'a, TUserData: Send> {
    dealer: Dealer<'a, TUserData>,
    event_rx: Option<Receiver<Message>>,
}

impl<'a, TUserData: Send> DapService<'a, TUserData> {
    #[allow(dead_code)]
    pub fn new(user_data: TUserData) -> DapService<'a, TUserData> {
        DapService {
            dealer: Dealer::new(user_data),
            event_rx: None,
        }
    }

    pub fn register<TArguments: DeserializeOwned + 'a, TResponseBody: Serialize + 'a>(
        mut self,
        fn_name: String,
        fn_handler: Box<dyn Fn(&mut TUserData, TArguments) -> Result<TResponseBody, String>>,
    ) -> Self {
        self.dealer.register(fn_name, fn_handler);
        self
    }

    pub fn new_with_poster<F>(init_fn: F) -> Self
    where
        F: FnOnce(EventPoster) -> TUserData,
    {
        let (event_tx, event_rx) = mpsc::channel();
        let event_poster = EventPoster { event_tx };
        DapService {
            dealer: Dealer::new(init_fn(event_poster)),
            event_rx: Some(event_rx),
        }
    }

    pub fn build(self) -> Self {
        self
    }

    pub fn start(&mut self) {
        // io thread to dap thread
        let (i2d_tx, i2d_rx) = mpsc::channel();

        thread::spawn(move || {
            Self::io_thread(i2d_tx);
        });

        self.dap_thread(i2d_rx);
    }

    fn io_thread(i2d_tx: Sender<Message>) {
        let mut stdin_cache = StdinCache::new();

        loop {
            stdin_cache.stdin_read_until("Content-Length: ");
            let len = stdin_cache.stdin_read_until("\r\n\r\n");
            let len = len.parse::<usize>().unwrap();

            let request = stdin_cache.stdin_read_exact(len);

            i2d_tx.send(request).unwrap();
        }
    }

    fn dap_thread(&mut self, i2d_rx: Receiver<Message>) {
        loop {
            if let Ok(io_request) = i2d_rx.try_recv() {
                let io_result = self.dealer.process_request(&io_request);
                print!(
                    "Content-Length: {}\r\n\r\n{}\r\n",
                    io_result.len(),
                    io_result
                );
            }

            if let Some(event_rx) = &self.event_rx {
                while let Ok(event) = event_rx.try_recv() {
                    print!("Content-Length: {}\r\n\r\n{}\r\n", event.len(), event);
                }
            }

            thread::sleep(time::Duration::from_millis(1));
        }
    }
}

/* ----------------- END: DAP Service for user ----------------- */
pub struct EventPoster {
    event_tx: Sender<Message>,
}
impl EventPoster {
    #[allow(dead_code)]
    pub fn send_event<T: Serialize>(&mut self, event: &T) {
        let event_str = serde_json::to_string(event).unwrap();
        print!("{}\r\n", event_str);
    }
    pub fn queue_event<T: Serialize>(&mut self, event: &T) {
        let event_str = serde_json::to_string(event).unwrap();
        self.event_tx.send(event_str).unwrap();
    }
}

struct StdinCache {
    stdin_cache: Vec<u8>,

    // [0, start_position) consumed part
    // [start_position, end) unconsumed part
    start_position: usize,
}

impl StdinCache {
    pub fn new() -> StdinCache {
        StdinCache {
            stdin_cache: vec![],
            start_position: 0,
        }
    }

    pub fn stdin_read_exact(&mut self, target_len: usize) -> String {
        loop {
            if self.stdin_cache.len() - self.start_position >= target_len {
                let result = String::from_utf8(
                    self.stdin_cache[self.start_position..self.start_position + target_len]
                        .to_vec(),
                )
                .unwrap();
                self.start_position += target_len;
                return result;
            }

            let stdin = &mut io::stdin().lock();
            let buffer = stdin.fill_buf().unwrap();
            let l = buffer.len();
            self.stdin_cache.append(&mut buffer.to_vec());
            stdin.consume(l);
        }
    }

    /**
     * Return the substring from start_position to target. Include start_postion and exclude target.
     * Pay attention that target is still be consumed.
     *
     * # Examples
     *
     * Input "Start Test Penguin" in stdin.
     * assert_eq!(read_until("Test"), "Start ");
     * assert_eq!(read_until(io::stdin().lock(), "n"), " Pengui");
     */
    pub fn stdin_read_until(&mut self, target: &str) -> String {
        // UTF-8
        let target = target.as_bytes();

        loop {
            // find target in stdin_cache
            match self.find_subsequence(target) {
                Some(result_len) => {
                    let result = String::from_utf8(
                        self.stdin_cache[self.start_position..self.start_position + result_len]
                            .to_vec(),
                    )
                    .unwrap();

                    self.start_position += result_len + target.len();

                    return result;
                }
                None => (),
            }

            let stdin = &mut io::stdin().lock();
            let buffer = stdin.fill_buf().unwrap();
            let l = buffer.len();
            self.stdin_cache.append(&mut buffer.to_vec());
            stdin.consume(l);
        }
    }

    /**
     * Find subsequence and return the position of target beginning.
     *
     * # Examples
     *
     * current stdin_cache = "qwertyuiop", start_position = 0
     * assert_eq!(find_subsequence(b"tyu"), Some(4));
     * assert_eq!(find_subsequence(b"asd"), None);
     */
    pub fn find_subsequence(&mut self, target: &[u8]) -> Option<usize> {
        self.stdin_cache
            .windows(target.len())
            .skip(self.start_position)
            .position(|window| window == target)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct DAPRequest {
    seq: usize,
    command: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct DAPRequestWithArguments<TArguments> {
    seq: usize,
    command: String,
    arguments: TArguments,
}

#[derive(Serialize)]
struct DAPResponseWithBody<TResponseBody> {
    #[serde(rename(serialize = "type"))]
    response_type: String,

    request_seq: usize,

    success: bool,

    command: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<TResponseBody>,
}

struct Dealer<'a, TUserData: 'a> {
    function_map: HashMap<String, Box<dyn Fn(&mut TUserData, String) -> String + 'a>>,
    user_data: TUserData,
}

impl<'a, TUserData> Dealer<'a, TUserData> {
    pub fn new(user_data: TUserData) -> Dealer<'a, TUserData> {
        Dealer {
            function_map: HashMap::new(),
            user_data,
        }
    }

    pub fn register<TArguments: DeserializeOwned + 'a, TResponseBody: Serialize + 'a>(
        &mut self,
        fn_name: String,
        fn_handler: Box<dyn Fn(&mut TUserData, TArguments) -> Result<TResponseBody, String>>,
    ) {
        let new_function = move |user_data: &mut TUserData, request_str: String| {
            let request_with_arg: DAPRequestWithArguments<TArguments> =
                serde_json::from_str(&request_str).unwrap();

            let result = match fn_handler(user_data, request_with_arg.arguments) {
                Ok(success_body) => DAPResponseWithBody::<TResponseBody> {
                    response_type: "response".to_string(),
                    request_seq: request_with_arg.seq,
                    success: true,
                    command: request_with_arg.command,
                    message: None,
                    body: Some(success_body),
                },
                Err(err) => DAPResponseWithBody::<TResponseBody> {
                    response_type: "response".to_string(),
                    request_seq: request_with_arg.seq,
                    success: false,
                    command: request_with_arg.command,
                    message: Some(err),
                    body: None,
                },
            };
            serde_json::to_string(&result).unwrap()
        };

        self.function_map.insert(fn_name, Box::new(new_function));
    }

    pub fn process_request(&mut self, io_request: &str) -> String {
        let dap_request: DAPRequest = serde_json::from_str(io_request).unwrap();
        let handler = self.function_map.get(&dap_request.command);
        // TODO:
        match handler {
            Some(h) => h(&mut self.user_data, io_request.to_string()),
            None => todo!(),
        }
    }
}
