package interpreter

import (
	"fmt"
	"time"

	"github.com/burnlang/burn/pkg/ast"
)

func (i *Interpreter) registerDateLibrary() {

	i.types["Date"] = &ast.TypeDefinition{
		Name: "Date",
		Fields: []ast.TypeField{
			{Name: "year", Type: "int"},
			{Name: "month", Type: "int"},
			{Name: "day", Type: "int"},
		},
	}

	dateClass := NewClass("Date")

	dateClass.AddStatic("now", &ast.FunctionDeclaration{
		Name:       "now",
		Parameters: []ast.Parameter{},
		ReturnType: "Date",
	})

	dateClass.AddStatic("formatDate", &ast.FunctionDeclaration{
		Name:       "formatDate",
		Parameters: []ast.Parameter{{Name: "date", Type: "Date"}},
		ReturnType: "string",
	})

	dateClass.AddStatic("currentYear", &ast.FunctionDeclaration{
		Name:       "currentYear",
		Parameters: []ast.Parameter{},
		ReturnType: "int",
	})

	dateClass.AddStatic("currentMonth", &ast.FunctionDeclaration{
		Name:       "currentMonth",
		Parameters: []ast.Parameter{},
		ReturnType: "int",
	})

	dateClass.AddStatic("currentDay", &ast.FunctionDeclaration{
		Name:       "currentDay",
		Parameters: []ast.Parameter{},
		ReturnType: "int",
	})

	dateClass.AddStatic("isLeapYear", &ast.FunctionDeclaration{
		Name:       "isLeapYear",
		Parameters: []ast.Parameter{{Name: "year", Type: "int"}},
		ReturnType: "bool",
	})

	dateClass.AddStatic("daysInMonth", &ast.FunctionDeclaration{
		Name:       "daysInMonth",
		Parameters: []ast.Parameter{{Name: "year", Type: "int"}, {Name: "month", Type: "int"}},
		ReturnType: "int",
	})

	dateClass.AddStatic("createDate", &ast.FunctionDeclaration{
		Name:       "createDate",
		Parameters: []ast.Parameter{{Name: "year", Type: "int"}, {Name: "month", Type: "int"}, {Name: "day", Type: "int"}},
		ReturnType: "Date",
	})

	dateClass.AddStatic("dayOfWeek", &ast.FunctionDeclaration{
		Name:       "dayOfWeek",
		Parameters: []ast.Parameter{{Name: "date", Type: "Date"}},
		ReturnType: "int",
	})

	dateClass.AddStatic("addDays", &ast.FunctionDeclaration{
		Name:       "addDays",
		Parameters: []ast.Parameter{{Name: "date", Type: "Date"}, {Name: "days", Type: "int"}},
		ReturnType: "Date",
	})

	dateClass.AddStatic("subtractDays", &ast.FunctionDeclaration{
		Name:       "subtractDays",
		Parameters: []ast.Parameter{{Name: "date", Type: "Date"}, {Name: "days", Type: "int"}},
		ReturnType: "Date",
	})

	dateClass.AddStatic("today", &ast.FunctionDeclaration{
		Name:       "today",
		Parameters: []ast.Parameter{},
		ReturnType: "string",
	})

	i.classes["Date"] = dateClass
	i.environment["Date"] = dateClass

	i.environment["Date.now"] = &BuiltinFunction{
		Name: "Date.now",
		Fn: func(args []Value) (Value, error) {
			now := time.Now()
			return map[string]interface{}{
				"year":  float64(now.Year()),
				"month": float64(int(now.Month())),
				"day":   float64(now.Day()),
			}, nil
		},
	}

	i.environment["Date.formatDate"] = &BuiltinFunction{
		Name: "Date.formatDate",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("Date.formatDate expects exactly one Date argument")
			}

			dateMap, ok := args[0].(map[string]interface{})
			if !ok {
				return nil, fmt.Errorf("Date.formatDate expects a Date argument")
			}

			year, _ := dateMap["year"].(float64)
			month, _ := dateMap["month"].(float64)
			day, _ := dateMap["day"].(float64)

			return fmt.Sprintf("%04.0f-%02.0f-%02.0f", year, month, day), nil
		},
	}

	i.environment["Date.currentYear"] = &BuiltinFunction{
		Name: "Date.currentYear",
		Fn: func(args []Value) (Value, error) {
			return float64(time.Now().Year()), nil
		},
	}

	i.environment["Date.currentMonth"] = &BuiltinFunction{
		Name: "Date.currentMonth",
		Fn: func(args []Value) (Value, error) {
			return float64(int(time.Now().Month())), nil
		},
	}

	i.environment["Date.currentDay"] = &BuiltinFunction{
		Name: "Date.currentDay",
		Fn: func(args []Value) (Value, error) {
			return float64(time.Now().Day()), nil
		},
	}

	i.environment["Date.isLeapYear"] = &BuiltinFunction{
		Name: "Date.isLeapYear",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("Date.isLeapYear expects exactly one numeric argument (year)")
			}

			year, ok := args[0].(float64)
			if !ok {
				return nil, fmt.Errorf("Date.isLeapYear expects a numeric year")
			}

			y := int(year)
			return (y%4 == 0 && y%100 != 0) || (y%400 == 0), nil
		},
	}

	i.environment["Date.daysInMonth"] = &BuiltinFunction{
		Name: "Date.daysInMonth",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 2 {
				return nil, fmt.Errorf("Date.daysInMonth expects exactly two numeric arguments (year, month)")
			}

			year, ok1 := args[0].(float64)
			month, ok2 := args[1].(float64)
			if !ok1 || !ok2 {
				return nil, fmt.Errorf("Date.daysInMonth expects numeric year and month")
			}

			t := time.Date(int(year), time.Month(int(month)), 32, 0, 0, 0, 0, time.UTC)
			return float64(32 - t.Day()), nil
		},
	}

	i.environment["Date.createDate"] = &BuiltinFunction{
		Name: "Date.createDate",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 3 {
				return nil, fmt.Errorf("Date.createDate expects exactly three numeric arguments (year, month, day)")
			}

			year, ok1 := args[0].(float64)
			month, ok2 := args[1].(float64)
			day, ok3 := args[2].(float64)
			if !ok1 || !ok2 || !ok3 {
				return nil, fmt.Errorf("Date.createDate expects numeric year, month, and day")
			}

			return map[string]interface{}{
				"year":  year,
				"month": month,
				"day":   day,
			}, nil
		},
	}

	i.environment["Date.dayOfWeek"] = &BuiltinFunction{
		Name: "Date.dayOfWeek",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("Date.dayOfWeek expects exactly one Date argument")
			}

			dateMap, ok := args[0].(map[string]interface{})
			if !ok {
				return nil, fmt.Errorf("Date.dayOfWeek expects a Date argument")
			}

			year, _ := dateMap["year"].(float64)
			month, _ := dateMap["month"].(float64)
			day, _ := dateMap["day"].(float64)

			t := time.Date(int(year), time.Month(int(month)), int(day), 0, 0, 0, 0, time.UTC)
			return float64((int(t.Weekday()) + 1) % 7), nil
		},
	}

	i.environment["Date.addDays"] = &BuiltinFunction{
		Name: "Date.addDays",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 2 {
				return nil, fmt.Errorf("Date.addDays expects exactly two arguments (date, days)")
			}

			dateMap, ok := args[0].(map[string]interface{})
			if !ok {
				return nil, fmt.Errorf("Date.addDays expects a Date as first argument")
			}

			days, ok := args[1].(float64)
			if !ok {
				return nil, fmt.Errorf("Date.addDays expects a numeric days value")
			}

			year, _ := dateMap["year"].(float64)
			month, _ := dateMap["month"].(float64)
			day, _ := dateMap["day"].(float64)

			t := time.Date(int(year), time.Month(int(month)), int(day), 0, 0, 0, 0, time.UTC)
			newTime := t.AddDate(0, 0, int(days))

			return map[string]interface{}{
				"year":  float64(newTime.Year()),
				"month": float64(int(newTime.Month())),
				"day":   float64(newTime.Day()),
			}, nil
		},
	}

	i.environment["Date.subtractDays"] = &BuiltinFunction{
		Name: "Date.subtractDays",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 2 {
				return nil, fmt.Errorf("Date.subtractDays expects exactly two arguments (date, days)")
			}

			dateMap, ok := args[0].(map[string]interface{})
			if !ok {
				return nil, fmt.Errorf("Date.subtractDays expects a Date as first argument")
			}

			days, ok := args[1].(float64)
			if !ok {
				return nil, fmt.Errorf("Date.subtractDays expects a numeric days value")
			}

			year, _ := dateMap["year"].(float64)
			month, _ := dateMap["month"].(float64)
			day, _ := dateMap["day"].(float64)

			t := time.Date(int(year), time.Month(int(month)), int(day), 0, 0, 0, 0, time.UTC)
			newTime := t.AddDate(0, 0, -int(days))

			return map[string]interface{}{
				"year":  float64(newTime.Year()),
				"month": float64(int(newTime.Month())),
				"day":   float64(newTime.Day()),
			}, nil
		},
	}

	i.environment["Date.today"] = &BuiltinFunction{
		Name: "Date.today",
		Fn: func(args []Value) (Value, error) {
			now := time.Now()
			return fmt.Sprintf("%04d-%02d-%02d", now.Year(), int(now.Month()), now.Day()), nil
		},
	}

	aliases := map[string]string{
		"now":          "Date.now",
		"formatDate":   "Date.formatDate",
		"currentYear":  "Date.currentYear",
		"currentMonth": "Date.currentMonth",
		"currentDay":   "Date.currentDay",
		"isLeapYear":   "Date.isLeapYear",
		"daysInMonth":  "Date.daysInMonth",
		"createDate":   "Date.createDate",
		"dayOfWeek":    "Date.dayOfWeek",
		"addDays":      "Date.addDays",
		"subtractDays": "Date.subtractDays",
		"today":        "Date.today",
	}

	for oldName, newName := range aliases {
		i.environment[oldName] = i.environment[newName]
	}
}
