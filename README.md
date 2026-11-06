# meter-reader

Reads the four large dials of a Schlumberger gas meter (cubic feet) from a
photo, and writes a debug overlay showing where it found each hub and pointer.

Written for a specific meter and camera setup. It is not a general-purpose
meter reader; see [Calibration](#calibration) before using it on another meter.

## Usage

Requires a recent Rust toolchain (`rustup`).

```sh
cargo run --release -- photo.jpg overlay.png
```

Output:

```
image 2576x1932 (scale 1.000)
dial 1 (  1000000/rev): hub=(686,929) pointer=62.5 deg cw -> position 8.26
...
digits (left to right): 8101
reading: 810100 cubic feet (nearest 100)
overlay: overlay.png
```

Open `overlay.png` and check that each green cross sits on a dial's hub and
each red line lies along its pointer. If they don't, the seeds need adjusting.

### Running in GitHub Actions

Go to **Actions → CI → Run workflow**, paste a public URL to a JPEG or PNG of
the meter, and run it. The run's summary shows the reading, and the overlay is
attached as the `meter-overlay` artifact.

Photos are downloaded from the URL you give, so use a link you control (for
example, an image you have uploaded to a private or public host). Do not commit
meter photos to this repository.

## How it works

1. Each big dial's hub is located as the darkest blob near a fixed seed point.
2. The pointer direction is the strongest dark ray sampled from that hub.
3. Each dial's position in turns becomes a digit. Digits are resolved from the
   1,000 dial upward, so a pointer sitting right on a digit is not misread by one.

Dials alternate direction: digits increase counter-clockwise on the 1,000,000
and 10,000 dials, and clockwise on the 100,000 and 1,000 dials.

## Calibration

The seeds in `src/main.rs` were measured on a 2576×1932 photo. They are scaled
by image width, so a different resolution with the same aspect ratio and
framing works. A different camera distance, angle, or meter needs new seeds:

```rust
const DIALS: [Dial; 4] = [
    Dial { seed_x: 675.0,  seed_y: 925.0, ccw: true,  cuft_per_rev: 1_000_000.0 },
    ...
];
```

Set each seed to the approximate hub pixel of its dial, then check the overlay.

## Limits

- Reads to the nearest 100 cubic feet, the resolution of the 1,000 dial.
- Glare and the plastic cover can shift the detected pointer.
- A pointer sitting exactly between two digits can read as either one.

## License

[Unlicense](LICENSE). Public domain.
