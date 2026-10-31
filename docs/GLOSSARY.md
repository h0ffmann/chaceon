# Glossary

| Term | Meaning |
|---|---|
| DB1 | WW3 switch: Battjes and Janssen depth-induced breaking, `W3SDB1` |
| DIA | Discrete Interaction Approximation of the nonlinear transfer; WW3's `NL1`, `W3SNL1` |
| FMA | Fused multiply-add: `a*b+c` with one rounding |
| GIMPLE | GCC's intermediate form, read to see what `-O3` computes |
| `host`, `cuda`, `spirv` | The three verdicts of ADR-0005 |
| `W3XYP3`, `W3KTP3` | WW3's third-order propagation in space and in spectral space (`PR3`) |
| `W3QCK1`, `W3QCK2`, `W3QCK3` | The ULTIMATE QUICKEST step, `UQ`: uniform, varying grid, with transparencies |
| `out_pnt.ww3` | WW3's point output: spectra and scalars at output points |
| lavapipe | Mesa's Vulkan implementation on the CPU |
| libdevice | NVIDIA's device math library |
| libmvec | glibc's vector math library |
| PR3 | WW3 switch: third-order propagation, `W3PRO3` |
| PTX | NVIDIA's virtual instruction set, what Rust-CUDA emits |
| `routine` | Status of ADR-0007: every captured call bit-identical |
| reference | metocean/WW3 at `92e2cf66`, built by `docker/Dockerfile.reference` |
| Rust-CUDA | Rust compiler backend for PTX (`rustc_codegen_nvvm`) |
| rust-gpu | Rust compiler backend for SPIR-V (`rustc_codegen_spirv`) |
| sm_89, sm_90 | NVIDIA shader models: RTX 4090, H100 |
| SPIR-V | Khronos' binary shader language, what Vulkan consumes |
| ST4 | WW3 switch: the Ardhuin et al. source terms, `W3SIN4`, `W3SDS4` |
| round to odd | A rounding that lets a later rounding to fewer bits stay correct (Boldo and Melquiond, 2008) |
| ULP | Unit in the last place |
| `VA` | WW3's action spectrum per sea point |
