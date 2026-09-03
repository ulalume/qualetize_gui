//! The "Ramps" palette sort: neutrals first, then the chromatic colors cut into
//! hue blocks, each block ordered dark to light in OKLab.
//!
//! `Cref`, the 90th-percentile chroma, scales every chroma threshold below so the
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
/// Hue gap, in degrees, that starts a new block of chromatic colors.
const BLOCK_GAP: f64 = 25.0;

/// Lightness, chroma and hue (radians) of one color in OKLab / OKLCh.
struct Lch {
    l: f64,
    c: f64,
    h: f64,
}

/// `colors` reordered into ramps: indices into `colors`, one permutation of
/// `0..colors.len()`. Deterministic regardless of the input order.
///
/// The neutral group comes first, then the chromatic colors walk the hue wheel
/// from the widest empty arc. A hue gap of at least [`BLOCK_GAP`] starts a new
/// block; within a block, merge groups run dark to light, and so do the colors
/// inside each merge group.
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

    let all: Vec<usize> = (0..n).collect();

    let cref = chroma_percentile(&p, 0.9);
    if cref < GRAY_PAL {
        return sorted_by_lightness(&p, &all);
    }

    let merge_threshold = MERGE_K * cref;
    let neutral_threshold = NEUTRAL_K * cref;

    let groups = single_link_groups(&p, &all, merge_threshold);
    let neutral = neutral_group(&p, groups, neutral_threshold);

    let mut is_neutral = vec![false; n];
    for &i in &neutral {
        is_neutral[i] = true;
    }
    let mut chromatic: Vec<usize> = (0..n).filter(|&i| !is_neutral[i]).collect();
    chromatic.sort_by(|&i, &j| cmp_f64(deg(&p, i), deg(&p, j)));

    let mut out = sorted_by_lightness(&p, &neutral);
    for block in hue_blocks(&p, &chromatic) {
        let mut subgroups = single_link_groups(&p, &block, merge_threshold);
        subgroups.sort_by(|a, b| subgroup_key(&p, a).cmp_key(&subgroup_key(&p, b)));
        for group in subgroups {
            out.extend(sorted_by_lightness(&p, &group));
        }
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

/// Hue in degrees, wrapped to `[0, 360)`.
fn deg(p: &[Lch], i: usize) -> f64 {
    py_mod(p[i].h.to_degrees(), 360.0)
}

/// Merge distance between two colors: chroma difference weighted against a
/// chroma-scaled hue difference.
fn dist(p: &[Lch], u: usize, v: usize) -> f64 {
    let dc = (p[u].c - p[v].c).abs();
    let dh = hue_dist(p[u].h, p[v].h);
    (CHROMA_W * dc).hypot(p[u].c.min(p[v].c) * dh)
}

/// Single-link clusters over `members`: a Euclidean MST (Prim, O(n^2)) cut at
/// edges longer than `merge_threshold`. Groups are returned in the order their
/// root is first reached while walking `members`, and each group keeps the
/// relative order of `members`.
fn single_link_groups(p: &[Lch], members: &[usize], merge_threshold: f64) -> Vec<Vec<usize>> {
    let n = members.len();
    if n == 0 {
        return Vec::new();
    }

    let mut in_tree = vec![false; n];
    let mut best = vec![f64::INFINITY; n];
    let mut parent_of: Vec<i64> = vec![-1; n];
    best[0] = 0.0;
    let mut edges: Vec<(f64, usize, usize)> = Vec::with_capacity(n - 1);

    for _ in 0..n {
        let mut u = (0..n).find(|&i| !in_tree[i]).unwrap_or(0);
        let mut best_val = best[u];
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
                let d = dist(p, members[u], members[v]);
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
    for (i, &member) in members.iter().enumerate() {
        let root = find(&mut union_parent, i);
        let group_idx = *root_to_group.entry(root).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[group_idx].push(member);
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

fn mean_chroma(p: &[Lch], g: &[usize]) -> f64 {
    g.iter().map(|&i| p[i].c).sum::<f64>() / g.len() as f64
}

fn mean_lightness(p: &[Lch], g: &[usize]) -> f64 {
    g.iter().map(|&i| p[i].l).sum::<f64>() / g.len() as f64
}

/// The merge group with the lowest mean chroma, when that mean is within
/// `neutral_threshold`. Empty when the palette has no such group.
fn neutral_group(p: &[Lch], mut groups: Vec<Vec<usize>>, neutral_threshold: f64) -> Vec<usize> {
    let mut candidate = 0;
    let mut best_mean = f64::INFINITY;
    for (idx, g) in groups.iter().enumerate() {
        let m = mean_chroma(p, g);
        if m < best_mean {
            best_mean = m;
            candidate = idx;
        }
    }

    if best_mean <= neutral_threshold {
        groups.remove(candidate)
    } else {
        Vec::new()
    }
}

/// `chromatic`, already sorted by hue, cut into blocks at every hue gap of at
/// least [`BLOCK_GAP`], starting with the block that follows the widest gap.
/// Without any such gap the whole wheel is one block.
fn hue_blocks(p: &[Lch], chromatic: &[usize]) -> Vec<Vec<usize>> {
    let m = chromatic.len();
    if m == 0 {
        return Vec::new();
    }

    let gap = |k: usize| py_mod(deg(p, chromatic[(k + 1) % m]) - deg(p, chromatic[k]), 360.0);

    let mut widest = 0;
    for k in 1..m {
        if gap(k) >= gap(widest) {
            widest = k;
        }
    }
    let cuts: Vec<bool> = (0..m).map(|k| gap(k) >= BLOCK_GAP || k == widest).collect();

    let start = (widest + 1) % m;
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    for t in 0..m {
        let k = (start + t) % m;
        current.push(chromatic[k]);
        if cuts[k] {
            blocks.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

/// Sort key of a merge group inside a block: mean lightness, then the darkest
/// and the least saturated color, so equal means stay deterministic.
struct SubgroupKey(f64, f64, f64);

impl SubgroupKey {
    fn cmp_key(&self, other: &Self) -> Ordering {
        cmp_f64(self.0, other.0)
            .then_with(|| cmp_f64(self.1, other.1))
            .then_with(|| cmp_f64(self.2, other.2))
    }
}

fn subgroup_key(p: &[Lch], g: &[usize]) -> SubgroupKey {
    let min_l = g.iter().map(|&i| p[i].l).fold(f64::INFINITY, f64::min);
    let min_c = g.iter().map(|&i| p[i].c).fold(f64::INFINITY, f64::min);
    SubgroupKey(mean_lightness(p, g), min_l, min_c)
}

/// The indices in `group` sorted by (lightness, chroma) ascending.
fn sorted_by_lightness(p: &[Lch], group: &[usize]) -> Vec<usize> {
    let mut out: Vec<usize> = group.to_vec();
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
