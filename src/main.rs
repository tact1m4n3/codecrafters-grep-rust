use std::env;
use std::io;
use std::process;

fn match_pattern(input_line: &str, mut pattern: &str) -> bool {
    if pattern.chars().count() == 1 {
        return input_line.contains(pattern);
    } else if pattern == "\\d" {
        return input_line.contains(|c: char| c.is_digit(10));
    } else if pattern == "\\w" {
        return input_line.contains(|c: char| c.is_alphanumeric());
    } else if pattern.starts_with('[') {
        assert!(pattern.ends_with(']'), "didn't find matching brace");
        pattern = pattern.get(1..pattern.len() - 1).unwrap();
        if pattern.starts_with('^') {
            pattern = pattern.get(1..).unwrap();
            return !input_line.contains(|c: char| pattern.contains(c));
        } else {
            return input_line.contains(|c: char| pattern.contains(c));
        }
    } else {
        panic!("Unhandled pattern: {}", pattern)
    }
}

// Usage: echo <input_text> | your_program.sh -E <pattern>
fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    if env::args().nth(1).unwrap() != "-E" {
        println!("Expected first argument to be '-E'");
        process::exit(1);
    }

    let pattern = env::args().nth(2).unwrap();
    let mut input_line = String::new();

    io::stdin().read_line(&mut input_line).unwrap();

    if match_pattern(&input_line, &pattern) {
        process::exit(0)
    } else {
        process::exit(1)
    }
}
