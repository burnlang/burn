package stdlib

// StdLibFiles contains all standard library files embedded directly in the executable
var StdLibFiles = map[string]string{
    "date":   DateLib,
    "http":   HTTPLib,
    "time":   TimeLib,
}