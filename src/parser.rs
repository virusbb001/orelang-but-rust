use chumsky::prelude::*;
use chumsky::Parser;

pub type Span = std::ops::Range<usize>;

// kind
#[derive(Debug, PartialEq)]
pub enum Token {
    LParen,
    RParen,
    Comment,
    Number,
    Ident,
}

pub fn lexer() -> impl Parser<char, Vec<(Token, Span)>, Error = Simple<char>> {
    let lparen = just("(").map(|_| Token::LParen);
    let rparen = just(")").map(|_| Token::RParen);

    let comment = just(";").then(take_until(text::newline().or(end()))).padded().map(|_| Token::Comment);

    let number = text::int(10).map(|_| Token::Number);

    let ident = text::ident().or(one_of("+-*/=").map(|c: char| c.to_string())).map(|_| Token::Ident);

    let token = lparen.or(rparen).or(comment).or(number).or(ident);

    token
        .map_with_span(|tok, span| (tok, span))
        .padded()
        .repeated()
}

#[cfg(test)]
mod test {
    use super::*;
    use super::Token::*;

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
        assert_eq!(tokens, vec![Token::Number]);

        let result = lexer().parse("abc").unwrap();
        let tokens: Vec<_> = result.into_iter().map(|v| v.0).collect();
        assert_eq!(tokens, vec![Token::Ident]);

        let result = lexer().parse(r#"
        ; comment
        (defun fact (n)
          (if (= n 0)
              1
              (* n (fact (- n 1)))))

        (print (fact 5)) ; => 120
        "#).unwrap();
        let tokens: Vec<_> = result.into_iter().map(|v| v.0).collect();
        assert_eq!(tokens, vec![
        Comment,
        LParen, Ident, Ident, LParen, Ident, RParen,
            LParen, Ident, LParen, Ident, Ident, Number, RParen,
                Number,
                LParen, Ident, Ident, LParen, Ident, LParen, Ident, Ident, Number, RParen,RParen,RParen,RParen,RParen,

        LParen, Ident, LParen, Ident, Number, RParen, RParen, Comment
        ]);
    }
}