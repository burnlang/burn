package parser

import (
	"github.com/burnlang/burn/pkg/ast"
	"github.com/burnlang/burn/pkg/lexer"
)

type Parser struct {
	tokens      []lexer.Token
	current     int
	currentFunc *ast.FunctionDeclaration
}

func New(tokens []lexer.Token) *Parser {
	return &Parser{
		tokens:  tokens,
		current: 0,
	}
}

func (p *Parser) Parse() (*ast.Program, error) {
	program := &ast.Program{
		Declarations: []ast.Declaration{},
	}

	for !p.isAtEnd() {
		declaration, err := p.declaration()
		if err != nil {
			return nil, err
		}
		program.Declarations = append(program.Declarations, declaration)
	}

	return program, nil
}

func (p *Parser) Position() int {
	if p.current < len(p.tokens) {
		return p.tokens[p.current].Position
	} else if len(p.tokens) > 0 {
		return p.tokens[len(p.tokens)-1].Position
	}
	return 0
}

func (p *Parser) match(types ...lexer.TokenType) bool {
	for _, tokenType := range types {
		if p.check(tokenType) {
			p.advance()
			return true
		}
	}
	return false
}

func (p *Parser) check(tokenType lexer.TokenType) bool {
	if p.isAtEnd() {
		return false
	}
	return p.peek().Type == tokenType
}

func (p *Parser) advance() lexer.Token {
	if !p.isAtEnd() {
		p.current++
	}
	return p.previous()
}

func (p *Parser) isAtEnd() bool {
	return p.peek().Type == lexer.TokenEOF
}

func (p *Parser) peek() lexer.Token {
	return p.tokens[p.current]
}

func (p *Parser) previous() lexer.Token {
	return p.tokens[p.current-1]
}
