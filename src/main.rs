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

        let pattern = Pattern::parse(&pattern)?;

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
        anyhow::bail!("No pattern provided.");
    }
}

trait CharsIterExt: Iterator<Item = char> {
    fn expect(&mut self) -> anyhow::Result<char>;
}

impl CharsIterExt for std::str::Chars<'_> {
    fn expect(&mut self) -> anyhow::Result<char> {
        if let Some(c) = self.next() {
            Ok(c)
        } else {
            Err(anyhow::anyhow!("expected a character, but ran out."))
        }
    }
}

type InputIter<'a> = std::iter::Peekable<std::iter::Enumerate<std::str::Chars<'a>>>;

struct Pattern {
    items: Vec<PatternItem>,
}

impl Pattern {
    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        let mut items = Vec::new();

        let mut chars = raw.chars();
        while let Some(c) = chars.next() {
            let item = match c {
                '\\' => {
                    let c = chars.expect()?;
                    match c {
                        'd' => PatternItem::Digit,
                        'w' => PatternItem::Alphanumeric,
                        c => return Err(anyhow::anyhow!("expected d|w, got {}", c)),
                    }
                }
                '[' => {
                    let mut group = String::new();

                    let c = chars.expect()?;
                    let positive = if c == '^' {
                        false
                    } else {
                        group.push(c);
                        true
                    };

                    loop {
                        let c = chars.expect()?;
                        if c == ']' {
                            break;
                        }
                        group.push(c);
                    }

                    PatternItem::CharacterGroup { positive, group }
                }
                '^' => PatternItem::StartAnchor,
                '$' => PatternItem::EndAnchor,
                c => PatternItem::Literal(c),
            };

            items.push(item);
        }

        Ok(Self { items })
    }

    pub fn matches(&self, iter: &mut InputIter) -> bool {
        for item in self.items.iter() {
            if !item.matches(iter) {
                return false;
            }
        }

        true
    }
}

#[derive(PartialEq, Eq)]
enum PatternItem {
    Literal(char),
    Digit,
    Alphanumeric,
    CharacterGroup { positive: bool, group: String },
    StartAnchor,
    EndAnchor,
}

impl PatternItem {
    fn matches(&self, iter: &mut InputIter) -> bool {
        if let Some((i, c)) = iter.peek().copied() {
            match self {
                PatternItem::Literal(expected) => {
                    iter.next();
                    *expected == c
                }
                PatternItem::Digit => {
                    iter.next();
                    c.is_digit(10)
                }
                PatternItem::Alphanumeric => {
                    iter.next();
                    c.is_alphanumeric()
                }
                PatternItem::CharacterGroup {
                    positive,
                    group: chars,
                } => {
                    iter.next();
                    !*positive ^ chars.contains(c)
                },
                PatternItem::StartAnchor => i == 0,
                PatternItem::EndAnchor => false,
            }
        } else {
            *self == PatternItem::EndAnchor
        }
    }
}
