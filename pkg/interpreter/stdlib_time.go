package interpreter

import (
	"fmt"
	"time"

	"github.com/burnlang/burn/pkg/ast"
)


func (i *Interpreter) registerTimeLibrary() {
	timeClass := NewClass("Time")

	timeClass.AddStatic("now", &ast.FunctionDeclaration{
		Name:       "now",
		Parameters: []ast.Parameter{},
		ReturnType: "time",
	})

	timeClass.AddStatic("sleep", &ast.FunctionDeclaration{
		Name: "sleep",
		Parameters: []ast.Parameter{
			{Name: "ms", Type: "int"},
		},
		ReturnType: "nil",
	})

	timeClass.AddStatic("timestamp", &ast.FunctionDeclaration{
		Name:       "timestamp",
		Parameters: []ast.Parameter{},
		ReturnType: "int",
	})

	timeClass.AddStatic("format", &ast.FunctionDeclaration{
		Name: "format",
		Parameters: []ast.Parameter{
			{Name: "format", Type: "string"},
		},
		ReturnType: "string",
	})

	i.classes["Time"] = timeClass
	i.environment["Time"] = timeClass

	i.environment["Time.now"] = &BuiltinFunction{
		Name: "Time.now",
		Fn: func(args []Value) (Value, error) {
			return time.Now().Format(time.RFC3339), nil
		},
	}

	i.environment["Time.sleep"] = &BuiltinFunction{
		Name: "Time.sleep",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("Time.sleep expects exactly one numeric argument (milliseconds)")
			}

			ms, ok := args[0].(float64)
			if !ok {
				return nil, fmt.Errorf("Time.sleep expects a numeric value")
			}

			time.Sleep(time.Duration(ms) * time.Millisecond)
			return nil, nil
		},
	}

	i.environment["Time.timestamp"] = &BuiltinFunction{
		Name: "Time.timestamp",
		Fn: func(args []Value) (Value, error) {
			return float64(time.Now().Unix()), nil
		},
	}

	i.environment["Time.format"] = &BuiltinFunction{
		Name: "Time.format",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("Time.format expects exactly one string argument")
			}

			format, ok := args[0].(string)
			if !ok {
				return nil, fmt.Errorf("Time.format expects a string argument")
			}

			return time.Now().Format(format), nil
		},
	}

	
	i.environment["now"] = i.environment["Time.now"]
	i.environment["sleep"] = i.environment["Time.sleep"]
	i.environment["timestamp"] = i.environment["Time.timestamp"]
	i.environment["format"] = i.environment["Time.format"]
}
