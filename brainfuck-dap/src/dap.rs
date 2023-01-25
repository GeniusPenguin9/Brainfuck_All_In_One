use std::io::BufRead;
use std::sync::mpsc::{self, Receiver};
use std::{io, thread, vec};

type Message = String;

pub fn io_thread() {
    // io thread to dap thread
    let (i2d_tx, i2d_rx) = mpsc::channel();

    thread::spawn(move || {
        dap_thread(i2d_rx);
    });

    let mut stdin_cache = StdinCache::new();

    loop {
        stdin_cache.stdin_read_until("Content-Length: ");
        println!("\n#########################\nGet Content-Length");
        println!("\n#########################");
        let len = stdin_cache.stdin_read_until("\r\n\r\n");
        let len = len.parse::<usize>().unwrap();
        println!("\n!!!!!!!!!!!!!!!!!!!!!!!!\nGet len = {}", len);
        println!("\n!!!!!!!!!!!!!!!!!!!!!!!!");
        let request = stdin_cache.stdin_read_exact(len);
        println!("\n==========================\nGet Request = {:?}", request);
        println!("\n==========================");
    }
}

pub fn dap_thread(i2d_rx: Receiver<Message>) {
    let io_message = i2d_rx.recv().unwrap();
    println!(
        "\n#########################Received message: {:?}",
        io_message
    );
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
