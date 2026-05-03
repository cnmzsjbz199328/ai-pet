#!/usr/bin/env python3
"""
Slice per-action sprite PNGs into numbered frame files.

Source asset: "FREE Cat 2D Pixel Art" by Mattz Art
  https://xzany.itch.io/cat-2d-pixel-art

Each source PNG is a horizontal strip of 64×64 frames (one animation per file).
This script crops each strip and writes frames as:
  assets/sprites/<action>/001.png  002.png  …

Usage:
    python tools/slice_sprites.py --src <path/to/Sprites/> --out assets/sprites

Default --src assumes the ZIP was extracted to the project root.
"""

import argparse
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    print("ERROR: Pillow is required.  pip install Pillow")
    sys.exit(1)

FRAME_W = 80
FRAME_H = 64

# (action_name, source_filename, start_col, end_col_exclusive)
ACTIONS = [
    ("idle",   "IDLE.png",     0,  8),
    ("walk",   "WALK.png",     0, 12),
    ("jump",   "JUMP.png",     0,  3),
    ("attack", "ATTACK 1.png", 0,  8),
    ("sleep",  "IDLE.png",     0,  4),  # reuse first 4 idle frames at slower pace
    ("happy",  "RUN.png",      0,  8),
    ("angry",  "HURT.png",     0,  4),
]


def slice_sprites(src_dir: Path, out_dir: Path) -> None:
    for action, fname, start, end in ACTIONS:
        src_path = src_dir / fname
        if not src_path.exists():
            print(f"WARN: {src_path} not found, skipping {action}")
            continue

        img = Image.open(src_path).convert("RGBA")
        action_dir = out_dir / action
        action_dir.mkdir(parents=True, exist_ok=True)

        for i, col in enumerate(range(start, end)):
            x = col * FRAME_W
            frame = img.crop((x, 0, x + FRAME_W, FRAME_H))
            frame.save(action_dir / f"{i + 1:03}.png", "PNG")

        print(f"  [{action:>8}]  {end - start} frames → {action_dir}")

    print(f"\nDone. Frames written to: {out_dir.resolve()}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--src", default="Sprites", help="Directory containing source PNGs (default: Sprites)")
    parser.add_argument("--out", default="assets/sprites", help="Output directory (default: assets/sprites)")
    args = parser.parse_args()

    src_dir = Path(args.src)
    if not src_dir.is_dir():
        print(f"ERROR: Source directory not found: {src_dir}")
        sys.exit(1)

    slice_sprites(src_dir, Path(args.out))


if __name__ == "__main__":
    main()
