// crates/ret-logic/src/dsl.rs
// ============================================================================
// Module: Requirement DSL Parser
// Description: Lightweight, author-facing DSL for requirement trees.
// Purpose: Turn human-readable boolean expressions into `Requirement<P>` with
//          validation and symbol resolution.
// Dependencies: crate::requirement, crate::serde_support::RequirementValidator
// ============================================================================

//! ## Overview
//!
//! The DSL provides a compact, author-friendly syntax for building requirement
//! trees without writing nested RON/JSON. It supports boolean composition
//! (`and`, `or`, `not`), the `require_group`/`at_least` operator, and condition
//! symbols that are resolved through a user-supplied [`ConditionResolver`].
//! Security posture: DSL input is untrusted; enforce validation and limits per
//! `Docs/security/threat_model.md`.
//!
//! ### Grammar (informal)
//! - **Conditions**: `is_alive`, `has_ap`, `stunned` (any identifier resolved by the resolver)
//! - **Boolean operators**:
//!   - Infix: `a && b`, `a || b`, `!a`
//!   - Functions: `all(a, b, c)`, `any(a, b)`, `not(a)`
//! - **Groups**: `at_least(2, a, b, c)` or `require_group(2, a, b, c)`
//! - **Parentheses**: `( ... )` for explicit grouping
//!
//! ### Example
//!
//! ```
//! use std::collections::HashMap;
//!
//! use ret_logic::Requirement;
//! use ret_logic::dsl::ConditionResolver;
//! use ret_logic::dsl::parse_requirement;
//!
//! let mut symbols = HashMap::new();
//! symbols.insert("is_alive".to_string(), 1u8);
//! symbols.insert("has_ap".to_string(), 2u8);
//! symbols.insert("in_range".to_string(), 3u8);
//!
//! let req: Requirement<u8> =
//!     parse_requirement("all(is_alive, any(has_ap, in_range))", &symbols).unwrap();
//! ```
//!
//! The parser validates structure (depth, group arity) using
//! [`RequirementValidator`](crate::serde_support::RequirementValidator).

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fmt;
use std::hash::BuildHasher;

use crate::requirement::Requirement;
use crate::serde_support::RequirementValidator;

// ============================================================================
// SECTION: Limits
// ============================================================================

/// Maximum allowed DSL input size in bytes.
const MAX_DSL_INPUT_BYTES: usize = 1024 * 1024;
/// Maximum supported nesting depth for DSL expressions.
const MAX_DSL_NESTING: usize = 32;

// ============================================================================
// SECTION: Public API
// ============================================================================

/// Errors that can occur while parsing or validating a DSL expression.
///
/// # Invariants
/// - None. Variants capture structured parse and validation failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DslError {
    /// Input was empty or contained only whitespace.
    EmptyInput,
    /// Input exceeded the configured size limit.
    InputTooLarge {
        /// Maximum allowed bytes.
        max_bytes: usize,
        /// Actual input length in bytes.
        actual_bytes: usize,
    },
    /// Input exceeded the configured nesting depth.
    NestingTooDeep {
        /// Maximum allowed nesting depth.
        max_depth: usize,
        /// Actual nesting depth when the error occurred.
        actual_depth: usize,
        /// Byte offset in the original input.
        position: usize,
    },
    /// Unexpected token encountered during parsing.
    UnexpectedToken {
        /// Human-friendly expectation summary.
        expected: &'static str,
        /// The token that was actually seen.
        found: String,
        /// Byte offset in the original input.
        position: usize,
    },
    /// Condition symbol was not found in the resolver.
    UnknownCondition {
        /// The unresolved symbol.
        name: String,
        /// Byte offset in the original input.
        position: usize,
    },
    /// DSL function name was not recognized.
    UnknownFunction {
        /// The unknown function identifier.
        name: String,
        /// Byte offset in the original input.
        position: usize,
    },
    /// Numeric literal failed to parse or overflowed.
    InvalidNumber {
        /// The raw numeric text.
        raw: String,
        /// Byte offset in the original input.
        position: usize,
    },
    /// Structural validation failed after parsing.
    Validation(String),
    /// Unexpected trailing input after a complete expression.
    TrailingInput {
        /// Byte offset where unexpected input begins.
        position: usize,
    },
}

impl fmt::Display for DslError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "input is empty"),
            Self::InputTooLarge {
                max_bytes,
                actual_bytes,
            } => {
                write!(f, "input exceeds size limit: {actual_bytes} bytes (max {max_bytes})")
            }
            Self::NestingTooDeep {
                max_depth,
                actual_depth,
                position,
            } => write!(
                f,
                "input nesting exceeds limit: depth {actual_depth} (max {max_depth}) at {position}"
            ),
            Self::UnexpectedToken {
                expected,
                found,
                position,
            } => {
                write!(f, "unexpected token `{found}` at {position}, expected {expected}")
            }
            Self::UnknownCondition {
                name,
                position,
            } => {
                write!(f, "unknown condition `{name}` at {position}")
            }
            Self::UnknownFunction {
                name,
                position,
            } => {
                write!(f, "unknown function `{name}` at {position}")
            }
            Self::InvalidNumber {
                raw,
                position,
            } => {
                write!(f, "invalid number `{raw}` at {position}")
            }
            Self::Validation(msg) => write!(f, "{msg}"),
            Self::TrailingInput {
                position,
            } => {
                write!(f, "unexpected trailing input at {position}")
            }
        }
    }
}

/// Resolves condition symbols to the domain-specific condition type `P`.
///
/// Implement this for your symbol table so the DSL can turn identifiers into
/// the condition values used by your domain.
pub trait ConditionResolver<P> {
    /// Returns a condition value for the given symbol, or `None` if unknown.
    fn resolve(&self, name: &str) -> Option<P>;
}

impl<P: Clone, S: BuildHasher> ConditionResolver<P> for HashMap<String, P, S> {
    fn resolve(&self, name: &str) -> Option<P> {
        self.get(name).cloned()
    }
}

impl<P: Clone> ConditionResolver<P> for BTreeMap<String, P> {
    fn resolve(&self, name: &str) -> Option<P> {
        self.get(name).cloned()
    }
}

impl<P, F> ConditionResolver<P> for F
where
    F: Fn(&str) -> Option<P>,
{
    fn resolve(&self, name: &str) -> Option<P> {
        (self)(name)
    }
}

/// Parses a DSL expression into a validated [`Requirement`] tree.
///
/// # Arguments
/// * `input` - DSL string (e.g., `"all(is_alive, any(has_ap, in_range))"`).
/// * `resolver` - Symbol resolver that maps identifiers to condition values.
///
/// # Errors
/// Returns [`DslError`] for syntax issues, unknown conditions, invalid numbers,
/// trailing input, or post-parse validation failures.
pub fn parse_requirement<P, R>(input: &str, resolver: &R) -> Result<Requirement<P>, DslError>
where
    P: Clone,
    R: ConditionResolver<P>,
{
    if input.len() > MAX_DSL_INPUT_BYTES {
        return Err(DslError::InputTooLarge {
            max_bytes: MAX_DSL_INPUT_BYTES,
            actual_bytes: input.len(),
        });
    }
    let mut lexer = Lexer::new(input);
    let tokens = lexer.lex()?;

    let mut parser = Parser::new(input, tokens, resolver);
    let requirement = parser.parse_expression()?;
    parser.expect_eof()?;

    RequirementValidator::with_defaults()
        .validate(&requirement)
        .map_err(|err| DslError::Validation(err.to_string()))?;

    Ok(requirement)
}

// ============================================================================
// SECTION: Lexer
// ============================================================================

/// Lexer token produced from the DSL input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token<'a> {
    /// Identifier token.
    Ident(&'a str),
    /// Numeric literal token.
    Number(&'a str),
    /// Logical AND operator.
    And,
    /// Logical OR operator.
    Or,
    /// Logical NOT operator.
    Not,
    /// Left parenthesis.
    LParen,
    /// Right parenthesis.
    RParen,
    /// Comma separator.
    Comma,
    /// End-of-input marker.
    Eof,
}

/// Token paired with its byte offset.
#[derive(Debug, Clone, Copy)]
struct SpannedToken<'a> {
    /// Token value.
    token: Token<'a>,
    /// Byte offset into the input.
    position: usize,
}

/// Lexer for the requirements DSL.
struct Lexer<'a> {
    /// Source input being tokenized.
    input: &'a str,
    /// Current byte offset into the input.
    offset: usize,
}

impl<'a> Lexer<'a> {
    /// Creates a new lexer for the given input.
    const fn new(input: &'a str) -> Self {
        Self {
            input,
            offset: 0,
        }
    }

    /// Lexes the input into a sequence of tokens.
    fn lex(&mut self) -> Result<Vec<SpannedToken<'a>>, DslError> {
        let mut tokens = Vec::new();
        let bytes = self.input.as_bytes();

        while self.offset < bytes.len() {
            let ch = bytes[self.offset];
            match ch {
                b' ' | b'\t' | b'\n' | b'\r' => {
                    self.offset += 1;
                }
                b'(' => {
                    tokens.push(self.simple(Token::LParen));
                    self.offset += 1;
                }
                b')' => {
                    tokens.push(self.simple(Token::RParen));
                    self.offset += 1;
                }
                b',' => {
                    tokens.push(self.simple(Token::Comma));
                    self.offset += 1;
                }
                b'!' => {
                    tokens.push(self.simple(Token::Not));
                    self.offset += 1;
                }
                b'&' => {
                    if self.peek_char(bytes) == Some(b'&') {
                        tokens.push(self.simple(Token::And));
                        self.offset += 2;
                    } else {
                        return Err(DslError::UnexpectedToken {
                            expected: "&&",
                            found: "&".to_string(),
                            position: self.offset,
                        });
                    }
                }
                b'|' => {
                    if self.peek_char(bytes) == Some(b'|') {
                        tokens.push(self.simple(Token::Or));
                        self.offset += 2;
                    } else {
                        return Err(DslError::UnexpectedToken {
                            expected: "||",
                            found: "|".to_string(),
                            position: self.offset,
                        });
                    }
                }
                b'0' ..= b'9' => {
                    let start = self.offset;
                    self.consume_while(bytes, |b| b.is_ascii_digit());
                    let slice = &self.input[start .. self.offset];
                    tokens.push(SpannedToken {
                        token: Token::Number(slice),
                        position: start,
                    });
                }
                b'a' ..= b'z' | b'A' ..= b'Z' | b'_' => {
                    let start = self.offset;
                    self.consume_while(bytes, |b| b.is_ascii_alphanumeric() || b == b'_');
                    let slice = &self.input[start .. self.offset];
                    tokens.push(SpannedToken {
                        token: Self::keyword_or_ident(slice),
                        position: start,
                    });
                }
                _ => {
                    return Err(DslError::UnexpectedToken {
                        expected: "identifier, number, or operator",
                        found: char::from(ch).to_string(),
                        position: self.offset,
                    });
                }
            }
        }

        if tokens.is_empty() {
            return Err(DslError::EmptyInput);
        }

        tokens.push(SpannedToken {
            token: Token::Eof,
            position: self.offset,
        });
        Ok(tokens)
    }

    /// Builds a token at the current offset.
    const fn simple(&self, token: Token<'a>) -> SpannedToken<'a> {
        SpannedToken {
            token,
            position: self.offset,
        }
    }

    /// Returns the next byte without advancing.
    fn peek_char(&self, bytes: &[u8]) -> Option<u8> {
        bytes.get(self.offset + 1).copied()
    }

    /// Advances while the condition matches the current byte.
    fn consume_while<F>(&mut self, bytes: &[u8], condition: F)
    where
        F: Fn(u8) -> bool,
    {
        while let Some(&b) = bytes.get(self.offset) {
            if condition(b) {
                self.offset += 1;
            } else {
                break;
            }
        }
    }

    /// Maps a slice to a keyword token or identifier token.
    fn keyword_or_ident(slice: &'a str) -> Token<'a> {
        match slice {
            "and" => Token::And,
            "or" => Token::Or,
            "not" => Token::Not,
            _ => Token::Ident(slice),
        }
    }
}

// ============================================================================
// SECTION: Parser
// ============================================================================

/// Recursive-descent parser for the requirements DSL.
struct Parser<'input, 'resolver, P, R> {
    /// Original input string (for diagnostics).
    _input: &'input str,
    /// Token stream with source positions.
    tokens: Vec<SpannedToken<'input>>,
    /// Current token index.
    index: usize,
    /// Condition resolver for identifiers.
    resolver: &'resolver R,
    /// Current nesting depth for bracketed or function expressions.
    nesting: usize,
    /// Marker for the condition type.
    _marker: std::marker::PhantomData<P>,
}

impl<'input, 'resolver, P, R> Parser<'input, 'resolver, P, R>
where
    P: Clone,
    R: ConditionResolver<P>,
{
    /// Creates a parser over the token stream.
    const fn new(
        input: &'input str,
        tokens: Vec<SpannedToken<'input>>,
        resolver: &'resolver R,
    ) -> Self {
        Self {
            _input: input,
            tokens,
            index: 0,
            resolver,
            nesting: 0,
            _marker: std::marker::PhantomData,
        }
    }

    /// Parses a full expression.
    fn parse_expression(&mut self) -> Result<Requirement<P>, DslError> {
        self.parse_or()
    }

    /// Parses OR expressions.
    fn parse_or(&mut self) -> Result<Requirement<P>, DslError> {
        let mut parts = Vec::new();
        parts.push(self.parse_and()?);

        while self.matches(Token::Or) {
            parts.push(self.parse_and()?);
        }

        if parts.len() == 1 { Ok(parts.remove(0)) } else { Ok(Requirement::or(parts)) }
    }

    /// Parses AND expressions.
    fn parse_and(&mut self) -> Result<Requirement<P>, DslError> {
        let mut parts = Vec::new();
        parts.push(self.parse_unary()?);

        while self.matches(Token::And) {
            parts.push(self.parse_unary()?);
        }

        if parts.len() == 1 { Ok(parts.remove(0)) } else { Ok(Requirement::and(parts)) }
    }

    /// Parses unary expressions, including NOT.
    fn parse_unary(&mut self) -> Result<Requirement<P>, DslError> {
        if self.matches(Token::Not) {
            let requirement = self.parse_unary()?;
            return Ok(Requirement::negate(requirement));
        }
        self.parse_primary()
    }

    /// Parses a primary expression.
    fn parse_primary(&mut self) -> Result<Requirement<P>, DslError> {
        match self.current().token {
            Token::Ident(name) => {
                let pos = self.current().position;
                self.advance();

                if self.matches(Token::LParen) {
                    self.parse_function(name, pos)
                } else {
                    self.resolve_condition(name, pos)
                }
            }
            Token::LParen => {
                let pos = self.current().position;
                self.advance();
                self.with_nesting(pos, |parser| {
                    let expr = parser.parse_expression()?;
                    parser.expect(Token::RParen, "`)`")?;
                    Ok(expr)
                })
            }
            Token::Number(raw) => Err(DslError::UnexpectedToken {
                expected: "identifier or `(`",
                found: raw.to_string(),
                position: self.current().position,
            }),
            Token::RParen | Token::Comma | Token::And | Token::Or | Token::Not | Token::Eof => {
                Err(DslError::UnexpectedToken {
                    expected: "condition or expression",
                    found: self.describe_current(),
                    position: self.current().position,
                })
            }
        }
    }

    /// Parses a function-style expression.
    fn parse_function(
        &mut self,
        name: &'input str,
        name_pos: usize,
    ) -> Result<Requirement<P>, DslError> {
        self.with_nesting(name_pos, |parser| match name {
            "at_least" | "require_group" => parser.parse_group(name_pos),
            "all" | "and" => {
                let args = parser.parse_argument_list()?;
                Ok(Requirement::and(args))
            }
            "any" | "or" => {
                let args = parser.parse_argument_list()?;
                Ok(Requirement::or(args))
            }
            "not" => {
                let args = parser.parse_argument_list()?;
                if args.len() != 1 {
                    return Err(DslError::UnexpectedToken {
                        expected: "exactly one argument to `not(...)`",
                        found: format!("{} arguments", args.len()),
                        position: name_pos,
                    });
                }
                let requirement =
                    args.into_iter().next().ok_or_else(|| DslError::UnexpectedToken {
                        expected: "exactly one argument to `not(...)`",
                        found: "0 arguments".to_string(),
                        position: name_pos,
                    })?;
                Ok(Requirement::negate(requirement))
            }
            _ => {
                let args = parser.parse_argument_list()?;
                if args.is_empty() {
                    // Allow zero-arg condition calls like `is_alive()`.
                    return parser.resolve_condition(name, name_pos);
                }

                Err(DslError::UnknownFunction {
                    name: name.to_string(),
                    position: name_pos,
                })
            }
        })
    }

    /// Parses a require-group expression with minimum count.
    fn parse_group(&mut self, _name_pos: usize) -> Result<Requirement<P>, DslError> {
        // First argument must be a numeric literal.
        let (min, min_pos) = self.parse_number_literal()?;
        if self.matches(Token::Comma) {
            // consume comma between count and first condition
        }

        let mut members = Vec::new();
        if self.matches(Token::RParen) {
            return Err(DslError::UnexpectedToken {
                expected: "at least one condition after the count",
                found: ")".to_string(),
                position: min_pos,
            });
        }

        loop {
            members.push(self.parse_expression()?);
            if self.matches(Token::Comma) {
                continue;
            }
            self.expect(Token::RParen, "`)` after `at_least(...)`")?;
            break;
        }

        Ok(Requirement::require_group(min, members))
    }

    /// Parses a numeric literal for group counts.
    fn parse_number_literal(&mut self) -> Result<(u8, usize), DslError> {
        let SpannedToken {
            token,
            position,
        } = *self.current();

        match token {
            Token::Number(raw) => {
                self.advance();

                let value: u8 = raw.parse().map_err(|_| DslError::InvalidNumber {
                    raw: raw.to_string(),
                    position,
                })?;
                Ok((value, position))
            }
            _ => Err(DslError::UnexpectedToken {
                expected: "numeric literal",
                found: self.describe_current(),
                position,
            }),
        }
    }

    /// Parses a comma-separated argument list.
    fn parse_argument_list(&mut self) -> Result<Vec<Requirement<P>>, DslError> {
        let mut args = Vec::new();
        if self.matches(Token::RParen) {
            return Ok(args);
        }

        loop {
            args.push(self.parse_expression()?);
            if self.matches(Token::Comma) {
                continue;
            }
            self.expect(Token::RParen, "`)` after arguments")?;
            break;
        }
        Ok(args)
    }

    /// Runs a parser step while enforcing the nesting limit.
    fn with_nesting<T>(
        &mut self,
        position: usize,
        f: impl FnOnce(&mut Self) -> Result<T, DslError>,
    ) -> Result<T, DslError> {
        let next_depth = self.nesting + 1;
        if next_depth > MAX_DSL_NESTING {
            return Err(DslError::NestingTooDeep {
                max_depth: MAX_DSL_NESTING,
                actual_depth: next_depth,
                position,
            });
        }
        self.nesting = next_depth;
        let result = f(self);
        self.nesting = self.nesting.saturating_sub(1);
        result
    }

    /// Resolves a condition identifier using the resolver.
    fn resolve_condition(
        &self,
        name: &'input str,
        position: usize,
    ) -> Result<Requirement<P>, DslError> {
        self.resolver.resolve(name).map(Requirement::condition).ok_or_else(|| {
            DslError::UnknownCondition {
                name: name.to_string(),
                position,
            }
        })
    }

    /// Consumes the expected token or returns an error.
    fn expect(&mut self, token: Token<'_>, expected: &'static str) -> Result<(), DslError> {
        if std::mem::discriminant(&self.current().token) == std::mem::discriminant(&token) {
            self.advance();
            Ok(())
        } else {
            Err(DslError::UnexpectedToken {
                expected,
                found: self.describe_current(),
                position: self.current().position,
            })
        }
    }

    /// Ensures the parser is at end-of-input.
    fn expect_eof(&self) -> Result<(), DslError> {
        if matches!(self.current().token, Token::Eof) {
            Ok(())
        } else {
            Err(DslError::TrailingInput {
                position: self.current().position,
            })
        }
    }

    /// Consumes the token if it matches the expected kind.
    fn matches(&mut self, kind: Token<'_>) -> bool {
        if std::mem::discriminant(&self.current().token) == std::mem::discriminant(&kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Returns the current token.
    fn current(&self) -> &SpannedToken<'input> {
        debug_assert!(self.index < self.tokens.len(), "parser index out of bounds");
        &self.tokens[self.index]
    }

    /// Advances to the next token.
    const fn advance(&mut self) {
        if self.index < self.tokens.len() - 1 {
            self.index += 1;
        }
    }

    /// Formats the current token for diagnostics.
    fn describe_current(&self) -> String {
        match &self.current().token {
            Token::Ident(name) => (*name).to_string(),
            Token::Number(raw) => (*raw).to_string(),
            Token::And => "&&".to_string(),
            Token::Or => "||".to_string(),
            Token::Not => "!".to_string(),
            Token::LParen => "(".to_string(),
            Token::RParen => ")".to_string(),
            Token::Comma => ",".to_string(),
            Token::Eof => "end of input".to_string(),
        }
    }
}
