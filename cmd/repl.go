package cmd

import (
	"fmt"
	"io"
	"strings"
)

func startREPL(stdin io.Reader, stdout, stderr io.Writer) int {
	fmt.Fprintf(stdout, "Burn Programming Language v%s\n", getVersion())
	fmt.Fprintln(stdout, "Type 'exit' to quit, 'help' for more information")

	buf := make([]byte, 1024)

	for {
		fmt.Fprint(stdout, "> ")
		n, err := stdin.Read(buf)
		if err != nil {
			if err == io.EOF {
				break
			}
			fmt.Fprintf(stderr, "Error reading input: %v\n", err)
			continue
		}

		line := strings.TrimSpace(string(buf[:n]))
		if line == "" {
			continue
		}

		if line == "exit" || line == "quit" {
			return 0
		}

		if line == "help" {
			printReplHelp(stdout)
			continue
		}

		result, err := execute(line, false, stdout)
		if err != nil {
			fmt.Fprintf(stderr, "Error: %v\n", err)
		} else if result != nil {
			fmt.Fprintf(stdout, "=> %v\n", result)
		}
	}

	return 0
}

func printReplHelp(w io.Writer) {
	fmt.Fprintln(w, "Burn REPL commands:")
	fmt.Fprintln(w, "  exit, quit  - Exit the REPL")
	fmt.Fprintln(w, "  help        - Show this help message")
	fmt.Fprintln(w, "")
	fmt.Fprintln(w, "Examples:")
	fmt.Fprintln(w, "  > print(\"Hello, world!\")")
	fmt.Fprintln(w, "  > var x = 5 + 3")
	fmt.Fprintln(w, "  > x * 2")
}
