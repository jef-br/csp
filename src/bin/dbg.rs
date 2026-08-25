// Debug overlay: run detection on an image, print the verdict, and draw the box + margin.
use csp::core::config::MARGIN_FRACTION;
use csp::core::{detect, load};
use image::{Rgb, RgbImage};

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let out = std::env::args().nth(2).unwrap_or_else(|| "/tmp/dbg.png".into());
    let loaded = load::load_image(std::path::Path::new(&path)).unwrap();
    let det = detect::detect(&loaded.rgb, loaded.alpha.as_ref());
    let b = det.box_;
    let margin = (b.w.max(b.h) as f64 * MARGIN_FRACTION).round() as i32;
    println!(
        "{}\n  kind={:?} conf={:.2} shadow={:.3}\n  box=({},{},{},{})  intersects T{} B{} L{} R{}  frame={}x{}",
        path, det.kind, det.confidence, det.hard_shadow_fraction,
        b.x, b.y, b.w, b.h,
        det.intersects.top as u8, det.intersects.bottom as u8, det.intersects.left as u8, det.intersects.right as u8,
        loaded.rgb.width(), loaded.rgb.height()
    );
    let mut img = loaded.rgb.clone();
    draw_rect(&mut img, b.x, b.y, b.w, b.h, Rgb([255, 0, 0]));
    draw_rect(&mut img, b.x - margin, b.y - margin, b.w + 2 * margin, b.h + 2 * margin, Rgb([0, 200, 0]));
    img.save(&out).unwrap();
    println!("  overlay -> {out}");

    // Also dump the raw foreground mask alongside (…_mask.png).
    let mask = detect::debug_mask(&loaded.rgb);
    let mask_path = out.replace(".png", "_mask.png");
    mask.save(&mask_path).unwrap();
    println!("  mask    -> {mask_path}");

    let shadow = detect::debug_shadow(&loaded.rgb);
    let sh_path = out.replace(".png", "_shadow.png");
    shadow.save(&sh_path).unwrap();
    println!("  shadow  -> {sh_path}");
}

fn draw_rect(img: &mut RgbImage, x: i32, y: i32, w: i32, h: i32, c: Rgb<u8>) {
    let (iw, ih) = (img.width() as i32, img.height() as i32);
    let th = (iw.max(ih) / 300).max(2);
    for t in 0..th {
        for xx in x..(x + w) {
            put(img, xx, y + t, c, iw, ih);
            put(img, xx, y + h - 1 - t, c, iw, ih);
        }
        for yy in y..(y + h) {
            put(img, x + t, yy, c, iw, ih);
            put(img, x + w - 1 - t, yy, c, iw, ih);
        }
    }
}

fn put(img: &mut RgbImage, x: i32, y: i32, c: Rgb<u8>, iw: i32, ih: i32) {
    if x >= 0 && y >= 0 && x < iw && y < ih {
        img.put_pixel(x as u32, y as u32, c);
    }
}
