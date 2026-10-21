# Values and types

Every value in Nifty has a **type** that says what kind of thing it is. Nifty checks
types before your app runs, so mixing up a number and some text is caught early instead
of crashing later.

## The basic types

| Type | What it holds | Examples |
|---|---|---|
| `int` | whole numbers | `0`, `42`, `-7`, `1_000_000` |
| `float` | numbers with a decimal point | `3.14`, `0.5`, `-2.0` |
| `bool` | yes or no | `true`, `false` |
| `string` | text | `"hello"`, `""` |
| `[T]` | a list of `T`s | `[1, 2, 3]`, `["a", "b"]` |
| a struct name | your own record type | `Todo(title: "Milk", done: false)` |

A few details:

- Underscores in numbers are just for reading: `1_000` is `1000`.
- A `float` needs digits on **both** sides of the dot: write `0.5`, not `.5`, and `2.0`,
  not `2.`.
- There is no `null` or "nothing" value. If something is a `string`, it's always a
  string.

## Nifty works out types for you

You rarely write types. Nifty figures them out from the starting value:

```nf
state count = 0         // an int
state name = "Ada"      // a string
state ready = false     // a bool
```

You only write a type where Nifty can't guess, like an empty list, function
parameters, and what a function returns:

```nf
state todos: [Todo] = []                // empty list: say what goes in it
fn area(w: float, h: float) -> float {  // parameters and return type
  return w * h
}
```

## Numbers don't mix

`int` and `float` are separate. Nifty never converts one into the other behind your
back, so this is an error:

```text
Error: can't use `+` with `int` and `float`
```

Write both sides as the same type, like `1.0 + 2.5` or `1 + 2`.

Dividing two `int`s gives an `int` and drops the remainder: `7 / 2` is `3`. Use `%` to
get the remainder: `7 % 2` is `1`. Dividing by zero stops the app with an error that
points at the line.

## Text and interpolation

Strings go in double quotes. To put a value inside text, wrap any expression in
`{ }`:

```nf
label("Hello, {name}!")
label("{done} of {total} done")
label("Twice as many: {count * 2}")
```

This is called **interpolation**, and it's how you turn numbers into text. You *can*
join two strings with `+`, but joining a string and a number with `+` is an error;
Nifty will suggest interpolation instead.

Special characters are written with a backslash:

| Write | To get |
|---|---|
| `\n` | a new line |
| `\t` | a tab |
| `\"` | a double quote |
| `\\` | a backslash |
| `\{` | a literal `{` (not interpolation) |

### String methods

Strings have methods for measuring, searching and cutting text. Positions count
characters, as `len` does, starting from `0`, so `"héllo".find("l")` is `2`.

| Method | Gives |
|---|---|
| `name.len()` | how many characters it has |
| `name.is_empty()` | whether it has none |
| `name.contains("x")` | whether `"x"` is in it |
| `name.starts_with("x")`, `name.ends_with("x")` | whether it begins or ends with `"x"` |
| `name.find("x")` | the position where `"x"` first starts, or `-1` when it isn't there |
| `name.slice(start, end)` | the characters from `start` up to, but not including, `end` |
| `name.split(", ")` | a `[string]` of the pieces between each `", "`; `split("")` gives its characters |
| `name.replace("a", "b")` | a copy with every `"a"` replaced by `"b"`; replacing `""` changes nothing |
| `name.trim()` | a copy without the spaces, tabs and line breaks at its start and end |

None of them change the string, so the UI can show them: `label(title.trim())`.
`slice` needs `0 <= start <= end <= name.len()`; anything else stops the app with an
error that says which positions were asked for, as a list position that isn't there
does. Check what `find` gives before slicing with it:

```nf
fn between(text: string, open: string, close: string) -> string {
  let start = text.find(open)
  if start < 0 {
    return ""
  }
  let rest = text.slice(start + open.len(), text.len())
  let end = rest.find(close)
  if end < 0 {
    return ""
  }
  return rest.slice(0, end)
}
```

## Comments

Anything after `//` on a line is a comment, and Nifty ignores it:

```nf
state count = 0  // how many times the button was clicked
```

## Operators

From the loosest to the tightest binding (the ones lower in the table happen first):

| Operator | Meaning |
|---|---|
| `a \|\| b` | `a` or `b` |
| `a && b` | `a` and `b` |
| `==  !=  <  <=  >  >=` | equal, not equal, less than, and so on |
| `+  -` | add, subtract (`+` also joins two strings) |
| `*  /  %` | multiply, divide, remainder |
| `!a`, `-a` | not, negative |
| `f(x)`, `a.b`, `a[i]` | call, field or method, list item |

So `1 + 2 * 3` is `7`, and `!done && count > 0` means `(!done) && (count > 0)`. Use
parentheses whenever it makes things clearer.

Comparisons don't chain: write `0 < x && x < 10`, not `0 < x < 10`.

## Lines, not semicolons

Each statement goes on its own line. There are no semicolons. If a line gets long, you
can break it:

- inside `( )` or `[ ]`, or
- right after an operator like `+` or `&&`.

```nf
let total = first_part +
  second_part

button(
  "Save",
  style: .primary,
) { save() }
```
