Wavelet is an experimental compiler from a Rust-based DSL to asynchronous dataflow circuits,
with its core passes formally verified in the Lean theorem prover.

Currently supported backends include the [`handshake` dialect](https://circt.llvm.org/docs/Dialects/Handshake) from [LLVM CIRCT](https://circt.llvm.org/)
and tagless/ordered CGRAs such as [RipTide](https://doi.org/10.1109/MICRO56248.2022.00046).

Read more about Wavelet in [our PLDI 2026 paper](https://lz.gy/papers/wavelet.pdf):
```
@article{wavelet,
  author = {Lin, Zhengyao and Cai, Yi and Surbatovich, Milijana},
  title = {Let It Flow: A Formally Verified Compilation Framework for Asynchronous Dataflow},
  year = {2026},
  issue_date = {June 2026},
  publisher = {Association for Computing Machinery},
  address = {New York, NY, USA},
  volume = {10},
  number = {PLDI},
  url = {https://doi.org/10.1145/3808263},
  doi = {10.1145/3808263},
  journal = {Proc. ACM Program. Lang.},
  month = jun,
  articleno = {185},
  numpages = {25}
}
```

## Build and Usage

Please first install the following dependencies:
- [Elan (Lean version manager)](https://github.com/leanprover/elan)
- [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)
- [cvc5-1.3.3](https://github.com/cvc5/cvc5/releases/tag/cvc5-1.3.3)

Then simply run
```
cargo build
```
which will produce the CLI binary at `target/debug/wavelet`.

Example source programs can be found in `integration-tests/tests/*/test.rs`.
To compile a simple example to the `handshake` MLIR dialect, run:
```
target/debug/wavelet compile integration-tests/tests/simple/test.rs \
    | target/debug/wavelet handshake
```

## Developing and Verifying Proofs

All of the Lean formalization can be found in `wavelet-core/lean`,
and by default, the `cargo build` command above does not check any proofs.
To actually verify all proofs:
```
cd wavelet-core/lean
lake exec cache get
lake build Thm
```
