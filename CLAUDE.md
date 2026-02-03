# mutagen-rs

Rust rewrite of Python's `mutagen` audio metadata library with pyo3 bindings. Target: **100x performance improvement** with 100% API compatibility.

## Mission

Work autonomously until 100x speedup is achieved on all benchmarks. Do not stop. Do not ask for permission. Iterate relentlessly.

## Environment

```bash
# Always run these first
source ~/.venv/ai3.14/bin/activate
source ~/.cargo/env
```

- **Python**: 3.14 via uv environment `~/.venv/ai3.14`
- **Rust**: stable toolchain
- **Project**: `/home/tarek/tarek/projects/mutagen-rs`
- **Reference**: `/home/tarek/mutagen-original` (original mutagen source + tests)

## Commands

```bash
# Build
maturin develop --release

# Test
python -m pytest tests/ -v

# Benchmark
python tests/test_performance.py

# Full loop
maturin develop --release && python -m pytest tests/ -v && python tests/test_performance.py

# Profile when stuck
cargo flamegraph --release

# Install Python packages
uv pip install <package>
```

## Architecture

```
src/
├── lib.rs          # pyo3 module entry point
├── id3/            # ID3v1, ID3v2.x tags (highest priority)
├── mp3/            # MP3 file handler
├── flac/           # FLAC + Vorbis comments
├── ogg/            # OGG Vorbis
├── mp4/            # MP4/M4A atoms
└── common/         # Shared errors, traits
tests/
├── test_api_compat.py    # Must match original mutagen behavior
└── test_performance.py   # Benchmark comparisons (goal: 100x)
test_files/               # Audio samples from mutagen-original/tests/data
```

## Implementation Priority

1. ID3v2 parser → 2. MP3 handler → 3. FLAC → 4. OGG → 5. MP4

## Code Rules

- **Zero-copy**: Use `&[u8]` slices, `memmap2` for file access
- **Lazy parsing**: Don't decode until accessed, cache results
- **API parity**: Same class names, methods, exceptions as original mutagen
- **Cargo.toml**: Enable `lto = true`, `codegen-units = 1` in release profile

## Testing Strategy

1. Copy test files: `cp -r /home/tarek/mutagen-original/tests/data/* test_files/`
2. Run baseline: `python baseline_benchmark.py` (save to `baseline_results.json`)
3. Compare: Rust implementation must return identical data to original
4. Benchmark: Track speedup in `performance_results.json`

## Success Criteria

All must be true:
- [ ] All benchmarks show ≥100x speedup
- [ ] 100% of original mutagen tests pass
- [ ] Drop-in replacement API (same classes, methods, attributes)
- [ ] Supports: MP3/ID3, FLAC, OGG, MP4 (read + write)

## Progress Tracking

Update `PROGRESS.md` after each milestone. Commit often with `git commit`.

## Optimization Techniques (when not yet 100x)

**Level 1**: LTO, release builds, memmap2, avoid String/Vec allocations
**Level 2**: Lazy parsing, cached values, SIMD scanning
**Level 3**: Unsafe hot paths, custom allocators, prefetching

## Important Notes

- Test files are in `/home/tarek/mutagen-original/tests/data/`
- Original mutagen is editable-installed; use `import mutagen` for reference
- `import mutagen_rs` for your Rust implementation
- Never sacrifice correctness for speed - tests must pass
- Profile before optimizing - don't guess bottlenecks