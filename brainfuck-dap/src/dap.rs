use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::BufRead;
use std::sync::mpsc::{self, Receiver};
use std::{io, thread, vec};

type Message = String;

/* ----------------- START: DAP Service for user ----------------- */

pub struct DapService<'a, TUserData> {
    dealer: Dealer<'a, TUserData>,
}
impl<'a, TUserData> DapService<'a, TUserData> {
    pub fn new(user_data: TUserData) -> DapService<'a, TUserData> {
        DapService {
            dealer: Dealer::new(user_data),
        }
    }

    pub fn register<TArguments: DeserializeOwned + 'a, TResult: Serialize + 'a>(
        mut self,
        fn_name: String,
        fn_handler: Box<dyn Fn(&mut TUserData, TArguments) -> TResult>,
    ) -> Self {
        self.dealer.register(fn_name, fn_handler);
        self
    }

    pub fn build(self) -> Self {
        self
    }

    pub fn start(&mut self) {
        todo!()
    }
}

/* ----------------- END: DAP Service for user ----------------- */

pub fn io_thread() {
    // io thread to dap thread
    let (i2d_tx, i2d_rx) = mpsc::channel();

    thread::spawn(move || {
        dap_thread(i2d_rx);
    });

    let mut stdin_cache = StdinCache::new();

    loop {
        stdin_cache.stdin_read_until("Content-Length: ");

        let len = stdin_cache.stdin_read_until("\r\n\r\n");
        let len = len.parse::<usize>().unwrap();

        let request = stdin_cache.stdin_read_exact(len);
        i2d_tx.send(request).unwrap();
    }
}

pub fn dap_thread(i2d_rx: Receiver<Message>) {
    // let dealer = Dealer::new();
    // // TODO: register function into dealer
    // loop {
    //     let io_request = i2d_rx.recv().unwrap();
    //     let io_result = dealer.process_request(&io_request);

    //     println!("{}", io_result);
    // }
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
            let stdin = &mut io::stdin().lock();
            let buffer = stdin.fill_buf().unwrap();
            let l = buffer.len();
            self.stdin_cache.append(&mut buffer.to_vec());
            stdin.consume(l);

            if self.stdin_cache.len() - self.start_position >= target_len {
                let result = String::from_utf8(
                    self.stdin_cache[self.start_position..self.start_position + target_len]
                        .to_vec(),
                )
                .unwrap();
                self.start_position += target_len;
                return result;
            }
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
            let stdin = &mut io::stdin().lock();
            let buffer = stdin.fill_buf().unwrap();
            let l = buffer.len();
            self.stdin_cache.append(&mut buffer.to_vec());
            stdin.consume(l);

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

    pub fn register<TArguments: DeserializeOwned + 'a, TResult: Serialize + 'a>(
        &mut self,
        fn_name: String,
        fn_handler: Box<dyn Fn(&mut TUserData, TArguments) -> TResult>,
    ) {
        let new_function = move |user_data: &mut TUserData, request_str: String| {
            let request_with_arg: DAPRequestWithArguments<TArguments> =
                serde_json::from_str(&request_str).unwrap();

            let result = fn_handler(user_data, request_with_arg.arguments);
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
