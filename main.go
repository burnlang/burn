package main

import (
	"os"

	"github.com/burnlang/burn/cmd"
)

func main() {
	args := os.Args[1:]

	exitCode := cmd.Execute(args, os.Stdin, os.Stdout, os.Stderr)

	os.Exit(exitCode)
}
