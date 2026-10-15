//! Decode benchmarks (brief §8.5): `cargo bench -p kinora_decode`.
//! Uses the committed fixtures, so numbers are comparable between machines.

use std::hint::black_box;
use std::path::PathBuf;

use criterion::{Criterion, criterion_group, criterion_main};
use kinora_decode::{DecodeOptions, Target, decode};

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../test_data/images")
        .join(name);
    std::fs::read(&path).unwrap_or_default()
}

fn decoders(c: &mut Criterion) {
    let full = DecodeOptions::default();
    let screen = DecodeOptions {
        target: Target::Screen {
            width: 480,
            height: 270,
            keep_full_below_bytes: 0,
        },
        ..DecodeOptions::default()
    };
    for name in [
        "photo.jpg",
        "rgba.png",
        "still.webp",
        "still.avif",
        "still.jxl",
        "still.heic",
    ] {
        let bytes = fixture(name);
        if bytes.is_empty() {
            continue;
        }
        c.bench_function(&format!("decode {name}"), |b| {
            b.iter(|| decode(black_box(&bytes), None, &full));
        });
    }
    let photo = fixture("photo.jpg");
    c.bench_function("decode photo.jpg for a 480×270 screen", |b| {
        b.iter(|| decode(black_box(&photo), None, &screen));
    });
}

criterion_group!(benches, decoders);
criterion_main!(benches);
