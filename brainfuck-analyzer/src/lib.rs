use std::str::Chars;

#[derive(Debug, PartialEq)]
pub struct TokenGroup {
    token_group: Vec<Token>,
}

#[derive(Debug, PartialEq)]
pub enum Token {
    PointerIncrement,
    PointerDecrement,
    Increment,
    Decrement,
    Output,
    Accept,
    LoopStart,
    LoopEnd,
    SubGroup(Box<TokenGroup>),
}

impl TokenGroup {
    pub fn tokens(&self) -> &Vec<Token> {
        &self.token_group
    }
}

pub fn parse(str: &str) -> TokenGroup {
    let mut iter = str.chars();
    _parse(&mut iter, true)
}

fn _parse(chars: &mut Chars, is_top: bool) -> TokenGroup {
    let mut v = Vec::new();
    let mut stopped = false;
    while let Some(c) = chars.next() {
        let res = match c {
            '[' => Token::SubGroup(Box::new(_parse(chars, false))),
            ']' => {
                stopped = true;
                break;
            }
            '>' => Token::PointerIncrement,
            '<' => Token::PointerDecrement,
            '+' => Token::Increment,
            '-' => Token::Decrement,
            '.' => Token::Output,
            ',' => Token::Accept,
            ' ' | '\n' | '\t' | '\r' => break,
            _ => panic!(),
        };
        v.push(res);
    }

    if is_top && chars.next().is_some() {
        panic!("More ] found")
    } else if (!is_top && !stopped) || (is_top && stopped) {
        panic!("More [ found")
    } else {
        TokenGroup { token_group: v }
    }
}

#[test]
fn parse_should_success() {
    let actual = parse(">[[]<]");

    assert_eq!(2, actual.token_group.len());
    assert_eq!(Token::PointerIncrement, actual.token_group[0]);
    match &actual.token_group[1] {
        Token::SubGroup(tg) => {
            assert!(matches!(& tg.token_group[0], Token::SubGroup(x) if x.token_group.len() ==0));
            assert_eq!(Token::PointerDecrement, tg.token_group[1]);
        }
        _ => assert!(false),
    }
}

#[test]
#[should_panic]
fn parse_more_start_should_panic() {
    parse("[");
}

#[test]
#[should_panic]
fn parse_more_stop_should_panic() {
    parse("]");
}
