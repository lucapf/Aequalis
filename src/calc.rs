//! Arithmetic expressions used as the value: `10**2`, `[100+20]/120`, `100 + 20 + 50 / 10`.
//!
//! Standard precedence; `**` / `^` is right-associative and binds tighter than unary minus
//! (`-2**2` = -4). Shell-safe spellings: `x` for `*`, `^` for `**`, `[ ]` or `{ }` for `( )`.
//!
//! ```text
//! sum     = product (("+" | "-") product)*
//! product = unary (("*" | "x" | "/") unary)*
//! unary   = ("-" | "+") unary | function unary | power
//! power   = atom ("°" | "deg")? (("**" | "^") unary)?
//! atom    = number | constant | function group | group
//! group   = "(" sum ")" | "[" sum "]" | "{" sum "}"
//! ```
//!
//! Percentages: `20%` is 0.2; added or subtracted it is a share of the left side (`150 + 20%` =
//! 180, `150 - 20%` = 120); `20% of 150` = 30, like `150 * 20%`.
//!
//! Functions: `sqrt`, `log` (`log10`), `ln` (`loge`), `sin cos tan sinh cosh tanh`, with the
//! argument in brackets (`sqrt(16)`, then a power applies to the result: `sin(1)^2`) or without
//! (`sqrt 16`, like a unary minus: `sqrt 16 + 9` = 13). Constants: `e`, `pi` (`π`). Angles are in
//! radians; `°` or `deg` after a value converts degrees: `sin(30°)` = 0.5.
//!
//! Parsing stops at the first character that cannot continue the expression, so a unit can
//! follow it with or without a space (`100+20sqft m2`, `100 + 20 sqft m2`).

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
            Op::Div => "/",
            Op::Pow => "**",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Func {
    Sqrt,
    Log,
    Ln,
    Sin,
    Cos,
    Tan,
    Sinh,
    Cosh,
    Tanh,
}

impl Func {
    /// Names and aliases, as written.
    const NAMES: [(&'static str, Func); 11] = [
        ("sqrt", Func::Sqrt),
        ("log", Func::Log),
        ("log10", Func::Log),
        ("ln", Func::Ln),
        ("loge", Func::Ln),
        ("sinh", Func::Sinh),
        ("cosh", Func::Cosh),
        ("tanh", Func::Tanh),
        ("sin", Func::Sin),
        ("cos", Func::Cos),
        ("tan", Func::Tan),
    ];

    fn named(name: &str) -> Option<Func> {
        Self::NAMES.iter().find(|(n, _)| *n == name).map(|(_, f)| *f)
    }

    fn name(self) -> &'static str {
        Self::NAMES.iter().find(|(_, f)| *f == self).map(|(n, _)| *n).expect("every function has a name")
    }

    fn apply(self, x: f64) -> Result<f64, CalcError> {
        let domain = |what: &str| Err(CalcError::Domain(format!("`{}` of {what} ({x})", self.name())));
        Ok(match self {
            Func::Sqrt if x < 0.0 => return domain("a negative number"),
            Func::Log | Func::Ln if x <= 0.0 => return domain("a number that is not positive"),
            Func::Sqrt => x.sqrt(),
            Func::Log => x.log10(),
            Func::Ln => x.ln(),
            Func::Sin => snap(x.sin(), x.abs()),
            Func::Cos => snap(x.cos(), x.abs()),
            Func::Tan => snap(x.tan(), x.abs()),
            Func::Sinh => x.sinh(),
            Func::Cosh => x.cosh(),
            Func::Tanh => x.tanh(),
        })
    }
}

/// Floating-point noise to 0: a result that is a rounding error next to `scale` (`sin(pi)`,
/// `0.1 + 0.2 - 0.3`), so it prints as 0 and not in scientific notation. Small real values (`1e-19`
/// on its own) are kept.
fn snap(result: f64, scale: f64) -> f64 {
    if result.abs() < scale * 1e-14 {
        0.0
    } else {
        result
    }
}

/// The constants: `e`, `pi` and `π`.
const CONSTANTS: [(&str, f64); 3] = [("e", std::f64::consts::E), ("pi", std::f64::consts::PI), ("π", std::f64::consts::PI)];

#[derive(Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    /// A named constant: `e`, `pi`.
    Const(&'static str, f64),
    Neg(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
    Call(Func, Box<Expr>),
    /// A value in degrees (`30°`), as radians.
    Degrees(Box<Expr>),
    /// A percentage (`20%`): a hundredth, or a share of the left side of `+` / `-`.
    Percent(Box<Expr>),
}

#[derive(Debug, PartialEq)]
pub enum CalcError {
    Syntax(String),
    /// A zero denominator (`1/0`, `0^-1`): the evaluation stops there, the result is undefined.
    DivisionByZero,
    /// A function outside its domain: `sqrt(-1)`, `log(0)`.
    Domain(String),
}

impl Expr {
    pub fn eval(&self) -> Result<f64, CalcError> {
        Ok(match self {
            Expr::Num(n) | Expr::Const(_, n) => *n,
            Expr::Neg(e) => -e.eval()?,
            Expr::Call(f, e) => f.apply(e.eval()?)?,
            Expr::Degrees(e) => e.eval()?.to_radians(),
            Expr::Percent(e) => e.eval()? / 100.0,
            // `150 + 20%`: 20% of 150 added (or subtracted).
            Expr::Bin(op @ (Op::Add | Op::Sub), l, r) if matches!(**r, Expr::Percent(_)) => {
                let (l, share) = (l.eval()?, r.eval()?);
                let change = l * share;
                if *op == Op::Add {
                    snap(l + change, l.abs())
                } else {
                    snap(l - change, l.abs())
                }
            }
            Expr::Bin(op, l, r) => {
                let (l, r) = (l.eval()?, r.eval()?);
                match op {
                    Op::Add => snap(l + r, l.abs().max(r.abs())),
                    Op::Sub => snap(l - r, l.abs().max(r.abs())),
                    Op::Mul => l * r,
                    Op::Div if r == 0.0 => return Err(CalcError::DivisionByZero),
                    // `0^-1` is `1/0`.
                    Op::Pow if l == 0.0 && r < 0.0 => return Err(CalcError::DivisionByZero),
                    // A denominator too large to store (`7 / 770^563`): zero.
                    Op::Div if r.is_infinite() && l.is_finite() => 0.0,
                    Op::Div => l / r,
                    // `(-3^1000)^0.5`: the base is too large to store, so its sign says nothing about the root.
                    Op::Pow if l == f64::NEG_INFINITY && r.is_finite() && r.fract() != 0.0 => {
                        return Err(CalcError::Domain("a number too large to store, raised to a fractional power".into()))
                    }
                    // `(-2)^0.5`: no real number (not a result too large).
                    Op::Pow if l < 0.0 && r.is_finite() && r.fract() != 0.0 => {
                        return Err(CalcError::Domain(format!("a negative number to a fractional power ({l})^{r}: not a real number")))
                    }
                    Op::Pow => l.powf(r),
                }
            }
        })
    }

    /// Number of operations; a negative literal (`-5`) is a number, not an operation.
    pub fn operations(&self) -> usize {
        match self {
            Expr::Num(_) | Expr::Const(..) => 0,
            Expr::Neg(e) if matches!(**e, Expr::Num(_)) => 0,
            Expr::Neg(e) | Expr::Call(_, e) => 1 + e.operations(),
            Expr::Degrees(e) | Expr::Percent(e) => e.operations(),
            Expr::Bin(_, l, r) => 1 + l.operations() + r.operations(),
        }
    }

    /// The expression with every inner operation in brackets: `(100 + 20) + (50 / 10)`.
    pub fn grouped(&self) -> String {
        self.group(true)
    }

    /// Like `grouped`, with the outermost operation in brackets too: `(12 + (12 / 3))`.
    pub fn bracketed(&self) -> String {
        self.group(false)
    }

    fn group(&self, top: bool) -> String {
        match self {
            Expr::Num(n) => n.to_string(),
            Expr::Const(name, _) => name.to_string(),
            Expr::Neg(e) => format!("-{}", e.group(false)),
            Expr::Call(f, e) => format!("{}({})", f.name(), e.group(true)),
            Expr::Degrees(e) => format!("{}°", e.group(false)),
            Expr::Percent(e) => format!("{}%", e.group(false)),
            Expr::Bin(op, l, r) if top => format!("{} {op} {}", l.group(false), r.group(false)),
            Expr::Bin(..) => format!("({})", self.group(true)),
        }
    }
}

/// True when `token` begins with something only an expression can start with, so it is a value
/// and not a unit: `100`, `.5`, `-40C`, `(1+2)`, `[1+2]km`, `sqrt(16)`, `sqrt`, `pi/2`, `e`.
pub fn starts_expression(token: &str) -> bool {
    let rest = token.strip_prefix(['-', '+']).unwrap_or(token);
    if rest.starts_with(|c: char| c.is_ascii_digit() || matches!(c, '.' | '(' | '[' | '{')) {
        return true;
    }
    let (word, after) = rest.split_at(word_len(rest));
    let next = after.chars().next();
    (Func::named(word).is_some() && next.is_none_or(|c| "([{ ".contains(c))) || (constant(word).is_some() && next.is_none_or(|c| !c.is_alphanumeric()))
}

/// The length of the name at the start of `s`: a letter, then letters and digits (`log10`), or `π`.
fn word_len(s: &str) -> usize {
    if s.starts_with('π') {
        return 'π'.len_utf8();
    }
    match s.starts_with(|c: char| c.is_ascii_alphabetic()) {
        true => s.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(s.len()),
        false => 0,
    }
}

fn constant(name: &str) -> Option<(&'static str, f64)> {
    CONSTANTS.iter().find(|(n, _)| *n == name).copied()
}

/// How many brackets, functions, signs and powers may stand inside one another (`((1))`, `--1`,
/// `2^2^2`): far more than anyone writes. Each level is a few calls deeper in the parser (about 1
/// KB of stack), so that a pasted `(((((…` is an error, not a crash: a phone runs the core on a
/// background thread, whose stack is small (1 MB on Android, 512 KB on iOS).
pub const MAX_NESTING: usize = 100;
/// How many operations a calculation may have: the evaluation goes one call deeper per operation
/// (`1+1+…+1` is a chain of them).
pub const MAX_OPERATIONS: usize = 1_000;

/// Parses the longest expression at the start of `input` and returns it with the rest of the
/// input (the units), trimmed.
pub fn parse_prefix(input: &str) -> Result<(Expr, &str), CalcError> {
    let mut p = Parser { s: input, pos: 0, calls: 0, nesting: 0, operations: 0 };
    let expr = p.sum()?;
    p.skip_ws();
    let rest = &input[p.pos..];
    // Something that can only belong to an expression, but doesn't fit where it is.
    if let Some(c) = rest.chars().next().filter(|c| c.is_ascii_digit() || ".)]}*/^".contains(*c)) {
        return Err(CalcError::Syntax(format!("unexpected `{c}` in `{}`", input.trim())));
    }
    Ok((expr, rest))
}

struct Parser<'a> {
    s: &'a str,
    pos: usize,
    /// Inside a function's argument: `°` / `deg` there are degrees (`sin(30°)`); elsewhere they
    /// are left for the angle unit (`30° rad`).
    calls: usize,
    /// How deep the parse is now (`MAX_NESTING`), and the operations so far (`MAX_OPERATIONS`).
    nesting: usize,
    operations: usize,
}

impl Parser<'_> {
    fn rest(&self) -> &str {
        &self.s[self.pos..]
    }

    fn skip_ws(&mut self) {
        self.pos = self.s.len() - self.rest().trim_start().len();
    }

    /// Consumes `tok` (after whitespace) if it is next.
    fn eat(&mut self, tok: &str) -> bool {
        self.skip_ws();
        let found = self.rest().starts_with(tok);
        if found {
            self.pos += tok.len();
        }
        found
    }

    /// One more operation (an operator, a sign, a function, `°`, `%`): an error past
    /// `MAX_OPERATIONS`.
    fn operation(&mut self) -> Result<(), CalcError> {
        self.operations += 1;
        match self.operations > MAX_OPERATIONS {
            true => Err(CalcError::Syntax(format!("the calculation is too long: at most {MAX_OPERATIONS} operations"))),
            false => Ok(()),
        }
    }

    fn sum(&mut self) -> Result<Expr, CalcError> {
        let mut left = self.product()?;
        loop {
            let op = if self.eat("+") {
                Op::Add
            } else if self.eat("-") {
                Op::Sub
            } else {
                return Ok(left);
            };
            self.operation()?;
            left = Expr::Bin(op, Box::new(left), Box::new(self.product()?));
        }
    }

    fn product(&mut self) -> Result<Expr, CalcError> {
        let mut left = self.unary()?;
        loop {
            // `20% of 150`.
            if matches!(left, Expr::Percent(_)) && self.eat_word("of") {
                self.operation()?;
                left = Expr::Bin(Op::Mul, Box::new(left), Box::new(self.unary()?));
                continue;
            }
            let op = if self.rest().trim_start().starts_with("**") {
                return Ok(left);
            } else if self.eat("*") || self.eat_times() {
                Op::Mul
            } else if self.eat("/") {
                Op::Div
            } else {
                return Ok(left);
            };
            self.operation()?;
            left = Expr::Bin(op, Box::new(left), Box::new(self.unary()?));
        }
    }

    /// Consumes the word `word` (`of`) if it is next and ends there.
    fn eat_word(&mut self, word: &str) -> bool {
        self.skip_ws();
        let found = self.rest().strip_prefix(word).is_some_and(|after| !after.starts_with(|c: char| c.is_alphanumeric()));
        if found {
            self.pos += word.len();
        }
        found
    }

    /// `x` means multiply only when an operand follows, so `2x3` is 6 but `2xyz` is left for the unit.
    fn eat_times(&mut self) -> bool {
        self.skip_ws();
        // The operand may have a sign of its own, spaced as `*` allows (`2 x - 3`, bug 014).
        let operand = |after: &str| {
            let after = after.trim_start();
            after.strip_prefix(['-', '+']).map_or(after, str::trim_start).to_string()
        };
        let found = self.rest().strip_prefix('x').is_some_and(|after| starts_expression(&operand(after)));
        if found {
            self.pos += 1;
        }
        found
    }

    /// Every nesting goes through here (a bracket's sum, a sign, a function, a power's exponent):
    /// one level deeper, at most `MAX_NESTING`.
    fn unary(&mut self) -> Result<Expr, CalcError> {
        if self.nesting == MAX_NESTING {
            return Err(CalcError::Syntax(format!(
                "the calculation is nested too deeply: at most {MAX_NESTING} brackets, functions, signs or powers inside one another"
            )));
        }
        self.nesting += 1;
        let result = self.signed();
        self.nesting -= 1;
        result
    }

    fn signed(&mut self) -> Result<Expr, CalcError> {
        if self.eat("-") {
            self.operation()?;
            Ok(Expr::Neg(Box::new(self.unary()?)))
        } else if self.eat("+") {
            self.unary()
        } else if let Some(f) = self.function(false) {
            // Without brackets: `sqrt 16`.
            self.operation()?;
            self.calls += 1;
            let argument = self.unary();
            self.calls -= 1;
            Ok(Expr::Call(f, Box::new(argument?)))
        } else {
            self.power()
        }
    }

    /// Consumes a function name if one is next and is followed by a bracket (`bracketed`) or
    /// by anything else.
    fn function(&mut self, bracketed: bool) -> Option<Func> {
        self.skip_ws();
        let len = word_len(self.rest());
        let f = Func::named(&self.rest()[..len])?;
        let next = self.rest()[len..].trim_start().chars().next();
        if next.is_some_and(|c| "([{".contains(c)) != bracketed || next.is_none() {
            return None;
        }
        self.pos += len;
        Some(f)
    }

    fn power(&mut self) -> Result<Expr, CalcError> {
        let mut base = self.atom()?;
        if self.eat_degrees() {
            self.operation()?;
            base = Expr::Degrees(Box::new(base));
        }
        if self.eat("%") {
            self.operation()?;
            base = Expr::Percent(Box::new(base));
        }
        if self.eat("**") || self.eat("^") {
            self.operation()?;
            return Ok(Expr::Bin(Op::Pow, Box::new(base), Box::new(self.unary()?)));
        }
        Ok(base)
    }

    /// `°` or `deg` after a value in a function's argument (`sin(30°)`, `cos(60 deg)`), unless the
    /// word goes on (`degree`, `°C`).
    fn eat_degrees(&mut self) -> bool {
        if self.calls == 0 {
            return false;
        }
        let rest = self.rest().trim_start();
        let skipped = self.rest().len() - rest.len();
        for sign in ["°", "deg"] {
            if let Some(after) = rest.strip_prefix(sign) {
                if !after.starts_with(|c: char| c.is_alphanumeric()) {
                    self.pos += skipped + sign.len();
                    return true;
                }
            }
        }
        false
    }

    fn atom(&mut self) -> Result<Expr, CalcError> {
        if let Some(f) = self.function(true) {
            self.operation()?;
            self.calls += 1;
            let argument = self.atom();
            self.calls -= 1;
            return Ok(Expr::Call(f, Box::new(argument?)));
        }
        self.skip_ws();
        let len = word_len(self.rest());
        if let Some((name, value)) = constant(&self.rest()[..len]) {
            self.pos += len;
            return Ok(Expr::Const(name, value));
        }
        for (open, close) in [("(", ")"), ("[", "]"), ("{", "}")] {
            if self.eat(open) {
                let inner = self.sum()?;
                if !self.eat(close) {
                    return Err(CalcError::Syntax(format!("missing `{close}` in `{}`", self.s.trim())));
                }
                return Ok(inner);
            }
        }
        self.number()
    }

    /// `12`, `0.5`, `.5`, `1e3`, `2.5E-3`. An `e` without exponent digits is left for the unit.
    fn number(&mut self) -> Result<Expr, CalcError> {
        self.skip_ws();
        let b = self.rest().as_bytes();
        let mut i = b.iter().take_while(|c| c.is_ascii_digit()).count();
        if b.get(i) == Some(&b'.') {
            i += 1 + b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
        }
        if !b[..i].iter().any(u8::is_ascii_digit) {
            let found = self.rest().split_whitespace().next();
            return Err(CalcError::Syntax(match found {
                Some(word) => format!("`{word}` is not a valid number"),
                None => format!("`{}` is incomplete", self.s.trim()),
            }));
        }
        if matches!(b.get(i), Some(b'e' | b'E')) {
            let sign = usize::from(matches!(b.get(i + 1), Some(b'+' | b'-')));
            let digits = b[(i + 1 + sign).min(b.len())..].iter().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 {
                i += 1 + sign + digits;
            }
        }
        let value = self.rest()[..i].parse().expect("digits form a valid f64");
        self.pos += i;
        Ok(Expr::Num(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(s: &str) -> f64 {
        let (e, rest) = parse_prefix(s).unwrap();
        assert_eq!(rest, "", "unparsed rest in `{s}`");
        e.eval().unwrap()
    }

    fn grouped(s: &str) -> String {
        parse_prefix(s).unwrap().0.grouped()
    }

    #[test]
    fn requested_examples() {
        assert_eq!(eval("10**2"), 100.0);
        assert_eq!(eval("(100+20)/120"), 1.0);
        assert!((eval("1/3") - 1.0 / 3.0).abs() < 1e-15);
        assert_eq!(eval("100 + 20 + 50 / 10"), 125.0);
    }

    #[test]
    fn precedence_and_associativity() {
        assert_eq!(eval("2 + 3 * 4"), 14.0);
        assert_eq!(eval("10 - 4 - 3"), 3.0);
        assert_eq!(eval("100 / 10 / 5"), 2.0);
        assert_eq!(eval("2 ** 3 ** 2"), 512.0);
        assert_eq!(eval("-2 ** 2"), -4.0);
        assert_eq!(eval("2 ** -1"), 0.5);
        assert_eq!(eval("--3"), 3.0);
        assert_eq!(eval("+3 - -2"), 5.0);
    }

    #[test]
    fn shell_safe_spellings() {
        assert_eq!(eval("10^2"), 100.0);
        assert_eq!(eval("[100+20]/120"), 1.0);
        assert_eq!(eval("{100+20}/120"), 1.0);
        assert_eq!(eval("3x4"), 12.0);
        assert_eq!(eval("3 x [1+1]"), 6.0);
        assert_eq!(eval("[{(1+1)*2}^2]"), 16.0);
    }

    #[test]
    fn numbers() {
        assert_eq!(eval(".5"), 0.5);
        assert_eq!(eval("1e3"), 1000.0);
        assert_eq!(eval("2.5E-3"), 0.0025);
        assert_eq!(eval("2e+3+1"), 2001.0);
    }

    #[test]
    fn stops_at_the_unit() {
        let rest = |s| parse_prefix(s).unwrap().1;
        assert_eq!(rest("100+20sqft m2"), "sqft m2");
        assert_eq!(rest("100 + 20 + 50 / 10 sqft m2"), "sqft m2");
        assert_eq!(rest("1e3m km"), "m km");
        assert_eq!(rest("-40C F"), "C F");
        assert_eq!(rest("10gal-uk l"), "gal-uk l");
        assert_eq!(rest("2xyz m"), "xyz m");
        assert_eq!(rest("20°C F"), "°C F");
    }

    #[test]
    fn grouping() {
        assert_eq!(grouped("100 + 20 + 50 / 10"), "(100 + 20) + (50 / 10)");
        assert_eq!(grouped("(100+20)/120"), "(100 + 20) / 120");
        assert_eq!(grouped("-2^2"), "-(2 ** 2)");
        assert_eq!(grouped("2x3+1"), "(2 * 3) + 1");
        assert_eq!(grouped("1/3"), "1 / 3");
    }

    #[test]
    fn fully_bracketed() {
        let bracketed = |s| parse_prefix(s).unwrap().0.bracketed();
        assert_eq!(bracketed("12+12/3"), "(12 + (12 / 3))");
        assert_eq!(bracketed("1/3"), "(1 / 3)");
        assert_eq!(bracketed("-2^2"), "-(2 ** 2)");
        assert_eq!(bracketed("100"), "100");
        assert_eq!(bracketed("-40"), "-40");
    }

    #[test]
    fn operation_count() {
        let ops = |s| parse_prefix(s).unwrap().0.operations();
        assert_eq!(ops("100"), 0);
        assert_eq!(ops("-40"), 0);
        assert_eq!(ops("1/3"), 1);
        assert_eq!(ops("-2**2"), 2);
        assert_eq!(ops("100 + 20 + 50 / 10"), 3);
    }

    /// Nested or long past the limits: an error, not a crash (the stack of a phone's thread).
    #[test]
    fn limits() {
        let nested = |n: usize| format!("{}1{}", "(".repeat(n), ")".repeat(n));
        assert_eq!(eval(&nested(MAX_NESTING - 1)), 1.0);
        for deep in [
            nested(MAX_NESTING),
            nested(50_000),
            format!("{}16{}", "sqrt(".repeat(50_000), ")".repeat(50_000)),
            "-".repeat(50_000) + "1",
            vec!["2"; 50_000].join("^"),
        ] {
            assert!(matches!(parse_prefix(&deep), Err(CalcError::Syntax(m)) if m.contains("nested too deeply")), "{}…", &deep[..20]);
        }
        assert_eq!(eval(&vec!["1"; MAX_OPERATIONS + 1].join("+")), (MAX_OPERATIONS + 1) as f64);
        let long = vec!["1"; 50_000].join("+");
        assert!(matches!(parse_prefix(&long), Err(CalcError::Syntax(m)) if m.contains("too long")));
    }

    #[test]
    fn errors() {
        let err = |s| parse_prefix(s).and_then(|(e, _)| e.eval()).unwrap_err();
        assert_eq!(err("1/0"), CalcError::DivisionByZero);
        assert_eq!(err("1/(2-2)"), CalcError::DivisionByZero);
        assert_eq!(err("0^-1"), CalcError::DivisionByZero);
        assert_eq!(err("0^-0.5"), CalcError::DivisionByZero);
        // Stops at the zero denominator, whatever follows.
        assert_eq!(err("1/0 * 0"), CalcError::DivisionByZero);
        assert_eq!(err("1/0 - 1/0"), CalcError::DivisionByZero);
        // A denominator too large to store: zero (not -0); a numerator too, no answer.
        let eval = |s| parse_prefix(s).and_then(|(e, _)| e.eval()).unwrap();
        assert_eq!(eval("7 / [770^563 / -3]").to_bits(), 0.0f64.to_bits());
        assert_eq!(eval("7/770^563 + 2"), 2.0);
        assert!(eval("770^563/770^563").is_nan());
        assert!(matches!(err("(1+2"), CalcError::Syntax(m) if m.contains("missing `)`")));
        assert!(matches!(err("1+2)"), CalcError::Syntax(m) if m.contains("unexpected `)`")));
        assert!(matches!(err("1+"), CalcError::Syntax(m) if m.contains("incomplete")));
        assert!(matches!(err("abc"), CalcError::Syntax(m) if m.contains("`abc` is not a valid number")));
        assert!(matches!(err("1 2"), CalcError::Syntax(_)));
        assert!(matches!(err("2***3"), CalcError::Syntax(_)));
    }

    #[test]
    fn functions_and_constants() {
        let close = |s: &str, want: f64| assert!((eval(s) - want).abs() < 1e-12, "{s} = {}", eval(s));
        close("sqrt(16)", 4.0);
        close("sqrt 16 + 9", 13.0);
        close("sqrt[2]^2", 2.0);
        close("log(1000)", 3.0);
        close("log10(0.01)", -2.0);
        close("ln(e)", 1.0);
        close("loge(e^3)", 3.0);
        close("e", std::f64::consts::E);
        close("2*pi", std::f64::consts::TAU);
        close("π/2", std::f64::consts::FRAC_PI_2);
        close("-e", -std::f64::consts::E);
        close("sin(pi/6)", 0.5);
        close("sin(30°)", 0.5);
        close("cos(60deg)", 0.5);
        close("cos(60 deg)", 0.5);
        close("sin 30°", 0.5);
        close("tan(45°)", 1.0);
        close("sin(1)^2 + cos(1)^2", 1.0);
        close("sinh(1)", 1f64.sinh());
        close("cosh(0)", 1.0);
        close("tanh(0)", 0.0);
        close("2 x pi", std::f64::consts::TAU);
        assert_eq!(grouped("sqrt 16 + 9"), "sqrt(16) + 9");
        assert_eq!(grouped("sin(30°)^2"), "sin(30°) ** 2");
        assert_eq!(grouped("loge(e^3)"), "ln(e ** 3)");
        // A function is one operation; a constant is not.
        assert_eq!(parse_prefix("sqrt(16)").unwrap().0.operations(), 1);
        assert_eq!(parse_prefix("pi").unwrap().0.operations(), 0);
    }

    #[test]
    fn percentages() {
        assert_eq!(eval("20%"), 0.2);
        assert_eq!(eval("150 + 20%"), 180.0);
        assert_eq!(eval("150 - 20%"), 120.0);
        assert_eq!(eval("150 * 20%"), 30.0);
        assert_eq!(eval("20% of 150"), 30.0);
        assert_eq!(eval("20% of 150 + 10"), 40.0);
        assert_eq!(eval("(100 + 50) + 10%"), 165.0);
        assert_eq!(eval("100 + 10% + 10%"), 121.0, "compounded, left to right");
        assert_eq!(eval("50% x 8"), 4.0);
        assert_eq!(grouped("150 + 20%"), "150 + 20%");
        assert_eq!(grouped("20% of 150"), "20% * 150");
        // Units still follow.
        assert_eq!(parse_prefix("20% of 150 km mi").unwrap().1, "km mi");
        assert_eq!(parse_prefix("150 + 20%km mi").unwrap().1, "km mi");
    }

    #[test]
    fn rounding_noise_is_zero() {
        assert_eq!(eval("sin(pi)"), 0.0);
        assert_eq!(eval("cos(pi/2)"), 0.0);
        assert_eq!(eval("tan(pi)"), 0.0);
        assert_eq!(eval("0.1 + 0.2 - 0.3"), 0.0);
        // Small values that are not noise.
        assert_eq!(eval("1.6e-19"), 1.6e-19);
        assert_eq!(eval("1e-19 + 1e-20"), 1.1e-19);
        assert_eq!(eval("sin(1e-20)"), 1e-20);
    }

    #[test]
    fn functions_leave_the_units() {
        let rest = |s| parse_prefix(s).unwrap().1;
        assert_eq!(rest("sqrt(2) km mi"), "km mi");
        assert_eq!(rest("sqrt(16)km m"), "km m");
        assert_eq!(rest("20°C F"), "°C F");
        assert_eq!(rest("2 degF"), "degF");
        // Outside a function, ° and deg are the angle unit.
        assert_eq!(rest("30° rad"), "° rad");
        assert_eq!(rest("30deg rad"), "deg rad");
        assert_eq!(rest("sin(30°) num"), "num");
    }

    #[test]
    fn domain_errors() {
        let err = |s| parse_prefix(s).and_then(|(e, _)| e.eval()).unwrap_err();
        assert!(matches!(err("sqrt(-1)"), CalcError::Domain(m) if m.contains("`sqrt` of a negative number")));
        assert!(matches!(err("log(0)"), CalcError::Domain(m) if m.contains("`log`")));
        assert!(matches!(err("ln(-e)"), CalcError::Domain(m) if m.contains("`ln`")));
        assert!(matches!(err("sqrt"), CalcError::Syntax(_)));
        // No real number, rather than a result out of range; whole powers of negatives are fine.
        assert!(matches!(err("(-2)^0.5"), CalcError::Domain(m) if m == "a negative number to a fractional power (-2)^0.5: not a real number"));
        assert_eq!(eval("(-2)^3"), -8.0);
        assert_eq!(eval("-2^0.5"), -std::f64::consts::SQRT_2, "the power first, then the sign");
    }

    #[test]
    fn expression_starts() {
        for t in ["100", ".5", "-40C", "+1", "(1+2)", "[1]km", "{2}", "-(1)", "sqrt(16)", "sqrt", "log10[2]", "e", "-pi", "pi/2", "π", "e^2"] {
            assert!(starts_expression(t), "{t}");
        }
        for t in ["F", "km", "-", "x2", "inf", "-s", "", "sqrtx", "eV", "pint", "log2", "-v"] {
            assert!(!starts_expression(t), "{t}");
        }
    }
}
