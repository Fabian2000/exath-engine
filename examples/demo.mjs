import { evaluate, isValid, supportedFunctions, ExathSession } from "../ffi-wasm/pkg/exath_engine_wasm.js";

// One-shot evaluation
console.log("=== One-shot ===");
console.log("2 + 3 * 4 =", evaluate("2 + 3 * 4", "rad").re);
console.log("sqrt(-4)  =", evaluate("sqrt(-4)", "rad").re, "+", evaluate("sqrt(-4)", "rad").im + "i");
console.log("sin(90)   =", evaluate("sin(90)", "deg").re);
console.log("10!       =", evaluate("10!", "rad").re);

// Validation
console.log("\n=== Validation ===");
console.log("isValid('2+3'):", isValid("2+3"));
console.log("isValid('2++'):", isValid("2++"));

// Supported functions
console.log("\n=== Supported functions ===");
const fns = supportedFunctions();
console.log(`${fns.length} functions:`, fns.slice(0, 10).join(", "), "...");

// Session with variables
console.log("\n=== Session ===");
const s = new ExathSession("rad");
s.eval("r = 5");
s.eval("h = 10");
const vol = s.eval("pi * r^2 * h");
console.log("Cylinder volume (r=5, h=10):", vol.re);

// User-defined functions
console.log("\n=== User-defined functions ===");
s.eval("hyp(a, b) = sqrt(a^2 + b^2)");
s.eval("circle_area(r) = pi * r^2");
console.log("hyp(3, 4)       =", s.eval("hyp(3, 4)").re);
console.log("circle_area(10) =", s.eval("circle_area(10)").re);
console.log("Functions:", s.fnNames());

// Complex results
console.log("\n=== Complex ===");
const c = s.eval("sqrt(-9) + 2");
console.log(`sqrt(-9) + 2 = ${c.re} + ${c.im}i  (isComplex: ${c.isComplex})`);

// Error handling
console.log("\n=== Error handling ===");
const err = s.eval("ln(0)");
console.log("ln(0) isError:", err.isError, "→", err.errorMessage);
