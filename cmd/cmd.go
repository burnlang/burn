package cmd

import (
	"fmt"
	"io"
	"strings"
)

func Execute(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	if len(args) < 1 {
		printUsage(stdout)
		return 1
	}

	nonOptions, options := parseArgs(args)

	if options["help"] {
		printUsage(stdout)
		return 0
	}

	if options["version"] {
		fmt.Fprintf(stdout, "Burn Programming Language v%s\n", getVersion())
		return 0
	}

	if options["repl"] {
		return startREPL(stdin, stdout, stderr)
	}

	if options["eval"] {
		if len(nonOptions) == 0 {
			fmt.Fprintln(stderr, "Error: no code provided for evaluation")
			return 1
		}
		return executeCode(nonOptions[0], options["debug"], stdout, stderr)
	}

	if options["exe"] {
		if len(nonOptions) == 0 {
			fmt.Fprintln(stderr, "Error: no source file provided for compilation")
			return 1
		}
		return compileToExecutable(nonOptions[0], nonOptions[len(nonOptions)-1], stdout, stderr)
	}

	if len(nonOptions) == 0 {
		printUsage(stdout)
		return 1
	}

	filename := nonOptions[0]
	debug := options["debug"]

	return executeFile(filename, debug, stdout, stderr)
}

func getVersion() string {
	return "0.1.0"
}

func parseArgs(args []string) ([]string, map[string]bool) {
	nonOptions := []string{}
	options := map[string]bool{
		"help":    false,
		"version": false,
		"repl":    false,
		"eval":    false,
		"debug":   false,
		"exe":     false,
	}

	for i := 0; i < len(args); i++ {
		arg := args[i]
		if strings.HasPrefix(arg, "-") {
			switch arg {
			case "-h", "--help":
				options["help"] = true
			case "-v", "--version":
				options["version"] = true
			case "-r", "--repl":
				options["repl"] = true
			case "-e", "--eval":
				options["eval"] = true
				if i+1 < len(args) {
					nonOptions = append(nonOptions, args[i+1])
					i++
				}
			case "-d", "--debug":
				options["debug"] = true
			case "-exe", "--executable":
				options["exe"] = true
			}
		} else {
			nonOptions = append(nonOptions, arg)
		}
	}

	return nonOptions, options
}

func printUsage(w io.Writer) {
	fmt.Fprintln(w, "Burn Programming Language")
	fmt.Fprintln(w, "Usage:")
	fmt.Fprintln(w, "  burn [options] [filename]")
	fmt.Fprintln(w, "")
	fmt.Fprintln(w, "Options:")
	fmt.Fprintln(w, "  -h, --help     Show this help message")
	fmt.Fprintln(w, "  -v, --version  Show version information")
	fmt.Fprintln(w, "  -r, --repl     Start interactive REPL (Read-Eval-Print Loop)")
	fmt.Fprintln(w, "  -e, --eval     Evaluate Burn code from command line")
	fmt.Fprintln(w, "  -d, --debug    Run in debug mode (show more information)")
	fmt.Fprintln(w, "  -exe, --executable  Compile to a standalone executable")
	fmt.Fprintln(w, "")
	fmt.Fprintln(w, "Examples:")
	fmt.Fprintln(w, "  burn main.bn              Execute a Burn program")
	fmt.Fprintln(w, "  burn -r                   Start REPL")
	fmt.Fprintln(w, "  burn -e 'print(\"Hello\")' Evaluate a single expression")
	fmt.Fprintln(w, "  burn -exe test/main.bn    Compile to executable")
}
