//! Bounded numeric expressions. No scripting engine, I/O, reflection, or recursion at runtime.
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub struct Equation {
    source: String,
    root: Node,
    pub variables: BTreeSet<String>,
}
#[derive(Clone, Debug)]
enum Node {
    Number(f64),
    Variable(String),
    Unary(u8, Box<Node>),
    Binary(String, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
}
impl Serialize for Equation {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.source.serialize(s)
    }
}
impl<'de> Deserialize<'de> for Equation {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}
impl Equation {
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn parse(source: &str) -> Result<Self> {
        ensure!(
            !source.is_empty() && source.len() <= 4096 && source.is_ascii(),
            "Equation must be 1–4096 ASCII bytes"
        );
        let mut parser = Parser {
            text: source.as_bytes(),
            at: 0,
            nodes: 0,
            variables: BTreeSet::new(),
        };
        let root = parser.expression(0, 0)?;
        parser.space();
        ensure!(
            parser.at == parser.text.len(),
            "Unexpected equation text at column {}",
            parser.at + 1
        );
        Ok(Self {
            source: source.into(),
            root,
            variables: parser.variables,
        })
    }
    pub fn evaluate(&self, lookup: impl Fn(&str) -> Option<f64>) -> Option<f64> {
        self.evaluate_at(lookup, 0.0)
    }
    pub fn evaluate_at(&self, lookup: impl Fn(&str) -> Option<f64>, seconds: f64) -> Option<f64> {
        self.root
            .evaluate(&lookup, seconds)
            .filter(|v| v.is_finite())
    }
}
impl Node {
    fn evaluate(&self, lookup: &impl Fn(&str) -> Option<f64>, seconds: f64) -> Option<f64> {
        let truth = |v: bool| if v { 1.0 } else { 0.0 };
        let value = match self {
            Self::Number(v) => *v,
            Self::Variable(n) => lookup(n)?,
            Self::Unary(op, node) => {
                let v = node.evaluate(lookup, seconds)?;
                match op {
                    b'-' => -v,
                    b'!' => truth(v == 0.0),
                    _ => v,
                }
            }
            Self::Binary(op, a, b) => {
                let a = a.evaluate(lookup, seconds)?;
                if op == "&&" && a == 0.0 {
                    return Some(0.0);
                }
                if op == "||" && a != 0.0 {
                    return Some(1.0);
                }
                let b = b.evaluate(lookup, seconds)?;
                match op.as_str() {
                    "+" => a + b,
                    "-" => a - b,
                    "*" => a * b,
                    "/" => a / b,
                    "^" => a.powf(b),
                    "<" => truth(a < b),
                    ">" => truth(a > b),
                    "<=" => truth(a <= b),
                    ">=" => truth(a >= b),
                    "==" => truth(a == b),
                    "!=" => truth(a != b),
                    "&&" | "||" => truth(b != 0.0),
                    _ => return None,
                }
            }
            Self::Call(name, args) => {
                let a = args[0].evaluate(lookup, seconds)?;
                if name == "if" {
                    return args[if a != 0.0 { 1 } else { 2 }].evaluate(lookup, seconds);
                }
                let b = if args.len() > 1 {
                    args[1].evaluate(lookup, seconds)?
                } else {
                    0.0
                };
                let c = if args.len() > 2 {
                    args[2].evaluate(lookup, seconds)?
                } else {
                    0.0
                };
                match name.as_str() {
                    "sin" => a.sin(),
                    "cos" => a.cos(),
                    "tan" => a.tan(),
                    "asin" => a.asin(),
                    "acos" => a.acos(),
                    "atan" => a.atan(),
                    "atan2" => a.atan2(b),
                    "sinh" => a.sinh(),
                    "cosh" => a.cosh(),
                    "tanh" => a.tanh(),
                    "abs" => a.abs(),
                    "sqrt" => a.sqrt(),
                    "log" => a.ln(),
                    "log10" => a.log10(),
                    "exp" => a.exp(),
                    "round" => a.round_ties_even(),
                    "floor" => a.floor(),
                    "ceil" => a.ceil(),
                    "sign" => {
                        if a == 0.0 {
                            0.0
                        } else {
                            a.signum()
                        }
                    }
                    "min" => a.min(b),
                    "max" => a.max(b),
                    "clamp" if b <= c => a.clamp(b, c),
                    "lerp" => a + (b - a) * c,
                    "approx" => truth((a - b).abs() <= c),
                    "time" if b > 0.0 => ((seconds * 60.0).floor() * a).rem_euclid(b),
                    "rand" if a <= b => {
                        // Reproducible 60 Hz noise; no OS entropy or frame-rate dependency.
                        let mut seed = (seconds * 60.0).floor() as u64
                            ^ a.to_bits()
                            ^ b.to_bits().rotate_left(17);
                        seed = seed.wrapping_add(0x9e3779b97f4a7c15);
                        seed = (seed ^ (seed >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                        seed = (seed ^ (seed >> 27)).wrapping_mul(0x94d049bb133111eb);
                        seed ^= seed >> 31;
                        a + (b - a) * (seed >> 11) as f64 / ((1_u64 << 53) as f64)
                    }
                    _ => return None,
                }
            }
        };
        value.is_finite().then_some(value)
    }
}
struct Parser<'a> {
    text: &'a [u8],
    at: usize,
    nodes: usize,
    variables: BTreeSet<String>,
}
impl Parser<'_> {
    fn space(&mut self) {
        while self.text.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }
    fn eat(&mut self, token: u8) -> bool {
        self.space();
        if self.text.get(self.at) == Some(&token) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn expression(&mut self, precedence: u8, depth: usize) -> Result<Node> {
        ensure!(
            depth <= 32 && self.nodes < 512,
            "Equation exceeds 32 levels or 512 nodes"
        );
        self.nodes += 1;
        self.space();
        let mut lhs = if self.text.get(self.at).is_some_and(|c| b"+-!".contains(c)) {
            let op = self.text[self.at];
            self.at += 1;
            Node::Unary(op, Box::new(self.expression(6, depth + 1)?))
        } else if self.eat(b'(') {
            let n = self.expression(0, depth + 1)?;
            ensure!(self.eat(b')'), "Missing closing parenthesis");
            n
        } else if self.eat(b'\'') {
            let n = self.expression(0, depth + 1)?;
            ensure!(self.eat(b'\''), "Missing closing quote");
            n
        } else if self
            .text
            .get(self.at)
            .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
        {
            let start = self.at;
            while self
                .text
                .get(self.at)
                .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
            {
                self.at += 1;
            }
            if self.text.get(self.at).is_some_and(|c| b"eE".contains(c)) {
                self.at += 1;
                if self.text.get(self.at).is_some_and(|c| b"+-".contains(c)) {
                    self.at += 1;
                }
                while self.text.get(self.at).is_some_and(u8::is_ascii_digit) {
                    self.at += 1;
                }
            }
            let value: f64 = std::str::from_utf8(&self.text[start..self.at])?.parse()?;
            ensure!(value.is_finite(), "Nonfinite numeric literal");
            Node::Number(value)
        } else {
            let start = self.at;
            while self
                .text
                .get(self.at)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
            {
                self.at += 1;
            }
            ensure!(
                self.at > start && self.at - start <= 128,
                "Expected a number or input at column {}",
                start + 1
            );
            let name = std::str::from_utf8(&self.text[start..self.at])?.to_owned();
            if self.eat(b'(') {
                let arity = match name.as_str() {
                    "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "sinh" | "cosh" | "tanh"
                    | "abs" | "sqrt" | "log" | "log10" | "exp" | "round" | "floor" | "ceil"
                    | "sign" => 1,
                    "atan2" | "min" | "max" | "time" | "rand" => 2,
                    "clamp" | "approx" | "lerp" | "if" => 3,
                    _ => bail!("Unsupported function {name}"),
                };
                let mut args = Vec::new();
                for i in 0..arity {
                    if i > 0 {
                        ensure!(self.eat(b','), "{name} needs {arity} arguments");
                    }
                    args.push(self.expression(0, depth + 1)?);
                }
                ensure!(
                    self.eat(b')'),
                    "{name} needs {arity} arguments and a closing parenthesis"
                );
                Node::Call(name, args)
            } else {
                match name.as_str() {
                    "pi" => Node::Number(std::f64::consts::PI),
                    "e" => Node::Number(std::f64::consts::E),
                    "true" => Node::Number(1.0),
                    "false" => Node::Number(0.0),
                    _ => {
                        self.variables.insert(name.clone());
                        Node::Variable(name)
                    }
                }
            }
        };
        loop {
            self.space();
            let rest = &self.text[self.at..];
            let Some((op, level)) = [
                ("||", 1),
                ("&&", 2),
                ("<=", 3),
                (">=", 3),
                ("==", 3),
                ("!=", 3),
                ("<", 3),
                (">", 3),
                ("+", 4),
                ("-", 4),
                ("*", 5),
                ("/", 5),
                ("^", 7),
            ]
            .into_iter()
            .find(|(op, _)| rest.starts_with(op.as_bytes())) else {
                break;
            };
            if level < precedence {
                break;
            }
            self.at += op.len();
            let rhs = self.expression(if op == "^" { level } else { level + 1 }, depth + 1)?;
            self.nodes += 1;
            ensure!(self.nodes <= 512, "Equation exceeds 512 nodes");
            lhs = Node::Binary(op.into(), Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }
}
