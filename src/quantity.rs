//! Calculations with units and currencies, their dimensions checked: `100 EUR + 2000 AED - 200 USD
//! to USD`, `100 km/h / 20 mi/h` (no dimension: 3.1069), `10 km + 5 mi`, `3 m * 2 m`, `1.85 EUR/l to
//! USD/gal`.
//!
//! Each value is reduced to SI units with its dimensions (`config::Dims`; money in US dollars): `+`
//! and `-` need the same dimensions, `*` and `/` add and subtract them, a power, a function, `!` and
//! `root` take plain numbers (`sqrt` also a square: `sqrt(4 m2)` = 2 m). Temperatures (an offset
//! scale) and reciprocal units (`l/100km`) are refused. Without a target the result is in the
//! local currency (money alone), in the first value's unit (its dimensions), a plain number (`no
//! dimension`), or in the units written, multiplied (`3 m * 2 m` = 6 m^2, `100 EUR / 8 h` = 12.5 EUR/h).
//!
//! A request is one with two values with units at least, with money in a compound unit (`EUR/l`),
//! or with one value whose unit can't be the whole value's (`10 km * 3`, `sqrt(16 m2)`): `10 km
//! mi`, `(10+5) km mi` and `100 + 20 sqft m2` stay conversions.

use crate::calc::{self, Expr, Func, Op};
use crate::config::{Config, Dims, MONEY};
use crate::convert::{self, ConvertError, Money};
use crate::currency;
use crate::CliError;

/// One unit of a result's unit, to a power: `h` in `EUR/h`. A compound unit as written is split
/// into these (`km/h`: km, h⁻¹), so `100 km/h * 2 h` is in km.
#[derive(Clone, Debug)]
struct Factor {
    symbol: String,
    /// One of it in SI units.
    si: f64,
    dims: Dims,
    power: i32,
}

/// A value: in SI units (money in US dollars), with its dimensions and the units it was written in.
#[derive(Clone, Debug)]
struct Quantity {
    si: f64,
    dims: Dims,
    units: Vec<Factor>,
}

impl Quantity {
    fn number(value: f64) -> Quantity {
        Quantity { si: value, dims: [0; 6], units: Vec::new() }
    }

    fn plain(&self) -> bool {
        self.dims == [0; 6]
    }

    /// Its unit as written (`km^2`, `EUR/h`), for people.
    fn symbol(&self) -> String {
        let part = |f: &Factor, power: i32| match power {
            1 => f.symbol.clone(),
            p => format!("{}^{p}", f.symbol),
        };
        let over: Vec<String> = self.units.iter().filter(|f| f.power > 0).map(|f| part(f, f.power)).collect();
        let under: Vec<String> = self.units.iter().filter(|f| f.power < 0).map(|f| part(f, -f.power)).collect();
        let over = if over.is_empty() { "1".to_string() } else { over.join("*") };
        std::iter::once(over).chain(under).collect::<Vec<_>>().join("/")
    }

    /// Its unit, or `a number`, for an error.
    fn described(&self) -> String {
        match self.units.is_empty() {
            true => "a number".to_string(),
            false => format!("`{}`", self.symbol()),
        }
    }

    /// One of its unit as written, in SI units.
    fn unit_si(&self) -> f64 {
        self.units.iter().map(|f| f.si.powi(f.power)).product()
    }

    /// `self` times `other` to `sign` (1, or -1 to divide): the dimensions added, each unit of
    /// `other` into the one of `self` with its dimensions (`km/h / mi/h`: none left).
    fn times(mut self, other: Quantity, sign: i8) -> Result<Quantity, ConvertError> {
        for (d, o) in self.dims.iter_mut().zip(other.dims) {
            *d = d.checked_add(o.checked_mul(sign).ok_or_else(too_large)?).ok_or_else(too_large)?;
        }
        for f in other.units {
            let power = f.power * i32::from(sign);
            match self.units.iter_mut().find(|u| u.dims == f.dims) {
                Some(u) => u.power += power,
                None => self.units.push(Factor { power, ..f }),
            }
        }
        self.units.retain(|u| u.power != 0);
        Ok(self)
    }
}

fn too_large() -> ConvertError {
    ConvertError::Domain("the powers of the units are too large (at most 127)".to_string())
}

/// A value as written, for the notes: `2000 AED`.
struct Term {
    value: f64,
    symbol: String,
    si: f64,
    dims: Dims,
}

/// The answer to a calculation with units.
pub struct Answer {
    /// The result with its unit: `426.33 USD`, `3.1069`.
    pub result: String,
    /// The calculation as read, without its target: `100 EUR + 2000 AED - 200 USD`.
    pub from: String,
    /// `no dimension`, and each value converted (`2000 AED = 544.59 USD`).
    pub notes: Vec<String>,
    pub warnings: Vec<String>,
    /// How old the exchange rates are (`2d`), when money was converted.
    pub age: Option<String>,
}

/// Whether `word` is a unit or a currency code: what may follow a value in a calculation. Not a
/// function or a constant (`sqrt`, `pi`).
fn is_unit(config: &Config, word: &str) -> bool {
    let code = |w: &str| currency::code_of(w).map(|_| Ok(1.0));
    let named = word.chars().all(char::is_alphanumeric) && (calc::starts_expression(word) || word.eq_ignore_ascii_case("of"));
    !named && convert::resolve_in(config, word, Some(&code)).is_ok()
}

/// The calculation, the text after it and its words (the target: `to USD`, `in km`, or none), when
/// `input` is a calculation with units.
fn read<'a>(config: &Config, input: &'a str) -> Option<(Expr, &'a str, Vec<&'a str>)> {
    let (expr, rest) = calc::parse_quantities(input, &|w| is_unit(config, w)).ok()?;
    let mut units = Vec::new();
    leaves(&expr, &mut units);
    let target: Vec<&str> = rest.split_whitespace().collect();
    let to = match target[..] {
        [to] | [_, to] => Some(to),
        _ => None,
    };
    // Money with a unit (`EUR/l`): a currency alone is no unit (`12 USD EUR` is a conversion).
    let compound = |u: &str| u.contains(['/', '*', '·']) && has_money(config, u);
    // One value with a unit: a calculation only where it can't be a conversion of the whole value
    // (`10 km * 3`, `sqrt(16 m2)`; but `10 km mi`, `100 + 20 sqft m2`).
    let conversion = || match calc::parse_prefix(input) {
        Ok((_, rest)) => rest.split_whitespace().count() <= 3 && !rest.split_whitespace().any(|w| w.starts_with(['+', '-', '*', '/', '^', '(', ')', '%'])),
        Err(_) => false,
    };
    let ours = units.len() >= 2 || units.iter().copied().chain(to).any(compound) || (units.len() == 1 && !matches!(expr, Expr::Quantity(..)) && !conversion());
    ours.then_some((expr, rest, target))
}

/// Whether `input` is a calculation with money, and a right one (`history.rs`: `12 eur -1d` is
/// euros a day ago, not a day taken from euros): its dimensions fit, every currency worth a dollar.
pub fn is_money_request(config: &Config, input: &str) -> bool {
    let Some((expr, _, target)) = read(config, input) else { return false };
    let mut units = Vec::new();
    leaves(&expr, &mut units);
    if !units.iter().copied().chain(target.last().copied()).any(|u| has_money(config, u)) {
        return false;
    }
    let shaped = match target[..] {
        [] | [_] => true,
        [c, _] => currency::connector(c),
        _ => false,
    };
    let dollar = |w: &str| currency::code_of(w).map(|_| Ok(1.0));
    shaped && (Eval { config, money: Some(&dollar), terms: Vec::new() }).eval(&expr).is_ok()
}

/// Whether a unit as written has a currency in it: `EUR`, `EUR/l` (not `cup`, a unit first).
fn has_money(config: &Config, unit: &str) -> bool {
    unit.split(['/', '*', '·']).any(|p| currency::code_of(p).is_some() && convert::find_unit(config, p).is_none())
}

/// The units written in `expr`, in order.
fn leaves<'e>(expr: &'e Expr, out: &mut Vec<&'e str>) {
    match expr {
        Expr::Quantity(e, unit) => {
            leaves(e, out);
            out.push(unit);
        }
        Expr::Num(_) | Expr::Const(..) => {}
        Expr::Neg(e) | Expr::Call(_, e) | Expr::Factorial(e) | Expr::Degrees(e) | Expr::Percent(e) => leaves(e, out),
        Expr::Root(a, b) | Expr::Bin(_, a, b) => {
            leaves(a, out);
            leaves(b, out);
        }
    }
}

/// A calculation with units, if `input` is one; `context` (the exchange rates) is asked for only
/// when money is in it.
pub fn request(config: &Config, input: &str, precision: usize, context: impl FnOnce() -> currency::Context) -> Option<Result<Answer, CliError>> {
    let (expr, rest, target) = read(config, input)?;
    // The calculation as written, without its target.
    let written = input[..input.len() - rest.len()].trim();
    Some(answer(config, written, precision, &expr, &target, context))
}

fn answer(
    config: &Config,
    written: &str,
    precision: usize,
    expr: &Expr,
    target: &[&str],
    context: impl FnOnce() -> currency::Context,
) -> Result<Answer, CliError> {
    let to = match target {
        [] => None,
        // The target comes next: `10 km + 5 mi to`.
        [c] if currency::connector(c) && !c.eq_ignore_ascii_case("in") => {
            return Err(CliError::Incomplete("now the unit of the result".to_string(), Vec::new()))
        }
        [to] => Some(*to),
        [c, to] if currency::connector(c) => Some(*to),
        _ => return Err(CliError::Usage("expected a calculation with units, then the unit of the result: `10 km + 5 mi to km`".to_string())),
    };
    let mut units = Vec::new();
    leaves(expr, &mut units);
    let money = units.iter().copied().chain(to).any(|u| has_money(config, u));
    let ctx = money.then(context);
    let day = match &ctx {
        Some(ctx) => Some(currency::rates_day(ctx).map_err(|e| CliError::from(currency::Error::NoRates(e)))?),
        None => None,
    };
    let dollars = |code: &str| ctx.as_ref().and_then(|ctx| currency::dollars(ctx, code));
    let mut eval = Eval { config, money: Some(&dollars), terms: Vec::new() };
    let q = eval.eval(expr)?;
    if !q.si.is_finite() {
        return Err(CliError::Convert(ConvertError::NotFinite));
    }
    let mut cash = [0; 6];
    cash[MONEY] = 1;
    // The local currency, with its rate (none: the first currency written).
    let local = ctx.as_ref().and_then(|c| c.local.as_ref()).and_then(|l| Some((l.code, dollars(l.code)?.ok()?)));
    // The unit of the result: the target; else the local currency for money, the first value's
    // unit, or the units written.
    let first = eval.terms.first().filter(|t| t.dims == q.dims);
    let (value, symbol) = match to {
        Some(to) => {
            let b = convert::resolve_in(config, to, Some(&dollars)).map_err(CliError::Convert)?;
            if b.dims != q.dims {
                let from = if q.units.is_empty() { "a number".to_string() } else { q.symbol() };
                return Err(CliError::Convert(ConvertError::Incompatible { from, to: to.to_string() }));
            }
            ((q.si - b.offset) / b.factor, b.symbol)
        }
        None if q.dims == cash && local.is_some() => {
            let (code, per) = local.expect("the local currency is there");
            (q.si / per, code.to_string())
        }
        None => match first {
            Some(t) => (q.si / t.si, t.symbol.clone()),
            None if q.plain() => (q.si, String::new()),
            None => (q.si / q.unit_si(), q.symbol()),
        },
    };
    if !value.is_finite() {
        return Err(CliError::Convert(ConvertError::NotFinite));
    }
    let show = |value: f64, dims: Dims| match dims == cash {
        true => convert::fixed(value, 2),
        false => convert::format_value(value, precision),
    };
    let result = match symbol.is_empty() {
        true => show(value, q.dims),
        false => format!("{} {symbol}", show(value, q.dims)),
    };
    let mut notes = Vec::new();
    if q.plain() && symbol.is_empty() {
        notes.push("no dimension".to_string());
    }
    // Each value in the unit it was converted to: the result's, else the first one written with
    // its dimensions (`20 mi/h = 32.1869 km/h`).
    let result_si = match symbol.is_empty() {
        true => None,
        false => Some(q.si / value),
    };
    for (i, t) in eval.terms.iter().enumerate() {
        let (si, into) = match result_si {
            Some(si) if t.dims == q.dims => (si, symbol.as_str()),
            _ => match eval.terms[..i].iter().find(|o| o.dims == t.dims && o.symbol != t.symbol) {
                Some(o) => (o.si, o.symbol.as_str()),
                None => continue,
            },
        };
        let converted = t.value * t.si / si;
        if !converted.is_finite() {
            continue;
        }
        let note = format!("{} {} = {} {into}", convert::format_value(t.value, precision), t.symbol, show(converted, t.dims));
        if into != t.symbol && eval.terms.len() > 1 && !notes.contains(&note) {
            notes.push(note);
        }
    }
    let (warnings, age) = match (&ctx, day) {
        (Some(ctx), Some(day)) => (currency::stale(ctx, day).into_iter().collect(), Some(currency::rates_age(ctx, day)).filter(|a| !a.is_empty())),
        _ => (Vec::new(), None),
    };
    let from = if expr.operations() < 2 { written.to_string() } else { expr.grouped() };
    Ok(Answer { result, from, notes, warnings, age })
}

struct Eval<'c> {
    config: &'c Config,
    money: Option<Money<'c>>,
    terms: Vec<Term>,
}

impl Eval<'_> {
    fn eval(&mut self, expr: &Expr) -> Result<Quantity, CliError> {
        // A chain (`1 m + 1 m + …`) leans to the left: folded in a loop, as `Expr::eval` does.
        let mut rights: Vec<(Op, &Expr)> = Vec::new();
        let mut node = expr;
        while let Expr::Bin(op, l, r) = node {
            rights.push((*op, r));
            node = l;
        }
        let mut value = self.one(node)?;
        for (op, r) in rights.into_iter().rev() {
            value = self.bin(op, value, r)?;
        }
        Ok(value)
    }

    /// A plain number, from what takes one only (`what`).
    fn number(&mut self, expr: &Expr, what: &str) -> Result<f64, CliError> {
        let q = self.eval(expr)?;
        match q.plain() {
            true => Ok(q.si),
            false => Err(CliError::Convert(ConvertError::NeedsNumber { what: what.to_string(), unit: q.symbol() })),
        }
    }

    fn one(&mut self, expr: &Expr) -> Result<Quantity, CliError> {
        let number = |r: Result<f64, calc::CalcError>| r.map(Quantity::number).map_err(crate::calc_error);
        match expr {
            Expr::Num(n) | Expr::Const(_, n) => Ok(Quantity::number(*n)),
            Expr::Neg(e) => self.eval(e).map(|q| Quantity { si: -q.si, ..q }),
            Expr::Quantity(e, unit) => self.quantity(e, unit),
            Expr::Call(Func::Sqrt, e) => {
                let q = self.eval(e)?;
                if q.plain() {
                    return number(Func::Sqrt.apply(q.si));
                }
                let halves = q.dims.iter().all(|d| d % 2 == 0) && q.units.iter().all(|f| f.power % 2 == 0);
                if !halves {
                    return Err(CliError::Convert(ConvertError::NeedsNumber { what: "`sqrt`".to_string(), unit: q.symbol() }));
                }
                let si = Func::Sqrt.apply(q.si).map_err(crate::calc_error)?;
                let units = q.units.into_iter().map(|f| Factor { power: f.power / 2, ..f }).collect();
                Ok(Quantity { si, dims: q.dims.map(|d| d / 2), units })
            }
            Expr::Call(f, e) => {
                let x = self.number(e, &format!("`{}`", f.name()))?;
                number(f.apply(x))
            }
            Expr::Factorial(e) => {
                let x = self.number(e, "`!`")?;
                number(calc::factorial(x))
            }
            Expr::Root(x, n) => {
                let (x, n) = (self.number(x, "`root`")?, self.number(n, "`root`")?);
                number(calc::root(x, n))
            }
            Expr::Degrees(e) => Ok(Quantity::number(self.number(e, "`°`")?.to_radians())),
            Expr::Percent(e) => Ok(Quantity::number(self.number(e, "`%`")? / 100.0)),
            Expr::Bin(..) => self.eval(expr),
        }
    }

    /// `100 EUR`, `20 mi/h`, `(1+2) km`.
    fn quantity(&mut self, value: &Expr, unit: &str) -> Result<Quantity, CliError> {
        let v = self.number(value, &format!("`{unit}`"))?;
        let r = convert::resolve_in(self.config, unit, self.money).map_err(CliError::Convert)?;
        if r.offset != 0.0 || r.unit.is_some_and(|u| u.inverse) {
            return Err(CliError::Convert(ConvertError::NotInCalculation(r.symbol)));
        }
        // A value too large to store (`1e400 Pa`, `1e308 km` in metres): no result, and no note
        // to write it in.
        if !(v * r.factor).is_finite() {
            return Err(CliError::Convert(ConvertError::NotFinite));
        }
        let units = self.factors(unit).unwrap_or_else(|| vec![Factor { symbol: r.symbol.clone(), si: r.factor, dims: r.dims, power: 1 }]);
        // A compound unit as written (`mi/h`, not `mph`), a named one by its symbol (`kilometres`: `km`).
        let symbol = if unit.contains(['/', '*', '·']) { unit.to_string() } else { r.symbol };
        self.terms.push(Term { value: v, symbol, si: r.factor, dims: r.dims });
        Ok(Quantity { si: v * r.factor, dims: r.dims, units })
    }

    /// A unit as written, split into its units to their powers (`km/h`: km, h⁻¹; `m/s^2`: m, s⁻²;
    /// `m2`: m²); `None` for a term with a number (`1/min`).
    fn factors(&self, unit: &str) -> Option<Vec<Factor>> {
        let mut out = Vec::new();
        let mut divide = false;
        let mut rest = unit;
        loop {
            let end = rest.find(['/', '*', '·']).unwrap_or(rest.len());
            let piece = &rest[..end];
            // A unit to a power (`s^2`, `m2`: the metre's square, an area of its own too).
            let powered = convert::split_power(piece).and_then(|(base, p)| {
                let power = p.trim_start_matches('^').replace('²', "2").replace('³', "3").parse::<i8>().ok()?;
                let (whole, one) = (convert::resolve(self.config, piece).ok()?, convert::resolve(self.config, base).ok()?);
                (one.dims.map(|d| d.saturating_mul(power)) == whole.dims && whole.dims != [0; 6]).then_some((base, i32::from(power)))
            });
            let (base, power) = powered.unwrap_or((piece, 1));
            let r = convert::resolve_in(self.config, base, self.money).ok()?;
            if r.offset != 0.0 || r.unit.is_some_and(|u| u.inverse) || base.starts_with(|c: char| c.is_ascii_digit()) {
                return None;
            }
            let factor = Factor { symbol: r.symbol, si: r.factor, dims: r.dims, power: if divide { -power } else { power } };
            out = Quantity { si: 1.0, dims: [0; 6], units: out }.times(Quantity { si: 1.0, dims: [0; 6], units: vec![factor] }, 1).ok()?.units;
            let Some(op) = rest[end..].chars().next() else { break };
            divide = op == '/';
            rest = &rest[end + op.len_utf8()..];
        }
        Some(out)
    }

    fn bin(&mut self, op: Op, l: Quantity, r: &Expr) -> Result<Quantity, CliError> {
        // `100 EUR + 10%`: a share of the left side.
        if let (Op::Add | Op::Sub, Expr::Percent(p)) = (op, r) {
            let share = l.si * self.number(p, "`%`")? / 100.0;
            let si = if op == Op::Add { l.si + share } else { l.si - share };
            return Ok(Quantity { si: calc::snap(si, l.si.abs()), ..l });
        }
        let r = self.eval(r)?;
        match op {
            Op::Add | Op::Sub => {
                if l.dims != r.dims {
                    return Err(CliError::Convert(ConvertError::Dimensions { add: op == Op::Add, left: l.described(), right: r.described() }));
                }
                let si = calc::apply(op, l.si, r.si).map_err(crate::calc_error)?;
                let units = if l.units.is_empty() { r.units } else { l.units };
                Ok(Quantity { si, dims: l.dims, units })
            }
            Op::Mul => {
                let si = l.si * r.si;
                Ok(Quantity { si, ..l.times(r, 1).map_err(CliError::Convert)? })
            }
            Op::Div => {
                let si = calc::apply(op, l.si, r.si).map_err(crate::calc_error)?;
                Ok(Quantity { si, ..l.times(r, -1).map_err(CliError::Convert)? })
            }
            Op::Pow => {
                if !r.plain() {
                    return Err(CliError::Convert(ConvertError::NeedsNumber { what: "a power".to_string(), unit: r.symbol() }));
                }
                if l.plain() {
                    return calc::apply(op, l.si, r.si).map(Quantity::number).map_err(crate::calc_error);
                }
                // A unit to a whole power only: `(2 m)^2` is 4 m^2, `(2 m)^0.5` is no unit.
                let n = r.si;
                if n.fract() != 0.0 {
                    return Err(CliError::Convert(ConvertError::Domain(format!("`{}` can be raised to a whole power only, not {n}", l.symbol()))));
                }
                if n.abs() > 127.0 {
                    return Err(CliError::Convert(too_large()));
                }
                let n = n as i8;
                let mut dims = l.dims;
                for d in &mut dims {
                    *d = d.checked_mul(n).ok_or_else(too_large).map_err(CliError::Convert)?;
                }
                let units = l.units.iter().map(|f| Factor { power: f.power * i32::from(n), ..f.clone() }).filter(|f| f.power != 0).collect();
                let si = calc::apply(op, l.si, r.si).map_err(crate::calc_error)?;
                Ok(Quantity { si, dims, units })
            }
        }
    }
}
