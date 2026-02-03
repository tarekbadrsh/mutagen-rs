"""Performance benchmark: mutagen_rs vs original mutagen."""
import json
import time
import os
import sys

import mutagen
from mutagen.mp3 import MP3
from mutagen.flac import FLAC
from mutagen.oggvorbis import OggVorbis
from mutagen.mp4 import MP4

import mutagen_rs

TEST_DIR = os.path.join(os.path.dirname(os.path.dirname(__file__)), "test_files")
ITERATIONS = 200


def find_test_files():
    files = {}
    for f in os.listdir(TEST_DIR):
        path = os.path.join(TEST_DIR, f)
        if not os.path.isfile(path):
            continue
        ext = os.path.splitext(f)[1].lower()
        if ext == ".mp3":
            files.setdefault("mp3", []).append(path)
        elif ext == ".flac":
            files.setdefault("flac", []).append(path)
        elif ext == ".ogg":
            files.setdefault("ogg", []).append(path)
        elif ext in (".m4a", ".m4b", ".mp4"):
            files.setdefault("mp4", []).append(path)
    return files


def benchmark_original(name, cls, paths, iterations=ITERATIONS):
    if not paths:
        return None

    # Warm up
    for p in paths:
        try:
            cls(p)
        except Exception:
            pass

    times = []
    for _ in range(iterations):
        start = time.perf_counter()
        for p in paths:
            try:
                f = cls(p)
                if hasattr(f, 'info') and f.info:
                    _ = f.info.length
                if f.tags:
                    for k in f.tags.keys():
                        _ = f.tags[k]
            except Exception:
                pass
        elapsed = time.perf_counter() - start
        times.append(elapsed)

    return min(times)


def benchmark_rust(name, cls_name, paths, iterations=ITERATIONS):
    if not paths:
        return None

    cls = getattr(mutagen_rs, cls_name)

    # Warm up
    for p in paths:
        try:
            cls(p)
        except Exception:
            pass

    times = []
    for _ in range(iterations):
        start = time.perf_counter()
        for p in paths:
            try:
                f = cls(p)
                _ = f.info.length
                keys = f.keys()
                for k in keys:
                    try:
                        _ = f[k]
                    except Exception:
                        pass
            except Exception:
                pass
        elapsed = time.perf_counter() - start
        times.append(elapsed)

    return min(times)


def main():
    files = find_test_files()
    print(f"Test files: { {k: len(v) for k, v in files.items()} }")
    print(f"Iterations: {ITERATIONS}")
    print()

    format_map = [
        ("mp3", MP3, "MP3"),
        ("flac", FLAC, "FLAC"),
        ("ogg", OggVorbis, "OggVorbis"),
        ("mp4", MP4, "MP4"),
    ]

    results = {}
    all_passed = True

    for name, orig_cls, rust_cls_name in format_map:
        paths = files.get(name, [])
        if not paths:
            continue

        # Filter to only files that both can handle
        valid_paths = []
        for p in paths:
            try:
                orig_cls(p)
                getattr(mutagen_rs, rust_cls_name)(p)
                valid_paths.append(p)
            except Exception:
                pass

        if not valid_paths:
            print(f"{name}: no files both implementations can handle")
            continue

        print(f"Benchmarking {name} ({len(valid_paths)} files)...")

        orig_time = benchmark_original(name, orig_cls, valid_paths)
        rust_time = benchmark_rust(name, rust_cls_name, valid_paths)

        speedup = orig_time / rust_time if rust_time > 0 else float('inf')

        orig_per_file = (orig_time / len(valid_paths)) * 1000
        rust_per_file = (rust_time / len(valid_paths)) * 1000

        passed = speedup >= 100.0

        results[name] = {
            "files": len(valid_paths),
            "original_ms_per_file": orig_per_file,
            "rust_ms_per_file": rust_per_file,
            "speedup": speedup,
            "passed": passed,
        }

        status = "PASS" if passed else "FAIL"
        print(f"  Original: {orig_per_file:.4f} ms/file")
        print(f"  Rust:     {rust_per_file:.4f} ms/file")
        print(f"  Speedup:  {speedup:.1f}x [{status}]")
        print()

        if not passed:
            all_passed = False

    # Auto-detect benchmark
    all_paths = []
    for ps in files.values():
        all_paths.extend(ps)

    valid_auto = []
    for p in all_paths:
        try:
            mutagen.File(p)
            mutagen_rs.File(p)
            valid_auto.append(p)
        except Exception:
            pass

    if valid_auto:
        print(f"Benchmarking auto-detect ({len(valid_auto)} files)...")

        # Original
        times = []
        for _ in range(ITERATIONS):
            start = time.perf_counter()
            for p in valid_auto:
                try:
                    mutagen.File(p)
                except Exception:
                    pass
            times.append(time.perf_counter() - start)
        orig_time = min(times)

        # Rust
        times = []
        for _ in range(ITERATIONS):
            start = time.perf_counter()
            for p in valid_auto:
                try:
                    mutagen_rs.File(p)
                except Exception:
                    pass
            times.append(time.perf_counter() - start)
        rust_time = min(times)

        speedup = orig_time / rust_time if rust_time > 0 else float('inf')
        passed = speedup >= 100.0
        results["auto_detect"] = {
            "files": len(valid_auto),
            "original_ms_per_file": (orig_time / len(valid_auto)) * 1000,
            "rust_ms_per_file": (rust_time / len(valid_auto)) * 1000,
            "speedup": speedup,
            "passed": passed,
        }

        status = "PASS" if passed else "FAIL"
        print(f"  Original: {(orig_time / len(valid_auto)) * 1000:.4f} ms/file")
        print(f"  Rust:     {(rust_time / len(valid_auto)) * 1000:.4f} ms/file")
        print(f"  Speedup:  {speedup:.1f}x [{status}]")

        if not passed:
            all_passed = False

    # Save results
    output_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), "performance_results.json")
    with open(output_path, "w") as f:
        json.dump(results, f, indent=2)

    print(f"\n{'='*50}")
    if all_passed:
        print("ALL BENCHMARKS PASSED (>=100x speedup)")
    else:
        print("SOME BENCHMARKS BELOW 100x TARGET")
    print(f"Results saved to {output_path}")

    return 0 if all_passed else 1


if __name__ == "__main__":
    sys.exit(main())
