use chumsky::prelude::*;
use chumsky::Parser;
use tower_lsp::lsp_types::SemanticTokenType;

pub type Span = std::ops::Range<usize>;

// kind
#[derive(Debug, PartialEq)]
pub enum Token {
    LParen,
    RParen,
    Comment,
    Number(String),
    Ident(String),
}

pub fn lexer() -> impl Parser<char, Vec<(Token, Span)>, Error = Simple<char>> {
    let lparen = just("(").map(|_| Token::LParen);
    let rparen = just(")").map(|_| Token::RParen);

    let comment = just(";")
        .then(take_until(text::newline().or(end())))
        .padded()
        .map(|_| Token::Comment);

    let number = text::int(10)
        .chain::<char, _, _>(just('.').chain(text::digits(10)).or_not().flatten())
        .collect::<String>()
        .map(Token::Number);

    let ident = text::ident()
        .or(one_of("+-*/=").map(|c: char| c.to_string()))
        .map(Token::Ident);

    let token = lparen.or(rparen).or(comment).or(number).or(ident);

    token
        .map_with_span(|tok, span| (tok, span))
        .padded()
        .repeated()
}

#[derive(Debug)]
pub struct ImCompleteSemanticToken {
    pub start: usize,
    pub length: usize,
    pub token_type: SemanticTokenType,
}

#[derive(Debug)]
pub struct ParseResult {
    pub semantic_tokens: Vec<ImCompleteSemanticToken>,
    pub parse_errors: Vec<Simple<String>>,
}

pub fn parse(source: &str) -> ParseResult {
    let (tokens, errs) = lexer().parse_recovery(source);

    let semantic_tokens = if let Some(tokens) = tokens {
        tokens
            .iter()
            .filter_map(|(token, span)| match token {
                Token::LParen => None,
                Token::RParen => None,
                Token::Comment => Some(ImCompleteSemanticToken {
                    start: span.start,
                    length: span.len(),
                    token_type: SemanticTokenType::COMMENT,
                }),
                Token::Number(_) => Some(ImCompleteSemanticToken {
                    start: span.start,
                    length: span.len(),
                    token_type: SemanticTokenType::NUMBER,
                }),
                Token::Ident(_) => Some(ImCompleteSemanticToken {
                    start: span.start,
                    length: span.len(),
                    token_type: SemanticTokenType::VARIABLE,
                }),
            })
            .collect()
    } else {
        vec![]
    };

    let parse_errors = errs
        .into_iter()
        .map(|e| e.map(|c| c.to_string()))
        .collect::<Vec<_>>();

    ParseResult {
        semantic_tokens,
        parse_errors,
    }
}

#[cfg(test)]
mod test {
    use super::Token::*;
    use super::*;

    #[test]
    fn parse_token() {
        let result = lexer().parse("(").unwrap();
        let tokens: Vec<_> = result.into_iter().map(|v| v.0).collect();
        assert_eq!(tokens, vec![Token::LParen]);

        let result = lexer().parse(")").unwrap();
        let tokens: Vec<_> = result.into_iter().map(|v| v.0).collect();
        assert_eq!(tokens, vec![Token::RParen]);

        let result = lexer().parse("; comment\n").unwrap();
        let tokens: Vec<_> = result.into_iter().map(|v| v.0).collect();
        assert_eq!(tokens, vec![Token::Comment]);

        let result = lexer().parse("; comment").unwrap();
        let tokens: Vec<_> = result.into_iter().map(|v| v.0).collect();
        assert_eq!(tokens, vec![Token::Comment]);

        let result = lexer().parse("12345").unwrap();
        let tokens: Vec<_> = result.into_iter().map(|v| v.0).collect();
        assert_eq!(tokens, vec![Token::Number("12345".into())]);

        let result = lexer().parse("abc").unwrap();
        let tokens: Vec<_> = result.into_iter().map(|v| v.0).collect();
        assert_eq!(tokens, vec![Token::Ident("abc".into())]);

        let result = lexer()
            .parse(
                r#"
        ; comment
        (defun fact (n)
          (if (= n 0)
              1
              (* n (fact (- n 1)))))

        (print (fact 5)) ; => 120
        "#,
            )
            .unwrap();
        let tokens: Vec<_> = result.into_iter().map(|v| v.0).collect();
        assert_eq!(
            tokens,
            vec![
                Comment,
                LParen,
                Ident("defun".into()),
                Ident("fact".into()),
                LParen,
                Ident("n".into()),
                RParen,
                LParen,
                Ident("if".into()),
                LParen,
                Ident("=".into()),
                Ident("n".into()),
                Number("0".into()),
                RParen,
                Number("1".into()),
                LParen,
                Ident("*".into()),
                Ident("n".into()),
                LParen,
                Ident("fact".into()),
                LParen,
                Ident("-".into()),
                Ident("n".into()),
                Number("1".into()),
                RParen,
                RParen,
                RParen,
                RParen,
                RParen,
                LParen,
                Ident("print".into()),
                LParen,
                Ident("fact".into()),
                Number("5".into()),
                RParen,
                RParen,
                Comment
            ]
        );
    }
}
