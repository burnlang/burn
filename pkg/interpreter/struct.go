package interpreter

import (
	"fmt"

	"github.com/burnlang/burn/pkg/ast"
)

type Struct struct {
	TypeName string
	Fields   map[string]interface{}
}

func (s *Struct) GetField(name string) (interface{}, bool) {
	value, exists := s.Fields[name]
	return value, exists
}

func (s *Struct) SetField(name string, value interface{}) {
	s.Fields[name] = value
}

func (s *Struct) HasField(name string) bool {
	_, exists := s.Fields[name]
	return exists
}

func (i *Interpreter) evalExpression(expr ast.Expression) (interface{}, error) {
	return nil, fmt.Errorf("evalExpression not implemented for %T", expr)
}

func (i *Interpreter) evalStructLiteral(expr *ast.StructLiteralExpression) (interface{}, error) {
	fields := make(map[string]interface{})

	for name, valueExpr := range expr.Fields {
		value, err := i.evalExpression(valueExpr)
		if err != nil {
			return nil, err
		}
		fields[name] = value
	}

	return &Struct{
		TypeName: expr.Type,
		Fields:   fields,
	}, nil
}

func (i *Interpreter) evalGetExpression(expr *ast.GetExpression) (interface{}, error) {
	object, err := i.evalExpression(expr.Object)
	if err != nil {
		return nil, err
	}

	structObj, ok := object.(*Struct)
	if !ok {
		i.setErrorPos(0)
		return nil, fmt.Errorf("cannot access field on non-struct value: %T", object)
	}

	value, exists := structObj.Fields[expr.Name]
	if !exists {
		i.setErrorPos(0)
		return nil, fmt.Errorf("undefined field '%s' on struct of type '%s'",
			expr.Name, structObj.TypeName)
	}

	return value, nil
}

func (i *Interpreter) evalSetExpression(expr *ast.SetExpression) (interface{}, error) {
	object, err := i.evalExpression(expr.Object)
	if err != nil {
		return nil, err
	}

	structObj, ok := object.(*Struct)
	if !ok {
		i.setErrorPos(0)
		return nil, fmt.Errorf("cannot set field on non-struct value: %T", object)
	}

	value, err := i.evalExpression(expr.Value)
	if err != nil {
		return nil, err
	}

	structObj.Fields[expr.Name] = value

	return value, nil
}
