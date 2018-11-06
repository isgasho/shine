#![feature(custom_attribute)]

extern crate log;
extern crate quickcheck;
extern crate shine_testutils;
extern crate shine_tri;

mod common;

use common::{SimpleTrif32, SimpleTrii64};
use quickcheck::quickcheck;
use shine_testutils::init_quickcheck_test;
use shine_tri::geometry::position::{Posf32, Posi64};
use shine_tri::{Builder, Checker};

#[test]
fn stress_exact_i64() {
    init_quickcheck_test(module_path!(), 1000);

    // Even tough the graph has a point type of i64, we are testing with i8 to
    //  - avoid overflow error.
    //  - test gets more dense to generate more extremal case
    fn fuzzer(xs: Vec<(i8, i8)>) -> bool {
        let mut tri = SimpleTrii64::default();
        {
            for &(x, y) in xs.iter() {
                tri.add_vertex(
                    Posi64 {
                        x: x as i64,
                        y: y as i64,
                    },
                    None,
                );
            }
        }
        tri.check(None) == Ok(())
    }

    quickcheck(fuzzer as fn(Vec<(i8, i8)>) -> bool);
}

#[test]
#[ignore]
fn stress_exactf32() {
    init_quickcheck_test(module_path!(), 1000);

    fn fuzzer(xs: Vec<(f32, f32)>) -> bool {
        let mut tri = SimpleTrif32/*Exact*/::default();
        {
            for &(x, y) in xs.iter() {
                tri.add_vertex(Posf32 { x, y }, None);
            }
        }
        tri.check(None) == Ok(())
    }

    fn fuzzer_01(xs: Vec<(f32, f32)>) -> bool {
        let mut tri = SimpleTrif32/*Exact*/::default();
        {
            for &(x, y) in xs.iter() {
                tri.add_vertex(
                    Posf32 {
                        x: x.fract(),
                        y: y.fract(),
                    },
                    None,
                );
            }
        }
        tri.check(None) == Ok(())
    }

    quickcheck(fuzzer as fn(Vec<(f32, f32)>) -> bool);
    quickcheck(fuzzer_01 as fn(Vec<(f32, f32)>) -> bool);
}
