# Changelog

All notable changes to exath-engine are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/), and the project aims for
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Computer algebra: `simplify` (canonical normal form + identities), `expand`,
  `factor`, `polygcd`, `solve` (exact for polynomials, verified numeric for
  transcendental), `nsolve`, indefinite and definite integration (rules +
  verified u-substitution + partial fractions + adaptive Simpson fallback),
  `taylor`, `limit`, `laplace`, `dsolve`, `sumc`, `assume`, `piecewise`.
- Multivariable calculus: `grad`, `jacobian`, `hessian`, `odesolve` (RK4),
  `minimize` / `maximize`.
- Linear algebra: matrix literals, `det`, `inv`, `transpose`, `trace`, `rank`,
  `norm`, `identity`, `linsolve`, `eigenvalues` / `eigenvectors` (Jacobi),
  `svdvals` (one-sided Jacobi SVD), `charpoly`.
- Functions: special (`gamma`, `lgamma`, `digamma`, `beta`, `erf`, `erfc`),
  statistics (`mean`, `median`, `variance`, `stddev`), distributions (`npdf`,
  `ncdf`, `binom`), number theory (`isprime`, `nextprime`, `totient`, `powmod`,
  `factorint`), and DSL forms for `sum`, `product`, `deriv`, `convert`.
- Exact rationals (i128) in the symbolic layer.
- Randomized verification harnesses (panic-safety fuzzing + differential checks).

### Changed
- **BREAKING:** the comma is now purely a separator; decimals use `.` only
  (`max(1, 2)`, `[1, 2, 3]`, `3.14`).
- **BREAKING:** a single eval gateway. Every operation is invoked through
  `evaluate` / `Session::eval` / `Session::eval_line`; per-operation typed
  wrappers were removed from the C-FFI and WASM so the Rust crate, C-FFI and
  WASM expose an identical surface.

## [0.1.0]
- Initial engine: complex-native evaluation, sessions, user-defined functions,
  rich elementary function set, symbolic differentiation and simplification.
