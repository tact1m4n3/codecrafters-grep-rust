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

        let mut input_iter = input_line.chars().peekable();
        while let Some(_) = input_iter.peek() {
            if pattern.matches(&mut input_iter) {
                return Ok(true);
            }
        }
        Ok(false)
    } else {
        anyhow::bail!("No pattern provided.");
    }
}

trait CharsExt: Iterator<Item = char> {
    fn expect(&mut self) -> anyhow::Result<char>;
}

impl CharsExt for std::str::Chars<'_> {
    fn expect(&mut self) -> anyhow::Result<char> {
        if let Some(c) = self.next() {
            Ok(c)
        } else {
            Err(anyhow::anyhow!("expected a character, but ran out."))
        }
    }
}

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
                c => PatternItem::Literal(c),
            };

            items.push(item);
        }

        Ok(Self { items })
    }

    pub fn matches(&self, iter: &mut impl Iterator<Item = char>) -> bool {
        for item in self.items.iter() {
            if !item.matches(iter) {
                return false;
            }
        }

        true
    }
}

enum PatternItem {
    Literal(char),
    Digit,
    Alphanumeric,
    CharacterGroup { positive: bool, group: String },
}

impl PatternItem {
    fn matches(&self, iter: &mut impl Iterator<Item = char>) -> bool {
        if let Some(inp_c) = iter.next() {
            match self {
                PatternItem::Literal(exp_c) => *exp_c == inp_c,
                PatternItem::Digit => inp_c.is_digit(10),
                PatternItem::Alphanumeric => inp_c.is_alphanumeric(),
                PatternItem::CharacterGroup {
                    positive,
                    group: chars,
                } => *positive ^ chars.contains(inp_c),
            }
        } else {
            false
        }
    }
}
