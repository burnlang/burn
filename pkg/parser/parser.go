package parser

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/s42yt/burn/pkg/ast"
	"github.com/s42yt/burn/pkg/lexer"
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

func (p *Parser) declaration() (ast.Declaration, error) {
	if p.match(lexer.TokenImport) {
		return p.importDeclaration()
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
		Name:    name,
		Type:    typeName,
		Value:   value,
		IsConst: isConst,
	}, nil
}

func (p *Parser) typeDefinition() (ast.Declaration, error) {
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
		Name:   name,
		Fields: fields,
	}, nil
}

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
	}, nil
}

func (p *Parser) whileStatement() (ast.Declaration, error) {
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
	}, nil
}

func (p *Parser) forStatement() (ast.Declaration, error) {
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
	}, nil
}

func (p *Parser) returnStatement() (ast.Declaration, error) {
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
		Value: value,
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
	expr, err := p.expression()
	if err != nil {
		return nil, err
	}

	if p.match(lexer.TokenSemicolon) {
	}

	return &ast.ExpressionStatement{
		Expression: expr,
	}, nil
}

func (p *Parser) expression() (ast.Expression, error) {
	return p.assignment()
}

func (p *Parser) assignment() (ast.Expression, error) {
	expr, err := p.logicalOr()
	if err != nil {
		return nil, err
	}

	if p.match(lexer.TokenAssign) {
		value, err := p.assignment()
		if err != nil {
			return nil, err
		}

		if varExpr, ok := expr.(*ast.VariableExpression); ok {
			return &ast.AssignmentExpression{
				Name:  varExpr.Name,
				Value: value,
			}, nil
		} else if getExpr, ok := expr.(*ast.GetExpression); ok {
			return &ast.SetExpression{
				Object: getExpr.Object,
				Name:   getExpr.Name,
				Value:  value,
			}, nil
		}

		return nil, fmt.Errorf("invalid assignment target at line %d", p.previous().Line)
	}

	return expr, nil
}

func (p *Parser) logicalOr() (ast.Expression, error) {
	expr, err := p.logicalAnd()
	if err != nil {
		return nil, err
	}

	for p.match(lexer.TokenOr) {
		operator := p.previous().Value
		right, err := p.logicalAnd()
		if err != nil {
			return nil, err
		}

		expr = &ast.BinaryExpression{
			Left:     expr,
			Operator: operator,
			Right:    right,
		}
	}

	return expr, nil
}

func (p *Parser) logicalAnd() (ast.Expression, error) {
	expr, err := p.equality()
	if err != nil {
		return nil, err
	}

	for p.match(lexer.TokenAnd) {
		operator := p.previous().Value
		right, err := p.equality()
		if err != nil {
			return nil, err
		}

		expr = &ast.BinaryExpression{
			Left:     expr,
			Operator: operator,
			Right:    right,
		}
	}

	return expr, nil
}

func (p *Parser) equality() (ast.Expression, error) {
	expr, err := p.comparison()
	if err != nil {
		return nil, err
	}

	for p.match(lexer.TokenEqual, lexer.TokenNotEqual) {
		operator := p.previous().Value
		right, err := p.comparison()
		if err != nil {
			return nil, err
		}

		expr = &ast.BinaryExpression{
			Left:     expr,
			Operator: operator,
			Right:    right,
		}
	}

	return expr, nil
}

func (p *Parser) comparison() (ast.Expression, error) {
	expr, err := p.term()
	if err != nil {
		return nil, err
	}

	for p.match(lexer.TokenLess, lexer.TokenGreater, lexer.TokenLessEqual, lexer.TokenGreaterEqual) {
		operator := p.previous().Value
		right, err := p.term()
		if err != nil {
			return nil, err
		}

		expr = &ast.BinaryExpression{
			Left:     expr,
			Operator: operator,
			Right:    right,
		}
	}

	return expr, nil
}

func (p *Parser) term() (ast.Expression, error) {
	expr, err := p.factor()
	if err != nil {
		return nil, err
	}

	for p.match(lexer.TokenPlus, lexer.TokenMinus) {
		operator := p.previous().Value
		right, err := p.factor()
		if err != nil {
			return nil, err
		}

		expr = &ast.BinaryExpression{
			Left:     expr,
			Operator: operator,
			Right:    right,
		}
	}

	return expr, nil
}

func (p *Parser) factor() (ast.Expression, error) {
	expr, err := p.unary()
	if err != nil {
		return nil, err
	}

	for p.match(lexer.TokenMultiply, lexer.TokenDivide, lexer.TokenModulo) {
		operator := p.previous().Value
		right, err := p.unary()
		if err != nil {
			return nil, err
		}

		expr = &ast.BinaryExpression{
			Left:     expr,
			Operator: operator,
			Right:    right,
		}
	}

	return expr, nil
}

func (p *Parser) unary() (ast.Expression, error) {
	if p.match(lexer.TokenMinus, lexer.TokenNot) {
		operator := p.previous().Value
		right, err := p.unary()
		if err != nil {
			return nil, err
		}

		return &ast.UnaryExpression{
			Operator: operator,
			Right:    right,
		}, nil
	}

	return p.call()
}

func (p *Parser) call() (ast.Expression, error) {
	expr, err := p.primary()
	if err != nil {
		return nil, err
	}

	for {
		if p.match(lexer.TokenLeftParen) {
			expr, err = p.finishCall(expr)
			if err != nil {
				return nil, err
			}
		} else if p.match(lexer.TokenDot) {
			if !p.check(lexer.TokenIdentifier) {
				return nil, fmt.Errorf("expected property name after '.' at line %d", p.peek().Line)
			}
			name := p.advance().Value
			expr = &ast.GetExpression{
				Object: expr,
				Name:   name,
			}
		} else if p.match(lexer.TokenLeftBracket) {
			index, err := p.expression()
			if err != nil {
				return nil, err
			}

			if !p.match(lexer.TokenRightBracket) {
				return nil, fmt.Errorf("expected ']' after array index at line %d", p.peek().Line)
			}

			expr = &ast.IndexExpression{
				Array: expr,
				Index: index,
			}
		} else {
			break
		}
	}

	return expr, nil
}

func (p *Parser) finishCall(callee ast.Expression) (ast.Expression, error) {
	arguments := []ast.Expression{}

	if !p.check(lexer.TokenRightParen) {
		for {
			expr, err := p.expression()
			if err != nil {
				return nil, err
			}
			arguments = append(arguments, expr)

			if !p.match(lexer.TokenComma) {
				break
			}
		}
	}

	if !p.match(lexer.TokenRightParen) {
		return nil, fmt.Errorf("expected ')' after arguments at line %d", p.peek().Line)
	}

	return &ast.CallExpression{
		Callee:    callee,
		Arguments: arguments,
	}, nil
}

func (p *Parser) primary() (ast.Expression, error) {
	if p.match(lexer.TokenTrue) {
		return &ast.LiteralExpression{
			Value: "true",
			Type:  "bool",
		}, nil
	}
	if p.match(lexer.TokenFalse) {
		return &ast.LiteralExpression{
			Value: "false",
			Type:  "bool",
		}, nil
	}
	if p.match(lexer.TokenNumber) {
		value := p.previous().Value
		if _, err := strconv.ParseFloat(value, 64); err != nil {
			return nil, fmt.Errorf("invalid number at line %d: %s", p.previous().Line, value)
		}
		return &ast.LiteralExpression{
			Value: value,
			Type:  "number",
		}, nil
	}
	if p.match(lexer.TokenString) {
		return &ast.LiteralExpression{
			Value: p.previous().Value,
			Type:  "string",
		}, nil
	}
	if p.match(lexer.TokenIdentifier) {
		return &ast.VariableExpression{
			Name: p.previous().Value,
		}, nil
	}
	if p.match(lexer.TokenLeftParen) {
		expr, err := p.expression()
		if err != nil {
			return nil, err
		}
		if !p.match(lexer.TokenRightParen) {
			return nil, fmt.Errorf("expected ')' after expression at line %d", p.peek().Line)
		}
		return expr, nil
	}
	if p.match(lexer.TokenLeftBrace) {
		var typeName string
		if p.currentFunc != nil && p.currentFunc.ReturnType != "" {
			typeName = p.currentFunc.ReturnType
		}

		fields := make(map[string]ast.Expression)
		if !p.check(lexer.TokenRightBrace) {
			for {
				if !p.check(lexer.TokenIdentifier) {
					return nil, fmt.Errorf("expected field name at line %d", p.peek().Line)
				}
				name := p.advance().Value
				if !p.match(lexer.TokenColon) {
					return nil, fmt.Errorf("expected ':' after field name at line %d", p.peek().Line)
				}
				value, err := p.expression()
				if err != nil {
					return nil, err
				}
				fields[name] = value
				if !p.match(lexer.TokenComma) {
					break
				}
			}
		}
		if !p.match(lexer.TokenRightBrace) {
			return nil, fmt.Errorf("expected '}' after struct literal at line %d", p.peek().Line)
		}

		return &ast.StructLiteralExpression{
			Type:   typeName,
			Fields: fields,
		}, nil
	}
	if p.match(lexer.TokenLeftBracket) {
		return p.arrayLiteral()
	}

	return nil, fmt.Errorf("expected expression at line %d", p.peek().Line)
}

func (p *Parser) arrayLiteral() (ast.Expression, error) {
	elements := []ast.Expression{}

	if !p.check(lexer.TokenRightBracket) {
		for {
			element, err := p.expression()
			if err != nil {
				return nil, err
			}
			elements = append(elements, element)

			if !p.match(lexer.TokenComma) {
				break
			}
		}
	}

	if !p.match(lexer.TokenRightBracket) {
		return nil, fmt.Errorf("expected ']' after array elements at line %d", p.peek().Line)
	}

	return &ast.ArrayLiteralExpression{
		Elements: elements,
	}, nil
}

func (p *Parser) Position() int {
	if p.current > 0 && p.current < len(p.tokens) {
		return p.tokens[p.current].Col + len(p.tokens[p.current].Value)
	} else if p.current > 0 {
		return p.tokens[p.current-1].Col + len(p.tokens[p.current-1].Value)
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
