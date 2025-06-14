package interpreter

import (
	"bufio"
	"fmt"
	"os"
	"strconv"
	"strings"
)

type Value interface{}

type BuiltinFunction struct {
	Name   string
	Fn     func(args []Value) (Value, error)
	Caller func(args []Value) (Value, error)
}

func (b *BuiltinFunction) Call(args []Value) (Value, error) {
	return b.Fn(args)
}

func (i *Interpreter) addBuiltins() {

	i.environment["print"] = &BuiltinFunction{
		Name: "print",
		Fn: func(args []Value) (Value, error) {
			var output string
			for _, arg := range args {
				output += fmt.Sprintf("%v ", arg)
			}
			if i.stdout != nil {
				fmt.Fprintln(i.stdout, output)
			} else {
				fmt.Println(output)
			}
			return nil, nil
		},
	}

	i.environment["input"] = &BuiltinFunction{
		Name: "input",
		Fn: func(args []Value) (Value, error) {
			if len(args) > 0 {
				fmt.Print(args[0])
			}
			reader := bufio.NewReader(os.Stdin)
			text, err := reader.ReadString('\n')
			if err != nil {
				return "", err
			}
			return strings.TrimSpace(text), nil
		},
	}

	i.environment["toString"] = &BuiltinFunction{
		Name: "toString",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("toString expects 1 argument, got %d", len(args))
			}

			arg := args[0]
			switch v := arg.(type) {
			case string:
				return v, nil
			case int:
				return fmt.Sprintf("%d", v), nil
			case float64:
				if v == float64(int(v)) {
					return fmt.Sprintf("%.0f", v), nil
				}
				return fmt.Sprintf("%g", v), nil
			case bool:
				return fmt.Sprintf("%t", v), nil
			case nil:
				return "null", nil
			default:
				return fmt.Sprintf("%v", v), nil
			}
		},
	}

	i.environment["toInt"] = &BuiltinFunction{
		Name: "toInt",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("toInt expects exactly one argument")
			}

			switch val := args[0].(type) {
			case float64:
				return float64(int(val)), nil
			case string:
				intVal, err := strconv.Atoi(val)
				if err != nil {
					return nil, fmt.Errorf("cannot convert string to int: %v", err)
				}
				return float64(intVal), nil
			default:
				return nil, fmt.Errorf("cannot convert %T to int", val)
			}
		},
	}

	i.environment["toFloat"] = &BuiltinFunction{
		Name: "toFloat",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("toFloat expects exactly one argument")
			}

			switch val := args[0].(type) {
			case float64:
				return val, nil
			case string:
				floatVal, err := strconv.ParseFloat(val, 64)
				if err != nil {
					return nil, fmt.Errorf("cannot convert string to float: %v", err)
				}
				return floatVal, nil
			default:
				return nil, fmt.Errorf("cannot convert %T to float", val)
			}
		},
	}

	i.environment["len"] = &BuiltinFunction{
		Name: "len",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("len expects exactly one argument")
			}

			switch val := args[0].(type) {
			case string:
				return float64(len(val)), nil
			case []Value:
				return float64(len(val)), nil
			default:
				return nil, fmt.Errorf("len expects string or array, got %T", val)
			}
		},
	}

	i.registerDateLibrary()
	i.registerHTTPLibrary()
	i.registerTimeLibrary()
}
