use brainfuck_analyzer::{Token, TokenGroup};

pub fn format_string(input: &str) -> String {
    let token_group = brainfuck_analyzer::parse(input);
    println!("Emmmmmmmmmm \n{:?}", token_group);

    match token_group {
        Ok(x) => _print(&x, 0),
        Err(_) => input.to_string(),
    }
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
    let actual = format_string(">[>[<,]]");
    print!("Actual value:\n{}", actual);
    assert_eq!(">\n[\n    >\n    [\n        <\n        ,\n    ]\n]", actual);
}
