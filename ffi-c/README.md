# exath-engine — C / C++ / C#

C-compatible shared library for the exath-engine expression evaluator.

## Build

```bash
cargo build --release -p exath-engine-ffi
```

Output:

| Platform | File |
| --- | --- |
| Linux | `target/release/libexath_engine_ffi.so` |
| Windows | `target/release/exath_engine_ffi.dll` |
| macOS | `target/release/libexath_engine_ffi.dylib` |

The C header is at `ffi-c/include/exath_engine.h`.

## Quick start

```c
#include "exath_engine.h"
#include <stdio.h>

int main(void) {
    // One-shot evaluation
    ExathResult r = exath_evaluate("2^10 + sqrt(9)", Rad);
    if (r.is_error) {
        printf("Error: %s\n", r.error_msg);
        exath_free_string(r.error_msg);
        return 1;
    }
    printf("Result: %f + %fi\n", r.re, r.im);  // 1027.000000 + 0.000000i

    // Session with variables
    ExathSession *s = exath_session_new(Deg);
    exath_session_eval(s, "x = 42");
    ExathResult r2 = exath_session_eval(s, "sin(x)");
    printf("sin(42 deg) = %f\n", r2.re);

    // User-defined functions
    exath_session_eval(s, "f(x) = x^2 + 1");
    ExathResult r3 = exath_session_eval(s, "f(5)");
    printf("f(5) = %f\n", r3.re);  // 26.000000

    // Anything symbolic, numeric or matrix goes through eval_line as a string.
    // Symbolic results come back as `expression`; numeric ones as re/im.
    ExathLineResult d = exath_session_eval_line(s, "diff(x^3, x)");
    printf("diff(x^3, x) = %s\n", d.expression);  // 3 * x^2
    exath_free_string(d.expression);

    ExathLineResult i = exath_session_eval_line(s, "integral(sin(x), x, 0, pi)");
    printf("definite integral = %f\n", i.re);     // 2.000000

    exath_session_free(s);
    return 0;
}
```

Compile:

```bash
gcc -o demo demo.c -L target/release -lexath_engine_ffi -lm
```

## API reference

### Types

```c
enum ExathAngleMode { Deg = 0, Rad = 1, Grad = 2 };

typedef struct {
    double   re;         // real part
    double   im;         // imaginary part
    int32_t  is_error;   // 0 = success, 1 = error
    char    *error_msg;  // null-terminated error string (NULL if no error)
} ExathResult;
```

### Evaluation

| Function | Description |
| --- | --- |
| `exath_evaluate(expr, mode)` | Evaluate an expression, returns `ExathResult` |
| `exath_is_valid(expr)` | Returns 1 if expression parses, 0 otherwise |
| `exath_supported_functions()` | Comma-separated list of built-in functions |

### Session

| Function | Description |
| --- | --- |
| `exath_session_new(mode)` | Create a new session |
| `exath_session_free(s)` | Free a session |
| `exath_session_eval(s, line)` | Evaluate a line, returns `ExathResult` (numeric only) |
| `exath_session_eval_line(s, line)` | Evaluate a line incl. symbolic/matrix forms, returns `ExathLineResult` |
| `exath_session_set_var(s, name, re, im)` | Set a variable |
| `exath_session_remove_var(s, name)` | Remove a variable |
| `exath_session_clear_vars(s)` | Clear all variables |
| `exath_session_remove_fn(s, name)` | Remove a user-defined function |
| `exath_session_fn_names(s)` | Comma-separated list of defined functions |
| `exath_session_var_names(s)` | Comma-separated list of variables |

### Memory

| Function | Description |
| --- | --- |
| `exath_free_string(s)` | Free any string returned by the API |

All `char *` returns (error messages, function lists) must be freed with `exath_free_string()`.

## The eval gateway — everything via one call

There are no per-operation functions. Every symbolic, numeric, matrix and unit
operation is invoked by passing its string form to `exath_session_eval_line`.
This is identical to the Rust crate and the WASM build.

```c
ExathSession *s = exath_session_new(Rad);
exath_session_eval_line(s, "diff(sin(x^2), x)");      // → "2 * x * cos(x^2)"
exath_session_eval_line(s, "factor(x^2 - 1, x)");     // → "(x + 1) * (x - 1)"
exath_session_eval_line(s, "solve(x^2 - 4, x)");      // → "x = 2, x = -2"
exath_session_eval_line(s, "integral(x^2, x)");       // → "x^3 / 3"
exath_session_eval_line(s, "det([[1,2],[3,4]])");     // → -2 (numeric)
exath_session_eval_line(s, "dsolve([1,3,2], x)");     // → "C1 * exp(-2*x) + C2 * exp(-x)"
exath_session_eval_line(s, "convert(5, km, m)");      // → 5000 (numeric)
exath_session_free(s);
```

`ExathLineResult` convention: if `is_error` → read `error_msg`; else if
`is_expression` → read `expression` (a string, free with `exath_free_string()`);
else read `re`/`im` for a numeric result.

```c
typedef struct {
    int32_t  is_expression;  // 1 = symbolic string result in `expression`
    char    *expression;     // expression string (free with exath_free_string)
    double   re;             // numeric real part (when is_expression == 0)
    double   im;             // numeric imaginary part
    int32_t  is_error;       // 1 = error in `error_msg`
    char    *error_msg;      // error string (free with exath_free_string)
} ExathLineResult;
```

See the main [README](../README.md) for the full DSL form reference.
