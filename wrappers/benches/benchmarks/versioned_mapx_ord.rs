use criterion::{criterion_group, Criterion};
use rand::Rng;
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use vsdb::{versioned::mapx_ord::MapxOrdVs, VersionName, VsMgmt};

fn read_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("** vsdb::versioned::mapx_ord::MapxOrdVs **");
    group
        .measurement_time(Duration::from_secs(9))
        .sample_size(100);

    let i = AtomicUsize::new(0);
    let mut db = MapxOrdVs::new();
    db.version_create(VersionName(b"version0")).unwrap();

    group.bench_function(" write ", |b| {
        b.iter(|| {
            let n = i.fetch_add(1, Ordering::SeqCst);

            db.insert(n, n).unwrap();
        })
    });

    group.bench_function(" read ", |b| {
        b.iter(|| {
            let n = i.fetch_sub(1, Ordering::SeqCst);
            db.get(&n);
        })
    });
    group.finish();
}

fn random_read_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("** vsdb::versioned::mapx_ord::MapxOrdVs **");
    group
        .measurement_time(Duration::from_secs(9))
        .sample_size(100);

    let mut keys = vec![];
    let mut rng = rand::thread_rng();
    let mut db = MapxOrdVs::new();
    db.version_create(VersionName(b"version0")).unwrap();

    group.bench_function(" random write ", |b| {
        b.iter(|| {
            let n = rng.gen::<usize>();
            db.insert(n, n).unwrap();
            keys.push(n);
        })
    });

    group.bench_function(" random read ", |b| {
        b.iter(|| {
            let index: usize = rng.gen_range(0..keys.len());
            keys.get(index).map(|key| db.get(key));
        })
    });
    group.finish();
}

fn version_read_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("** vsdb::versioned::mapx_ord::MapxOrdVs **");
    group
        .measurement_time(Duration::from_secs(9))
        .sample_size(100);

    let i = AtomicUsize::new(0);
    let mut db = MapxOrdVs::new();

    group.bench_function(" write ", |b| {
        b.iter(|| {
            let n = i.fetch_add(1, Ordering::SeqCst);
            db.version_create(VersionName(&n.to_be_bytes())).unwrap();
            db.insert(n, n).unwrap();
        })
    });

    group.bench_function(" read ", |b| {
        b.iter(|| {
            let n = i.fetch_sub(1, Ordering::SeqCst);
            db.get_by_branch_version(
                &n,
                db.branch_get_default().as_deref(),
                VersionName(&n.to_be_bytes()),
            );
        })
    });
    group.finish();
}

fn version_random_read_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("** vsdb::versioned::mapx_ord::MapxOrdVs **");
    group
        .measurement_time(Duration::from_secs(9))
        .sample_size(100);

    let mut keys = vec![];
    let mut rng = rand::thread_rng();
    let mut db = MapxOrdVs::new();

    group.bench_function(" random write ", |b| {
        b.iter(|| {
            let n = rng.gen::<usize>();
            db.version_create(VersionName(&n.to_be_bytes())).unwrap();
            db.insert(n, n).unwrap();
            keys.push(n);
        })
    });

    group.bench_function(" random read ", |b| {
        b.iter(|| {
            let index: usize = rng.gen_range(0..keys.len());
            keys.get(index).map(|key| db.get(key));
        })
    });
    group.finish();
}

fn branch_version_read_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("** vsdb::versioned::mapx_ord::MapxOrdVs **");
    group
        .measurement_time(Duration::from_secs(9))
        .sample_size(100);

    let i = AtomicUsize::new(0);
    let mut db = MapxOrdVs::new();
    db.version_create(VersionName(b"version0")).unwrap();

    group.bench_function(" write ", |b| {
        b.iter(|| {
            let n = i.fetch_add(1, Ordering::SeqCst);

            db.insert(n, n).unwrap();
        })
    });

    group.bench_function(" read ", |b| {
        b.iter(|| {
            let n = i.fetch_sub(1, Ordering::SeqCst);
            db.get(&n);
        })
    });
    group.finish();
}

fn branch_version_random_read_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("** vsdb::versioned::mapx_ord::MapxOrdVs **");
    group
        .measurement_time(Duration::from_secs(9))
        .sample_size(100);

    let mut keys = vec![];
    let mut rng = rand::thread_rng();
    let mut db = MapxOrdVs::new();
    db.version_create(VersionName(b"version0")).unwrap();

    group.bench_function(" random write ", |b| {
        b.iter(|| {
            let n = rng.gen::<usize>();
            db.insert(n, n).unwrap();
            keys.push(n);
        })
    });

    group.bench_function(" random read ", |b| {
        b.iter(|| {
            let index: usize = rng.gen_range(0..keys.len());
            keys.get(index).map(|key| db.get(key));
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    read_write,
    random_read_write,
    version_read_write,
    version_random_read_write,
    branch_version_read_write,
    branch_version_random_read_write
);
