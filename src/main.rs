use anyhow::Context;
use std::env;
use std::io;
use std::process;

// Usage: echo <input_text> | your_program.sh -E <pattern>
fn main() {
    match run() {
        Ok(ok) => {
            if ok {
                process::exit(0);
            } else {
                process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    }
}

fn run() -> anyhow::Result<bool> {
    if !matches!(env::args().nth(1), Some(flag) if flag == "-E") {
        anyhow::bail!("Expected -E as the first argument.");
    }

    if let Some(pattern) = env::args().nth(2) {
        let mut input_line = String::new();

        io::stdin()
            .read_line(&mut input_line)
            .context("reading input")?;

        if let Some(pattern) = Pattern::parse_either(&mut pattern.chars().peekable(), EndFlags::empty())? {
            println!("{pattern:?}");
            let mut input_iter = input_line.chars().enumerate().peekable();
            while input_iter.peek() != None {
                if pattern.matches(&mut input_iter) {
                    return Ok(true);
                } else {
                    input_iter.next();
                }
            }

            Ok(false)
        } else {
            Ok(true)
        }
    } else {
        anyhow::bail!("No pattern provided.");
    }
}

trait CharsIterExt {
    fn expect(&mut self) -> anyhow::Result<char>;
}

impl<I> CharsIterExt for I
where
    I: Iterator<Item = char>,
{
    fn expect(&mut self) -> anyhow::Result<char> {
        if let Some(c) = self.next() {
            Ok(c)
        } else {
            Err(anyhow::anyhow!("expected a character, but ran out."))
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    struct EndFlags: u8 {
        const RPAREN = 1 << 0;
        const PIPE = 1 << 1;
    }
}

type InputIter<'a> = std::iter::Peekable<std::iter::Enumerate<std::str::Chars<'a>>>;
type PatternIter<'a> = std::iter::Peekable<std::str::Chars<'a>>;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Pattern {
    Literal(char),
    Digit,
    Alphanumeric,
    CharacterGroup { positive: bool, group: String },
    StartAnchor,
    EndAnchor,
    OneOrMore(Box<Pattern>),
    ZeroOrOne(Box<Pattern>),
    Wildcard,
    List(Vec<Pattern>),
    Either(Vec<Pattern>),
}

impl Pattern {
    pub fn parse_either(iter: &mut PatternIter, end: EndFlags) -> anyhow::Result<Option<Self>> {
        let mut pattern = None;

        while let Some(item) = Self::parse_list(iter, end | EndFlags::PIPE)? {
            pattern = if let Some(pattern) = pattern.take() {
                if let Pattern::Either(mut items) = pattern {
                    items.push(item);
                    Some(Pattern::Either(items))
                } else {
                    let items = vec![pattern, item];
                    Some(Pattern::Either(items))
                }
            } else {
                Some(item)
            };

            if let Some(c) = iter.peek().copied() {
                if c == '|' {
                    iter.next();
                } else {
                    break;
                }
            }
        }

        Ok(pattern)
    }

    pub fn parse_list(iter: &mut PatternIter, end: EndFlags) -> anyhow::Result<Option<Self>> {
        let mut pattern = None;

        while let Some(item) = Self::parse_one(iter)? {
            pattern = if let Some(pattern) = pattern.take() {
                if let Pattern::List(mut items) = pattern {
                    items.push(item);
                    Some(Pattern::List(items))
                } else {
                    let items = vec![pattern, item];
                    Some(Pattern::List(items))
                }
            } else {
                Some(item)
            };

            if let Some(c) = iter.peek().copied() {
                if c == ')' && end.contains(EndFlags::RPAREN) || c == '|' && end.contains(EndFlags::PIPE) {
                    break;
                }
            }
        }

        Ok(pattern)
    }

    pub fn parse_one(iter: &mut PatternIter) -> anyhow::Result<Option<Self>> {
        if let Some(c) = iter.next() {
            let mut item = match c {
                '\\' => {
                    let c = iter.expect()?;
                    match c {
                        'd' => Pattern::Digit,
                        'w' => Pattern::Alphanumeric,
                        c => return Err(anyhow::anyhow!("expected 'd' or 'w', got '{}'", c)),
                    }
                }
                '(' => {
                    if let Some(item) = Self::parse_either(iter, EndFlags::RPAREN)? {
                        let c = iter.expect()?;
                        anyhow::ensure!(c == ')', "expected ')'");
                        item
                    } else {
                        anyhow::bail!("empty group is not allowed");
                    }
                }
                '[' => {
                    let mut group = String::new();

                    let c = iter.expect()?;
                    let positive = if c == '^' {
                        false
                    } else {
                        group.push(c);
                        true
                    };

                    loop {
                        let c = iter.expect()?;
                        if c == ']' {
                            break;
                        }
                        group.push(c);
                    }

                    Pattern::CharacterGroup { positive, group }
                }
                '^' => Pattern::StartAnchor,
                '$' => Pattern::EndAnchor,
                '+' => anyhow::bail!("can't use '+' at the start of the pattern"),
                '?' => anyhow::bail!("can't use '+' at the start of the pattern"),
                '.' => Pattern::Wildcard,
                c => Pattern::Literal(c),
            };

            while let Some(c) = iter.peek().copied() {
                if c == '+' {
                    item = Pattern::OneOrMore(Box::new(item))
                } else if c == '?' {
                    item = Pattern::ZeroOrOne(Box::new(item))
                } else {
                    break;
                }
                iter.next();
            }

            Ok(Some(item))
        } else {
            Ok(None)
        }
    }

    fn matches(&self, iter: &mut InputIter) -> bool {
        if let Some((i, c)) = iter.peek().copied() {
            match self {
                Pattern::Literal(expected) => {
                    if *expected == c {
                        iter.next();
                        true
                    } else {
                        false
                    }
                }
                Pattern::Digit => {
                    if c.is_digit(10) {
                        iter.next();
                        true
                    } else {
                        false
                    }
                }
                Pattern::Alphanumeric => {
                    if c.is_alphanumeric() {
                        iter.next();
                        true
                    } else {
                        false
                    }
                }
                Pattern::CharacterGroup {
                    positive,
                    group: chars,
                } => {
                    if !*positive ^ chars.contains(c) {
                        iter.next();
                        true
                    } else {
                        false
                    }
                }
                Pattern::StartAnchor => i == 0,
                Pattern::EndAnchor => false,
                Pattern::OneOrMore(inner) => {
                    if !inner.matches(iter) {
                        false
                    } else {
                        while inner.matches(iter) {}
                        true
                    }
                }
                Pattern::ZeroOrOne(inner) => {
                    inner.matches(iter);
                    true
                }
                Pattern::Wildcard => {
                    iter.next();
                    true
                }
                Pattern::Either(items) => {
                    for item in items.iter() {
                        let mut tmp_iter = iter.clone();
                        if item.matches(&mut tmp_iter) {
                            *iter = tmp_iter;
                            return true;
                        }
                    }

                    false
                }
                Pattern::List(items) => {
                    for item in items.iter() {
                        if !item.matches(iter) {
                            return false;
                        }
                    }

                    true
                }
            }
        } else {
            *self == Pattern::EndAnchor
        }
    }
}
