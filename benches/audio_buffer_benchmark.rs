use criterion::{black_box, criterion_group, criterion_main, Criterion};
use life_logging::audio::buffer::AudioBuffer;

fn benchmark_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("write");
    let data = vec![0.1f32; 1000];

    group.bench_function("optimized", |b| {
        let mut buffer = AudioBuffer::new(10000, 44100);
        b.iter(|| buffer.write_fast(black_box(&data)));
    }); // time:   [54.448 ns 55.129 ns 55.903 ns]

    group.bench_function("simple", |b| {
        let mut buffer = AudioBuffer::new(10000, 44100);
        b.iter(|| buffer.write(black_box(&data)));
    }); // time:   [647.37 ns 648.06 ns 648.89 ns]

    group.finish();
}

criterion_group!(benches, benchmark_write);
criterion_main!(benches);
