package interpreter

import (
	"bufio"
	"fmt"
	"os"
	"strconv"
	"strings"
	"time"
)

type Value interface{}

type BuiltinFunction struct {
	Name string
	Fn   func(args []Value) (Value, error)
}

func (b *BuiltinFunction) Call(args []Value) (Value, error) {
	return b.Fn(args)
}

func (i *Interpreter) addBuiltins() {
	i.environment["print"] = &BuiltinFunction{
		Name: "print",
		Fn: func(args []Value) (Value, error) {
			for _, arg := range args {
				fmt.Println(arg)
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
				return nil, fmt.Errorf("toString expects exactly one argument")
			}

			switch val := args[0].(type) {
			case float64:
				if val == float64(int(val)) {
					return fmt.Sprintf("%.0f", val), nil
				}
				return fmt.Sprintf("%g", val), nil
			case int:
				return fmt.Sprintf("%d", val), nil
			case string:
				return val, nil
			case bool:
				return fmt.Sprintf("%t", val), nil
			case nil:
				return "null", nil
			default:
				return fmt.Sprintf("%v", val), nil
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

	i.environment["now"] = &BuiltinFunction{
		Name: "now",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 0 {
				return nil, fmt.Errorf("now expects no arguments")
			}
			currentTime := float64(time.Now().UnixNano()) / 1e9
			return currentTime, nil
		},
	}
	i.registerDateLibrary()
	i.registerHTTPLibrary()
	i.registerTimeLibrary()
}
