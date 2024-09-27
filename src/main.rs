use anyhow::Context;
use std::env;
use std::io;
use std::ops::Range;
use std::process;

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

        let mut capture_group_count = 0;
        if let Some(pattern) = Pattern::parse_either(
            &mut pattern.chars().peekable(),
            EndFlags::empty(),
            &mut capture_group_count,
            None,
        )? {
            println!("{pattern:?}");
            let mut input_iter = input_line.char_indices().peekable();
            let mut state = Vec::new();
            while input_iter.peek() != None {
                state.clear();
                state.resize(capture_group_count, None);

                if pattern.matches(&input_line, &mut input_iter, &mut state) {
                    println!("{:?}", state);
                    return Ok(true);
                } else {
                    println!("{:?}", state);
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

type PatternIter<'a> = std::iter::Peekable<std::str::Chars<'a>>;
type InputIter<'a> = std::iter::Peekable<std::str::CharIndices<'a>>;

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
    Reference(usize),
    CaptureGroup { id: usize, item: Box<Pattern> },
}

impl Pattern {
    pub fn parse_either(
        iter: &mut PatternIter,
        end: EndFlags,
        capture_group_count: &mut usize,
        parent_capture_group: Option<usize>,
    ) -> anyhow::Result<Option<Self>> {
        let mut pattern = None;

        while let Some(item) = Self::parse_list(
            iter,
            end | EndFlags::PIPE,
            capture_group_count,
            parent_capture_group,
        )? {
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

    pub fn parse_list(
        iter: &mut PatternIter,
        end: EndFlags,
        capture_group_count: &mut usize,
        parent_capture_group: Option<usize>,
    ) -> anyhow::Result<Option<Self>> {
        let mut pattern = None;

        while let Some(item) = Self::parse_one(iter, capture_group_count, parent_capture_group)? {
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
                if c == ')' && end.contains(EndFlags::RPAREN)
                    || c == '|' && end.contains(EndFlags::PIPE)
                {
                    break;
                }
            }
        }

        Ok(pattern)
    }

    pub fn parse_one(
        iter: &mut PatternIter,
        capture_group_count: &mut usize,
        parent_capture_group: Option<usize>,
    ) -> anyhow::Result<Option<Self>> {
        if let Some(c) = iter.next() {
            let mut item = match c {
                '\\' => {
                    let c = iter.expect()?;
                    match c {
                        'd' => Pattern::Digit,
                        'w' => Pattern::Alphanumeric,
                        c => {
                            if let Some(d) = c.to_digit(10) {
                                let mut num = d;
                                while let Some(c) = iter.peek() {
                                    if let Some(d) = c.to_digit(10) {
                                        iter.next();
                                        num *= 10;
                                        num += d;
                                    } else {
                                        break;
                                    }
                                }

                                anyhow::ensure!(num != 0, "back reference id can't be 0");
                                let id = num as usize - 1;
                                anyhow::ensure!(
                                    id < *capture_group_count,
                                    "back reference invalid"
                                );
                                if let Some(parent) = parent_capture_group {
                                    anyhow::ensure!(
                                        parent != id,
                                        "back reference to current capture group"
                                    );
                                }

                                Pattern::Reference(id)
                            } else {
                                anyhow::bail!("expected 'd', 'w' or number, got '{}'", c);
                            }
                        }
                    }
                }
                '(' => {
                    let id = *capture_group_count;
                    *capture_group_count += 1;
                    if let Some(item) =
                        Self::parse_either(iter, EndFlags::RPAREN, capture_group_count, Some(id))?
                    {
                        let c = iter.expect()?;
                        anyhow::ensure!(c == ')', "expected ')'");
                        Pattern::CaptureGroup {
                            id,
                            item: Box::new(item),
                        }
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

    fn matches(
        &self,
        input: &str,
        iter: &mut InputIter,
        state: &mut [Option<Range<usize>>],
    ) -> bool {
        if let Some((i, c)) = iter.peek().copied() {
            match self {
                Pattern::Literal(expected) => {
                    iter.next();
                    *expected == c
                }
                Pattern::Digit => {
                    iter.next();
                    c.is_digit(10)
                }
                Pattern::Alphanumeric => {
                    iter.next();
                    c.is_alphanumeric()
                }
                Pattern::CharacterGroup {
                    positive,
                    group: chars,
                } => {
                    iter.next();
                    !*positive ^ chars.contains(c)
                }
                Pattern::StartAnchor => i == 0,
                Pattern::EndAnchor => false,
                Pattern::OneOrMore(inner) => {
                    if !inner.matches(input, iter, state) {
                        false
                    } else {
                        let mut tmp_iter = iter.clone();
                        while inner.matches(input, &mut tmp_iter, state) {
                            *iter = tmp_iter.clone();
                        }
                        true
                    }
                }
                Pattern::ZeroOrOne(inner) => {
                    let mut tmp_iter = iter.clone();
                    if inner.matches(input, &mut tmp_iter, state) {
                        *iter = tmp_iter;
                    }
                    true
                }
                Pattern::Wildcard => {
                    iter.next();
                    true
                }
                Pattern::Either(items) => {
                    for item in items.iter() {
                        let mut tmp_iter = iter.clone();
                        if item.matches(input, &mut tmp_iter, state) {
                            *iter = tmp_iter;
                            return true;
                        }
                    }

                    false
                }
                Pattern::List(items) => {
                    for item in items.iter() {
                        if !item.matches(input, iter, state) {
                            return false;
                        }
                    }

                    true
                }
                Pattern::Reference(id) => {
                    if let Some(range) = &state[*id] {
                        let content = input.get(range.clone()).unwrap();

                        for exp_c in content.chars() {
                            if let Some((_, c)) = iter.next() {
                                if exp_c != c {
                                    return false;
                                }
                            } else {
                                return false;
                            }
                        }

                        true
                    } else {
                        false
                    }
                }
                Pattern::CaptureGroup { id, item } => {
                    let start = i;
                    if item.matches(input, iter, state) {
                        if let Some((end, _)) = iter.peek().copied() {
                            state[*id] = Some(start..end);
                        } else {
                            state[*id] = Some(start..input.len());
                        }
                        true
                    } else {
                        false
                    }
                }
            }
        } else {
            *self == Pattern::EndAnchor
        }
    }
}
