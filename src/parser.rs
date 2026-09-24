use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while, take_while1},
    character::complete::{char as nom_char, multispace0},
    combinator::{opt, value},
    multi::separated_list0,
    number::complete::double,
    sequence::pair,
    IResult,
};

use crate::ast::{BinaryOp, Equation, Expr, ExprKind, Span, UnaryOp};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at {}:{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for ParseError {}

type Res<'a, T> = IResult<&'a str, T, nom::error::Error<&'a str>>;

fn p_char(c: char) -> impl Fn(&str) -> Res<char> {
    move |i| nom_char(c)(i)
}

fn p_tag<'a>(t: &'static str) -> impl Fn(&'a str) -> Res<'a, &'a str> {
    move |i| tag(t)(i)
}

fn p_tag_no_case<'a>(t: &'static str) -> impl Fn(&'a str) -> Res<'a, &'a str> {
    move |i| tag_no_case(t)(i)
}

fn ws<'a, F, O>(mut inner: F) -> impl FnMut(&'a str) -> Res<'a, O>
where
    F: FnMut(&'a str) -> Res<'a, O>,
{
    move |i: &'a str| {
        let (i, _) = multispace0(i)?;
        let (i, res) = inner(i)?;
        let (i, _) = multispace0(i)?;
        Ok((i, res))
    }
}

fn parse_ident_str<'a>(i: &'a str) -> Res<'a, &'a str> {
    let (rest, _) = multispace0(i)?;
    let (rest, (first, next)) = pair(
        take_while1(|c: char| c.is_alphabetic() || c == '_'),
        take_while(|c: char| c.is_alphanumeric() || c == '_'),
    )(rest)?;
    let total_len = first.len() + next.len();
    Ok((
        rest,
        &i[i.len() - rest.len() - total_len..i.len() - rest.len()],
    ))
}

fn parse_offset<'a>(i: &'a str) -> Res<'a, i64> {
    let (i, _) = multispace0(i)?;
    if let Ok((i_rest, _)) = p_tag("n")(i) {
        let (i_rest, _) = multispace0(i_rest)?;
        if let Ok((i_after_plus, _)) = p_char('+')(i_rest) {
            let (i_num, _) = multispace0(i_after_plus)?;
            let (i_end, num_str) = take_while1(|c: char| c.is_ascii_digit())(i_num)?;
            let val = num_str.parse::<i64>().unwrap_or(0);
            Ok((i_end, val))
        } else if let Ok((i_after_minus, _)) = p_char('-')(i_rest) {
            let (i_num, _) = multispace0(i_after_minus)?;
            let (i_end, num_str) = take_while1(|c: char| c.is_ascii_digit())(i_num)?;
            let val = num_str.parse::<i64>().unwrap_or(0);
            Ok((i_end, -val))
        } else {
            Ok((i_rest, 0))
        }
    } else {
        let (i, sign) = opt(alt((p_char('+'), p_char('-'))))(i)?;
        let (i, num_str) = take_while1(|c: char| c.is_ascii_digit())(i)?;
        let mut val = num_str.parse::<i64>().unwrap_or(0);
        if sign == Some('-') {
            val = -val;
        }
        Ok((i, val))
    }
}

// Primary expressions: numbers, calls, cond, staterefs, parenthesized
fn parse_primary<'a>(orig: &'a str, i: &'a str) -> Res<'a, Expr> {
    let (i, _) = multispace0(i)?;
    let start_offset = (i.as_ptr() as usize) - (orig.as_ptr() as usize);

    // 1. Parenthesized: ( expr )
    if let Ok((rest, _)) = p_char('(')(i) {
        let (rest, inner) = parse_or_expr(orig, rest)?;
        let (rest, _) = ws(p_char(')'))(rest)?;
        let end_offset = (rest.as_ptr() as usize) - (orig.as_ptr() as usize);
        return Ok((
            rest,
            Expr::new(inner.kind, Span::new(start_offset, end_offset)),
        ));
    }

    // 2. cond(c, t, f)
    if let Ok((rest, _)) = ws(p_tag_no_case("cond"))(i) {
        if rest.starts_with('(') {
            let (rest, _) = p_char('(')(rest)?;
            let (rest, cond_e) = parse_or_expr(orig, rest)?;
            let (rest, _) = ws(p_char(','))(rest)?;
            let (rest, true_e) = parse_or_expr(orig, rest)?;
            let (rest, _) = ws(p_char(','))(rest)?;
            let (rest, false_e) = parse_or_expr(orig, rest)?;
            let (rest, _) = ws(p_char(')'))(rest)?;
            let end_offset = (rest.as_ptr() as usize) - (orig.as_ptr() as usize);
            return Ok((
                rest,
                Expr::new(
                    ExprKind::Cond {
                        condition: Box::new(cond_e),
                        true_val: Box::new(true_e),
                        false_val: Box::new(false_e),
                    },
                    Span::new(start_offset, end_offset),
                ),
            ));
        }
    }

    // 3. Numbers: float or integer literal via nom double
    if !i.starts_with(|c: char| c.is_alphabetic() || c == '_') {
        let double_res: Res<f64> = double(i);
        if let Ok((rest, val)) = double_res {
            let end_offset = (rest.as_ptr() as usize) - (orig.as_ptr() as usize);
            return Ok((
                rest,
                Expr::new(ExprKind::Literal(val), Span::new(start_offset, end_offset)),
            ));
        }
    }

    // 4. Identifiers: stateref a[n+1], function call func(...), or parameter k
    if let Ok((rest, name)) = parse_ident_str(i) {
        let (rest_after_ws, _) = multispace0(rest)?;

        // State ref: name[ ... ]
        if rest_after_ws.starts_with('[') {
            let (after_bracket, _) = p_char('[')(rest_after_ws)?;
            let (after_offset, offset) = parse_offset(after_bracket)?;
            let (after_close, _) = ws(p_char(']'))(after_offset)?;
            let end_offset = (after_close.as_ptr() as usize) - (orig.as_ptr() as usize);
            return Ok((
                after_close,
                Expr::new(
                    ExprKind::StateRef {
                        variable: name.to_string(),
                        offset,
                    },
                    Span::new(start_offset, end_offset),
                ),
            ));
        }

        // Function call: name( ... )
        if rest_after_ws.starts_with('(') {
            let (after_open, _) = p_char('(')(rest_after_ws)?;
            let (after_args, args) =
                separated_list0(ws(p_char(',')), |s| parse_or_expr(orig, s))(after_open)?;
            let (after_close, _) = ws(p_char(')'))(after_args)?;
            let end_offset = (after_close.as_ptr() as usize) - (orig.as_ptr() as usize);

            if name == "cond" && args.len() == 3 {
                let mut args = args;
                let false_val = args.pop().unwrap();
                let true_val = args.pop().unwrap();
                let condition = args.pop().unwrap();
                return Ok((
                    after_close,
                    Expr::new(
                        ExprKind::Cond {
                            condition: Box::new(condition),
                            true_val: Box::new(true_val),
                            false_val: Box::new(false_val),
                        },
                        Span::new(start_offset, end_offset),
                    ),
                ));
            }

            return Ok((
                after_close,
                Expr::new(
                    ExprKind::Call {
                        func: name.to_string(),
                        args,
                    },
                    Span::new(start_offset, end_offset),
                ),
            ));
        }

        // Bare identifier -> Parameter
        let end_offset = (rest.as_ptr() as usize) - (orig.as_ptr() as usize);
        return Ok((
            rest,
            Expr::new(
                ExprKind::Parameter(name.to_string()),
                Span::new(start_offset, end_offset),
            ),
        ));
    }

    Err(nom::Err::Error(nom::error::Error::new(
        i,
        nom::error::ErrorKind::Tag,
    )))
}

// Unary prefix: -, not, !
fn parse_unary<'a>(orig: &'a str, i: &'a str) -> Res<'a, Expr> {
    let (i, _) = multispace0(i)?;
    let start_offset = (i.as_ptr() as usize) - (orig.as_ptr() as usize);

    if let Ok((rest, _)) = p_char('-')(i) {
        let (rest, inner) = parse_unary(orig, rest)?;
        let span = Span::new(start_offset, inner.span.end);
        return Ok((
            rest,
            Expr::new(
                ExprKind::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(inner),
                },
                span,
            ),
        ));
    }

    let not_res: Res<&str> = alt((p_tag_no_case("not"), p_tag("!")))(i);
    if let Ok((rest, _)) = not_res {
        let (rest, inner) = parse_unary(orig, rest)?;
        let span = Span::new(start_offset, inner.span.end);
        return Ok((
            rest,
            Expr::new(
                ExprKind::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(inner),
                },
                span,
            ),
        ));
    }

    parse_primary(orig, i)
}

// Exponentiation ^ (right-associative)
fn parse_power<'a>(orig: &'a str, i: &'a str) -> Res<'a, Expr> {
    let (i, left) = parse_unary(orig, i)?;
    let (i_peek, _) = multispace0(i)?;
    if let Ok((rest, _)) = p_char('^')(i_peek) {
        let (rest, right) = parse_power(orig, rest)?;
        let span = left.span.merge(right.span);
        Ok((
            rest,
            Expr::new(
                ExprKind::Binary {
                    op: BinaryOp::Pow,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            ),
        ))
    } else {
        Ok((i, left))
    }
}

// Multiplication, Division, Remainder: *, /, %
fn parse_factor<'a>(orig: &'a str, i: &'a str) -> Res<'a, Expr> {
    let (mut i, mut left) = parse_power(orig, i)?;

    loop {
        let (i_next, _) = multispace0(i)?;
        let op_res: Res<BinaryOp> = alt((
            value(BinaryOp::Mul, p_char('*')),
            value(BinaryOp::Div, p_char('/')),
            value(BinaryOp::Rem, p_char('%')),
        ))(i_next);

        if let Ok((rest, op)) = op_res {
            let (rest, right) = parse_power(orig, rest)?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
            i = rest;
        } else {
            break;
        }
    }

    Ok((i, left))
}

// Addition and Subtraction: +, -
fn parse_term<'a>(orig: &'a str, i: &'a str) -> Res<'a, Expr> {
    let (mut i, mut left) = parse_factor(orig, i)?;

    loop {
        let (i_next, _) = multispace0(i)?;
        let op_res: Res<BinaryOp> = alt((
            value(BinaryOp::Add, p_char('+')),
            value(BinaryOp::Sub, p_char('-')),
        ))(i_next);

        if let Ok((rest, op)) = op_res {
            let (rest, right) = parse_factor(orig, rest)?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
            i = rest;
        } else {
            break;
        }
    }

    Ok((i, left))
}

// Comparisons: ==, !=, <=, >=, <, >
fn parse_comparison<'a>(orig: &'a str, i: &'a str) -> Res<'a, Expr> {
    let (mut i, mut left) = parse_term(orig, i)?;

    loop {
        let (i_next, _) = multispace0(i)?;
        let op_res: Res<BinaryOp> = alt((
            value(BinaryOp::Eq, p_tag("==")),
            value(BinaryOp::Ne, p_tag("!=")),
            value(BinaryOp::Le, p_tag("<=")),
            value(BinaryOp::Ge, p_tag(">=")),
            value(BinaryOp::Lt, p_tag("<")),
            value(BinaryOp::Gt, p_tag(">")),
        ))(i_next);

        if let Ok((rest, op)) = op_res {
            let (rest, right) = parse_term(orig, rest)?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
            i = rest;
        } else {
            break;
        }
    }

    Ok((i, left))
}

// Logical AND: and, &&
fn parse_and_expr<'a>(orig: &'a str, i: &'a str) -> Res<'a, Expr> {
    let (mut i, mut left) = parse_comparison(orig, i)?;

    loop {
        let (i_next, _) = multispace0(i)?;
        let op_res: Res<BinaryOp> = alt((
            value(BinaryOp::And, p_tag_no_case("and")),
            value(BinaryOp::And, p_tag("&&")),
        ))(i_next);

        if let Ok((rest, op)) = op_res {
            let (rest, right) = parse_comparison(orig, rest)?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
            i = rest;
        } else {
            break;
        }
    }

    Ok((i, left))
}

// Logical OR: or, ||
fn parse_or_expr<'a>(orig: &'a str, i: &'a str) -> Res<'a, Expr> {
    let (mut i, mut left) = parse_and_expr(orig, i)?;

    loop {
        let (i_next, _) = multispace0(i)?;
        let op_res: Res<BinaryOp> = alt((
            value(BinaryOp::Or, p_tag_no_case("or")),
            value(BinaryOp::Or, p_tag("||")),
        ))(i_next);

        if let Ok((rest, op)) = op_res {
            let (rest, right) = parse_and_expr(orig, rest)?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
            i = rest;
        } else {
            break;
        }
    }

    Ok((i, left))
}

// Public API Functions

pub fn parse_expression_str(source: &str) -> Result<Expr, ParseError> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Err(ParseError {
            message: "Empty expression".to_string(),
            span: Span::new(0, 0),
        });
    }

    match parse_or_expr(source, source) {
        Ok((rest, expr)) => {
            let rest_trimmed = rest.trim();
            if !rest_trimmed.is_empty() {
                let err_start = (rest.as_ptr() as usize) - (source.as_ptr() as usize);
                return Err(ParseError {
                    message: format!("Unexpected trailing token: '{rest_trimmed}'"),
                    span: Span::new(err_start, source.len()),
                });
            }
            Ok(expr)
        }
        Err(err) => Err(ParseError {
            message: format!("Syntax error: {err}"),
            span: Span::new(0, source.len()),
        }),
    }
}

pub fn parse_equation_str(source: &str) -> Result<Equation, ParseError> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Err(ParseError {
            message: "Empty equation".to_string(),
            span: Span::new(0, 0),
        });
    }

    let i = source.trim_start();

    // Target variable name
    let (i, target_var) = parse_ident_str(i).map_err(|_| ParseError {
        message: "Expected equation target variable name".to_string(),
        span: Span::new(0, source.len()),
    })?;

    // Optional [n+1] or [n] or [n+k]
    let i = i.trim_start();
    let (i, target_offset) = if let Some(stripped) = i.strip_prefix('[') {
        let (after_offset, offset) = parse_offset(stripped).map_err(|_| ParseError {
            message: "Invalid index inside brackets".to_string(),
            span: Span::new(0, source.len()),
        })?;
        let after_offset = after_offset.trim_start();
        let after_bracket = after_offset.strip_prefix(']').ok_or_else(|| ParseError {
            message: "Expected ']' after index".to_string(),
            span: Span::new(0, source.len()),
        })?;
        (after_bracket, offset)
    } else {
        (i, 1) // default to n+1 for recurrence
    };

    // Expect '='
    let (i, _) = ws(p_char('='))(i).map_err(|_| ParseError {
        message: "Expected '=' in equation".to_string(),
        span: Span::new(0, source.len()),
    })?;

    // Parse RHS expression
    let (rest, rhs) = parse_or_expr(source, i).map_err(|e| ParseError {
        message: format!("Error parsing equation expression: {e}"),
        span: Span::new(0, source.len()),
    })?;

    let rest_trimmed = rest.trim();
    if !rest_trimmed.is_empty() {
        let err_start = (rest.as_ptr() as usize) - (source.as_ptr() as usize);
        return Err(ParseError {
            message: format!("Unexpected trailing characters: '{rest_trimmed}'"),
            span: Span::new(err_start, source.len()),
        });
    }

    Ok(Equation {
        target_var: target_var.to_string(),
        target_offset,
        rhs,
        span: Span::new(0, source.len()),
    })
}
