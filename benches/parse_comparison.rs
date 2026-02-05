use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::io::Cursor;

// Large benchmark files (from lofty-rs bench assets)
const LARGE_MP3: &[u8] = include_bytes!("../lofty-rs/benches/assets/01 TempleOS Hymn Risen (Remix).mp3");
const LARGE_OGG: &[u8] = include_bytes!("../lofty-rs/benches/assets/01 TempleOS Hymn Risen (Remix).ogg");
const LARGE_M4A: &[u8] = include_bytes!("../lofty-rs/benches/assets/01 TempleOS Hymn Risen (Remix).m4a");
// Small test files
const SMALL_MP3: &[u8] = include_bytes!("../test_files/id3v1v2-combined.mp3");
const SMALL_M4A: &[u8] = include_bytes!("../test_files/has-tags.m4a");
const SMALL_OGG: &[u8] = include_bytes!("../test_files/empty.ogg");

fn parse_lofty(data: &[u8]) -> lofty::file::TaggedFile {
    let cursor = Cursor::new(data);
    lofty::probe::Probe::new(cursor).guess_file_type().unwrap().read().unwrap()
}

fn bench_mp3(c: &mut Criterion) {
    let mut group = c.benchmark_group("mp3_large");
    group.bench_function("mutagen_rs", |b| {
        b.iter(|| {
            mutagen_rs::mp3::MP3File::parse(black_box(LARGE_MP3), "test.mp3").unwrap()
        })
    });
    group.bench_function("lofty", |b| {
        b.iter(|| parse_lofty(black_box(LARGE_MP3)))
    });
    group.finish();

    let mut group = c.benchmark_group("mp3_small");
    group.bench_function("mutagen_rs", |b| {
        b.iter(|| {
            mutagen_rs::mp3::MP3File::parse(black_box(SMALL_MP3), "test.mp3").unwrap()
        })
    });
    group.bench_function("lofty", |b| {
        b.iter(|| parse_lofty(black_box(SMALL_MP3)))
    });
    group.finish();
}

fn bench_flac(c: &mut Criterion) {
    // FLAC is 26MB - load at runtime to avoid bloating binary
    let large_flac = std::fs::read("lofty-rs/benches/assets/01 TempleOS Hymn Risen (Remix).flac")
        .expect("FLAC bench file not found");
    let small_flac = std::fs::read("test_files/flac_application.flac")
        .unwrap_or_else(|_| {
            std::fs::read_dir("test_files")
                .unwrap()
                .filter_map(|e| e.ok())
                .find(|e| e.path().extension().is_some_and(|ext| ext == "flac"))
                .map(|e| std::fs::read(e.path()).unwrap())
                .expect("No FLAC test files found")
        });

    let mut group = c.benchmark_group("flac_large");
    group.bench_function("mutagen_rs", |b| {
        b.iter(|| {
            mutagen_rs::flac::FLACFile::parse(black_box(&large_flac), "test.flac").unwrap()
        })
    });
    group.bench_function("lofty", |b| {
        b.iter(|| parse_lofty(black_box(&large_flac)))
    });
    group.finish();

    let mut group = c.benchmark_group("flac_small");
    group.bench_function("mutagen_rs", |b| {
        b.iter(|| {
            mutagen_rs::flac::FLACFile::parse(black_box(&small_flac), "test.flac").unwrap()
        })
    });
    group.bench_function("lofty", |b| {
        b.iter(|| parse_lofty(black_box(&small_flac)))
    });
    group.finish();
}

fn bench_ogg(c: &mut Criterion) {
    let mut group = c.benchmark_group("ogg_large");
    group.bench_function("mutagen_rs", |b| {
        b.iter(|| {
            mutagen_rs::ogg::OggVorbisFile::parse(black_box(LARGE_OGG), "test.ogg").unwrap()
        })
    });
    group.bench_function("lofty", |b| {
        b.iter(|| parse_lofty(black_box(LARGE_OGG)))
    });
    group.finish();

    let mut group = c.benchmark_group("ogg_small");
    group.bench_function("mutagen_rs", |b| {
        b.iter(|| {
            mutagen_rs::ogg::OggVorbisFile::parse(black_box(SMALL_OGG), "test.ogg").unwrap()
        })
    });
    group.bench_function("lofty", |b| {
        b.iter(|| parse_lofty(black_box(SMALL_OGG)))
    });
    group.finish();
}

fn bench_mp4(c: &mut Criterion) {
    let mut group = c.benchmark_group("mp4_large");
    group.bench_function("mutagen_rs", |b| {
        b.iter(|| {
            mutagen_rs::mp4::MP4File::parse(black_box(LARGE_M4A), "test.m4a").unwrap()
        })
    });
    group.bench_function("lofty", |b| {
        b.iter(|| parse_lofty(black_box(LARGE_M4A)))
    });
    group.finish();

    let mut group = c.benchmark_group("mp4_small");
    group.bench_function("mutagen_rs", |b| {
        b.iter(|| {
            mutagen_rs::mp4::MP4File::parse(black_box(SMALL_M4A), "test.m4a").unwrap()
        })
    });
    group.bench_function("lofty", |b| {
        b.iter(|| parse_lofty(black_box(SMALL_M4A)))
    });
    group.finish();
}

criterion_group!(benches, bench_mp3, bench_flac, bench_ogg, bench_mp4);
criterion_main!(benches);
