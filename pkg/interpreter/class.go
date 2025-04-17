package interpreter

import (
	"fmt"

	"github.com/burnlang/burn/pkg/ast"
)

type Class struct {
	Name       string
	Methods    map[string]*ast.FunctionDeclaration
	Statics    map[string]*ast.FunctionDeclaration
	Fields     []ast.TypeField
	Interfaces []string
}

func NewClass(name string) *Class {
	return &Class{
		Name:       name,
		Methods:    make(map[string]*ast.FunctionDeclaration),
		Statics:    make(map[string]*ast.FunctionDeclaration),
		Fields:     []ast.TypeField{},
		Interfaces: []string{},
	}
}

func (c *Class) AddMethod(name string, fn *ast.FunctionDeclaration) {
	c.Methods[name] = fn
}

func (c *Class) AddStatic(name string, fn *ast.FunctionDeclaration) {
	c.Statics[name] = fn
}

func (c *Class) AddField(name string, typeName string) {
	c.Fields = append(c.Fields, ast.TypeField{
		Name: name,
		Type: typeName,
	})
}

func (c *Class) ImplementsInterface(name string) {
	c.Interfaces = append(c.Interfaces, name)
}

func (c *Class) Call(methodName string, interpreter *Interpreter, args []Value) (Value, error) {
	if method, exists := c.Methods[methodName]; exists {
		return interpreter.executeFunction(method, args)
	}

	if static, exists := c.Statics[methodName]; exists {
		return interpreter.executeFunction(static, args)
	}

	builtinMethodName := fmt.Sprintf("%s.%s", c.Name, methodName)
	if builtinFunc, exists := interpreter.environment[builtinMethodName]; exists {
		if bf, ok := builtinFunc.(*BuiltinFunction); ok {
			return bf.Call(args)
		}
	}

	return nil, fmt.Errorf("undefined method '%s' in class '%s'", methodName, c.Name)
}

func (c *Class) CallStatic(methodName string, interpreter *Interpreter, args []Value) (Value, error) {
	if static, exists := c.Statics[methodName]; exists {
		return interpreter.executeFunction(static, args)
	}

	builtinFuncName := fmt.Sprintf("%s.%s", c.Name, methodName)
	if builtinFunc, exists := interpreter.environment[builtinFuncName]; exists {
		if bf, ok := builtinFunc.(*BuiltinFunction); ok {
			return bf.Call(args)
		}
	}

	return nil, fmt.Errorf("undefined static method '%s' in class '%s'", methodName, c.Name)
}

func (c *Class) ToTypeDefinition() *ast.TypeDefinition {
	return &ast.TypeDefinition{
		Name:   c.Name,
		Fields: c.Fields,
	}
}

func TypeDefinitionToClass(typeDef *ast.TypeDefinition) *Class {
	class := NewClass(typeDef.Name)
	class.Fields = typeDef.Fields
	return class
}
