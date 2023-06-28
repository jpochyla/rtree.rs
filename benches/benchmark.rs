use blink_alloc::Blink;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use std::hint::black_box;
use std::time::Duration;

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(200)
        .warm_up_time(Duration::from_secs(10))
        .measurement_time(Duration::from_secs(10));
    targets = benchmark
}
criterion_main!(benches);

fn benchmark(c: &mut Criterion) {
    fastrand::seed(0xdeadbeef);

    let data = data();
    let pts = || data.pts.iter().copied().enumerate();

    static mut BLINK: Blink = Blink::new();

    unsafe {
        let mut my_tr = rtree::RTree::new(&BLINK);
        pts().for_each(|(i, p)| my_tr.insert(rtree::Rect::point(p[0], p[1]), i));

        let mut tr = rtree_rs::RTree::new();
        pts().for_each(|(i, p)| tr.insert(rtree_rs::Rect::new_point(p), i));

        my_tr.iter().zip(tr.scan()).for_each(|(x, y)| {
            assert_eq!(*x.data, *y.data);
        });
    }

    c.bench_function("rtree_rs insert", |b| {
        b.iter_batched_ref(
            || rtree_rs::RTree::new(),
            |tr| pts().for_each(|(i, p)| tr.insert(rtree_rs::Rect::new_point(p), i)),
            BatchSize::LargeInput,
        );
    });
    c.bench_function("rtree_rs search-item", |b| {
        b.iter_batched_ref(
            || {
                let mut tr = rtree_rs::RTree::new();
                pts().for_each(|(i, p)| tr.insert(rtree_rs::Rect::new_point(p), i));
                tr
            },
            |tr| {
                pts().for_each(|(_, p)| {
                    tr.search(rtree_rs::Rect::new_point(p)).for_each(|x| {
                        black_box(x);
                    });
                })
            },
            BatchSize::LargeInput,
        );
    });

    c.bench_function("rtree insert", |b| {
        b.iter_batched_ref(
            || unsafe {
                BLINK.reset();
                rtree::RTree::new(&BLINK)
            },
            |tr| pts().for_each(|(i, [x, y])| tr.insert(rtree::Rect::point(x, y), i)),
            BatchSize::LargeInput,
        );
    });
    c.bench_function("rtree search-item", |b| {
        b.iter_batched_ref(
            || unsafe {
                BLINK.reset();
                let mut tr = rtree::RTree::new(&BLINK);
                pts().for_each(|(i, [x, y])| tr.insert(rtree::Rect::point(x, y), i));
                tr
            },
            |tr| {
                pts().for_each(|(_, [x, y])| {
                    tr.search(rtree::Rect::point(x, y)).for_each(|x| {
                        black_box(x);
                    });
                })
            },
            BatchSize::LargeInput,
        );
    });

    c.bench_function("rstar insert", |b| {
        b.iter_batched_ref(
            || rstar::RTree::new(),
            |tr| pts().for_each(|(_, p)| tr.insert(p)),
            BatchSize::LargeInput,
        );
    });
    c.bench_function("rstar search-item", |b| {
        b.iter_batched_ref(
            || {
                let mut tr = rstar::RTree::new();
                pts().for_each(|(_, p)| tr.insert(p));
                tr
            },
            |tr| {
                pts().for_each(|(_, p)| {
                    let rect = rstar::AABB::from_corners(p, p);
                    tr.locate_in_envelope_intersecting(&rect).for_each(|x| {
                        black_box(x);
                    });
                })
            },
            BatchSize::LargeInput,
        );
    });
}

type P = [f32; 2];

struct Data {
    pts: Vec<P>,
    r1: Vec<(P, P)>,
    r5: Vec<(P, P)>,
    r10: Vec<(P, P)>,
}

const N: usize = 1_000;

fn data() -> Data {
    let mut pts = Vec::new();
    for _ in 0..N {
        let pt = [
            fastrand::f32() * 360.0 - 180.0,
            fastrand::f32() * 180.0 - 90.0,
        ];
        pts.push(pt);
    }

    // 1%
    let mut r1 = Vec::new();
    for _ in 0..10_000 {
        let p = 0.01;
        let min = [
            fastrand::f32() * 360.0 - 180.0,
            fastrand::f32() * 180.0 - 90.0,
        ];
        let max = [min[0] + 360.0 * p, min[1] + 180.0 * p];
        r1.push((min, max));
    }
    // 5%
    let mut r5 = Vec::new();
    for _ in 0..10_000 {
        let p = 0.05;
        let min = [
            fastrand::f32() * 360.0 - 180.0,
            fastrand::f32() * 180.0 - 90.0,
        ];
        let max = [min[0] + 360.0 * p, min[1] + 180.0 * p];
        r5.push((min, max));
    }
    // 10%
    let mut r10 = Vec::new();
    for _ in 0..10_000 {
        let p = 0.10;
        let min = [
            fastrand::f32() * 360.0 - 180.0,
            fastrand::f32() * 180.0 - 90.0,
        ];
        let max = [min[0] + 360.0 * p, min[1] + 180.0 * p];
        r10.push((min, max));
    }

    Data { pts, r1, r5, r10 }
}

// fn manual_benchmark() {
//     let Data { pts, r1, r5, r10 } = data();
//
//     println!(">>> rtree_rs::RTree <<<");
//     let mut tr = rtree_rs::RTree::new();
//     print!("insert:        ");
//     lotsa::ops(pts.len(), 1, |i, _| {
//         tr.insert(rtree_rs::Rect::new(pts[i], pts[i]), i);
//     });
//     print!("search-item:   ");
//     lotsa::ops(pts.len(), 1, |i, _| {
//         for _ in tr.search(rtree_rs::Rect::new(pts[i], pts[i])) {
//             break;
//         }
//     });
//     print!("search-1%:     ");
//     lotsa::ops(r1.len(), 1, |i, _| {
//         for _ in tr.search(rtree_rs::Rect::new(r1[i].0, r1[i].1)) {}
//     });
//     print!("search-5%:     ");
//     lotsa::ops(r5.len(), 1, |i, _| {
//         for _ in tr.search(rtree_rs::Rect::new(r5[i].0, r5[i].1)) {}
//     });
//     print!("search-10%:    ");
//     lotsa::ops(r10.len(), 1, |i, _| {
//         for _ in tr.search(rtree_rs::Rect::new(r10[i].0, r10[i].1)) {}
//     });
//     print!("remove-half:   ");
//     lotsa::ops(pts.len() / 2, 1, |i, _| {
//         tr.remove(rtree_rs::Rect::new(pts[i * 2], pts[i * 2]), &(i * 2))
//             .unwrap();
//     });
//     print!("reinsert-half: ");
//     lotsa::ops(pts.len() / 2, 1, |i, _| {
//         tr.insert(rtree_rs::Rect::new(pts[i * 2], pts[i * 2]), i * 2);
//     });
//     print!("search-item:   ");
//     lotsa::ops(pts.len(), 1, |i, _| {
//         for _ in tr.search(rtree_rs::Rect::new(pts[i], pts[i])) {
//             break;
//         }
//     });
//     print!("search-1%:     ");
//     lotsa::ops(r1.len(), 1, |i, _| {
//         for _ in tr.search(rtree_rs::Rect::new(r1[i].0, r1[i].1)) {}
//     });
//     print!("remove-all:    ");
//     lotsa::ops(pts.len(), 1, |i, _| {
//         tr.remove(rtree_rs::Rect::new(pts[i], pts[i]), &i).unwrap();
//     });
//
//     println!(">>> rtree::RTree <<<");
//     let mut tr = rtree::RTree::new();
//     print!("insert:        ");
//     lotsa::ops(pts.len(), 1, |i, _| {
//         tr.insert(rtree::Rect::new(pts[i], pts[i]), i);
//     });
//     print!("search-item:   ");
//     lotsa::ops(pts.len(), 1, |i, _| {
//         for _ in tr.search(rtree::Rect::new(pts[i], pts[i])) {
//             break;
//         }
//     });
//     print!("search-1%:     ");
//     lotsa::ops(r1.len(), 1, |i, _| {
//         for _ in tr.search(rtree::Rect::new(r1[i].0, r1[i].1)) {}
//     });
//     print!("search-5%:     ");
//     lotsa::ops(r5.len(), 1, |i, _| {
//         for _ in tr.search(rtree::Rect::new(r5[i].0, r5[i].1)) {}
//     });
//     print!("search-10%:    ");
//     lotsa::ops(r10.len(), 1, |i, _| {
//         for _ in tr.search(rtree::Rect::new(r10[i].0, r10[i].1)) {}
//     });
//     print!("remove-half:   ");
//     lotsa::ops(pts.len() / 2, 1, |i, _| {
//         tr.remove(rtree::Rect::new(pts[i * 2], pts[i * 2]), &(i * 2))
//             .unwrap();
//     });
//     print!("reinsert-half: ");
//     lotsa::ops(pts.len() / 2, 1, |i, _| {
//         tr.insert(rtree::Rect::new(pts[i * 2], pts[i * 2]), i * 2);
//     });
//     print!("search-item:   ");
//     lotsa::ops(pts.len(), 1, |i, _| {
//         for _ in tr.search(rtree::Rect::new(pts[i], pts[i])) {
//             break;
//         }
//     });
//     print!("search-1%:     ");
//     lotsa::ops(r1.len(), 1, |i, _| {
//         for _ in tr.search(rtree::Rect::new(r1[i].0, r1[i].1)) {}
//     });
//     print!("remove-all:    ");
//     lotsa::ops(pts.len(), 1, |i, _| {
//         tr.remove(rtree::Rect::new(pts[i], pts[i]), &i).unwrap();
//     });
//
//     println!();
//     println!(">>> rstar::RTree <<<");
//     let mut tr = rstar::RTree::new();
//     print!("insert:        ");
//     lotsa::ops(N, 1, |i, _| {
//         tr.insert(pts[i]);
//     });
//     print!("search-item:   ");
//     lotsa::ops(N, 1, |i, _| {
//         let rect = rstar::AABB::from_corners(pts[i], pts[i]);
//         for _ in tr.locate_in_envelope_intersecting(&rect) {
//             break;
//         }
//     });
//     print!("search-1%:     ");
//     lotsa::ops(r1.len(), 1, |i, _| {
//         let rect = rstar::AABB::from_corners(r1[i].0, r1[i].1);
//         for _ in tr.locate_in_envelope_intersecting(&rect) {}
//     });
//     print!("search-5%:     ");
//     lotsa::ops(r5.len(), 1, |i, _| {
//         let rect = rstar::AABB::from_corners(r5[i].0, r5[i].1);
//         for _ in tr.locate_in_envelope_intersecting(&rect) {}
//     });
//     print!("search-10%:    ");
//     lotsa::ops(r10.len(), 1, |i, _| {
//         let rect = rstar::AABB::from_corners(r10[i].0, r10[i].1);
//         for _ in tr.locate_in_envelope_intersecting(&rect) {}
//     });
//     print!("remove-half:   ");
//     lotsa::ops(pts.len() / 2, 1, |i, _| {
//         tr.remove(&pts[i * 2]).unwrap();
//     });
//     print!("reinsert-half: ");
//     lotsa::ops(pts.len() / 2, 1, |i, _| {
//         tr.insert(pts[i * 2]);
//     });
//     print!("search-item:   ");
//     lotsa::ops(N, 1, |i, _| {
//         let rect = rstar::AABB::from_corners(pts[i], pts[i]);
//         for _ in tr.locate_in_envelope_intersecting(&rect) {
//             break;
//         }
//     });
//     print!("search-1%:     ");
//     lotsa::ops(r1.len(), 1, |i, _| {
//         let rect = rstar::AABB::from_corners(r1[i].0, r1[i].1);
//         for _ in tr.locate_in_envelope_intersecting(&rect) {}
//     });
//     print!("remove-all:    ");
//     lotsa::ops(pts.len(), 1, |i, _| {
//         tr.remove(&pts[i]).unwrap();
//     });
// }
