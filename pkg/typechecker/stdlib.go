package typechecker

func initStandardLibrary(tc *TypeChecker) {

	tc.functions["print"] = FunctionType{
		Parameters: []string{"any"},
		ReturnType: "",
	}

	tc.functions["toString"] = FunctionType{
		Parameters: []string{"any"},
		ReturnType: "string",
	}

	tc.functions["input"] = FunctionType{
		Parameters: []string{"string"},
		ReturnType: "string",
	}

	tc.functions["now"] = FunctionType{
		Parameters: []string{},
		ReturnType: "Date",
	}

	tc.functions["formatDate"] = FunctionType{
		Parameters: []string{"Date"},
		ReturnType: "string",
	}

	tc.functions["createDate"] = FunctionType{
		Parameters: []string{"int", "int", "int"},
		ReturnType: "Date",
	}

	tc.functions["currentYear"] = FunctionType{
		Parameters: []string{},
		ReturnType: "int",
	}

	tc.functions["currentMonth"] = FunctionType{
		Parameters: []string{},
		ReturnType: "int",
	}

	tc.functions["currentDay"] = FunctionType{
		Parameters: []string{},
		ReturnType: "int",
	}

	tc.functions["isLeapYear"] = FunctionType{
		Parameters: []string{"int"},
		ReturnType: "bool",
	}

	tc.functions["daysInMonth"] = FunctionType{
		Parameters: []string{"int", "int"},
		ReturnType: "int",
	}

	tc.functions["dayOfWeek"] = FunctionType{
		Parameters: []string{"Date"},
		ReturnType: "int",
	}

	tc.functions["today"] = FunctionType{
		Parameters: []string{},
		ReturnType: "string",
	}

	tc.functions["addDays"] = FunctionType{
		Parameters: []string{"Date", "int"},
		ReturnType: "Date",
	}

	tc.functions["subtractDays"] = FunctionType{
		Parameters: []string{"Date", "int"},
		ReturnType: "Date",
	}

	tc.types["Date"] = map[string]string{
		"year":  "int",
		"month": "int",
		"day":   "int",
	}

	tc.types["array"] = map[string]string{}
	tc.types["any"] = map[string]string{}
	tc.types["void"] = map[string]string{}
	tc.types["Object"] = map[string]string{}

	tc.types["HTTPResponse"] = map[string]string{
		"statusCode": "int",
		"body":       "string",
		"headers":    "array",
	}

	tc.classes["HTTP"] = map[string]FunctionType{
		"get": {
			Parameters: []string{"string"},
			ReturnType: "HTTPResponse",
		},
		"post": {
			Parameters: []string{"string", "string"},
			ReturnType: "HTTPResponse",
		},
		"put": {
			Parameters: []string{"string", "string"},
			ReturnType: "HTTPResponse",
		},
		"delete": {
			Parameters: []string{"string"},
			ReturnType: "HTTPResponse",
		},
		"setHeaders": {
			Parameters: []string{"array"},
			ReturnType: "bool",
		},
		"getHeader": {
			Parameters: []string{"HTTPResponse", "string"},
			ReturnType: "string",
		},
		"parseJSON": {
			Parameters: []string{"string"},
			ReturnType: "any",
		},
	}
}
