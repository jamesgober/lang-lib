# Benchmarks

This repository includes a small Criterion benchmark suite for the hot paths
that matter most in a request-driven application.

## What Is Measured

- `resolve_accept_language`: maps an `Accept-Language` header to a supported locale
- `translate_lookup`: direct in-memory translation lookup for a loaded locale
- `translate_fallback_chain_miss`: lookup that misses in the requested locale and resolves through the fallback chain
- `translate_complete_miss_inline_fallback`: complete miss that returns the inline fallback string
- `translate_complete_miss_key_return`: complete miss that returns the key itself
- `translate_hit_concurrent`: single-key hit-path latency under 1, 4, 16, and 64 contending threads
  (scaled by the bench harness so Criterion reports per-thread per-iter time, not aggregate wall time)

In addition to the Criterion suite, `tests/concurrency.rs` runs three
parallelism stress tests under `cargo test`: a 64-thread translate
storm, a concurrent-reload-during-reads test, and an unload-during-reads
test. These verify correctness, not performance, but they catch any
regression in the lock-free read path that would only show up under
contention.

## Run Locally

```powershell
cargo bench --bench performance
```

Criterion writes detailed output under `target/criterion/`.

## How To Use The Numbers

- Compare results on the same machine when possible.
- Treat small differences as noise until they reproduce across multiple runs.
- Investigate changes that are consistently worse by roughly 10% or more.
- Check both the direct lookup path and the fallback-chain path before claiming a performance win.
- Check complete-miss behavior as well, because fallback-string and key-return paths are common in real apps.

## CI Policy

- The main CI workflow compiles the benchmark target on every pull request and push (`cargo bench --bench performance --no-run`).
- The benchmark workflow runs the full Criterion suite on Ubuntu and uploads the `target/criterion/` artifact for inspection.
- Benchmark execution is separated from normal CI so correctness checks stay fast and performance runs stay reproducible.
- The benchmark workflow uses `actions/upload-artifact@v7` (Node 24) for the artifact upload step.

## Regression Checklist

If a benchmark regresses:

1. Re-run the benchmark locally on the same machine.
2. Check whether the regression appears in direct lookup, fallback lookup, or header resolution.
3. Review recent changes to fallback behavior, string allocation, or locale parsing.
4. Capture before-and-after numbers in the pull request if the change is intentional.