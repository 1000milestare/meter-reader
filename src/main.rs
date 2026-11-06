//! Reads the four large dials of a Schlumberger gas meter (cubic feet).
//!
//! Calibrated for photos from the same camera/framing as IMG_4450.jpeg
//! (2576x1932). Seeds scale automatically if the image has the same aspect
//! ratio but a different resolution.
//!
//! Usage: meter_read <photo.jpg> [overlay.png]

use image::{GrayImage, Rgb, RgbImage};
use std::f64::consts::PI;

/// Reference resolution the seeds were measured at.
const REF_W: f64 = 2576.0;

struct Dial {
    /// Approximate hub (pointer pivot) in reference-resolution pixels.
    seed_x: f64,
    seed_y: f64,
    /// Digits increase counter-clockwise (dials 1 and 3 on this meter).
    ccw: bool,
    /// Cubic feet per full revolution.
    cuft_per_rev: f64,
}

const DIALS: [Dial; 4] = [
    Dial { seed_x: 675.0,  seed_y: 925.0, ccw: true,  cuft_per_rev: 1_000_000.0 },
    Dial { seed_x: 1030.0, seed_y: 925.0, ccw: false, cuft_per_rev: 100_000.0 },
    Dial { seed_x: 1390.0, seed_y: 925.0, ccw: true,  cuft_per_rev: 10_000.0 },
    Dial { seed_x: 1755.0, seed_y: 920.0, ccw: false, cuft_per_rev: 1_000.0 },
];

// Geometry in reference pixels.
const HUB_SEARCH: i64 = 35; // how far from the seed to look for the hub
const HUB_HALF: i64 = 38; // half-size of the box used to find the dark hub blob
const RAY_R0: f64 = 64.0; // sample the pointer just outside the hub blob...
const RAY_R1: f64 = 95.0; // ...and inside the ring of numerals
const RAY_HALF_W: f64 = 5.0; // sample +/- this many px across the ray

fn darkness(img: &GrayImage, x: f64, y: f64) -> f64 {
    // Bilinear sample of (255 - gray).
    let (w, h) = img.dimensions();
    if x < 0.0 || y < 0.0 || x >= (w - 1) as f64 || y >= (h - 1) as f64 {
        return 0.0;
    }
    let (x0, y0) = (x.floor() as u32, y.floor() as u32);
    let (fx, fy) = (x - x0 as f64, y - y0 as f64);
    let p = |xx: u32, yy: u32| 255.0 - img.get_pixel(xx, yy)[0] as f64;
    p(x0, y0) * (1.0 - fx) * (1.0 - fy)
        + p(x0 + 1, y0) * fx * (1.0 - fy)
        + p(x0, y0 + 1) * (1.0 - fx) * fy
        + p(x0 + 1, y0 + 1) * fx * fy
}

/// Summed-area table of darkness for fast box sums.
fn integral(img: &GrayImage) -> (Vec<u64>, usize, usize) {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut t = vec![0u64; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row = 0u64;
        for x in 0..w {
            row += 255 - img.get_pixel(x as u32, y as u32)[0] as u64;
            t[(y + 1) * (w + 1) + (x + 1)] = t[y * (w + 1) + (x + 1)] + row;
        }
    }
    (t, w, h)
}

fn box_sum(t: &[u64], w: usize, x0: usize, y0: usize, x1: usize, y1: usize) -> u64 {
    // inclusive-exclusive [x0,x1) x [y0,y1)
    let s = w + 1;
    t[y1 * s + x1] + t[y0 * s + x0] - t[y0 * s + x1] - t[y1 * s + x0]
}

/// The hub is the largest dark blob near the seed.
fn find_hub(sat: &(Vec<u64>, usize, usize), sx: f64, sy: f64, scale: f64) -> (f64, f64) {
    let (t, w, h) = (&sat.0, sat.1, sat.2);
    let half = (HUB_HALF as f64 * scale) as i64;
    let search = (HUB_SEARCH as f64 * scale) as i64;
    let (mut best, mut best_s) = ((sx, sy), 0u64);
    for dy in -search..=search {
        for dx in -search..=search {
            let (cx, cy) = (sx as i64 + dx, sy as i64 + dy);
            let (x0, y0, x1, y1) = (cx - half, cy - half, cx + half, cy + half);
            if x0 < 0 || y0 < 0 || x1 >= w as i64 || y1 >= h as i64 {
                continue;
            }
            let s = box_sum(t, w, x0 as usize, y0 as usize, x1 as usize, y1 as usize);
            if s > best_s {
                best_s = s;
                best = (cx as f64, cy as f64);
            }
        }
    }
    best
}

/// Pointer direction as an angle in radians, clockwise from 12 o'clock.
fn find_pointer(img: &GrayImage, hx: f64, hy: f64, scale: f64) -> f64 {
    let steps = 1440;
    let mut scores = vec![0.0f64; steps];
    for (i, sc) in scores.iter_mut().enumerate() {
        let th = 2.0 * PI * i as f64 / steps as f64;
        let (dx, dy) = (th.sin(), -th.cos()); // along the ray
        let (px, py) = (-dy, dx); // across the ray
        let mut s = 0.0;
        let mut n = 0.0;
        let mut r = RAY_R0 * scale;
        while r <= RAY_R1 * scale {
            let mut o = -RAY_HALF_W * scale;
            while o <= RAY_HALF_W * scale {
                s += darkness(img, hx + dx * r + px * o, hy + dy * r + py * o);
                n += 1.0;
                o += 2.0 * scale;
            }
            r += 2.0 * scale;
        }
        *sc = s / n;
    }
    // Smooth over a few degrees, then take the peak.
    let k = 12;
    let mut best = (0usize, f64::MIN);
    for i in 0..steps {
        let mut s = 0.0;
        for j in 0..=2 * k {
            s += scores[(i + steps + j - k) % steps];
        }
        if s > best.1 {
            best = (i, s);
        }
    }
    // Centroid refinement around the peak (handles a peak split across bins).
    let (mut sw, mut sx, mut sy) = (0.0, 0.0, 0.0);
    let lo = scores.iter().cloned().fold(f64::MAX, f64::min);
    for j in 0..=2 * k * 2 {
        let idx = (best.0 + steps + j - 2 * k) % steps;
        let w = scores[idx] - lo;
        let th = 2.0 * PI * idx as f64 / steps as f64;
        sw += w;
        sx += w * th.sin();
        sy += w * th.cos();
    }
    let _ = sw;
    sx.atan2(sy).rem_euclid(2.0 * PI)
}

fn draw_line(out: &mut RgbImage, x0: f64, y0: f64, x1: f64, y1: f64, c: Rgb<u8>, thick: i64) {
    let n = ((x1 - x0).abs().max((y1 - y0).abs())) as usize + 1;
    for s in 0..=n {
        let t = s as f64 / n as f64;
        let (x, y) = ((x0 + (x1 - x0) * t) as i64, (y0 + (y1 - y0) * t) as i64);
        for oy in -thick..=thick {
            for ox in -thick..=thick {
                let (px, py) = (x + ox, y + oy);
                if px >= 0 && py >= 0 && (px as u32) < out.width() && (py as u32) < out.height() {
                    out.put_pixel(px as u32, py as u32, c);
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: meter_read <photo.jpg> [overlay.png]")?;
    let overlay_path = args.next().unwrap_or_else(|| "meter_debug.png".into());

    let rgb = image::open(&path)?.to_rgb8();
    let gray = image::DynamicImage::ImageRgb8(rgb.clone()).to_luma8();
    let scale = gray.width() as f64 / REF_W;
    eprintln!("image {}x{} (scale {:.3})", gray.width(), gray.height(), scale);

    let sat = integral(&gray);
    let mut overlay = rgb;

    // Fractional pointer position (0..1 turns, in the direction digits increase).
    let mut frac = [0.0f64; 4];
    for (i, d) in DIALS.iter().enumerate() {
        let (hx, hy) = find_hub(&sat, d.seed_x * scale, d.seed_y * scale, scale);
        let theta = find_pointer(&gray, hx, hy, scale);
        let turns = theta / (2.0 * PI);
        frac[i] = if d.ccw { (1.0 - turns).rem_euclid(1.0) } else { turns };

        let green = Rgb([0, 255, 0]);
        let red = Rgb([255, 0, 0]);
        draw_line(&mut overlay, hx - 12.0, hy, hx + 12.0, hy, green, 2);
        draw_line(&mut overlay, hx, hy - 12.0, hx, hy + 12.0, green, 2);
        draw_line(
            &mut overlay, hx, hy,
            hx + 170.0 * scale * theta.sin(), hy - 170.0 * scale * theta.cos(),
            red, 2,
        );
        eprintln!(
            "dial {} ({:>9.0}/rev): hub=({:.0},{:.0}) pointer={:.1} deg cw -> position {:.2}",
            i + 1, d.cuft_per_rev, hx, hy, theta.to_degrees(), frac[i] * 10.0
        );
    }
    overlay.save(&overlay_path)?;

    // Resolve digits from the least significant dial upward. Each higher
    // dial's pointer sits at (its digit + the lower dial's fraction of a turn),
    // so the digit is whatever makes that true. This stops a pointer that is
    // sitting right on a digit from being mis-read by one.
    let mut x = frac[3] * DIALS[3].cuft_per_rev; // continuous reading so far
    let mut digits = [0u32; 4];
    digits[3] = (frac[3] * 10.0).round() as u32 % 10;
    for i in (0..3).rev() {
        let per_digit = DIALS[i].cuft_per_rev / 10.0;
        let d = ((frac[i] * DIALS[i].cuft_per_rev - x) / per_digit).round();
        let d = d.rem_euclid(10.0);
        digits[i] = d as u32;
        x += d * per_digit;
    }
    // Lowest-dial digit consistent with the carry chain:
    let rounded = (x / 100.0).round() * 100.0;
    println!("digits (left to right): {}{}{}{}", digits[0], digits[1], digits[2], digits[3]);
    println!("reading: {:.0} cubic feet (nearest 100)", rounded);
    println!("overlay: {}", overlay_path);
    Ok(())
}
