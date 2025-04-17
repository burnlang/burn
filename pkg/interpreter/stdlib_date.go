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

	dateClass.AddStatic("today", &ast.FunctionDeclaration{
		Name:       "today",
		Parameters: []ast.Parameter{},
		ReturnType: "string",
	})

	dateClass.AddStatic("formatDate", &ast.FunctionDeclaration{
		Name: "formatDate",
		Parameters: []ast.Parameter{
			{Name: "date", Type: "Date"},
		},
		ReturnType: "string",
	})

	dateClass.AddStatic("createDate", &ast.FunctionDeclaration{
		Name: "createDate",
		Parameters: []ast.Parameter{
			{Name: "year", Type: "int"},
			{Name: "month", Type: "int"},
			{Name: "day", Type: "int"},
		},
		ReturnType: "Date",
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
		Name: "isLeapYear",
		Parameters: []ast.Parameter{
			{Name: "year", Type: "int"},
		},
		ReturnType: "bool",
	})

	dateClass.AddStatic("daysInMonth", &ast.FunctionDeclaration{
		Name: "daysInMonth",
		Parameters: []ast.Parameter{
			{Name: "year", Type: "int"},
			{Name: "month", Type: "int"},
		},
		ReturnType: "int",
	})

	dateClass.AddStatic("dayOfWeek", &ast.FunctionDeclaration{
		Name: "dayOfWeek",
		Parameters: []ast.Parameter{
			{Name: "date", Type: "Date"},
		},
		ReturnType: "int",
	})

	dateClass.AddStatic("addDays", &ast.FunctionDeclaration{
		Name: "addDays",
		Parameters: []ast.Parameter{
			{Name: "date", Type: "Date"},
			{Name: "days", Type: "int"},
		},
		ReturnType: "Date",
	})

	dateClass.AddStatic("subtractDays", &ast.FunctionDeclaration{
		Name: "subtractDays",
		Parameters: []ast.Parameter{
			{Name: "date", Type: "Date"},
			{Name: "days", Type: "int"},
		},
		ReturnType: "Date",
	})

	
	i.classes["Date"] = dateClass
	i.environment["Date"] = dateClass

	

	i.environment["Date.now"] = &BuiltinFunction{
		Name: "Date.now",
		Fn: func(args []Value) (Value, error) {
			currentTime := time.Now()
			return &Struct{
				TypeName: "Date",
				Fields: map[string]interface{}{
					"year":  currentTime.Year(),
					"month": int(currentTime.Month()),
					"day":   currentTime.Day(),
				},
			}, nil
		},
	}

	i.environment["Date.today"] = &BuiltinFunction{
		Name: "Date.today",
		Fn: func(args []Value) (Value, error) {
			currentTime := time.Now()
			year := currentTime.Year()
			month := int(currentTime.Month())
			day := currentTime.Day()
			monthStr := fmt.Sprintf("%02d", month)
			dayStr := fmt.Sprintf("%02d", day)
			return fmt.Sprintf("%d-%s-%s", year, monthStr, dayStr), nil
		},
	}

	i.environment["Date.formatDate"] = &BuiltinFunction{
		Name: "Date.formatDate",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("Date.formatDate expects exactly one Date argument")
			}
			dateStruct, ok := args[0].(*Struct)
			if !ok || dateStruct.TypeName != "Date" {
				return nil, fmt.Errorf("Date.formatDate expects a Date struct")
			}
			year, _ := dateStruct.Fields["year"].(int)
			month, _ := dateStruct.Fields["month"].(int)
			day, _ := dateStruct.Fields["day"].(int)
			monthStr := fmt.Sprintf("%02d", month)
			dayStr := fmt.Sprintf("%02d", day)
			return fmt.Sprintf("%d-%s-%s", year, monthStr, dayStr), nil
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
				return nil, fmt.Errorf("Date.isLeapYear expects exactly one integer argument")
			}
			yearFloat, ok := args[0].(float64)
			if !ok {
				return nil, fmt.Errorf("Date.isLeapYear expects an integer")
			}
			year := int(yearFloat)
			isLeap := false
			if year%400 == 0 {
				isLeap = true
			} else if year%100 == 0 {
				isLeap = false
			} else if year%4 == 0 {
				isLeap = true
			}
			return isLeap, nil
		},
	}

	i.environment["Date.daysInMonth"] = &BuiltinFunction{
		Name: "Date.daysInMonth",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 2 {
				return nil, fmt.Errorf("Date.daysInMonth expects exactly two integer arguments")
			}
			yearFloat, ok := args[0].(float64)
			if !ok {
				return nil, fmt.Errorf("Date.daysInMonth expects year as an integer")
			}
			monthFloat, ok := args[1].(float64)
			if !ok {
				return nil, fmt.Errorf("Date.daysInMonth expects month as an integer")
			}
			year := int(yearFloat)
			month := int(monthFloat)
			daysInMonth := 31
			if month == 4 || month == 6 || month == 9 || month == 11 {
				daysInMonth = 30
			} else if month == 2 {
				isLeap := false
				if year%400 == 0 {
					isLeap = true
				} else if year%100 == 0 {
					isLeap = false
				} else if year%4 == 0 {
					isLeap = true
				}
				if isLeap {
					daysInMonth = 29
				} else {
					daysInMonth = 28
				}
			}
			return float64(daysInMonth), nil
		},
	}

	i.environment["Date.createDate"] = &BuiltinFunction{
		Name: "Date.createDate",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 3 {
				return nil, fmt.Errorf("Date.createDate expects exactly three integer arguments")
			}
			yearFloat, ok := args[0].(float64)
			if !ok {
				return nil, fmt.Errorf("Date.createDate expects year as an integer")
			}
			monthFloat, ok := args[1].(float64)
			if !ok {
				return nil, fmt.Errorf("Date.createDate expects month as an integer")
			}
			dayFloat, ok := args[2].(float64)
			if !ok {
				return nil, fmt.Errorf("Date.createDate expects day as an integer")
			}
			dateStruct := &Struct{
				TypeName: "Date",
				Fields: map[string]interface{}{
					"year":  int(yearFloat),
					"month": int(monthFloat),
					"day":   int(dayFloat),
				},
			}
			return dateStruct, nil
		},
	}

	i.environment["Date.dayOfWeek"] = &BuiltinFunction{
		Name: "Date.dayOfWeek",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 1 {
				return nil, fmt.Errorf("Date.dayOfWeek expects exactly one Date argument")
			}
			dateStruct, ok := args[0].(*Struct)
			if !ok || dateStruct.TypeName != "Date" {
				return nil, fmt.Errorf("Date.dayOfWeek expects a Date struct")
			}
			year, _ := dateStruct.Fields["year"].(int)
			month, _ := dateStruct.Fields["month"].(int)
			day, _ := dateStruct.Fields["day"].(int)
			if month < 3 {
				month += 12
				year--
			}
			k := year % 100
			j := year / 100
			h := (day + ((13 * (month + 1)) / 5) + k + (k / 4) + (j / 4) - (2 * j)) % 7
			if h < 0 {
				h += 7
			}
			return float64(h), nil
		},
	}

	i.environment["Date.addDays"] = &BuiltinFunction{
		Name: "Date.addDays",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 2 {
				return nil, fmt.Errorf("Date.addDays expects exactly two arguments: a Date and an integer")
			}
			dateStruct, ok := args[0].(*Struct)
			if !ok || dateStruct.TypeName != "Date" {
				return nil, fmt.Errorf("Date.addDays expects a Date struct as first argument")
			}
			daysFloat, ok := args[1].(float64)
			if !ok {
				return nil, fmt.Errorf("Date.addDays expects an integer as second argument")
			}
			year, _ := dateStruct.Fields["year"].(int)
			month, _ := dateStruct.Fields["month"].(int)
			day, _ := dateStruct.Fields["day"].(int)
			t := time.Date(year, time.Month(month), day, 0, 0, 0, 0, time.UTC)
			newTime := t.AddDate(0, 0, int(daysFloat))
			newDateStruct := &Struct{
				TypeName: "Date",
				Fields: map[string]interface{}{
					"year":  newTime.Year(),
					"month": int(newTime.Month()),
					"day":   newTime.Day(),
				},
			}
			return newDateStruct, nil
		},
	}

	i.environment["Date.subtractDays"] = &BuiltinFunction{
		Name: "Date.subtractDays",
		Fn: func(args []Value) (Value, error) {
			if len(args) != 2 {
				return nil, fmt.Errorf("Date.subtractDays expects exactly two arguments: a Date and an integer")
			}
			dateStruct, ok := args[0].(*Struct)
			if !ok || dateStruct.TypeName != "Date" {
				return nil, fmt.Errorf("Date.subtractDays expects a Date struct as first argument")
			}
			daysFloat, ok := args[1].(float64)
			if !ok {
				return nil, fmt.Errorf("Date.subtractDays expects an integer as second argument")
			}
			year, _ := dateStruct.Fields["year"].(int)
			month, _ := dateStruct.Fields["month"].(int)
			day, _ := dateStruct.Fields["day"].(int)
			t := time.Date(year, time.Month(month), day, 0, 0, 0, 0, time.UTC)
			newTime := t.AddDate(0, 0, -int(daysFloat))
			newDateStruct := &Struct{
				TypeName: "Date",
				Fields: map[string]interface{}{
					"year":  newTime.Year(),
					"month": int(newTime.Month()),
					"day":   newTime.Day(),
				},
			}
			return newDateStruct, nil
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
