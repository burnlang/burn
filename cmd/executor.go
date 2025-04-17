package cmd

import (
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/burnlang/burn/pkg/interpreter"
	"github.com/burnlang/burn/pkg/lexer"
	"github.com/burnlang/burn/pkg/parser"
	"github.com/burnlang/burn/pkg/typechecker"
)

// executeFile executes a Burn source file
func executeFile(filename string, debug bool, stdout, stderr io.Writer) int {
	if !strings.HasSuffix(filename, ".bn") {
		fmt.Fprintf(stderr, "Warning: File %s does not have the .bn extension\n", filename)
	}

	source, err := os.ReadFile(filename)
	if err != nil {
		fmt.Fprintf(stderr, "Error reading file: %v\n", err)
		return 1
	}

	return executeCode(string(source), debug, stdout, stderr)
}

// executeCode executes Burn code from a string
func executeCode(source string, debug bool, stdout, stderr io.Writer) int {
	result, err := execute(source, debug, stdout)
	if err != nil {
		fmt.Fprintf(stderr, "Error: %v\n", err)
		return 1
	}

	if result != nil && debug {
		fmt.Fprintln(stdout, "Program result:", result)
	}

	return 0
}

// execute performs the actual execution of Burn code
func execute(source string, debug bool, stdout io.Writer) (interface{}, error) {
	lex := lexer.New(source)
	tokens, err := lex.Tokenize()
	if err != nil {
		return nil, formattedError("Lexical error", err, source, lex.Position())
	}

	if debug {
		fmt.Fprintln(stdout, "--- Tokens ---")
		for _, token := range tokens {
			if token.Type != lexer.TokenEOF {
				fmt.Fprintf(stdout, "%s '%s' at position %d\n",
					tokenTypeToString(token.Type), token.Value, token.Position)
			}
		}
		fmt.Fprintln(stdout)
	}

	p := parser.New(tokens)
	program, err := p.Parse()
	if err != nil {
		return nil, formattedError("Parse error", err, source, p.Position())
	}

	if debug {
		fmt.Fprintln(stdout, "--- AST ---")
		printAST(program, 0, stdout)
		fmt.Fprintln(stdout)
	}

	tc := typechecker.New()
	if err := tc.Check(program.Declarations); err != nil {
		return nil, formattedError("Type error", err, source, tc.Position())
	}

	if debug {
		fmt.Fprintln(stdout, "--- Type Check Passed ---")
		fmt.Fprintln(stdout)
	}

	interpreter := interpreter.New()
	result, err := interpreter.Interpret(program)
	if err != nil {
		return nil, formattedError("Runtime error", err, source, interpreter.Position())
	}

	return result, nil
}
