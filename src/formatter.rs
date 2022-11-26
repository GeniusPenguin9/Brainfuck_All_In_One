use brainfuck_analyzer::{Token, TokenGroup};

pub fn my_format(str: &str) -> String {
    let token_group = brainfuck_analyzer::parse(str);
    println!("Emmmmmmmmmm \n{:?}", token_group);
    _print(&token_group, 0)
}

fn _print(token_group: &TokenGroup, tab_number: usize) -> String {
    let tab = "    ".to_string();
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
    let actual = my_format(">[>[<,]]");
    print!("Actual value:\n{}", actual);
    assert_eq!(">\n[\n    >\n    [\n        <\n        ,\n    ]\n]", actual);
}
