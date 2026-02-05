# Benchmark History

## Python: mutagen_rs vs mutagen

Speedup over Python mutagen (higher = better). Target: **100x**

| Run | MP3 | FLAC | OGG | MP4 | Auto | B.MP3 | B.FLAC | B.OGG | B.MP4 | B.Auto |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `2026-02-05 (baseline)` | 18.5x | 7.5x | 12.1x | 15.5x | 18.9x | **108.9x** | 29.1x | 20.5x | 64.5x | **103.5x** |
| `2026-02-05_10-06-04` | 18.5x (=) | 7.4x (-0.1) | 12.2x (+0.1) | 15.4x (-0.1) | 19.1x (+0.2) | **101.4x (-7.5)** | 23.2x (-5.9) | 17.9x (-2.6) | 61.2x (-3.3) | **100.2x (-3.3)** |
| `2026-02-05_10-21-47` | 17.8x (-0.7) | 7.5x (+0.1) | 12.0x (-0.2) | 15.8x (+0.4) | 18.8x (-0.3) | **110.5x (+9.1)** | 25.0x (+1.8) | 20.5x (+2.6) | 63.6x (+2.4) | **106.9x (+6.7)** |

## Rust Criterion: mutagen_rs vs lofty-rs

Ratio = lofty_time / mutagen_rs_time (higher = mutagen_rs is faster)

| Run | MP3s | MP3L | FLACs | FLACL | OGGs | OGGL | MP4s | MP4L |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `2026-02-05 (baseline)` | 2.5x | 2.83x | 19.99x | 134.89x | 7.88x | 129.8x | 22.42x | 3.54x |
| `2026-02-05_10-06-04` | 2.63x (+0.1) | 2.76x (-0.1) | 19.99x (=) | 130.73x (-4.2) | 7.88x (=) | 129.8x (=) | 21.99x (-0.4) | 3.47x (-0.1) |
| `2026-02-05_10-21-47` | 39.6x (+37.0) | 132.91x (+130.2) | 20.39x (+0.4) | 129.34x (-1.4) | 43.02x (+35.1) | 12242.27x (+12112.5) | 40.82x (+18.8) | 259.72x (+256.2) |

### Latest Criterion times

| Benchmark | mutagen_rs | lofty | Ratio |
| --- | --- | --- | --- |
| mp3_small | 109ns | 4.33us | **39.6x** |
| mp3_large | 106ns | 14.03us | **132.91x** |
| flac_small | 139ns | 2.83us | **20.39x** |
| flac_large | 108ns | 13.97us | **129.34x** |
| ogg_small | 15ns | 657ns | **43.02x** |
| ogg_large | 15ns | 179.94us | **12242.27x** |
| mp4_small | 58ns | 2.36us | **40.82x** |
| mp4_large | 66ns | 17.08us | **259.72x** |

## Summary

- **Date**: 2026-02-05_10-21-47
- **Python benchmarks >= 100x**: 2/10
- **Criterion faster than lofty**: 8/8

### Changes from previous run

- MP3: 18.5x -> 17.8x (-0.7)
- FLAC: 7.4x -> 7.5x (+0.1)
- OGG: 12.2x -> 12.0x (-0.2)
- MP4: 15.4x -> 15.8x (+0.4)
- Auto: 19.1x -> 18.8x (-0.3)
- B.MP3: 101.4x -> 110.5x (+9.1)
- B.FLAC: 23.2x -> 25.0x (+1.8)
- B.OGG: 17.9x -> 20.5x (+2.6)
- B.MP4: 61.2x -> 63.6x (+2.4)
- B.Auto: 100.2x -> 106.9x (+6.7)
- mp3_small (vs lofty): 2.63x -> 39.6x (+36.97)
- mp3_large (vs lofty): 2.76x -> 132.91x (+130.15)
- flac_small (vs lofty): 19.99x -> 20.39x (+0.40)
- flac_large (vs lofty): 130.73x -> 129.34x (-1.39)
- ogg_small (vs lofty): 7.88x -> 43.02x (+35.14)
- ogg_large (vs lofty): 129.8x -> 12242.27x (+12112.47)
- mp4_small (vs lofty): 21.99x -> 40.82x (+18.83)
- mp4_large (vs lofty): 3.47x -> 259.72x (+256.25)
