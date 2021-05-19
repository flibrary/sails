use nom::branch::alt;
use nom::bytes::complete::{escaped, is_not, tag, take_until};
use nom::character::complete::{anychar, char, digit1};
use nom::combinator::{complete, map, opt, recognize, value};
use nom::error::{Error, ParseError};
use nom::multi::{many0, many1, separated_list0, separated_list1};
use nom::sequence::{delimited, pair, tuple};
use nom::{self, error_position, Compare, IResult, InputTake};
use std::str;

use crate::{CompileError, Syntax};

#[derive(Debug, PartialEq)]
pub enum Node<'a> {
    Lit(&'a str, &'a str, &'a str),
    Comment(Ws),
    Expr(Ws, Expr<'a>),
    Call(Ws, Option<&'a str>, &'a str, Vec<Expr<'a>>),
    LetDecl(Ws, Target<'a>),
    Let(Ws, Target<'a>, Expr<'a>),
    Cond(Vec<(Ws, Option<Expr<'a>>, Vec<Node<'a>>)>, Ws),
    Match(Ws, Expr<'a>, Vec<When<'a>>, Ws),
    Loop(Ws, Target<'a>, Expr<'a>, Vec<Node<'a>>, Ws),
    Extends(Expr<'a>),
    BlockDef(Ws, &'a str, Vec<Node<'a>>, Ws),
    Include(Ws, &'a str),
    Import(Ws, &'a str, &'a str),
    Macro(&'a str, Macro<'a>),
    Raw(Ws, &'a str, Ws),
}

#[derive(Debug, PartialEq)]
pub enum Expr<'a> {
    BoolLit(&'a str),
    NumLit(&'a str),
    StrLit(&'a str),
    CharLit(&'a str),
    Var(&'a str),
    VarCall(&'a str, Vec<Expr<'a>>),
    Path(Vec<&'a str>),
    PathCall(Vec<&'a str>, Vec<Expr<'a>>),
    Array(Vec<Expr<'a>>),
    Attr(Box<Expr<'a>>, &'a str),
    Index(Box<Expr<'a>>, Box<Expr<'a>>),
    Filter(&'a str, Vec<Expr<'a>>),
    Unary(&'a str, Box<Expr<'a>>),
    BinOp(&'a str, Box<Expr<'a>>, Box<Expr<'a>>),
    Range(&'a str, Option<Box<Expr<'a>>>, Option<Box<Expr<'a>>>),
    Group(Box<Expr<'a>>),
    MethodCall(Box<Expr<'a>>, &'a str, Vec<Expr<'a>>),
    RustMacro(&'a str, &'a str),
}

impl Expr<'_> {
    /// Returns `true` if enough assumptions can be made,
    /// to determine that `self` is copyable.
    pub fn is_copyable(&self) -> bool {
        self.is_copyable_within_op(false)
    }

    fn is_copyable_within_op(&self, within_op: bool) -> bool {
        use Expr::*;
        match self {
            BoolLit(_) | NumLit(_) | StrLit(_) | CharLit(_) => true,
            Unary(.., expr) => expr.is_copyable_within_op(true),
            BinOp(_, lhs, rhs) => {
                lhs.is_copyable_within_op(true) && rhs.is_copyable_within_op(true)
            }
            Range(..) => true,
            // The result of a call likely doesn't need to be borrowed,
            // as in that case the call is more likely to return a
            // reference in the first place then.
            VarCall(..) | Path(..) | PathCall(..) | MethodCall(..) => true,
            // If the `expr` is within a `Unary` or `BinOp` then
            // an assumption can be made that the operand is copy.
            // If not, then the value is moved and adding `.clone()`
            // will solve that issue. However, if the operand is
            // implicitly borrowed, then it's likely not even possible
            // to get the template to compile.
            _ => within_op && self.is_attr_self(),
        }
    }

    /// Returns `true` if this is an `Attr` where the `obj` is `"self"`.
    pub fn is_attr_self(&self) -> bool {
        match self {
            Expr::Attr(obj, _) if matches!(obj.as_ref(), Expr::Var("self")) => true,
            Expr::Attr(obj, _) if matches!(obj.as_ref(), Expr::Attr(..)) => obj.is_attr_self(),
            _ => false,
        }
    }
}

pub type When<'a> = (
    Ws,
    Option<MatchVariant<'a>>,
    MatchParameters<'a>,
    Vec<Node<'a>>,
);

#[derive(Debug, PartialEq)]
pub enum MatchParameters<'a> {
    Simple(Vec<MatchParameter<'a>>),
    Named(Vec<(&'a str, Option<MatchParameter<'a>>)>),
}

impl<'a> Default for MatchParameters<'a> {
    fn default() -> Self {
        MatchParameters::Simple(vec![])
    }
}

#[derive(Debug, PartialEq)]
pub enum MatchParameter<'a> {
    Name(&'a str),
    NumLit(&'a str),
    StrLit(&'a str),
    CharLit(&'a str),
}

#[derive(Debug, PartialEq)]
pub enum MatchVariant<'a> {
    Path(Vec<&'a str>),
    Name(&'a str),
    NumLit(&'a str),
    StrLit(&'a str),
    CharLit(&'a str),
}

#[derive(Debug, PartialEq)]
pub struct Macro<'a> {
    pub ws1: Ws,
    pub args: Vec<&'a str>,
    pub nodes: Vec<Node<'a>>,
    pub ws2: Ws,
}

#[derive(Debug, PartialEq)]
pub enum Target<'a> {
    Name(&'a str),
    Tuple(Vec<&'a str>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ws(pub bool, pub bool);

pub type Cond<'a> = (Ws, Option<Expr<'a>>, Vec<Node<'a>>);

fn ws<F, I, O, E>(mut inner: F) -> impl FnMut(I) -> IResult<I, O, E>
where
    F: FnMut(I) -> IResult<I, O, E>,
    I: InputTake + Clone + PartialEq + for<'a> Compare<&'a [u8; 1]>,
    E: ParseError<I>,
{
    move |i: I| {
        let mut ws = many0(alt::<_, _, (), _>((
            tag(b" "),
            tag(b"\t"),
            tag(b"\r"),
            tag(b"\n"),
        )));
        let i = ws(i.clone()).map(|(i, _)| i).unwrap_or(i);
        let (i, res) = inner(i)?;
        let i = ws(i.clone()).map(|(i, _)| i).unwrap_or(i);
        Ok((i, res))
    }
}

fn split_ws_parts(s: &[u8]) -> Node {
    if s.is_empty() {
        let rs = str::from_utf8(s).unwrap();
        return Node::Lit(rs, rs, rs);
    }

    let is_ws = |c: &u8| *c != b' ' && *c != b'\t' && *c != b'\r' && *c != b'\n';
    let start = s.iter().position(&is_ws);
    let res = if let Some(start) = start {
        let end = s.iter().rposition(&is_ws);
        if let Some(end) = end {
            (&s[..start], &s[start..=end], &s[end + 1..])
        } else {
            (&s[..start], &s[start..], &[] as &[u8])
        }
    } else {
        (s, &[] as &[u8], &[] as &[u8])
    };

    Node::Lit(
        str::from_utf8(res.0).unwrap(),
        str::from_utf8(res.1).unwrap(),
        str::from_utf8(res.2).unwrap(),
    )
}

#[derive(Debug)]
enum ContentState {
    Start,
    Any,
    Brace(usize),
    End(usize),
}

fn take_content<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> ParserError<'a, Node<'a>> {
    use crate::parser::ContentState::*;
    let bs = s.block_start.as_bytes()[0];
    let be = s.block_start.as_bytes()[1];
    let cs = s.comment_start.as_bytes()[0];
    let ce = s.comment_start.as_bytes()[1];
    let es = s.expr_start.as_bytes()[0];
    let ee = s.expr_start.as_bytes()[1];

    let mut state = Start;
    for (idx, c) in i.iter().enumerate() {
        state = match state {
            Start | Any => {
                if *c == bs || *c == es || *c == cs {
                    Brace(idx)
                } else {
                    Any
                }
            }
            Brace(start) => {
                if *c == be || *c == ee || *c == ce {
                    End(start)
                } else {
                    Any
                }
            }
            End(_) => unreachable!(),
        };
        if let End(_) = state {
            break;
        }
    }

    match state {
        Any | Brace(_) => Ok((&i[..0], split_ws_parts(i))),
        Start | End(0) => Err(nom::Err::Error(error_position!(
            i,
            nom::error::ErrorKind::TakeUntil
        ))),
        End(start) => Ok((&i[start..], split_ws_parts(&i[..start]))),
    }
}

fn identifier(input: &[u8]) -> ParserError<&str> {
    if !nom::character::is_alphabetic(input[0]) && input[0] != b'_' && !non_ascii(input[0]) {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::AlphaNumeric,
        )));
    }
    for (i, ch) in input.iter().enumerate() {
        if i == 0 || nom::character::is_alphanumeric(*ch) || *ch == b'_' || non_ascii(*ch) {
            continue;
        }
        return Ok((&input[i..], str::from_utf8(&input[..i]).unwrap()));
    }
    Ok((&input[1..], str::from_utf8(&input[..1]).unwrap()))
}

#[inline]
fn non_ascii(chr: u8) -> bool {
    (0x80..=0xFD).contains(&chr)
}

fn expr_bool_lit(i: &[u8]) -> IResult<&[u8], Expr> {
    map(alt((tag("false"), tag("true"))), |s| {
        Expr::BoolLit(str::from_utf8(s).unwrap())
    })(i)
}

fn num_lit(i: &[u8]) -> IResult<&[u8], &str> {
    map(recognize(pair(digit1, opt(pair(tag("."), digit1)))), |s| {
        str::from_utf8(s).unwrap()
    })(i)
}

fn expr_num_lit(i: &[u8]) -> IResult<&[u8], Expr> {
    map(num_lit, |s| Expr::NumLit(s))(i)
}

fn expr_array_lit(i: &[u8]) -> IResult<&[u8], Expr> {
    delimited(
        ws(tag("[")),
        map(separated_list1(ws(tag(",")), expr_any), |arr| {
            Expr::Array(arr)
        }),
        ws(tag("]")),
    )(i)
}

fn variant_num_lit(i: &[u8]) -> IResult<&[u8], MatchVariant> {
    map(num_lit, |s| MatchVariant::NumLit(s))(i)
}

fn param_num_lit(i: &[u8]) -> IResult<&[u8], MatchParameter> {
    map(num_lit, |s| MatchParameter::NumLit(s))(i)
}

fn str_lit(i: &[u8]) -> IResult<&[u8], &str> {
    map(
        delimited(
            char('\"'),
            opt(escaped(is_not("\\\""), '\\', anychar)),
            char('\"'),
        ),
        |s| s.map(|s| str::from_utf8(s).unwrap()).unwrap_or(""),
    )(i)
}

fn expr_str_lit(i: &[u8]) -> IResult<&[u8], Expr> {
    map(str_lit, |s| Expr::StrLit(s))(i)
}

fn variant_str_lit(i: &[u8]) -> IResult<&[u8], MatchVariant> {
    map(str_lit, |s| MatchVariant::StrLit(s))(i)
}

fn param_str_lit(i: &[u8]) -> IResult<&[u8], MatchParameter> {
    map(str_lit, |s| MatchParameter::StrLit(s))(i)
}

fn char_lit(i: &[u8]) -> IResult<&[u8], &str> {
    map(
        delimited(
            char('\''),
            opt(escaped(is_not("\\\'"), '\\', anychar)),
            char('\''),
        ),
        |s| s.map(|s| str::from_utf8(s).unwrap()).unwrap_or(""),
    )(i)
}

fn expr_char_lit(i: &[u8]) -> IResult<&[u8], Expr> {
    map(char_lit, |s| Expr::CharLit(s))(i)
}

fn variant_char_lit(i: &[u8]) -> IResult<&[u8], MatchVariant> {
    map(char_lit, |s| MatchVariant::CharLit(s))(i)
}

fn param_char_lit(i: &[u8]) -> IResult<&[u8], MatchParameter> {
    map(char_lit, |s| MatchParameter::CharLit(s))(i)
}

fn expr_var(i: &[u8]) -> IResult<&[u8], Expr> {
    map(identifier, |s| Expr::Var(s))(i)
}

fn expr_var_call(i: &[u8]) -> IResult<&[u8], Expr> {
    let (i, (s, args)) = tuple((ws(identifier), arguments))(i)?;
    Ok((i, Expr::VarCall(s, args)))
}

fn path(i: &[u8]) -> IResult<&[u8], Vec<&str>> {
    let root = opt(value("", ws(tag("::"))));
    let tail = separated_list1(ws(tag("::")), identifier);

    match tuple((root, identifier, ws(tag("::")), tail))(i) {
        Ok((i, (root, start, _, rest))) => {
            let mut path = Vec::new();
            path.extend(root);
            path.push(start);
            path.extend(rest);
            Ok((i, path))
        }
        Err(err) => {
            if let Ok((i, name)) = identifier(i) {
                // The returned identifier can be assumed to be path if:
                // - Contains both a lowercase and uppercase character, i.e. a type name like `None`
                // - Doesn't contain any lowercase characters, i.e. it's a constant
                // In short, if it contains any uppercase characters it's a path.
                if name.contains(char::is_uppercase) {
                    return Ok((i, vec![name]));
                }
            }

            // If `identifier()` fails then just return the original error
            Err(err)
        }
    }
}

fn expr_path(i: &[u8]) -> IResult<&[u8], Expr> {
    let (i, path) = path(i)?;
    Ok((i, Expr::Path(path)))
}

fn expr_path_call(i: &[u8]) -> IResult<&[u8], Expr> {
    let (i, (path, args)) = tuple((ws(path), arguments))(i)?;
    Ok((i, Expr::PathCall(path, args)))
}

fn variant_path(i: &[u8]) -> IResult<&[u8], MatchVariant> {
    map(separated_list1(ws(tag("::")), identifier), |path| {
        MatchVariant::Path(path)
    })(i)
}

fn target_single(i: &[u8]) -> IResult<&[u8], Target> {
    map(identifier, |s| Target::Name(s))(i)
}

fn target_tuple(i: &[u8]) -> IResult<&[u8], Target> {
    let parts = separated_list0(tag(","), ws(identifier));
    let trailing = opt(ws(tag(",")));
    let mut full = delimited(tag("("), tuple((parts, trailing)), tag(")"));

    let (i, (elems, _)) = full(i)?;
    Ok((i, Target::Tuple(elems)))
}

fn variant_name(i: &[u8]) -> IResult<&[u8], MatchVariant> {
    map(identifier, |s| MatchVariant::Name(s))(i)
}

fn param_name(i: &[u8]) -> IResult<&[u8], MatchParameter> {
    map(identifier, |s| MatchParameter::Name(s))(i)
}

fn arguments(i: &[u8]) -> IResult<&[u8], Vec<Expr>> {
    delimited(
        ws(tag("(")),
        separated_list0(tag(","), ws(expr_any)),
        ws(tag(")")),
    )(i)
}

fn macro_arguments(i: &[u8]) -> IResult<&[u8], &str> {
    delimited(char('('), nested_parenthesis, char(')'))(i)
}

fn nested_parenthesis(i: &[u8]) -> ParserError<&str> {
    let mut nested = 0;
    let mut last = 0;
    let mut in_str = false;
    let mut escaped = false;

    for (i, b) in i.iter().enumerate() {
        if !(*b == b'(' || *b == b')') || !in_str {
            match *b {
                b'(' => nested += 1,
                b')' => {
                    if nested == 0 {
                        last = i;
                        break;
                    }
                    nested -= 1;
                }
                b'"' => {
                    if in_str {
                        if !escaped {
                            in_str = false;
                        }
                    } else {
                        in_str = true;
                    }
                }
                b'\\' => {
                    escaped = !escaped;
                }
                _ => (),
            }
        }

        if escaped && *b != b'\\' {
            escaped = false;
        }
    }

    if nested == 0 {
        Ok((&i[last..], str::from_utf8(&i[..last]).unwrap()))
    } else {
        Err(nom::Err::Error(error_position!(
            i,
            nom::error::ErrorKind::SeparatedNonEmptyList
        )))
    }
}

fn parameters(i: &[u8]) -> IResult<&[u8], Vec<&str>> {
    delimited(
        ws(tag("(")),
        separated_list0(tag(","), ws(identifier)),
        ws(tag(")")),
    )(i)
}

fn with_parameters(i: &[u8]) -> IResult<&[u8], MatchParameters> {
    let (i, (_, value)) = tuple((
        tag("with"),
        alt((match_simple_parameters, match_named_parameters)),
    ))(i)?;
    Ok((i, value))
}

fn match_simple_parameters(i: &[u8]) -> IResult<&[u8], MatchParameters> {
    delimited(
        ws(tag("(")),
        map(separated_list0(tag(","), ws(match_parameter)), |mps| {
            MatchParameters::Simple(mps)
        }),
        tag(")"),
    )(i)
}

fn match_named_parameters(i: &[u8]) -> IResult<&[u8], MatchParameters> {
    delimited(
        ws(tag("{")),
        map(
            separated_list0(tag(","), ws(match_named_parameter)),
            MatchParameters::Named,
        ),
        tag("}"),
    )(i)
}

fn expr_group(i: &[u8]) -> IResult<&[u8], Expr> {
    map(delimited(ws(char('(')), expr_any, ws(char(')'))), |s| {
        Expr::Group(Box::new(s))
    })(i)
}

fn expr_single(i: &[u8]) -> IResult<&[u8], Expr> {
    alt((
        expr_bool_lit,
        expr_num_lit,
        expr_str_lit,
        expr_char_lit,
        expr_path_call,
        expr_path,
        expr_rust_macro,
        expr_array_lit,
        expr_var_call,
        expr_var,
        expr_group,
    ))(i)
}

fn match_variant(i: &[u8]) -> IResult<&[u8], MatchVariant> {
    alt((
        variant_path,
        variant_name,
        variant_num_lit,
        variant_str_lit,
        variant_char_lit,
    ))(i)
}

fn match_parameter(i: &[u8]) -> IResult<&[u8], MatchParameter> {
    alt((param_name, param_num_lit, param_str_lit, param_char_lit))(i)
}

fn match_named_parameter(i: &[u8]) -> IResult<&[u8], (&str, Option<MatchParameter>)> {
    let param = tuple((ws(tag(":")), match_parameter));
    let (i, (name, param)) = tuple((identifier, opt(param)))(i)?;
    Ok((i, (name, param.map(|s| s.1))))
}

fn attr(i: &[u8]) -> IResult<&[u8], (&str, Option<Vec<Expr>>)> {
    let (i, (_, attr, args)) =
        tuple((ws(tag(".")), alt((num_lit, identifier)), ws(opt(arguments))))(i)?;
    Ok((i, (attr, args)))
}

fn expr_attr(i: &[u8]) -> IResult<&[u8], Expr> {
    let (i, (obj, attrs)) = tuple((expr_single, many0(attr)))(i)?;

    let mut res = obj;
    for (aname, args) in attrs {
        res = if let Some(args) = args {
            Expr::MethodCall(Box::new(res), aname, args)
        } else {
            Expr::Attr(Box::new(res), aname)
        };
    }

    Ok((i, res))
}

fn expr_index(i: &[u8]) -> IResult<&[u8], Expr> {
    let key = opt(tuple((ws(tag("[")), expr_any, ws(tag("]")))));
    let (i, (obj, key)) = tuple((expr_attr, key))(i)?;
    let key = key.map(|(_, key, _)| key);

    Ok((
        i,
        match key {
            Some(key) => Expr::Index(Box::new(obj), Box::new(key)),
            None => obj,
        },
    ))
}

fn filter(i: &[u8]) -> IResult<&[u8], (&str, Option<Vec<Expr>>)> {
    let (i, (_, fname, args)) = tuple((tag("|"), ws(identifier), opt(arguments)))(i)?;
    Ok((i, (fname, args)))
}

fn expr_filtered(i: &[u8]) -> IResult<&[u8], Expr> {
    let (i, (obj, filters)) = tuple((expr_unary, many0(filter)))(i)?;

    let mut res = obj;
    for (fname, args) in filters {
        res = Expr::Filter(fname, {
            let mut args = match args {
                Some(inner) => inner,
                None => Vec::new(),
            };
            args.insert(0, res);
            args
        });
    }

    Ok((i, res))
}

fn expr_unary(i: &[u8]) -> IResult<&[u8], Expr> {
    let (i, (op, expr)) = tuple((opt(alt((ws(tag("!")), ws(tag("-"))))), expr_index))(i)?;
    Ok((
        i,
        match op {
            Some(op) => Expr::Unary(str::from_utf8(op).unwrap(), Box::new(expr)),
            None => expr,
        },
    ))
}

fn expr_rust_macro(i: &[u8]) -> IResult<&[u8], Expr> {
    let (i, (mname, _, args)) = tuple((identifier, tag("!"), macro_arguments))(i)?;
    Ok((i, Expr::RustMacro(mname, args)))
}

macro_rules! expr_prec_layer {
    ( $name:ident, $inner:ident, $op:expr ) => {
        fn $name(i: &[u8]) -> IResult<&[u8], Expr> {
            let (i, left) = $inner(i)?;
            let (i, right) = many0(pair(
                ws(tag($op)),
                $inner,
            ))(i)?;
            Ok((
                i,
                right.into_iter().fold(left, |left, (op, right)| {
                    Expr::BinOp(str::from_utf8(op).unwrap(), Box::new(left), Box::new(right))
                }),
            ))
        }
    };
    ( $name:ident, $inner:ident, $( $op:expr ),+ ) => {
        fn $name(i: &[u8]) -> IResult<&[u8], Expr> {
            let (i, left) = $inner(i)?;
            let (i, right) = many0(pair(
                ws(alt(($( tag($op) ),*,))),
                $inner,
            ))(i)?;
            Ok((
                i,
                right.into_iter().fold(left, |left, (op, right)| {
                    Expr::BinOp(str::from_utf8(op).unwrap(), Box::new(left), Box::new(right))
                }),
            ))
        }
    }
}

expr_prec_layer!(expr_muldivmod, expr_filtered, "*", "/", "%");
expr_prec_layer!(expr_addsub, expr_muldivmod, "+", "-");
expr_prec_layer!(expr_shifts, expr_addsub, ">>", "<<");
expr_prec_layer!(expr_band, expr_shifts, "&");
expr_prec_layer!(expr_bxor, expr_band, "^");
expr_prec_layer!(expr_bor, expr_bxor, "|");
expr_prec_layer!(expr_compare, expr_bor, "==", "!=", ">=", ">", "<=", "<");
expr_prec_layer!(expr_and, expr_compare, "&&");
expr_prec_layer!(expr_or, expr_and, "||");

fn range_right(i: &[u8]) -> IResult<&[u8], Expr> {
    let (i, (_, incl, right)) = tuple((ws(tag("..")), opt(ws(tag("="))), opt(expr_or)))(i)?;
    Ok((
        i,
        Expr::Range(
            if incl.is_some() { "..=" } else { ".." },
            None,
            right.map(Box::new),
        ),
    ))
}

fn expr_any(i: &[u8]) -> IResult<&[u8], Expr> {
    let compound = map(tuple((expr_or, range_right)), |(left, rest)| match rest {
        Expr::Range(op, _, right) => Expr::Range(op, Some(Box::new(left)), right),
        _ => unreachable!(),
    });
    alt((range_right, compound, expr_or))(i)
}

fn expr_node<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], Node<'a>> {
    let mut p = tuple((
        |i| tag_expr_start(i, s),
        opt(tag("-")),
        ws(expr_any),
        opt(tag("-")),
        |i| tag_expr_end(i, s),
    ));
    let (i, (_, pws, expr, nws, _)) = p(i)?;
    Ok((i, Node::Expr(Ws(pws.is_some(), nws.is_some()), expr)))
}

fn block_call(i: &[u8]) -> IResult<&[u8], Node> {
    let mut p = tuple((
        opt(tag("-")),
        ws(tag("call")),
        opt(tuple((ws(identifier), ws(tag("::"))))),
        ws(identifier),
        ws(arguments),
        opt(tag("-")),
    ));
    let (i, (pws, _, scope, name, args, nws)) = p(i)?;
    let scope = scope.map(|(scope, _)| scope);
    Ok((
        i,
        Node::Call(Ws(pws.is_some(), nws.is_some()), scope, name, args),
    ))
}

fn cond_if(i: &[u8]) -> IResult<&[u8], Expr> {
    let (i, (_, cond)) = tuple((ws(tag("if")), ws(expr_any)))(i)?;
    Ok((i, cond))
}

fn cond_block<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], Cond<'a>> {
    let mut p = tuple((
        |i| tag_block_start(i, s),
        opt(tag("-")),
        ws(tag("else")),
        opt(cond_if),
        opt(tag("-")),
        |i| tag_block_end(i, s),
        |i| parse_template(i, s),
    ));
    let (i, (_, pws, _, cond, nws, _, block)) = p(i)?;
    Ok((i, (Ws(pws.is_some(), nws.is_some()), cond, block)))
}

fn block_if<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], Node<'a>> {
    let mut p = tuple((
        opt(tag("-")),
        cond_if,
        opt(tag("-")),
        |i| tag_block_end(i, s),
        |i| parse_template(i, s),
        many0(|i| cond_block(i, s)),
        |i| tag_block_start(i, s),
        opt(tag("-")),
        ws(tag("endif")),
        opt(tag("-")),
    ));
    let (i, (pws1, cond, nws1, _, block, elifs, _, pws2, _, nws2)) = p(i)?;

    let mut res = vec![(Ws(pws1.is_some(), nws1.is_some()), Some(cond), block)];
    res.extend(elifs);
    Ok((i, Node::Cond(res, Ws(pws2.is_some(), nws2.is_some()))))
}

fn match_else_block<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], When<'a>> {
    let mut p = tuple((
        |i| tag_block_start(i, s),
        opt(tag("-")),
        ws(tag("else")),
        opt(tag("-")),
        |i| tag_block_end(i, s),
        |i| parse_template(i, s),
    ));
    let (i, (_, pws, _, nws, _, block)) = p(i)?;
    Ok((
        i,
        (
            Ws(pws.is_some(), nws.is_some()),
            None,
            MatchParameters::Simple(vec![]),
            block,
        ),
    ))
}

fn when_block<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], When<'a>> {
    let mut p = tuple((
        |i| tag_block_start(i, s),
        opt(tag("-")),
        ws(tag("when")),
        ws(match_variant),
        opt(ws(with_parameters)),
        opt(tag("-")),
        |i| tag_block_end(i, s),
        |i| parse_template(i, s),
    ));
    let (i, (_, pws, _, variant, params, nws, _, block)) = p(i)?;
    Ok((
        i,
        (
            Ws(pws.is_some(), nws.is_some()),
            Some(variant),
            params.unwrap_or_default(),
            block,
        ),
    ))
}

fn block_match<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], Node<'a>> {
    let mut p = tuple((
        opt(tag("-")),
        ws(tag("match")),
        ws(expr_any),
        opt(tag("-")),
        |i| tag_block_end(i, s),
        opt(|i| take_content(i, s)),
        many1(|i| when_block(i, s)),
        opt(|i| match_else_block(i, s)),
        ws(|i| tag_block_start(i, s)),
        opt(tag("-")),
        ws(tag("endmatch")),
        opt(tag("-")),
    ));
    let (i, (pws1, _, expr, nws1, _, inter, arms, else_arm, _, pws2, _, nws2)) = p(i)?;

    let mut arms = arms;
    if let Some(arm) = else_arm {
        arms.push(arm);
    }

    match inter {
        Some(Node::Lit(_, val, rws)) => {
            assert!(
                val.is_empty(),
                "only whitespace allowed between match and first when, found {}",
                val
            );
            assert!(
                rws.is_empty(),
                "only whitespace allowed between match and first when, found {}",
                rws
            );
        }
        None => {}
        _ => panic!("only literals allowed between match and first when"),
    }

    Ok((
        i,
        Node::Match(
            Ws(pws1.is_some(), nws1.is_some()),
            expr,
            arms,
            Ws(pws2.is_some(), nws2.is_some()),
        ),
    ))
}

fn block_let(i: &[u8]) -> IResult<&[u8], Node> {
    let mut p = tuple((
        opt(tag("-")),
        ws(alt((tag("let"), tag("set")))),
        ws(alt((target_single, target_tuple))),
        opt(tuple((ws(tag("=")), ws(expr_any)))),
        opt(tag("-")),
    ));
    let (i, (pws, _, var, val, nws)) = p(i)?;

    Ok((
        i,
        if let Some((_, val)) = val {
            Node::Let(Ws(pws.is_some(), nws.is_some()), var, val)
        } else {
            Node::LetDecl(Ws(pws.is_some(), nws.is_some()), var)
        },
    ))
}

fn block_for<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], Node<'a>> {
    let mut p = tuple((
        opt(tag("-")),
        ws(tag("for")),
        ws(alt((target_single, target_tuple))),
        ws(tag("in")),
        ws(expr_any),
        opt(tag("-")),
        |i| tag_block_end(i, s),
        |i| parse_template(i, s),
        |i| tag_block_start(i, s),
        opt(tag("-")),
        ws(tag("endfor")),
        opt(tag("-")),
    ));
    let (i, (pws1, _, var, _, iter, nws1, _, block, _, pws2, _, nws2)) = p(i)?;
    Ok((
        i,
        Node::Loop(
            Ws(pws1.is_some(), nws1.is_some()),
            var,
            iter,
            block,
            Ws(pws2.is_some(), nws2.is_some()),
        ),
    ))
}

fn block_extends(i: &[u8]) -> IResult<&[u8], Node> {
    let (i, (_, name)) = tuple((ws(tag("extends")), ws(expr_str_lit)))(i)?;
    Ok((i, Node::Extends(name)))
}

fn block_block<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], Node<'a>> {
    let mut start = tuple((
        opt(tag("-")),
        ws(tag("block")),
        ws(identifier),
        opt(tag("-")),
        |i| tag_block_end(i, s),
        |i| parse_template(i, s),
    ));
    let (i, (pws1, _, name, nws1, _, contents)) = start(i)?;

    let mut end = tuple((
        |i| tag_block_start(i, s),
        opt(tag("-")),
        ws(tag("endblock")),
        opt(ws(tag(name))),
        opt(tag("-")),
    ));
    let (i, (_, pws2, _, _, nws2)) = end(i)?;

    Ok((
        i,
        Node::BlockDef(
            Ws(pws1.is_some(), nws1.is_some()),
            name,
            contents,
            Ws(pws2.is_some(), nws2.is_some()),
        ),
    ))
}

fn block_include(i: &[u8]) -> IResult<&[u8], Node> {
    let mut p = tuple((
        opt(tag("-")),
        ws(tag("include")),
        ws(expr_str_lit),
        opt(tag("-")),
    ));
    let (i, (pws, _, name, nws)) = p(i)?;
    Ok((
        i,
        Node::Include(
            Ws(pws.is_some(), nws.is_some()),
            match name {
                Expr::StrLit(s) => s,
                _ => panic!("include path must be a string literal"),
            },
        ),
    ))
}

fn block_import(i: &[u8]) -> IResult<&[u8], Node> {
    let mut p = tuple((
        opt(tag("-")),
        ws(tag("import")),
        ws(expr_str_lit),
        ws(tag("as")),
        ws(identifier),
        opt(tag("-")),
    ));
    let (i, (pws, _, name, _, scope, nws)) = p(i)?;
    Ok((
        i,
        Node::Import(
            Ws(pws.is_some(), nws.is_some()),
            match name {
                Expr::StrLit(s) => s,
                _ => panic!("import path must be a string literal"),
            },
            scope,
        ),
    ))
}

fn block_macro<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], Node<'a>> {
    let mut p = tuple((
        opt(tag("-")),
        ws(tag("macro")),
        ws(identifier),
        ws(parameters),
        opt(tag("-")),
        |i| tag_block_end(i, s),
        |i| parse_template(i, s),
        |i| tag_block_start(i, s),
        opt(tag("-")),
        ws(tag("endmacro")),
        opt(tag("-")),
    ));

    let (i, (pws1, _, name, params, nws1, _, contents, _, pws2, _, nws2)) = p(i)?;
    if name == "super" {
        panic!("invalid macro name 'super'");
    }

    Ok((
        i,
        Node::Macro(
            name,
            Macro {
                ws1: Ws(pws1.is_some(), nws1.is_some()),
                args: params,
                nodes: contents,
                ws2: Ws(pws2.is_some(), nws2.is_some()),
            },
        ),
    ))
}

fn block_raw<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], Node<'a>> {
    let mut p = tuple((
        opt(tag("-")),
        ws(tag("raw")),
        opt(tag("-")),
        |i| tag_block_end(i, s),
        take_until("{% endraw %}"),
        |i| tag_block_start(i, s),
        opt(tag("-")),
        ws(tag("endraw")),
        opt(tag("-")),
    ));

    let (i, (pws1, _, nws1, _, contents, _, pws2, _, nws2)) = p(i)?;
    let str_contents = str::from_utf8(contents).unwrap();
    Ok((
        i,
        Node::Raw(
            Ws(pws1.is_some(), nws1.is_some()),
            str_contents,
            Ws(pws2.is_some(), nws2.is_some()),
        ),
    ))
}

fn block_node<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], Node<'a>> {
    let mut p = tuple((
        |i| tag_block_start(i, s),
        alt((
            block_call,
            block_let,
            |i| block_if(i, s),
            |i| block_for(i, s),
            |i| block_match(i, s),
            block_extends,
            block_include,
            block_import,
            |i| block_block(i, s),
            |i| block_macro(i, s),
            |i| block_raw(i, s),
        )),
        |i| tag_block_end(i, s),
    ));
    let (i, (_, contents, _)) = p(i)?;
    Ok((i, contents))
}

fn block_comment_body<'a>(mut i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], &'a [u8]> {
    let mut level = 0;
    loop {
        let (end, tail) = take_until(s.comment_end)(i)?;
        match take_until::<_, _, Error<_>>(s.comment_start)(i) {
            Ok((start, _)) if start.as_ptr() < end.as_ptr() => {
                level += 1;
                i = &start[2..];
            }
            _ if level > 0 => {
                level -= 1;
                i = &end[2..];
            }
            _ => return Ok((end, tail)),
        }
    }
}

fn block_comment<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], Node<'a>> {
    let mut p = tuple((
        |i| tag_comment_start(i, s),
        opt(tag("-")),
        |i| block_comment_body(i, s),
        |i| tag_comment_end(i, s),
    ));
    let (i, (_, pws, tail, _)) = p(i)?;
    Ok((i, Node::Comment(Ws(pws.is_some(), tail.ends_with(b"-")))))
}

fn parse_template<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], Vec<Node<'a>>> {
    many0(alt((
        complete(|i| take_content(i, s)),
        complete(|i| block_comment(i, s)),
        complete(|i| expr_node(i, s)),
        complete(|i| block_node(i, s)),
    )))(i)
}

fn tag_block_start<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], &'a [u8]> {
    tag(s.block_start)(i)
}
fn tag_block_end<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], &'a [u8]> {
    tag(s.block_end)(i)
}
fn tag_comment_start<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], &'a [u8]> {
    tag(s.comment_start)(i)
}
fn tag_comment_end<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], &'a [u8]> {
    tag(s.comment_end)(i)
}
fn tag_expr_start<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], &'a [u8]> {
    tag(s.expr_start)(i)
}
fn tag_expr_end<'a>(i: &'a [u8], s: &'a Syntax<'a>) -> IResult<&'a [u8], &'a [u8]> {
    tag(s.expr_end)(i)
}

pub fn parse<'a>(src: &'a str, syntax: &'a Syntax<'a>) -> Result<Vec<Node<'a>>, CompileError> {
    match parse_template(src.as_bytes(), syntax) {
        Ok((left, res)) => {
            if !left.is_empty() {
                let s = str::from_utf8(left).unwrap();
                Err(format!("unable to parse template:\n\n{:?}", s).into())
            } else {
                Ok(res)
            }
        }
        Err(nom::Err::Error(err)) => {
            Err(format!("problems parsing template source: {:?}", err).into())
        }
        Err(nom::Err::Failure(err)) => {
            Err(format!("problems parsing template source: {:?}", err).into())
        }
        Err(nom::Err::Incomplete(_)) => Err("parsing incomplete".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{Expr, Node, Ws};
    use crate::Syntax;

    fn check_ws_split(s: &str, res: &(&str, &str, &str)) {
        let node = super::split_ws_parts(s.as_bytes());
        match node {
            Node::Lit(lws, s, rws) => {
                assert_eq!(lws, res.0);
                assert_eq!(s, res.1);
                assert_eq!(rws, res.2);
            }
            _ => {
                panic!("fail");
            }
        }
    }

    #[test]
    fn test_ws_splitter() {
        check_ws_split("", &("", "", ""));
        check_ws_split("a", &("", "a", ""));
        check_ws_split("\ta", &("\t", "a", ""));
        check_ws_split("b\n", &("", "b", "\n"));
        check_ws_split(" \t\r\n", &(" \t\r\n", "", ""));
    }

    #[test]
    #[should_panic]
    fn test_invalid_block() {
        super::parse("{% extend \"blah\" %}", &Syntax::default()).unwrap();
    }

    #[test]
    fn test_parse_filter() {
        use Expr::*;
        let syntax = Syntax::default();
        assert_eq!(
            super::parse("{{ strvar|e }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                Filter("e", vec![Var("strvar")]),
            )],
        );
        assert_eq!(
            super::parse("{{ 2|abs }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                Filter("abs", vec![NumLit("2")]),
            )],
        );
        assert_eq!(
            super::parse("{{ -2|abs }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                Filter("abs", vec![Unary("-", NumLit("2").into())]),
            )],
        );
        assert_eq!(
            super::parse("{{ (1 - 2)|abs }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                Filter(
                    "abs",
                    vec![Group(
                        BinOp("-", NumLit("1").into(), NumLit("2").into()).into()
                    )]
                ),
            )],
        );
    }

    #[test]
    fn test_parse_numbers() {
        let syntax = Syntax::default();
        assert_eq!(
            super::parse("{{ 2 }}", &syntax).unwrap(),
            vec![Node::Expr(Ws(false, false), Expr::NumLit("2"),)],
        );
        assert_eq!(
            super::parse("{{ 2.5 }}", &syntax).unwrap(),
            vec![Node::Expr(Ws(false, false), Expr::NumLit("2.5"),)],
        );
    }

    #[test]
    fn test_parse_var() {
        let s = Syntax::default();

        assert_eq!(
            super::parse("{{ foo }}", &s).unwrap(),
            vec![Node::Expr(Ws(false, false), Expr::Var("foo"))],
        );
        assert_eq!(
            super::parse("{{ foo_bar }}", &s).unwrap(),
            vec![Node::Expr(Ws(false, false), Expr::Var("foo_bar"))],
        );

        assert_eq!(
            super::parse("{{ none }}", &s).unwrap(),
            vec![Node::Expr(Ws(false, false), Expr::Var("none"))],
        );
    }

    #[test]
    fn test_parse_const() {
        let s = Syntax::default();

        assert_eq!(
            super::parse("{{ FOO }}", &s).unwrap(),
            vec![Node::Expr(Ws(false, false), Expr::Path(vec!["FOO"]))],
        );
        assert_eq!(
            super::parse("{{ FOO_BAR }}", &s).unwrap(),
            vec![Node::Expr(Ws(false, false), Expr::Path(vec!["FOO_BAR"]))],
        );

        assert_eq!(
            super::parse("{{ NONE }}", &s).unwrap(),
            vec![Node::Expr(Ws(false, false), Expr::Path(vec!["NONE"]))],
        );
    }

    #[test]
    fn test_parse_path() {
        let s = Syntax::default();

        assert_eq!(
            super::parse("{{ None }}", &s).unwrap(),
            vec![Node::Expr(Ws(false, false), Expr::Path(vec!["None"]))],
        );
        assert_eq!(
            super::parse("{{ Some(123) }}", &s).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                Expr::PathCall(vec!["Some"], vec![Expr::NumLit("123")],),
            )],
        );

        assert_eq!(
            super::parse("{{ Ok(123) }}", &s).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                Expr::PathCall(vec!["Ok"], vec![Expr::NumLit("123")],),
            )],
        );
        assert_eq!(
            super::parse("{{ Err(123) }}", &s).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                Expr::PathCall(vec!["Err"], vec![Expr::NumLit("123")],),
            )],
        );
    }

    #[test]
    fn test_parse_var_call() {
        assert_eq!(
            super::parse("{{ function(\"123\", 3) }}", &Syntax::default()).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                Expr::VarCall("function", vec![Expr::StrLit("123"), Expr::NumLit("3")]),
            )],
        );
    }

    #[test]
    fn test_parse_path_call() {
        let s = Syntax::default();

        assert_eq!(
            super::parse("{{ Option::None }}", &s).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                Expr::Path(vec!["Option", "None"])
            )],
        );
        assert_eq!(
            super::parse("{{ Option::Some(123) }}", &s).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                Expr::PathCall(vec!["Option", "Some"], vec![Expr::NumLit("123")],),
            )],
        );

        assert_eq!(
            super::parse("{{ self::function(\"123\", 3) }}", &s).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                Expr::PathCall(
                    vec!["self", "function"],
                    vec![Expr::StrLit("123"), Expr::NumLit("3")],
                ),
            )],
        );
    }

    #[test]
    fn test_parse_root_path() {
        let syntax = Syntax::default();
        assert_eq!(
            super::parse("{{ std::string::String::new() }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                Expr::PathCall(vec!["std", "string", "String", "new"], vec![]),
            )],
        );
        assert_eq!(
            super::parse("{{ ::std::string::String::new() }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                Expr::PathCall(vec!["", "std", "string", "String", "new"], vec![]),
            )],
        );
    }

    #[test]
    fn change_delimiters_parse_filter() {
        let syntax = Syntax {
            expr_start: "{~",
            expr_end: "~}",
            ..Syntax::default()
        };

        super::parse("{~ strvar|e ~}", &syntax).unwrap();
    }

    #[test]
    fn test_precedence() {
        use Expr::*;
        let syntax = Syntax::default();
        assert_eq!(
            super::parse("{{ a + b == c }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                BinOp(
                    "==",
                    BinOp("+", Var("a").into(), Var("b").into()).into(),
                    Var("c").into(),
                )
            )],
        );
        assert_eq!(
            super::parse("{{ a + b * c - d / e }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                BinOp(
                    "-",
                    BinOp(
                        "+",
                        Var("a").into(),
                        BinOp("*", Var("b").into(), Var("c").into()).into(),
                    )
                    .into(),
                    BinOp("/", Var("d").into(), Var("e").into()).into(),
                )
            )],
        );
        assert_eq!(
            super::parse("{{ a * (b + c) / -d }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                BinOp(
                    "/",
                    BinOp(
                        "*",
                        Var("a").into(),
                        Group(BinOp("+", Var("b").into(), Var("c").into()).into()).into()
                    )
                    .into(),
                    Unary("-", Var("d").into()).into()
                )
            )],
        );
        assert_eq!(
            super::parse("{{ a || b && c || d && e }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                BinOp(
                    "||",
                    BinOp(
                        "||",
                        Var("a").into(),
                        BinOp("&&", Var("b").into(), Var("c").into()).into(),
                    )
                    .into(),
                    BinOp("&&", Var("d").into(), Var("e").into()).into(),
                )
            )],
        );
    }

    #[test]
    fn test_associativity() {
        use Expr::*;
        let syntax = Syntax::default();
        assert_eq!(
            super::parse("{{ a + b + c }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                BinOp(
                    "+",
                    BinOp("+", Var("a").into(), Var("b").into()).into(),
                    Var("c").into()
                )
            )],
        );
        assert_eq!(
            super::parse("{{ a * b * c }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                BinOp(
                    "*",
                    BinOp("*", Var("a").into(), Var("b").into()).into(),
                    Var("c").into()
                )
            )],
        );
        assert_eq!(
            super::parse("{{ a && b && c }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                BinOp(
                    "&&",
                    BinOp("&&", Var("a").into(), Var("b").into()).into(),
                    Var("c").into()
                )
            )],
        );
        assert_eq!(
            super::parse("{{ a + b - c + d }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                BinOp(
                    "+",
                    BinOp(
                        "-",
                        BinOp("+", Var("a").into(), Var("b").into()).into(),
                        Var("c").into()
                    )
                    .into(),
                    Var("d").into()
                )
            )],
        );
        assert_eq!(
            super::parse("{{ a == b != c > d > e == f }}", &syntax).unwrap(),
            vec![Node::Expr(
                Ws(false, false),
                BinOp(
                    "==",
                    BinOp(
                        ">",
                        BinOp(
                            ">",
                            BinOp(
                                "!=",
                                BinOp("==", Var("a").into(), Var("b").into()).into(),
                                Var("c").into()
                            )
                            .into(),
                            Var("d").into()
                        )
                        .into(),
                        Var("e").into()
                    )
                    .into(),
                    Var("f").into()
                )
            )],
        );
    }

    #[test]
    fn test_parse_comments() {
        let s = &Syntax::default();

        assert_eq!(
            super::parse("{##}", s).unwrap(),
            vec![Node::Comment(Ws(false, false))],
        );
        assert_eq!(
            super::parse("{#- #}", s).unwrap(),
            vec![Node::Comment(Ws(true, false))],
        );
        assert_eq!(
            super::parse("{# -#}", s).unwrap(),
            vec![Node::Comment(Ws(false, true))],
        );
        assert_eq!(
            super::parse("{#--#}", s).unwrap(),
            vec![Node::Comment(Ws(true, true))],
        );

        assert_eq!(
            super::parse("{#- foo\n bar -#}", s).unwrap(),
            vec![Node::Comment(Ws(true, true))],
        );
        assert_eq!(
            super::parse("{#- foo\n {#- bar\n -#} baz -#}", s).unwrap(),
            vec![Node::Comment(Ws(true, true))],
        );
        assert_eq!(
            super::parse("{# foo {# bar #} {# {# baz #} qux #} #}", s).unwrap(),
            vec![Node::Comment(Ws(false, false))],
        );
    }
}

type ParserError<'a, T> = Result<(&'a [u8], T), nom::Err<nom::error::Error<&'a [u8]>>>;
