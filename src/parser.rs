use crate::ast::{Expr, Function, Program, Resolved, Stmt};
use crate::token::{one_line, Token, TokenType};
use crate::value::Value;

pub fn parse(tokens: Vec<Token>) -> (Program, Vec<String>) {
    let mut p = Parser {
        tokens,
        current: 0,
        depth: 0,
        program: Program::default(),
        errors: Vec::new(),
    };

    while !p.at_end() {
        let before = p.current;

        match p.declaration() {
            Ok(s) => p.program.stmts.push(s),
            Err(()) => p.synchronise(),
        }

        if p.current == before && !p.at_end() {
            p.current += 1;
        }
    }

    (p.program, p.errors)
}

struct Parser {
    tokens: Vec<Token>,
    current: usize,
    depth: usize,
    program: Program,
    errors: Vec<String>,
}

const MAX_NESTING: usize = 250;

type Parsed<T> = Result<T, ()>;

impl Parser {

    fn declaration(&mut self) -> Parsed<Stmt> {
        if self.matches(&[TokenType::Fun]) {
            return self.fun_declaration();
        }

        if self.matches(&[TokenType::Var]) {
            return self.var_declaration();
        }

        self.statement()
    }

    fn fun_declaration(&mut self) -> Parsed<Stmt> {
        let name = self.consume(
            TokenType::Identifier,
            "Expect function name.",
        )?;

        self.consume(
            TokenType::LParen,
            "Expect '(' after function name.",
        )?;

        let mut params = Vec::new();

        if !self.check(TokenType::RParen) {
            loop {
                let param = self.consume(
                    TokenType::Identifier,
                    "Expect parameter name.",
                )?;

                params.push(param.lexeme);

                if !self.matches(&[TokenType::Comma]) {
                    break;
                }
            }
        }

        self.consume(
            TokenType::RParen,
            "Expect ')' after parameters.",
        )?;

        self.consume(
            TokenType::LBrace,
            "Expect '{' before function body.",
        )?;

        let body = self.block()?;

        let function_name = name.lexeme.clone();
        let function_line = name.line;
        let index = self.program.functions.len();

        self.program.functions.push(Function {
            name: function_name.clone(),
            params,
            body,
            line: function_line,
        });

        Ok(Stmt::Function {
            index,
            name: function_name,
            slot: None,
        })
    }

    fn var_declaration(&mut self) -> Parsed<Stmt> {
        let name = self.consume(
            TokenType::Identifier,
            "Expect variable name.",
        )?;

        let initialiser = if self.matches(&[TokenType::Equal]) {
            Some(self.expression()?)
        } else {
            None
        };

        self.consume(
            TokenType::Semicolon,
            "Expect ';' after variable declaration.",
        )?;

        Ok(Stmt::Var {
            name,
            initialiser,
            slot: None,
        })
    }

    fn statement(&mut self) -> Parsed<Stmt> {
        self.depth += 1;
        let result = self.statement_inner();
        self.depth -= 1;
        result
    }

    fn statement_inner(&mut self) -> Parsed<Stmt> {
        if self.matches(&[TokenType::Print]) {
            let value = self.expression()?;

            self.consume(
                TokenType::Semicolon,
                "Expect ';' after value.",
            )?;

            return Ok(Stmt::Print(value));
        }

        if self.matches(&[TokenType::LBrace]) {
            let body = self.block()?;

            return Ok(Stmt::Block {
                body,
                locals: 0,
            });
        }

        if self.matches(&[TokenType::If]) {
            return self.if_statement();
        }

        if self.matches(&[TokenType::While]) {
            return self.while_statement();
        }

        if self.matches(&[TokenType::Return]) {
            return self.return_statement();
        }

        let expr = self.expression()?;

        self.consume(
            TokenType::Semicolon,
            "Expect ';' after expression.",
        )?;

        Ok(Stmt::Expression(expr))
    }

    fn return_statement(&mut self) -> Parsed<Stmt> {
        let value = if self.check(TokenType::Semicolon) {
            Some(self.expression()?)
        } else {
            None
        };

        self.consume(
            TokenType::Semicolon,
            "Expect ';' after return value.",
        )?;

        Ok(Stmt::Return(value))
    }

    fn if_statement(&mut self) -> Parsed<Stmt> {
        self.consume(
            TokenType::LParen,
            "Expect '(' after 'if'.",
        )?;

        let condition = self.expression()?;

        self.consume(
            TokenType::RParen,
            "Expect ')' after if condition.",
        )?;

        let then_branch = Box::new(self.statement()?);

        let else_branch = if self.matches(&[TokenType::Else]) {
            Some(Box::new(self.statement()?))
        } else {
            None
        };

        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn while_statement(&mut self) -> Parsed<Stmt> {
        self.consume(
            TokenType::LParen,
            "Expect '(' after 'while'.",
        )?;

        let condition = self.expression()?;

        self.consume(
            TokenType::RParen,
            "Expect ')' after while condition.",
        )?;

        let body = Box::new(self.statement()?);

        Ok(Stmt::While { condition, body })
    }

    fn block(&mut self) -> Parsed<Vec<Stmt>> {
        let mut statements = Vec::new();

        while !self.check(TokenType::RBrace) && !self.at_end() {
            let before = self.current;

            match self.declaration() {
                Ok(statement) => statements.push(statement),
                Err(()) => self.synchronise(),
            }

            if self.current == before && !self.at_end() {
                self.current += 1;
            }
        }

        self.consume(
            TokenType::RBrace,
            "Expect '}' after block.",
        )?;

        Ok(statements)
    }

    fn expression(&mut self) -> Parsed<Expr> {
        self.depth += 1;

        let result = if self.depth > MAX_NESTING {
            self.error("Too deeply nested.");
            Err(())
        } else {
            self.assignment()
        };

        self.depth -= 1;
        result
    }

    fn assignment(&mut self) -> Parsed<Expr> {
        let expr = self.or()?;

        if self.matches(&[TokenType::Equal]) {
            let equals = self.previous();
            let value = self.assignment()?;

            if let Expr::Variable { name, at } = expr {
                return Ok(Expr::Assign {
                    name,
                    value: Box::new(value),
                    at,
                });
            }

            self.error_at(
                &equals,
                "Cannot assign to this expression.",
            );

            return Err(());
        }

        Ok(expr)
    }

    fn or(&mut self) -> Parsed<Expr> {
        let mut expr = self.and()?;

        while self.matches(&[TokenType::Or]) {
            let op = self.previous();
            let right = self.and()?;

            expr = Expr::Logical {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn and(&mut self) -> Parsed<Expr> {
        let mut expr = self.equality()?;

        while self.matches(&[TokenType::And]) {
            let op = self.previous();
            let right = self.equality()?;

            expr = Expr::Logical {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn equality(&mut self) -> Parsed<Expr> {
        let mut expr = self.comparison()?;

        while self.matches(&[
            TokenType::BangEqual,
            TokenType::EqualEqual,
        ]) {
            let op = self.previous();
            let right = self.comparison()?;

            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Parsed<Expr> {
        let mut expr = self.term()?;

        while self.matches(&[
            TokenType::Greater,
            TokenType::GreaterEqual,
            TokenType::Less,
            TokenType::LessEqual,
        ]) {
            let op = self.previous();
            let right = self.term()?;

            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn term(&mut self) -> Parsed<Expr> {
        let mut expr = self.factor()?;

        while self.matches(&[
            TokenType::Plus,
            TokenType::Minus,
        ]) {
            let op = self.previous();
            let right = self.factor()?;

            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn factor(&mut self) -> Parsed<Expr> {
        let mut expr = self.unary()?;

        while self.matches(&[
            TokenType::Star,
            TokenType::Slash,
        ]) {
            let op = self.previous();
            let right = self.unary()?;

            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Parsed<Expr> {
        if self.matches(&[
            TokenType::Bang,
            TokenType::Minus,
        ]) {
            let op = self.previous();
            let right = self.unary()?;

            return Ok(Expr::Unary {
                op,
                right: Box::new(right),
            });
        }

        self.call()
    }

    fn call(&mut self) -> Parsed<Expr> {
        let mut expr = self.primary()?;

        loop {
            if self.matches(&[TokenType::LParen]) {
                let args = self.arguments()?;

                let paren = self.consume(
                    TokenType::RParen,
                    "Expect ')' after arguments.",
                )?;

                expr = Expr::Call {
                    callee: Box::new(expr),
                    paren,
                    args,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn arguments(&mut self) -> Parsed<Vec<Expr>> {
        let mut args = Vec::new();

        if !self.check(TokenType::RParen) {
            loop {
                args.push(self.expression()?);

                if !self.matches(&[TokenType::Comma]) {
                    break;
                }
            }
        }

        Ok(args)
    }

    fn primary(&mut self) -> Parsed<Expr> {
        if self.matches(&[TokenType::False]) {
            return Ok(self.literal(Value::Bool(false)));
        }

        if self.matches(&[TokenType::True]) {
            return Ok(self.literal(Value::Bool(true)));
        }

        if self.matches(&[TokenType::Nil]) {
            return Ok(self.literal(Value::Nil));
        }

        if self.matches(&[TokenType::Number]) {
            let number = self.previous().lexeme.parse::<f64>().unwrap();
            return Ok(self.literal(Value::Number(number)));
        }

        if self.matches(&[TokenType::Str]) {
            let token = self.previous();
            let value = token.lexeme[1..token.lexeme.len() - 1].to_string();

            return Ok(Expr::Literal {
                value: Value::Str(value),
                line: token.line,
            });
        }

        if self.matches(&[TokenType::Identifier]) {
            return Ok(Expr::Variable {
                name: self.previous(),
                at: Resolved::Global,
            });
        }

        if self.matches(&[TokenType::LParen]) {
            let expr = self.expression()?;

            self.consume(
                TokenType::RParen,
                "Expect ')' after expression.",
            )?;

            return Ok(Expr::Grouping(Box::new(expr)));
        }

        self.error("Expect expression.");
        Err(())
    }

    fn synchronise(&mut self) {
        while !self.at_end() {
            if self.previous().kind == TokenType::Semicolon {
                return;
            }

            match self.peek().kind {
                TokenType::Fun
                | TokenType::Var
                | TokenType::If
                | TokenType::While
                | TokenType::Print
                | TokenType::Return => return,

                _ => self.current += 1,
            }
        }
    }

    fn literal(&self, value: Value) -> Expr {
        Expr::Literal {
            value,
            line: self.previous().line,
        }
    }

    fn matches(&mut self, kinds: &[TokenType]) -> bool {
        for kind in kinds {
            if self.check(*kind) {
                self.current += 1;
                return true;
            }
        }

        false
    }

    fn check(&self, kind: TokenType) -> bool {
        !self.at_end() && self.peek().kind == kind
    }

    fn at_end(&self) -> bool {
        self.peek().kind == TokenType::Eof
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn previous(&self) -> Token {
        self.tokens[self.current - 1].clone()
    }

    fn consume(
        &mut self,
        kind: TokenType,
        message: &str,
    ) -> Parsed<Token> {
        if self.check(kind) {
            self.current += 1;
            return Ok(self.previous());
        }

        self.error(message);
        Err(())
    }

    fn error(&mut self, message: &str) {
        let token = self.peek().clone();
        self.error_at(&token, message);
    }

    fn error_at(&mut self, token: &Token, message: &str) {
        let at = if token.kind == TokenType::Eof {
            "end".to_string()
        } else {
            format!("'{}'", one_line(&token.lexeme))
        };

        self.errors.push(format!(
            "[line {}] Error at {}: {}",
            token.line, at, message
        ));
    }
}