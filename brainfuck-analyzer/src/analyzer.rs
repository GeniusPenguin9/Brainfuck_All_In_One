use std::str::Chars;

#[derive(Debug, PartialEq, Clone)]
pub struct TokenGroup {
    pub token_group: Vec<Token>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Token {
    pub range: Range,
    pub token_type: TokenType,
}

#[derive(Debug, PartialEq, Clone)]
pub enum TokenType {
    PointerIncrement,
    PointerDecrement,
    Increment,
    Decrement,
    Output,
    Input,
    LoopStart,
    LoopEnd,
    SubGroup(Box<TokenGroup>),
    Comment(String),
    Breakpoint,
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
/// A range is comparable to a selection in an editor. Therefore the end position is been excluded.
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
#[derive(Debug)]
pub struct ParseResult {
    pub position: Position,
    pub parse_token_group: TokenGroup,
}

pub type Result<T> = std::result::Result<T, ParseError>;

enum ParseState {
    BrainFuck,
    LineComment((Position, String)),
    ParagraphComment((Position, String)),
}

struct CharsWithPosition<'a> {
    last_position: Option<Position>,
    position: Position,
    chars: Chars<'a>,
    state: ParseState,
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

    pub fn tokens_mut(&mut self) -> &mut Vec<Token> {
        &mut self.token_group
    }
}

pub fn parse(str: &str) -> Result<ParseResult> {
    let chars = str.chars();
    let mut chars_with_position = CharsWithPosition {
        last_position: None,
        position: Position {
            line: 0,
            character: 0,
        },
        chars: chars,
        state: ParseState::BrainFuck,
    };

    _parse(&mut chars_with_position, true)
}

pub fn token_to_char(token: &Token) -> char {
    match token.token_type {
        TokenType::PointerIncrement => '>',
        TokenType::PointerDecrement => '<',
        TokenType::Increment => '+',
        TokenType::Decrement => '-',
        TokenType::Output => '.',
        TokenType::Input => ',',
        TokenType::LoopStart => '[',
        TokenType::LoopEnd => ']',
        _ => '?',
    }
}
//ddddd\n
fn _parse(chars: &mut CharsWithPosition, is_top: bool) -> Result<ParseResult> {
    let mut v = Vec::new();
    let mut stopped = false;
    while let Some(c) = chars.next() {
        match &mut chars.state {
            ParseState::BrainFuck => {
                let start = chars.last_position.unwrap_or_default();
                let res = match c {
                    '[' => TokenType::SubGroup(Box::new(_parse(chars, false)?.parse_token_group)),
                    ']' => {
                        stopped = true;
                        break;
                    }
                    '>' => TokenType::PointerIncrement,
                    '<' => TokenType::PointerDecrement,
                    '+' => TokenType::Increment,
                    '-' => TokenType::Decrement,
                    '.' => TokenType::Output,
                    ',' => TokenType::Input,
                    ' ' | '\n' | '\t' | '\r' => continue,
                    '/' => match chars.next() {
                        Some('/') => {
                            chars.state = ParseState::LineComment((start, "//".to_string()));
                            continue;
                        }
                        Some('*') => {
                            chars.state = ParseState::ParagraphComment((start, "/*".to_string()));
                            continue;
                        }
                        _ => {
                            return Err(ParseError {
                                range: Range {
                                    start: chars.last_position.unwrap_or_default(),
                                    end: chars.last_position.unwrap_or_default(),
                                },
                                error_message: "Invalid token".to_string(),
                            })
                        }
                    },
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
                let range = Range {
                    start: start,
                    end: chars.position,
                };
                v.push(Token {
                    range,
                    token_type: res,
                });
            }
            ParseState::LineComment((start_position, org_str)) => {
                org_str.push(c);
                match c {
                    '\n' | '\r' => {
                        v.push(Token {
                            range: Range {
                                start: *start_position,
                                end: chars.position,
                            },
                            token_type: TokenType::Comment(org_str.clone()),
                        });
                        chars.state = ParseState::BrainFuck;
                    }
                    _ => (),
                };
            }
            ParseState::ParagraphComment((start_position, org_str)) => {
                org_str.push(c);
                if c == '/' {
                    if org_str.len() >= 4 && &org_str[org_str.len() - 2..org_str.len() - 1] == "*" {
                        v.push(Token {
                            range: Range {
                                start: *start_position,
                                end: chars.position,
                            },
                            token_type: TokenType::Comment(org_str.clone()),
                        });
                        chars.state = ParseState::BrainFuck;
                        continue;
                    }
                }
            }
        }
    }

    if let ParseState::LineComment((start_position, org_str)) = &chars.state {
        v.push(Token {
            range: Range {
                start: *start_position,
                end: chars.position,
            },
            token_type: TokenType::Comment(org_str.clone()),
        });
    }

    if let ParseState::ParagraphComment(_) = &chars.state {
        Err(ParseError {
            range: Range {
                start: chars.last_position.unwrap_or_default(),
                end: chars.last_position.unwrap_or_default(),
            },
            error_message: "Paragraph comment missing end flag.".to_string(),
        })
    } else if is_top && chars.next().is_some() {
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
        Ok(ParseResult {
            position: chars.position,
            parse_token_group: TokenGroup { token_group: v },
        })
    }
}

#[test]
fn parse_should_success() {
    let actual = parse(">[[]<]").unwrap();

    assert_eq!(2, actual.parse_token_group.token_group.len());
    assert_eq!(
        TokenType::PointerIncrement,
        actual.parse_token_group.token_group[0].token_type
    );
    match &actual.parse_token_group.token_group[1].token_type {
        TokenType::SubGroup(tg) => {
            assert!(
                matches!(& tg.token_group[0].token_type, TokenType::SubGroup(x) if x.token_group.len() ==0)
            );
            assert_eq!(TokenType::PointerDecrement, tg.token_group[1].token_type);
        }
        _ => assert!(false),
    }
}

#[test]
fn parse_with_comment() {
    let line_comment = parse(">>//todo").unwrap();
    print!("{:?}", line_comment);
    assert_eq!(3, line_comment.parse_token_group.token_group.len());
    match &line_comment.parse_token_group.token_group[2].token_type {
        TokenType::Comment(str) => {
            assert_eq!(str, "//todo");
        }
        _ => assert!(false),
    }

    let paragraph_commet_success = parse(">>/*todo*/").unwrap();
    print!("{:?}", paragraph_commet_success);
    assert_eq!(
        3,
        paragraph_commet_success.parse_token_group.token_group.len()
    );
    match &paragraph_commet_success.parse_token_group.token_group[2].token_type {
        TokenType::Comment(str) => {
            assert_eq!(str, "/*todo*/");
        }
        _ => assert!(false),
    }

    let paragraph_commet_error = parse(">>/*todo??");
    if let Err(parse_error) = paragraph_commet_error {
        assert_eq!(
            parse_error.error_message,
            "Paragraph comment missing end flag."
        );
    } else {
        assert!(false)
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

#[test]
fn parse_success() {
    assert_eq!(true, parse("[\r\n    >\r\n    >\r\n    ,\r\n][]").is_ok());
}
