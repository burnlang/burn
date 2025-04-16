package cmd

import (
	"fmt"
	"io"
	"strings"

	"github.com/burnlang/burn/pkg/ast"
	"github.com/burnlang/burn/pkg/lexer"
)

// formattedError creates a nicely formatted error message with line and column information
func formattedError(errType string, err error, source string, pos int) error {
	errMsg := err.Error()

	if strings.Contains(errMsg, "at line") {
		return fmt.Errorf("%s: %v", errType, err)
	}

	if pos < 0 {
		pos = 0
	}
	if pos >= len(source) {
		pos = len(source) - 1
		if pos < 0 {
			pos = 0
		}
	}

	line, col := getLineAndCol(source, pos)
	return fmt.Errorf("%s at line %d, column %d: %v", errType, line, col, err)
}

// getLineAndCol calculates line and column numbers from a position in the source
func getLineAndCol(source string, pos int) (int, int) {
	lineStart := 0
	line := 1

	for i := 0; i < pos && i < len(source); i++ {
		if source[i] == '\n' {
			lineStart = i + 1
			line++
		}
	}

	column := pos - lineStart + 1
	return line, column
}

// tokenTypeToString converts a lexer token type to a human-readable string
func tokenTypeToString(tokenType lexer.TokenType) string {
	switch tokenType {
	case lexer.TokenIdentifier:
		return "IDENTIFIER"
	case lexer.TokenNumber:
		return "NUMBER"
	case lexer.TokenString:
		return "STRING"
	case lexer.TokenFun:
		return "FUN"
	case lexer.TokenVar:
		return "VAR"
	case lexer.TokenConst:
		return "CONST"
	case lexer.TokenDef:
		return "DEF"
	default:
		return fmt.Sprintf("TOKEN(%d)", int(tokenType))
	}
}

// printAST prints an AST node and its children with proper indentation
func printAST(node ast.Node, indent int, w io.Writer) {
	indentStr := strings.Repeat("  ", indent)

	switch n := node.(type) {
	case *ast.Program:
		fmt.Fprintln(w, indentStr+"Program")
		for _, decl := range n.Declarations {
			printAST(decl, indent+1, w)
		}
	case *ast.FunctionDeclaration:
		fmt.Fprintf(w, "%sFunctionDeclaration: %s\n", indentStr, n.Name)
		fmt.Fprintf(w, "%s  Parameters: ", indentStr)
		for i, param := range n.Parameters {
			if i > 0 {
				fmt.Fprint(w, ", ")
			}
			fmt.Fprintf(w, "%s: %s", param.Name, param.Type)
		}
		fmt.Fprintln(w)
		if n.ReturnType != "" {
			fmt.Fprintf(w, "%s  ReturnType: %s\n", indentStr, n.ReturnType)
		}
		fmt.Fprintf(w, "%s  Body:\n", indentStr)
		for _, stmt := range n.Body {
			printAST(stmt, indent+2, w)
		}
	case *ast.ClassDeclaration:
		fmt.Fprintf(w, "%sClassDeclaration: %s\n", indentStr, n.Name)
		fmt.Fprintf(w, "%s  Methods:\n", indentStr)
		for _, method := range n.Methods {
			printAST(method, indent+2, w)
		}
	case *ast.VariableDeclaration:
		fmt.Fprintf(w, "%sVariableDeclaration: %s", indentStr, n.Name)
		if n.Type != "" {
			fmt.Fprintf(w, " : %s", n.Type)
		}
		fmt.Fprintln(w)
		if n.Value != nil {
			printAST(n.Value, indent+1, w)
		}
	case *ast.ExpressionStatement:
		fmt.Fprintf(w, "%sExpressionStatement:\n", indentStr)
		printAST(n.Expression, indent+1, w)
	case *ast.CallExpression:
		fmt.Fprintf(w, "%sCallExpression: ", indentStr)
		printAST(n.Callee, indent, w)
		fmt.Fprintf(w, "%s  Arguments:\n", indentStr)
		for _, arg := range n.Arguments {
			printAST(arg, indent+2, w)
		}
	case *ast.ClassMethodCallExpression:
		fmt.Fprintf(w, "%sClassMethodCallExpression: %s.%s\n", indentStr, n.ClassName, n.MethodName)
		fmt.Fprintf(w, "%s  Arguments:\n", indentStr)
		for _, arg := range n.Arguments {
			printAST(arg, indent+2, w)
		}
	case *ast.LiteralExpression:
		fmt.Fprintf(w, "%sLiteral: %v (%T)\n", indentStr, n.Value, n.Value)
	case *ast.VariableExpression:
		fmt.Fprintf(w, "%sVariable: %s\n", indentStr, n.Name)
	default:
		fmt.Fprintf(w, "%sNode: %T\n", indentStr, node)
	}
}
