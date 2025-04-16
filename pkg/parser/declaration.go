package parser

import (
	"fmt"
	"strings"

	"github.com/burnlang/burn/pkg/ast"
	"github.com/burnlang/burn/pkg/lexer"
)

func (p *Parser) declaration() (ast.Declaration, error) {
	if p.match(lexer.TokenImport) {
		return p.importDeclaration()
	}
	if p.match(lexer.TokenClass) {
		return p.classDeclaration()
	}
	if p.match(lexer.TokenFun) {
		return p.functionDeclaration()
	}
	if p.match(lexer.TokenVar) {
		return p.variableDeclaration(false)
	}
	if p.match(lexer.TokenConst) {
		return p.variableDeclaration(true)
	}
	if p.match(lexer.TokenDef) {
		return p.typeDefinition()
	}

	return p.statement()
}

func (p *Parser) importDeclaration() (ast.Declaration, error) {
	if p.match(lexer.TokenLeftParen) {
		imports := []*ast.ImportDeclaration{}

		for !p.check(lexer.TokenRightParen) && !p.isAtEnd() {
			if !p.match(lexer.TokenString) {
				return nil, fmt.Errorf("expected string in import block at line %d", p.peek().Line)
			}

			path := p.previous().Value
			processedPath := p.processImportPath(path)

			imports = append(imports, &ast.ImportDeclaration{
				Path: processedPath,
			})
		}

		if !p.match(lexer.TokenRightParen) {
			return nil, fmt.Errorf("expected ')' after import block at line %d", p.peek().Line)
		}

		return &ast.MultiImportDeclaration{
			Imports: imports,
		}, nil
	}

	if !p.match(lexer.TokenString) {
		return nil, fmt.Errorf("expected string after import at line %d", p.peek().Line)
	}

	path := p.previous().Value
	processedPath := p.processImportPath(path)

	return &ast.ImportDeclaration{
		Path: processedPath,
	}, nil
}

func (p *Parser) processImportPath(path string) string {
	trimmedPath := strings.Trim(path, "\"")

	if !strings.Contains(trimmedPath, "/") && !strings.Contains(trimmedPath, "\\") {
		return "src/lib/std/" + trimmedPath + ".bn"
	}

	if strings.HasSuffix(trimmedPath, ".bn") {
		return trimmedPath
	}

	return trimmedPath + ".bn"
}

func (p *Parser) functionDeclaration() (ast.Declaration, error) {
	if !p.check(lexer.TokenIdentifier) {
		return nil, fmt.Errorf("expected function name at line %d", p.peek().Line)
	}

	name := p.advance().Value

	if !p.match(lexer.TokenLeftParen) {
		return nil, fmt.Errorf("expected '(' after function name at line %d", p.peek().Line)
	}

	parameters := []ast.Parameter{}

	if !p.check(lexer.TokenRightParen) {
		for {
			if !p.check(lexer.TokenIdentifier) {
				return nil, fmt.Errorf("expected parameter name at line %d", p.peek().Line)
			}

			paramName := p.advance().Value

			if !p.match(lexer.TokenColon) {
				return nil, fmt.Errorf("expected ':' after parameter name at line %d", p.peek().Line)
			}

			if !p.check(lexer.TokenTypeInt) && !p.check(lexer.TokenTypeFloat) &&
				!p.check(lexer.TokenTypeString) && !p.check(lexer.TokenTypeBool) &&
				!p.check(lexer.TokenIdentifier) {
				return nil, fmt.Errorf("expected type after ':' at line %d", p.peek().Line)
			}

			paramType := p.advance().Value

			parameters = append(parameters, ast.Parameter{
				Name: paramName,
				Type: paramType,
			})

			if !p.match(lexer.TokenComma) {
				break
			}
		}
	}

	if !p.match(lexer.TokenRightParen) {
		return nil, fmt.Errorf("expected ')' after parameters at line %d", p.peek().Line)
	}

	returnType := ""
	if p.match(lexer.TokenColon) {
		if !p.check(lexer.TokenTypeInt) && !p.check(lexer.TokenTypeFloat) &&
			!p.check(lexer.TokenTypeString) && !p.check(lexer.TokenTypeBool) &&
			!p.check(lexer.TokenTypeVoid) &&
			!p.check(lexer.TokenIdentifier) {
			return nil, fmt.Errorf("expected return type after ':' at line %d", p.peek().Line)
		}
		returnType = p.advance().Value
	}

	if !p.match(lexer.TokenLeftBrace) {
		return nil, fmt.Errorf("expected '{' for function body at line %d", p.peek().Line)
	}

	fn := &ast.FunctionDeclaration{
		Name:       name,
		Parameters: parameters,
		ReturnType: returnType,
	}

	prevFunc := p.currentFunc
	p.currentFunc = fn

	body, err := p.block()
	if err != nil {
		return nil, err
	}

	fn.Body = body
	p.currentFunc = prevFunc

	return fn, nil
}

func (p *Parser) variableDeclaration(isConst bool) (ast.Declaration, error) {
	pos := p.peek().Position

	if !p.check(lexer.TokenIdentifier) {
		return nil, fmt.Errorf("expected variable name at line %d", p.peek().Line)
	}

	name := p.advance().Value
	typeName := ""

	if p.match(lexer.TokenColon) {
		if !p.check(lexer.TokenTypeInt) && !p.check(lexer.TokenTypeFloat) &&
			!p.check(lexer.TokenTypeString) && !p.check(lexer.TokenTypeBool) &&
			!p.check(lexer.TokenIdentifier) {
			return nil, fmt.Errorf("expected type after ':' at line %d", p.peek().Line)
		}
		typeName = p.advance().Value
	}

	var value ast.Expression
	if p.match(lexer.TokenAssign) {
		var err error
		value, err = p.expression()
		if err != nil {
			return nil, err
		}
	} else if isConst {
		return nil, fmt.Errorf("const declaration must have initializer at line %d", p.peek().Line)
	}

	if p.match(lexer.TokenSemicolon) {
	}

	return &ast.VariableDeclaration{
		Name:     name,
		Type:     typeName,
		Value:    value,
		IsConst:  isConst,
		Position: pos,
	}, nil
}

func (p *Parser) typeDefinition() (ast.Declaration, error) {
	pos := p.peek().Position

	if !p.check(lexer.TokenIdentifier) {
		return nil, fmt.Errorf("expected type name at line %d", p.peek().Line)
	}

	name := p.advance().Value

	if !p.match(lexer.TokenLeftBrace) {
		return nil, fmt.Errorf("expected '{' after type name at line %d", p.peek().Line)
	}

	fields := []ast.TypeField{}

	if !p.check(lexer.TokenRightBrace) {
		for {
			if !p.check(lexer.TokenIdentifier) {
				return nil, fmt.Errorf("expected field name at line %d", p.peek().Line)
			}

			fieldName := p.advance().Value

			if !p.match(lexer.TokenColon) {
				return nil, fmt.Errorf("expected ':' after field name at line %d", p.peek().Line)
			}

			if !p.check(lexer.TokenTypeInt) && !p.check(lexer.TokenTypeFloat) &&
				!p.check(lexer.TokenTypeString) && !p.check(lexer.TokenTypeBool) &&
				!p.check(lexer.TokenIdentifier) {
				return nil, fmt.Errorf("expected type after ':' at line %d", p.peek().Line)
			}

			fieldType := p.advance().Value

			fields = append(fields, ast.TypeField{
				Name: fieldName,
				Type: fieldType,
			})

			if p.match(lexer.TokenComma) {
				continue
			} else {
				break
			}
		}
	}

	if !p.match(lexer.TokenRightBrace) {
		return nil, fmt.Errorf("expected '}' after fields at line %d", p.peek().Line)
	}

	return &ast.TypeDefinition{
		Name:     name,
		Fields:   fields,
		Position: pos,
	}, nil
}

func (p *Parser) classDeclaration() (ast.Declaration, error) {
	pos := p.peek().Position

	if !p.check(lexer.TokenIdentifier) {
		return nil, fmt.Errorf("expected class name at line %d", p.peek().Line)
	}

	name := p.advance().Value

	if !p.match(lexer.TokenLeftBrace) {
		return nil, fmt.Errorf("expected '{' after class name at line %d", p.peek().Line)
	}

	methods := []*ast.FunctionDeclaration{}

	for !p.check(lexer.TokenRightBrace) && !p.isAtEnd() {
		if !p.match(lexer.TokenFun) {
			return nil, fmt.Errorf("expected function in class body at line %d", p.peek().Line)
		}

		method, err := p.functionDeclaration()
		if err != nil {
			return nil, err
		}

		if fnDecl, ok := method.(*ast.FunctionDeclaration); ok {
			methods = append(methods, fnDecl)
		}
	}

	if !p.match(lexer.TokenRightBrace) {
		return nil, fmt.Errorf("expected '}' after class body at line %d", p.peek().Line)
	}

	return &ast.ClassDeclaration{
		Name:     name,
		Methods:  methods,
		Position: pos,
	}, nil
}
