package parser

import (
	"fmt"

	"github.com/burnlang/burn/pkg/ast"
	"github.com/burnlang/burn/pkg/lexer"
)

func (p *Parser) statement() (ast.Declaration, error) {
	if p.match(lexer.TokenIf) {
		return p.ifStatement()
	}
	if p.match(lexer.TokenWhile) {
		return p.whileStatement()
	}
	if p.match(lexer.TokenFor) {
		return p.forStatement()
	}
	if p.match(lexer.TokenReturn) {
		return p.returnStatement()
	}
	if p.match(lexer.TokenLeftBrace) {
		statements, err := p.block()
		if err != nil {
			return nil, err
		}
		return &ast.BlockStatement{Statements: statements}, nil
	}

	return p.expressionStatement()
}

func (p *Parser) ifStatement() (ast.Declaration, error) {
	pos := p.peek().Position

	if !p.match(lexer.TokenLeftParen) {
		p.current--
	}

	condition, err := p.expression()
	if err != nil {
		return nil, err
	}

	if p.match(lexer.TokenRightParen) {
	}

	if !p.match(lexer.TokenLeftBrace) {
		return nil, fmt.Errorf("expected '{' after if condition at line %d", p.peek().Line)
	}

	thenBranch, err := p.block()
	if err != nil {
		return nil, err
	}

	var elseBranch []ast.Declaration
	if p.match(lexer.TokenElse) {
		if p.match(lexer.TokenIf) {
			elseIfStmt, err := p.ifStatement()
			if err != nil {
				return nil, err
			}
			elseBranch = []ast.Declaration{elseIfStmt}
		} else if p.match(lexer.TokenLeftBrace) {
			elseBranch, err = p.block()
			if err != nil {
				return nil, err
			}
		} else {
			return nil, fmt.Errorf("expected '{' or 'if' after 'else' at line %d", p.peek().Line)
		}
	}

	return &ast.IfStatement{
		Condition:  condition,
		ThenBranch: thenBranch,
		ElseBranch: elseBranch,
		Position:   pos,
	}, nil
}

func (p *Parser) whileStatement() (ast.Declaration, error) {
	pos := p.peek().Position

	if !p.match(lexer.TokenLeftParen) {
		p.current--
	}

	condition, err := p.expression()
	if err != nil {
		return nil, err
	}

	if p.match(lexer.TokenRightParen) {
	}

	if !p.match(lexer.TokenLeftBrace) {
		return nil, fmt.Errorf("expected '{' after while condition at line %d", p.peek().Line)
	}

	body, err := p.block()
	if err != nil {
		return nil, err
	}

	return &ast.WhileStatement{
		Condition: condition,
		Body:      body,
		Position:  pos,
	}, nil
}

func (p *Parser) forStatement() (ast.Declaration, error) {
	pos := p.peek().Position

	if !p.match(lexer.TokenLeftParen) {
		p.current--
	}

	var initializer ast.Declaration
	if !p.check(lexer.TokenSemicolon) {
		var err error
		if p.match(lexer.TokenVar) {
			initializer, err = p.variableDeclaration(false)
		} else {
			initializer, err = p.expressionStatement()
		}
		if err != nil {
			return nil, err
		}
	}
	if p.match(lexer.TokenSemicolon) {
	}

	var condition ast.Expression
	if !p.check(lexer.TokenSemicolon) {
		var err error
		condition, err = p.expression()
		if err != nil {
			return nil, err
		}
	}
	if p.match(lexer.TokenSemicolon) {
	}

	var increment ast.Expression
	if !p.check(lexer.TokenRightParen) {
		var err error
		increment, err = p.expression()
		if err != nil {
			return nil, err
		}
	}

	if p.match(lexer.TokenRightParen) {
	}

	if !p.match(lexer.TokenLeftBrace) {
		return nil, fmt.Errorf("expected '{' after for clauses at line %d", p.peek().Line)
	}

	body, err := p.block()
	if err != nil {
		return nil, err
	}

	return &ast.ForStatement{
		Initializer: initializer,
		Condition:   condition,
		Increment:   increment,
		Body:        body,
		Position:    pos,
	}, nil
}

func (p *Parser) returnStatement() (ast.Declaration, error) {
	pos := p.peek().Position

	var value ast.Expression
	var err error

	if !p.check(lexer.TokenSemicolon) && !p.check(lexer.TokenRightBrace) {
		value, err = p.expression()
		if err != nil {
			return nil, err
		}
	}

	if p.match(lexer.TokenSemicolon) {
	}

	return &ast.ReturnStatement{
		Value:    value,
		Position: pos,
	}, nil
}

func (p *Parser) block() ([]ast.Declaration, error) {
	statements := []ast.Declaration{}

	for !p.check(lexer.TokenRightBrace) && !p.isAtEnd() {
		decl, err := p.declaration()
		if err != nil {
			return nil, err
		}
		statements = append(statements, decl)
	}

	if !p.match(lexer.TokenRightBrace) {
		return nil, fmt.Errorf("expected '}' at line %d", p.peek().Line)
	}

	return statements, nil
}

func (p *Parser) expressionStatement() (ast.Declaration, error) {
	pos := p.peek().Position

	expr, err := p.expression()
	if err != nil {
		return nil, err
	}

	if p.match(lexer.TokenSemicolon) {
	}

	return &ast.ExpressionStatement{
		Expression: expr,
		Position:   pos,
	}, nil
}
