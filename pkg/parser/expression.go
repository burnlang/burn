package parser

import (
	"fmt"
	"strconv"

	"github.com/burnlang/burn/pkg/ast"
	"github.com/burnlang/burn/pkg/lexer"
)

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
