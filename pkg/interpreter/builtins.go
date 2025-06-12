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
			if len(args) == 0 {
				fmt.Println()
				os.Stdout.Sync()
				return nil, nil
			}

			var output strings.Builder
			for j, arg := range args {
				if j > 0 {
					output.WriteString(" ")
				}

				switch v := arg.(type) {
				case string:
					output.WriteString(v)
				case int:
					output.WriteString(strconv.Itoa(v))
				case float64:
					if v == float64(int(v)) {
						output.WriteString(fmt.Sprintf("%.0f", v))
					} else {
						output.WriteString(fmt.Sprintf("%g", v))
					}
				case bool:
					output.WriteString(strconv.FormatBool(v))
				case nil:
					output.WriteString("null")
				default:
					output.WriteString(fmt.Sprintf("%v", v))
				}
			}

			fmt.Print(output.String() + "\n")
			os.Stdout.Sync()
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

	i.registerDateLibrary()
	i.registerHTTPLibrary()
	i.registerTimeLibrary()
}
