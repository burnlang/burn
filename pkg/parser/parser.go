package parser

import (
	"fmt"
	"strconv"
	"strings"

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
				Name:     varExpr.Name,
				Value:    value,
				Position: varExpr.Position,
			}, nil
		} else if getExpr, ok := expr.(*ast.GetExpression); ok {
			return &ast.SetExpression{
				Object:   getExpr.Object,
				Name:     getExpr.Name,
				Value:    value,
				Position: getExpr.Position,
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
			Position: p.previous().Position,
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
			Position: p.previous().Position,
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
		opPos := p.previous().Position

		right, err := p.comparison()
		if err != nil {
			return nil, err
		}

		expr = &ast.BinaryExpression{
			Left:     expr,
			Operator: operator,
			Right:    right,
			Position: opPos,
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
			Position: p.previous().Position,
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
			Position: p.previous().Position,
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
			Position: p.previous().Position,
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
			Position: p.previous().Position,
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
				Object:   expr,
				Name:     name,
				Position: p.previous().Position,
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
				Array:    expr,
				Index:    index,
				Position: p.previous().Position,
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
		Position:  p.previous().Position,
	}, nil
}

func (p *Parser) primary() (ast.Expression, error) {
	pos := p.peek().Position

	if p.match(lexer.TokenTrue) {
		return &ast.LiteralExpression{
			Value:    "true",
			Type:     "bool",
			Position: pos,
		}, nil
	}
	if p.match(lexer.TokenFalse) {
		return &ast.LiteralExpression{
			Value:    "false",
			Type:     "bool",
			Position: p.previous().Position,
		}, nil
	}
	if p.match(lexer.TokenNumber) {
		value := p.previous().Value
		if _, err := strconv.ParseFloat(value, 64); err != nil {
			return nil, fmt.Errorf("invalid number at line %d: %s", p.previous().Line, value)
		}
		return &ast.LiteralExpression{
			Value:    value,
			Type:     "number",
			Position: p.previous().Position,
		}, nil
	}
	if p.match(lexer.TokenString) {
		return &ast.LiteralExpression{
			Value:    p.previous().Value,
			Type:     "string",
			Position: p.previous().Position,
		}, nil
	}

	if p.match(lexer.TokenIdentifier) {
		return &ast.VariableExpression{
			Name:     p.previous().Value,
			Position: p.previous().Position,
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
			Type:     typeName,
			Fields:   fields,
			Position: p.previous().Position,
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
		Position: p.previous().Position,
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
