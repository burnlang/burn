fn fib(n) {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
}

fn sum(xs: arr) {
    let total = 0
    let i = 0
    while i < len(xs) {
        total = total + xs[i]
        i = i + 1
    }
    return total
}

fn describe(n): str {
    if n % 15 == 0 { return "FizzBuzz" }
    else if n % 3 == 0 { return "Fizz" }
    else if n % 5 == 0 { return "Buzz" }
    return "" + n
}

let i = 0
let squares = []
while i < 10 {
    push(squares, square(i))
    i = i + 1
}
print("squares:", squares)
print("sum of squares:", sum(squares))
print("fib(20) =", fib(20))

let line = ""
let n = 1
while n <= 15 {
    line = line + describe(n) + " "
    n = n + 1
}
print(line)
