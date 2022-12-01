use brainfuck_analyzer::{ParseError, Position, Range, Token, TokenGroup};

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
            Token::Accept => output.push_str(","),
            _ => output.push_str("?"),
        };
    }

    output
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
