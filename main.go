package main

import (
    "os"

	"github.com/s42yt/burn/cmd"
)

func main() {
    args := os.Args[1:]

    exitCode := cmd.Execute(args, os.Stdin, os.Stdout, os.Stderr)

    os.Exit(exitCode)
}