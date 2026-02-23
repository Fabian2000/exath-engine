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

    // Numerical derivative
    ExathResult d = exath_deriv("x^3", "x", 2.0, Rad);
    printf("d/dx x^3 at x=2 = %f\n", d.re);  // 12.000000

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

### Numerical methods

| Function | Description |
| --- | --- |
| `exath_deriv(expr, var, x, mode)` | f'(x) via central finite difference |
| `exath_integrate(expr, var, a, b, mode)` | Definite integral via Simpson's rule |
| `exath_sum(expr, var, from, to, mode)` | Summation over integer range |
| `exath_prod(expr, var, from, to, mode)` | Product over integer range |

### Session

| Function | Description |
| --- | --- |
| `exath_session_new(mode)` | Create a new session |
| `exath_session_free(s)` | Free a session |
| `exath_session_eval(s, line)` | Evaluate a line (expression or assignment) |
| `exath_session_set_var(s, name, re, im)` | Set a variable |
| `exath_session_remove_var(s, name)` | Remove a variable |
| `exath_session_clear_vars(s)` | Clear all variables |
| `exath_session_remove_fn(s, name)` | Remove a user-defined function |
| `exath_session_fn_names(s)` | Comma-separated list of defined functions |

### Memory

| Function | Description |
| --- | --- |
| `exath_free_string(s)` | Free any string returned by the API |

All `char *` returns (error messages, function lists) must be freed with `exath_free_string()`.
