//! Basic sanity checking on the `Value` type.
//!
//! These tests primarily test the round-trip (i.e converting to text and back)
//! behavior of `Value` using quickcheck. Note these do not test Emacs Lisp
//! syntax, as it is not round-trip safe for all values tested here.

#![cfg_attr(tarpaulin, skip)]

use quickcheck::{Arbitrary, Gen, QuickCheck, StdGen};
use quickcheck_macros::quickcheck;
use rand::{seq::SliceRandom, Rng};

use std::f64;
use std::str;

use crate as lexpr;

use lexpr::{datum, parse, print, Number, Value};

enum ValueKind {
    Nil,
    Null,
    Bool,
    Number,
    Char,
    String,
    Symbol,
    Keyword,
    Bytes,
    Cons,
    Vector,
}

fn gen_value<G: Gen>(g: &mut G, depth: usize) -> Value {
    use ValueKind::*;
    let choices = if depth >= g.size() {
        &[
            Nil, Null, Bool, Number, Char, String, Symbol, Keyword, Bytes,
        ] as &[ValueKind]
    } else {
        &[
            Nil, Null, Bool, Number, Char, String, Symbol, Keyword, Bytes, Cons, Vector,
        ]
    };
    match choices.choose(g).unwrap() {
        Nil => Value::Nil,
        Null => Value::Null,
        Bool => Value::Bool(g.gen()),
        Number => Value::Number(Arbitrary::arbitrary(g)),
        Char => Value::Char(Arbitrary::arbitrary(g)),
        String => {
            let choices = ["", "foo", "\"", "\t", "\x01"];
            Value::string(*choices.choose(g).unwrap())
        }
        Symbol => {
            let choices = [
                "foo", "a-symbol", "$?:!", "+", "+foo", "-", "-foo", "..", ".foo",
            ];
            Value::symbol(*choices.choose(g).unwrap())
        }
        Keyword => {
            let choices = ["foo", "a-keyword", "$?:!"];
            Value::keyword(*choices.choose(g).unwrap())
        }
        Bytes => {
            let choices = [b"".as_ref(), b"\x01\x02\x03", b"Hello World\x00"];
            Value::Bytes(choices.choose(g).map(|&bytes| bytes.into()).unwrap())
        }
        Cons => Value::from((gen_value(g, depth + 1), gen_value(g, depth + 1))),
        Vector => {
            let elements: Vec<Value> = Arbitrary::arbitrary(g);
            Value::from(elements)
        }
    }
}

impl Arbitrary for Value {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        gen_value(g, 0)
    }
    fn shrink(&self) -> Box<dyn Iterator<Item = Value>> {
        let nothing = || Box::new(None.into_iter());
        // TODO
        nothing()
    }
}

enum NumberKind {
    I64,
    U64,
    F64,
}

impl Arbitrary for Number {
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        use NumberKind::*;
        let choices = [I64, U64, F64];
        // We do not use the `Arbitrary` implementations for the
        // numbers, as we want to cover the whole range.
        match choices.choose(g).unwrap() {
            I64 => Number::from(g.gen_range(i64::min_value(), i64::max_value())),
            U64 => Number::from(g.gen_range(0, i64::max_value())),
            F64 => {
                if cfg!(feature = "fast-float-parsing") {
                    Number::from(
                        *[-9876.5e10, -1.0, 0.0, 1.0, 1.4, 123.45e10]
                            .choose(g)
                            .unwrap(),
                    )
                } else {
                    Number::from(g.gen_range(f64::MIN, f64::MAX))
                }
            }
        }
    }
}

#[quickcheck]
fn write_parse_number(input: Number) -> bool {
    let bytes = lexpr::to_vec(&Value::from(input.clone())).expect("conversion to bytes failed");
    let parsed = lexpr::from_slice(&bytes).expect("parsing failed");
    let output = parsed.as_number().expect("parsed as a non-number");
    input == *output
}

#[test]
fn write_number() {
    #[allow(clippy::unreadable_literal)]
    let value = Value::from(Number::from(-11.287888289184039));
    let bytes = lexpr::to_vec(&value).expect("conversion to bytes failed");
    assert_eq!(str::from_utf8(&bytes).unwrap(), "-11.287888289184039");
}

#[test]
fn print_parse_roundtrip_default() {
    fn prop(input: Value) -> bool {
        let string = lexpr::to_string(&input).expect("conversion to string failed");
        let output = lexpr::from_str(&string).expect("parsing failed");
        input == output
    }
    QuickCheck::new()
        .tests(1000)
        .max_tests(2000)
        .gen(StdGen::new(::rand::thread_rng(), 4))
        .quickcheck(prop as fn(Value) -> bool);
}

// The printer uses different code paths depending on whether it was customized,
// so do a roundtrip test using this path as well.
#[test]
fn print_parse_roundtrip_custom_default() {
    fn prop(input: Value) -> bool {
        let string = lexpr::to_string_custom(&input, print::Options::default())
            .expect("conversion to string failed");
        let output =
            lexpr::from_str_custom(&string, parse::Options::default()).expect("parsing failed");
        input == output
    }
    QuickCheck::new()
        .tests(1000)
        .max_tests(2000)
        .gen(StdGen::new(::rand::thread_rng(), 4))
        .quickcheck(prop as fn(Value) -> bool);
}

#[test]
fn print_parse_roundtrip_io_default() {
    use std::io::Cursor;
    fn prop(input: Value) -> bool {
        let mut vec = Vec::new();
        lexpr::to_writer(Cursor::new(&mut vec), &input).expect("conversion to string failed");
        let output = lexpr::from_reader(Cursor::new(&vec)).expect("parsing failed");
        input == output
    }
    QuickCheck::new()
        .tests(1000)
        .max_tests(2000)
        .gen(StdGen::new(::rand::thread_rng(), 4))
        .quickcheck(prop as fn(Value) -> bool);
}

#[test]
fn test_list_index() {
    let list = Value::list(vec![23, 24, 25]);
    assert_eq!(list[0], Value::from(23));
    assert_eq!(list[1], Value::from(24));
    assert_eq!(list[2], Value::from(25));
}

#[test]
fn test_alist_index() {
    let alist = Value::list(vec![("foo", 42), ("bar", 23), ("baz", 127)]);
    assert_eq!(alist["foo"], Value::from(42));
    assert_eq!(alist["bar"], Value::from(23));
    assert_eq!(alist["baz"], Value::from(127));
}

#[test]
fn test_unidiomatic_space() {
    let nested_value = Value::list(vec![
        Value::symbol("feedback"),
        Value::list(vec![Value::symbol("nested")]),
    ]);
    let value = lexpr::from_str("(feedback(nested))").expect("failed to parse");
    assert_eq!(value, nested_value);
    let value = lexpr::from_str("feedback; some comment").expect("failed to parse");
    assert_eq!(value, Value::symbol("feedback"));

    use std::io::Cursor;
    let value = lexpr::from_reader(Cursor::new("(feedback(nested))")).expect("failed to parse");
    assert_eq!(value, nested_value);
    let value = lexpr::from_reader(Cursor::new("feedback; some comment")).expect("failed to parse");
    assert_eq!(value, Value::symbol("feedback"));
}

/// Checks that an arbitrary S-expression value can be parsed including span
/// information, the span information can be obtained and passes basic sanity
/// checking.
#[test]
fn parse_datum_span_sanity() {
    fn prop(input: Value) -> bool {
        let string = lexpr::to_string(&input).expect("conversion to string failed");
        let output = lexpr::datum::from_str(&string).expect("parsing failed");
        output.value() == &input && check_span_sanity(output.as_ref())
    }
    QuickCheck::new()
        .tests(1000)
        .max_tests(2000)
        .gen(StdGen::new(::rand::thread_rng(), 4))
        .quickcheck(prop as fn(Value) -> bool);
}

fn check_span_sanity(datum: datum::Ref<'_>) -> bool {
    let span = datum.span();
    // Each datum spans at least one byte
    if span.start() >= span.end() {
        return false;
    }
    if let Some(mut items) = datum.list_iter() {
        let mut pos = span.start();
        for datum in items.by_ref() {
            // Strictly speaking there are cases where one datum can follow
            // immediatly after another, but the writer should not generate such
            // non-idiomatic output.
            if datum.span().start() <= pos {
                return false;
            }
            check_span_sanity(datum);
            pos = datum.span().end();
        }
        for datum in items.by_ref() {
            if datum.span().start() <= pos {
                return false;
            }
            check_span_sanity(datum);
            pos = datum.span().end();
        }
        if pos >= span.end() {
            return false;
        }
    } else if let Some(mut items) = datum.vector_iter() {
        let mut pos = span.start();
        for datum in items.by_ref() {
            if datum.span().start() <= pos {
                return false;
            }
            check_span_sanity(datum);
            pos = datum.span().end();
        }
        if pos >= span.end() {
            return false;
        }
    }
    true
}
