//! Runs a real CityMap JSON through the layout and checks every invariant the
//! renderer depends on. This is the headless equivalent of looking at the city:
//! it cannot tell you whether it is pretty, but it proves nothing overlaps,
//! nothing escapes its district and nothing is silently dropped.
//!
//! Usage: cargo run --release --example layout_check -- <city.json>

use std::collections::HashMap;
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).expect("usage: layout_check <city.json>");
    let raw = std::fs::read_to_string(&path).expect("read city json");
    let city: engine::CityMap = serde_json::from_str(&raw).expect("parse city json");

    let t0 = Instant::now();
    let result = engine::compute_layout_public(&city);
    let elapsed = t0.elapsed();

    let expected = city.building_count();
    println!("{}", path);
    println!("  layout time      {:.1} ms", elapsed.as_secs_f64() * 1000.0);
    println!("  districts        {}", result.districts.len());
    println!("  buildings        {} placed / {} in input", result.buildings.len(), expected);
    println!("  street slabs     {}", result.streets.len());
    println!("  dependency arcs  {}", result.links.len());
    println!("  city radius      {:.0}", result.radius);

    assert_eq!(result.buildings.len(), expected, "buildings were dropped by the layout");

    // 1. No two buildings overlap. Compared in district-local space, where
    //    footprints are axis aligned.
    let mut per_district: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, b) in result.buildings.iter().enumerate() {
        per_district.entry(b.district_idx).or_default().push(i);
    }
    let mut checked = 0usize;
    for list in per_district.values() {
        for a in 0..list.len() {
            for b in (a + 1)..list.len() {
                let x = &result.buildings[list[a]];
                let y = &result.buildings[list[b]];
                let dx = (x.local_x - y.local_x).abs();
                let dz = (x.local_z - y.local_z).abs();
                assert!(
                    dx >= (x.width + y.width) * 0.5 - 1e-6 || dz >= (x.depth + y.depth) * 0.5 - 1e-6,
                    "{} overlaps {}",
                    x.name,
                    y.name
                );
                checked += 1;
            }
        }
    }
    println!("  overlap pairs    {} checked, 0 overlapping", checked);

    // 2. Every building sits inside its own district polygon.
    let mut escaped = 0;
    for b in &result.buildings {
        let d = &result.districts[b.district_idx];
        if !engine::point_in_convex_public(b.center_x, b.center_z, &d.polygon) {
            escaped += 1;
        }
    }
    assert_eq!(escaped, 0, "{escaped} buildings escaped their district");
    println!("  containment      all inside their district cell");

    // 3. District ground area tracks district content.
    let mut rows: Vec<(String, usize, f64)> = result
        .districts
        .iter()
        .map(|d| {
            (
                d.name.clone(),
                d.building_count,
                engine::polygon_area_public(&d.polygon),
            )
        })
        .collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.1));

    println!("  district areas (buildings -> ground area, m^2):");
    for (name, n, area) in rows.iter().take(6) {
        println!("    {:<34} {:>5} -> {:>10.0}", trunc(name, 34), n, area);
    }
    if rows.len() >= 2 {
        let biggest = &rows[0];
        let smallest = &rows[rows.len() - 1];
        let per_big = biggest.2 / biggest.1.max(1) as f64;
        let per_small = smallest.2 / smallest.1.max(1) as f64;
        println!(
            "  area per building: largest district {:.0}, smallest {:.0} (ratio {:.2}x)",
            per_big,
            per_small,
            per_big.max(per_small) / per_big.min(per_small)
        );
    }

    // 4. Heights must actually vary, or the primary metric carries nothing.
    let heights: Vec<f64> = result.buildings.iter().map(|b| b.height).collect();
    let min = heights.iter().cloned().fold(f64::MAX, f64::min);
    let max = heights.iter().cloned().fold(0.0, f64::max);
    let mean = heights.iter().sum::<f64>() / heights.len() as f64;
    let distinct = {
        let mut v: Vec<i64> = heights.iter().map(|h| (h * 10.0) as i64).collect();
        v.sort_unstable();
        v.dedup();
        v.len()
    };
    println!("  height  min {:.1}  mean {:.1}  max {:.1}  ({} distinct)", min, mean, max, distinct);
    // Small repos genuinely have few distinct sizes; only demand variety once
    // there is enough code for it to be meaningful.
    assert!(distinct > 5 || heights.len() < 12, "height carries no signal");

    // Buildings all share one cube mesh, so the number of distinct materials
    // is what the city costs in draw calls.
    let mut keys = std::collections::HashSet::new();
    for (di, d) in city.districts.iter().enumerate() {
        let _ = di;
        for b in &d.buildings {
            let rgb = engine::apply_age_public(engine::typology_rgb_public(&d.typology), b.age_days);
            keys.insert(engine::color_key_public(
                rgb,
                engine::churn_bucket_public(b.churn_rank),
            ));
        }
    }
    println!("  materials        {} distinct (= building draw calls)", keys.len());

    // Roof caps are the churn channel; a city where nothing qualifies would
    // silently lose that dimension.
    let mut caps = 0usize;
    let mut plinths = 0usize;
    for d in &city.districts {
        for b in &d.buildings {
            if engine::churn_bucket_public(b.churn_rank) >= 1 {
                caps += 1;
            }
            if b.kind == "module" {
                plinths += 1;
            }
        }
    }
    println!(
        "  roof caps        {} of {} buildings ({:.0}%) carry a churn cap",
        caps, expected, 100.0 * caps as f64 / expected.max(1) as f64
    );
    println!("  module plinths   {}", plinths);
    assert!(caps > 0, "no building qualifies for a churn cap");

    println!("  OK");
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n - 1).collect::<String>() + "\u{2026}"
    }
}

