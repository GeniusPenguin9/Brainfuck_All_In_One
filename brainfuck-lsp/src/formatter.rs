use core::slice::Iter;

use brainfuck_analyzer::{token_to_char, ParseError, Position, Range, Token, TokenGroup};

pub struct FormatResult {
    pub range: Range,
    pub format_result: String,
}

pub fn format_string(input: &str) -> Result<FormatResult, ParseError> {
    let token_group = brainfuck_analyzer::parse(input)?;
    Ok(FormatResult {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: token_group.position,
        },
        format_result: _print(&token_group.parse_token_group, 0),
    })
}

pub fn format_pretty_string(input: &str) -> Result<FormatResult, ParseError> {
    let token_group = brainfuck_analyzer::parse(input)?;
    Ok(FormatResult {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: token_group.position,
        },
        format_result: _pretty_print(&token_group.parse_token_group, 0),
    })
}

fn _print(token_group: &TokenGroup, tab_number: usize) -> String {
    let enter: String = "\n".to_string();

    let mut output = String::new();

    for token in token_group.tokens().into_iter() {
        if !output.is_empty() {
            output.push_str(&enter);
        }
        output.push_str(&n_tab(tab_number));
        match token {
            Token::SubGroup(x) => {
                output.push_str("[\n");

                output.push_str(&format!("{}", _print(x, tab_number + 1)));
                output.push_str("\n");
                output.push_str(&n_tab(tab_number));
                output.push_str("]");
            }
            Token::PointerIncrement => output.push_str(">"),
            Token::PointerDecrement => output.push_str("<"),
            Token::Increment => output.push_str("+"),
            Token::Decrement => output.push_str("-"),
            Token::Output => output.push_str("."),
            Token::Input => output.push_str(","),
            _ => output.push_str("?"),
        };
    }
    output
}

struct TokenIter<'a> {
    token_iter: Iter<'a, Token>,
    state: TokenState,
    tab_number: usize,
}

enum TokenState {
    Move,
    Change,
    IO,
    Default,
}

impl<'a> TokenIter<'a> {
    fn new(token_iter: Iter<'a, Token>, tab_number: usize) -> TokenIter {
        TokenIter {
            token_iter,
            state: TokenState::Default,
            tab_number,
        }
    }

    fn next(&mut self) -> Option<String> {
        let mut result: String = String::new();
        let token_option = self.token_iter.next();
        if let Some(token) = token_option {
            match self.state {
                TokenState::Move => match token {
                    Token::PointerDecrement | Token::PointerIncrement => {
                        result.push(token_to_char(token));
                    }
                    Token::Decrement | Token::Increment => {
                        self.state = TokenState::Change;
                        result.push(token_to_char(token));
                    }
                    Token::Input | Token::Output => {
                        self.state = TokenState::IO;
                        result.push(token_to_char(token));
                    }
                    Token::SubGroup(sg) => {
                        self.state = TokenState::Default;
                        result.push('\n');

                        result.push_str(&n_tab(self.tab_number));
                        result.push_str("[\n");

                        result.push_str(&format!("{}\n", _pretty_print(sg, self.tab_number + 1)));

                        result.push_str(&n_tab(self.tab_number));
                        result.push_str("]\n");
                    }
                    _ => (),
                },
                TokenState::Change => match token {
                    Token::PointerDecrement | Token::PointerIncrement => {
                        self.state = TokenState::Default;
                        result.push('\n');

                        self.state = TokenState::Move;
                        result.push_str(&n_tab(self.tab_number));
                        result.push(token_to_char(token));
                    }
                    Token::Decrement | Token::Increment => {
                        result.push(token_to_char(token));
                    }
                    Token::Input | Token::Output => {
                        self.state = TokenState::IO;
                        result.push(token_to_char(token));
                    }
                    Token::SubGroup(sg) => {
                        self.state = TokenState::Default;
                        result.push('\n');

                        result.push_str(&n_tab(self.tab_number));
                        result.push_str("[\n");

                        result.push_str(&format!("{}\n", _pretty_print(sg, self.tab_number + 1)));

                        result.push_str(&n_tab(self.tab_number));
                        result.push_str("]\n");
                    }
                    _ => (),
                },
                TokenState::IO => match token {
                    Token::PointerDecrement | Token::PointerIncrement => {
                        self.state = TokenState::Default;
                        result.push('\n');

                        self.state = TokenState::Move;
                        result.push_str(&n_tab(self.tab_number));
                        result.push(token_to_char(token));
                    }
                    Token::Decrement | Token::Increment => {
                        self.state = TokenState::Default;
                        result.push('\n');

                        self.state = TokenState::Change;
                        result.push_str(&n_tab(self.tab_number));
                        result.push(token_to_char(token));
                    }
                    Token::Input | Token::Output => {
                        result.push(token_to_char(token));
                    }
                    Token::SubGroup(sg) => {
                        self.state = TokenState::Default;
                        result.push('\n');

                        result.push_str(&n_tab(self.tab_number));
                        result.push_str("[\n");

                        result.push_str(&format!("{}\n", _pretty_print(sg, self.tab_number + 1)));

                        result.push_str(&n_tab(self.tab_number));
                        result.push_str("]\n");
                    }
                    _ => (),
                },
                TokenState::Default => match token {
                    Token::PointerDecrement | Token::PointerIncrement => {
                        self.state = TokenState::Move;
                        result.push_str(&n_tab(self.tab_number));
                        result.push(token_to_char(token));
                    }
                    Token::Decrement | Token::Increment => {
                        self.state = TokenState::Change;
                        result.push_str(&n_tab(self.tab_number));
                        result.push(token_to_char(token));
                    }
                    Token::Input | Token::Output => {
                        self.state = TokenState::IO;
                        result.push_str(&n_tab(self.tab_number));
                        result.push(token_to_char(token));
                    }
                    Token::SubGroup(sg) => {
                        result.push_str(&n_tab(self.tab_number));
                        result.push_str("[\n");

                        result.push_str(&format!("{}\n", _pretty_print(sg, self.tab_number + 1)));

                        result.push_str(&n_tab(self.tab_number));
                        result.push_str("]\n");
                    }
                    _ => (),
                },
            }

            Some(result)
        } else {
            None
        }
    }
}

fn _pretty_print(token_group: &TokenGroup, tab_number: usize) -> String {
    let mut result = String::new();
    let iter = token_group.tokens().into_iter();
    let mut token_iter = TokenIter::new(iter, tab_number);
    while let Some(s) = token_iter.next() {
        result.push_str(&s);
    }
    result
}

fn n_tab(tab_number: usize) -> String {
    std::iter::repeat(" ")
        .take(tab_number * 4)
        .collect::<String>()
}

#[test]
fn test_should_success() {
    let actual = format_string(">[>[<,]]").unwrap();
    print!("Actual value:\n{}", actual.format_result);
    assert_eq!(
        ">\n[\n    >\n    [\n        <\n        ,\n    ]\n]",
        actual.format_result
    );
}

#[test]
fn test_format_pretty_string() {
    let actual = format_pretty_string(">[>>+]").unwrap();
    print!("Actual value:\n{}", actual.format_result);
    assert_eq!(">\n[\n    >>+\n]\n", actual.format_result);
}
