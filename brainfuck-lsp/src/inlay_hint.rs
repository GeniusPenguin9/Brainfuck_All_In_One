use brainfuck_analyzer::{ParseError, Position, Token, TokenGroup, TokenType};
use core::slice::Iter;

#[derive(Debug, PartialEq)]
pub struct InlayHint {
    pub position: Position,
    pub label: String,
}

impl InlayHint {
    pub fn inlay_hint_string(input: &str) -> Result<Vec<InlayHint>, ParseError> {
        let token_group = brainfuck_analyzer::parse(input)?;
        Ok(Self::_inlay_hint(&token_group.parse_token_group))
    }

    pub fn _inlay_hint(token_group: &TokenGroup) -> Vec<InlayHint> {
        let mut result = Vec::new();
        let iter = token_group.tokens().into_iter();
        let mut token_iter = TokenIter::new(iter);
        while let Some(mut v) = token_iter.next() {
            result.append(&mut v);
        }
        result
    }
}

enum TokenState {
    Move,
    Change,
    IO,
    Default,
}

struct TokenIter<'a> {
    token_iter: Iter<'a, Token>,
    state: TokenState,
    last_position: Option<Position>,
    position: Position,

    pointer_increment_count: i32,
    pointer_decrement_count: i32,
    increment_count: i32,
    decrement_count: i32,
    output_count: i32,
    input_count: i32,
}

impl<'a> TokenIter<'a> {
    fn new(token_iter: Iter<'a, Token>) -> TokenIter {
        TokenIter {
            token_iter,
            state: TokenState::Default,
            last_position: Option::None,
            position: Position {
                line: 0,
                character: 0,
            },

            pointer_increment_count: 0,
            pointer_decrement_count: 0,
            increment_count: 0,
            decrement_count: 0,
            output_count: 0,
            input_count: 0,
        }
    }

    fn _reset_count(&mut self) {
        self.pointer_increment_count = 0;
        self.pointer_decrement_count = 0;
        self.increment_count = 0;
        self.decrement_count = 0;
        self.output_count = 0;
        self.input_count = 0;
    }

    fn next(&mut self) -> Option<Vec<InlayHint>> {
        let mut result = Vec::new();
        let token_option = self.token_iter.next();
        self.last_position = Some(self.position);
        if let Some(token) = token_option {
            self.position = token.range.end;
            match self.state {
                TokenState::Move => match &token.token_type {
                    TokenType::PointerDecrement => {
                        self.pointer_decrement_count += 1;
                    }
                    TokenType::PointerIncrement => {
                        self.pointer_increment_count += 1;
                    }
                    TokenType::Decrement => {
                        self.state = TokenState::Change;
                        self.decrement_count += 1;
                    }
                    TokenType::Increment => {
                        self.state = TokenState::Change;
                        self.increment_count += 1;
                    }
                    TokenType::Input => {
                        self.state = TokenState::IO;
                        self.input_count += 1;
                    }
                    TokenType::Output => {
                        self.state = TokenState::IO;
                        self.output_count += 1;
                    }
                    TokenType::SubGroup(sg) => {
                        self.state = TokenState::Default;
                        result.push(InlayHint {
                            position: self.last_position.unwrap(),
                            label: self._count_to_label(),
                        });
                        self._reset_count();

                        let mut sub_group_result = InlayHint::_inlay_hint(sg);
                        result.append(&mut sub_group_result);
                    }
                    _ => (),
                },
                TokenState::Change => match &token.token_type {
                    TokenType::PointerDecrement => {
                        self.state = TokenState::Default;
                        result.push(InlayHint {
                            position: self.last_position.unwrap(),
                            label: self._count_to_label(),
                        });
                        self._reset_count();

                        self.state = TokenState::Move;
                        self.pointer_decrement_count += 1;
                    }
                    TokenType::PointerIncrement => {
                        self.state = TokenState::Default;
                        result.push(InlayHint {
                            position: self.last_position.unwrap(),
                            label: self._count_to_label(),
                        });
                        self._reset_count();

                        self.state = TokenState::Move;
                        self.pointer_increment_count += 1;
                    }
                    TokenType::Decrement => {
                        self.decrement_count += 1;
                    }
                    TokenType::Increment => {
                        self.increment_count += 1;
                    }
                    TokenType::Input => {
                        self.state = TokenState::IO;
                        self.input_count += 1;
                    }
                    TokenType::Output => {
                        self.state = TokenState::IO;
                        self.output_count += 1;
                    }
                    TokenType::SubGroup(sg) => {
                        self.state = TokenState::Default;
                        result.push(InlayHint {
                            position: self.last_position.unwrap(),
                            label: self._count_to_label(),
                        });
                        self._reset_count();

                        let mut sub_group_result = InlayHint::_inlay_hint(sg);
                        result.append(&mut sub_group_result);
                    }
                    _ => (),
                },
                TokenState::IO => match &token.token_type {
                    TokenType::PointerDecrement => {
                        self.state = TokenState::Default;
                        result.push(InlayHint {
                            position: self.last_position.unwrap(),
                            label: self._count_to_label(),
                        });
                        self._reset_count();

                        self.state = TokenState::Move;
                        self.pointer_decrement_count += 1;
                    }
                    TokenType::PointerIncrement => {
                        self.state = TokenState::Default;
                        result.push(InlayHint {
                            position: self.last_position.unwrap(),
                            label: self._count_to_label(),
                        });
                        self._reset_count();

                        self.state = TokenState::Move;
                        self.pointer_increment_count += 1;
                    }
                    TokenType::Decrement => {
                        self.state = TokenState::Default;
                        result.push(InlayHint {
                            position: self.last_position.unwrap(),
                            label: self._count_to_label(),
                        });
                        self._reset_count();

                        self.state = TokenState::Change;
                        self.decrement_count += 1;
                    }
                    TokenType::Increment => {
                        self.state = TokenState::Default;
                        result.push(InlayHint {
                            position: self.last_position.unwrap(),
                            label: self._count_to_label(),
                        });
                        self._reset_count();

                        self.state = TokenState::Change;
                        self.increment_count += 1;
                    }
                    TokenType::Input => {
                        self.state = TokenState::Default;
                        result.push(InlayHint {
                            position: self.last_position.unwrap(),
                            label: self._count_to_label(),
                        });
                        self._reset_count();

                        self.state = TokenState::IO;
                        self.input_count += 1;
                    }
                    TokenType::Output => {
                        self.state = TokenState::Default;
                        result.push(InlayHint {
                            position: self.last_position.unwrap(),
                            label: self._count_to_label(),
                        });
                        self._reset_count();

                        self.state = TokenState::IO;
                        self.output_count += 1;
                    }
                    TokenType::SubGroup(sg) => {
                        self.state = TokenState::Default;
                        result.push(InlayHint {
                            position: self.last_position.unwrap(),
                            label: self._count_to_label(),
                        });
                        self._reset_count();

                        let mut sub_group_result = InlayHint::_inlay_hint(sg);
                        result.append(&mut sub_group_result);
                    }
                    _ => (),
                },
                TokenState::Default => match &token.token_type {
                    TokenType::PointerDecrement => {
                        self.state = TokenState::Move;
                        self.pointer_decrement_count += 1;
                    }
                    TokenType::PointerIncrement => {
                        self.state = TokenState::Move;
                        self.pointer_increment_count += 1;
                    }
                    TokenType::Decrement => {
                        self.state = TokenState::Change;
                        self.decrement_count += 1;
                    }
                    TokenType::Increment => {
                        self.state = TokenState::Change;
                        self.increment_count += 1;
                    }
                    TokenType::Input => {
                        self.state = TokenState::IO;
                        self.input_count += 1;
                    }
                    TokenType::Output => {
                        self.state = TokenState::IO;
                        self.output_count += 1;
                    }
                    TokenType::SubGroup(_) => {
                        result.push(InlayHint {
                            position: self.last_position.unwrap(),
                            label: self._count_to_label(),
                        });
                    }
                    _ => (),
                },
            }
            Some(result)
        } else {
            match self.state {
                TokenState::Default => None,
                _ => {
                    result.push(InlayHint {
                        position: self.last_position.unwrap(),
                        label: self._count_to_label(),
                    });
                    self._reset_count();
                    self.state = TokenState::Default;

                    Some(result)
                }
            }
        }
    }

    fn _count_to_label(&self) -> String {
        let mut result = String::new();

        match self.pointer_increment_count - self.pointer_decrement_count {
            x if x > 0 => result.push_str(&format!(">{} ", x)),
            x if x < 0 => result.push_str(&format!("<{} ", x.abs())),
            _ => (),
        }

        match self.increment_count - self.decrement_count {
            x if x > 0 => result.push_str(&format!("+{} ", x)),
            x if x < 0 => result.push_str(&format!("-{} ", x.abs())),
            _ => (),
        }

        match self.output_count {
            x if x > 0 => result.push_str(&format!("output")),
            x if x < 0 => panic!(),
            _ => (),
        }

        match self.input_count {
            x if x > 0 => result.push_str(&format!("input")),
            x if x < 0 => panic!(),
            _ => (),
        }

        result
    }
}

#[test]

fn test_inlay_hint_string_should_success() {
    let actual = InlayHint::inlay_hint_string(">>+,,..>>[<]");
    print!("Actual value:\n{:?}", actual);
    assert_eq!(6, actual.unwrap().len());
}
