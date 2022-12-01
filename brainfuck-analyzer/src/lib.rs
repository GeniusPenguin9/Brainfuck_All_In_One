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

/// Position in a text document expressed as zero-based line and character offset.
/// A position is between two characters like an 'insert' cursor in a editor.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Default)]
pub struct Position {
    /// Line position in a document (zero-based).
    pub line: u32,
    /// Character offset on a line in a document (zero-based). The meaning of this
    /// offset is determined by the negotiated `PositionEncodingKind`.
    ///
    /// If the character value is greater than the line length it defaults back
    /// to the line length.
    pub character: u32,
}

impl Position {
    pub fn new(line: u32, character: u32) -> Position {
        Position { line, character }
    }
    pub fn move_right(&mut self) {
        self.character += 1;
    }
    pub fn move_down(&mut self) {
        self.line += 1;
        self.character = 0;
    }
}

/// A range in a text document expressed as (zero-based) start and end positions.
/// A range is comparable to a selection in an editor. Therefore the end position is exclusive.
#[derive(Debug, Eq, PartialEq, Copy, Clone, Default)]
pub struct Range {
    /// The range's start position.
    pub start: Position,
    /// The range's end position.
    pub end: Position,
}

impl Range {
    pub fn new(start: Position, end: Position) -> Range {
        Range { start, end }
    }
}
#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct ParseError {
    pub range: Range,
    pub error_message: String,
}

pub type Result<T> = std::result::Result<T, ParseError>;

struct CharsWithPosition<'a> {
    last_position: Option<Position>,
    position: Position,
    chars: Chars<'a>,
}
impl<'a> CharsWithPosition<'a> {
    fn next(&mut self) -> Option<char> {
        let c_option = self.chars.next();
        self.last_position = Some(self.position);
        if let Some(c) = c_option {
            match c {
                '\n' => self.position.move_down(),
                _ => self.position.move_right(),
            }
        }
        c_option
    }
}

impl TokenGroup {
    pub fn tokens(&self) -> &Vec<Token> {
        &self.token_group
    }
}

pub fn parse(str: &str) -> Result<TokenGroup> {
    let chars = str.chars();
    let mut chars_with_position = CharsWithPosition {
        last_position: None,
        position: Position {
            line: 0,
            character: 0,
        },
        chars: chars,
    };

    _parse(&mut chars_with_position, true)
}

fn _parse(chars: &mut CharsWithPosition, is_top: bool) -> Result<TokenGroup> {
    let mut v = Vec::new();
    let mut stopped = false;
    while let Some(c) = chars.next() {
        let res = match c {
            '[' => Token::SubGroup(Box::new(_parse(chars, false)?)),
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
            _ => {
                return Err(ParseError {
                    range: Range {
                        start: chars.last_position.unwrap_or_default(),
                        end: chars.last_position.unwrap_or_default(),
                    },
                    error_message: "Invalid token".to_string(),
                })
            }
        };
        v.push(res);
    }

    if is_top && chars.next().is_some() {
        Err(ParseError {
            range: Range {
                start: chars.last_position.unwrap_or_default(),
                end: chars.last_position.unwrap_or_default(),
            },
            error_message: "More ] found".to_string(),
        })
    } else if (!is_top && !stopped) || (is_top && stopped) {
        Err(ParseError {
            range: Range {
                start: chars.last_position.unwrap_or_default(),
                end: chars.last_position.unwrap_or_default(),
            },
            error_message: "More [ found".to_string(),
        })
    } else {
        Ok(TokenGroup { token_group: v })
    }
}

#[test]
fn parse_should_success() {
    let actual = parse(">[[]<]").unwrap();

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
fn parse_more_start_should_error() {
    assert_eq!(true, parse("[").is_err());
}

#[test]
fn parse_more_stop_should_error() {
    assert_eq!(true, parse("]").is_err());
}
