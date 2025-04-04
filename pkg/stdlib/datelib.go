package stdlib

// DateLib contains the source code for the date standard library
const DateLib = `// Burn Standard Library: Date Module
// This provides date and time functionality for Burn programs

// Date struct to represent a date with day, month, and year
def Date {
    year: int,
    month: int,
    day: int
}

// Get the current date as a Date struct
fun now(): Date {
    // Implementation provided by interpreter
}

// Format a date as string (YYYY-MM-DD)
fun formatDate(date: Date): string {
    // Implementation provided by interpreter
}

// Get current year
fun currentYear(): int {
    // Implementation provided by interpreter
}

// Get current month
fun currentMonth(): int {
    // Implementation provided by interpreter
}

// Get current day
fun currentDay(): int {
    // Implementation provided by interpreter
}

// Check if a year is a leap year
fun isLeapYear(year: int): bool {
    // Implementation provided by interpreter
}

// Get the number of days in a specific month of a specific year
fun daysInMonth(year: int, month: int): int {
    // Implementation provided by interpreter
}

// Create a Date from year, month, and day
fun createDate(year: int, month: int, day: int): Date {
    // Implementation provided by interpreter
}

// Get the day of the week (0 = Saturday, 1 = Sunday, ..., 6 = Friday)
fun dayOfWeek(date: Date): int {
    // Implementation provided by interpreter
}

// Add days to a date
fun addDays(date: Date, days: int): Date {
    // Implementation provided by interpreter
}

// Subtract days from a date
fun subtractDays(date: Date, days: int): Date {
    // Implementation provided by interpreter
}

// Get today's date as a formatted string
fun today(): string {
    // Implementation provided by interpreter
}
`
