//! The "Ramps" palette sort: colors are grouped into ramps by hue and chroma
//! in OKLab, neutrals first, each ramp ordered dark to light.
//!
//! `Cref`, the 90th-percentile chroma, scales every threshold below so the
//! grouping adapts to how saturated the palette is.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::f64::consts::PI;

/// Merge radius for single-link clustering, as a multiple of `Cref`.
const MERGE_K: f64 = 0.12;
/// Mean chroma, as a multiple of `Cref`, below which a group counts as neutral.
const NEUTRAL_K: f64 = 0.30;
/// Weight on the chroma term of the merge distance, relative to the hue term.
const CHROMA_W: f64 = 0.55;
/// `Cref` below which the whole palette is treated as grayscale.
const GRAY_PAL: f64 = 0.02;

/// Lightness, chroma and hue (radians) of one color in OKLab / OKLCh.
struct Lch {
    l: f64,
    c: f64,
    h: f64,
}

/// `colors` reordered into ramps: indices into `colors`, one permutation of
/// `0..colors.len()`. Deterministic regardless of the input order.
pub fn ramp_order(colors: &[[u8; 3]]) -> Vec<usize> {
    let n = colors.len();
    if n < 2 {
        return (0..n).collect();
    }

    let p: Vec<Lch> = colors
        .iter()
        .map(|&rgb| {
            let (l, a, b) = oklab(rgb);
            Lch {
                l,
                c: a.hypot(b),
                h: b.atan2(a),
            }
        })
        .collect();

    let cref = chroma_percentile(&p, 0.9);
    if cref < GRAY_PAL {
        return sorted_by_lightness(&p, 0..n);
    }

    let merge_threshold = MERGE_K * cref;
    let neutral_threshold = NEUTRAL_K * cref;

    let groups = single_link_groups(&p, merge_threshold);

    let (neutral, chrom) = split_neutral(&p, groups, neutral_threshold);
    let chrom = rotate_at_widest_gap(&p, chrom);

    let mut out = Vec::with_capacity(n);
    if let Some(group) = neutral {
        out.extend(sorted_by_lightness(&p, group.into_iter()));
    }
    for group in chrom {
        out.extend(sorted_by_lightness(&p, group.into_iter()));
    }
    out
}

/// sRGB8 -> linear -> OKLab, Ottosson's constants.
fn oklab(rgb: [u8; 3]) -> (f64, f64, f64) {
    let r = srgb_to_linear(rgb[0]);
    let g = srgb_to_linear(rgb[1]);
    let b = srgb_to_linear(rgb[2]);

    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).powf(1.0 / 3.0);
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).powf(1.0 / 3.0);
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).powf(1.0 / 3.0);

    (
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    )
}

fn srgb_to_linear(channel: u8) -> f64 {
    let v = channel as f64 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// The `q`th percentile (0..1) of the chroma values, nearest-rank with
/// round-half-up: `cs[min(n - 1, floor(q * (n - 1) + 0.5))]`.
fn chroma_percentile(p: &[Lch], q: f64) -> f64 {
    let mut cs: Vec<f64> = p.iter().map(|x| x.c).collect();
    cs.sort_by(|a, b| cmp_f64(*a, *b));
    let n = cs.len();
    let idx = ((q * (n as f64 - 1.0) + 0.5) as usize).min(n - 1);
    cs[idx]
}

fn cmp_f64(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

/// `a mod b`, with the sign of `b` (Python's `%`, not Rust's `%`).
fn py_mod(a: f64, b: f64) -> f64 {
    let r = a % b;
    if r != 0.0 && r.is_sign_negative() != b.is_sign_negative() {
        r + b
    } else {
        r
    }
}

/// Shortest angular difference between two hues, in radians, unsigned.
fn hue_dist(h1: f64, h2: f64) -> f64 {
    (py_mod(h2 - h1 + PI, 2.0 * PI) - PI).abs()
}

/// Merge distance between two colors: chroma difference weighted against a
/// chroma-scaled hue difference.
fn dist(p: &[Lch], u: usize, v: usize) -> f64 {
    let dc = (p[u].c - p[v].c).abs();
    let dh = hue_dist(p[u].h, p[v].h);
    (CHROMA_W * dc).hypot(p[u].c.min(p[v].c) * dh)
}

/// Single-link clusters: a Euclidean MST (Prim, O(n^2)) cut at edges longer
/// than `merge_threshold`. Groups are returned in the order their root was
/// first reached while walking `0..n`.
fn single_link_groups(p: &[Lch], merge_threshold: f64) -> Vec<Vec<usize>> {
    let n = p.len();

    let mut in_tree = vec![false; n];
    let mut best = vec![f64::INFINITY; n];
    let mut parent_of: Vec<i64> = vec![-1; n];
    best[0] = 0.0;
    let mut edges: Vec<(f64, usize, usize)> = Vec::with_capacity(n.saturating_sub(1));

    for _ in 0..n {
        let mut u = 0;
        let mut best_val = f64::INFINITY;
        for i in 0..n {
            if !in_tree[i] && best[i] < best_val {
                best_val = best[i];
                u = i;
            }
        }
        in_tree[u] = true;
        if parent_of[u] >= 0 {
            edges.push((best[u], parent_of[u] as usize, u));
        }
        for v in 0..n {
            if !in_tree[v] {
                let d = dist(p, u, v);
                if d < best[v] {
                    best[v] = d;
                    parent_of[v] = u as i64;
                }
            }
        }
    }

    let mut union_parent: Vec<usize> = (0..n).collect();
    for (d, u, v) in edges {
        if d <= merge_threshold {
            let ru = find(&mut union_parent, u);
            let rv = find(&mut union_parent, v);
            if ru != rv {
                union_parent[ru] = rv;
            }
        }
    }

    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut root_to_group: HashMap<usize, usize> = HashMap::new();
    for i in 0..n {
        let root = find(&mut union_parent, i);
        let group_idx = *root_to_group.entry(root).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[group_idx].push(i);
    }
    groups
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

/// The group with the lowest mean chroma becomes the neutral group, but only
/// when that mean is within `neutral_threshold`. The rest come back as the
/// chromatic groups, in their original order.
fn split_neutral(
    p: &[Lch],
    mut groups: Vec<Vec<usize>>,
    neutral_threshold: f64,
) -> (Option<Vec<usize>>, Vec<Vec<usize>>) {
    let mean_chroma =
        |g: &[usize]| -> f64 { g.iter().map(|&i| p[i].c).sum::<f64>() / g.len() as f64 };

    let mut candidate = 0;
    let mut best_mean = f64::INFINITY;
    for (idx, g) in groups.iter().enumerate() {
        let m = mean_chroma(g);
        if m < best_mean {
            best_mean = m;
            candidate = idx;
        }
    }

    if best_mean <= neutral_threshold {
        let neutral = groups.remove(candidate);
        (Some(neutral), groups)
    } else {
        (None, groups)
    }
}

/// Hue in degrees, wrapped to `[0, 360)`.
fn deg(p: &[Lch], i: usize) -> f64 {
    py_mod(p[i].h.to_degrees(), 360.0)
}

fn group_hue_min(p: &[Lch], g: &[usize]) -> f64 {
    g.iter().map(|&i| deg(p, i)).fold(f64::INFINITY, f64::min)
}

fn group_hue_max(p: &[Lch], g: &[usize]) -> f64 {
    g.iter()
        .map(|&i| deg(p, i))
        .fold(f64::NEG_INFINITY, f64::max)
}

/// Chromatic groups ordered by their minimum hue, then rotated to start right
/// after the widest empty hue arc between consecutive groups (wrapping
/// around the wheel).
fn rotate_at_widest_gap(p: &[Lch], mut chrom: Vec<Vec<usize>>) -> Vec<Vec<usize>> {
    if chrom.is_empty() {
        return chrom;
    }

    chrom.sort_by(|a, b| cmp_f64(group_hue_min(p, a), group_hue_min(p, b)));

    let len = chrom.len();
    let mut best_gap = f64::NEG_INFINITY;
    let mut best_k = 0;
    for k in 0..len {
        let next = &chrom[(k + 1) % len];
        let cur = &chrom[k];
        let gap = py_mod(group_hue_min(p, next) - group_hue_max(p, cur), 360.0);
        if gap >= best_gap {
            best_gap = gap;
            best_k = k;
        }
    }

    let mut rotated = Vec::with_capacity(len);
    rotated.extend(chrom.drain(best_k + 1..));
    rotated.append(&mut chrom);
    rotated
}

/// The indices in `group` sorted by (lightness, chroma) ascending.
fn sorted_by_lightness(p: &[Lch], group: impl Iterator<Item = usize>) -> Vec<usize> {
    let mut out: Vec<usize> = group.collect();
    out.sort_by(|&i, &j| cmp_f64(p[i].l, p[j].l).then_with(|| cmp_f64(p[i].c, p[j].c)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray(v: u8) -> [u8; 3] {
        [v, v, v]
    }

    #[test]
    fn grayscale_palette_sorts_dark_to_light() {
        let colors = [gray(200), gray(0), gray(120), gray(60), gray(255)];
        let order = ramp_order(&colors);
        let sorted: Vec<u8> = order.iter().map(|&i| colors[i][0]).collect();
        assert_eq!(sorted, vec![0, 60, 120, 200, 255]);
    }

    #[test]
    fn the_permutation_is_a_bijection() {
        let colors = [
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [255, 255, 0],
            [0, 255, 255],
            [255, 0, 255],
            [10, 10, 10],
            [245, 245, 245],
        ];
        let order = ramp_order(&colors);
        let mut seen: Vec<usize> = order.clone();
        seen.sort_unstable();
        assert_eq!(seen, (0..colors.len()).collect::<Vec<_>>());
    }

    #[test]
    fn shuffling_the_input_gives_the_same_colors_in_the_same_order() {
        let colors = [
            [228, 59, 68],
            [255, 178, 79],
            [255, 231, 98],
            [153, 229, 80],
            [46, 154, 63],
            [30, 74, 122],
            [93, 39, 93],
            [215, 123, 186],
            [255, 255, 255],
            [155, 173, 183],
            [87, 85, 92],
            [40, 40, 43],
        ];

        let order_a = ramp_order(&colors);
        let result_a: Vec<[u8; 3]> = order_a.iter().map(|&i| colors[i]).collect();

        let mut shuffled: Vec<[u8; 3]> = colors.into_iter().rev().collect();
        shuffled.rotate_left(3);
        let order_b = ramp_order(&shuffled);
        let result_b: Vec<[u8; 3]> = order_b.iter().map(|&i| shuffled[i]).collect();

        assert_eq!(result_a, result_b);
    }
}
