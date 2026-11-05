# Keep the C# oracle external and pinned

The C# `WzComparerR2.Oracle` harness is not vendored or added as a submodule. It is
checked out at an explicit pinned commit (`bb98c25…`) and run only by the manual
differential workflow. Keeping it external keeps the production repository pure Rust
and avoids shipping a .NET toolchain with the crate, at the cost of requiring an
external checkout for parity verification.

**Consequences**: `scripts/differential.sh`, `tools/oracle-diff`, and the CI workflow
take the oracle repository/ref as input. `docs/oracle-spec.md` owns the pin table.
