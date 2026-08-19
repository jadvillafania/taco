use image::RgbaImage;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::AppHandle;

pub struct Frozen {
    pub image: RgbaImage,
    pub frame_png: PathBuf,
    pub mon_x: i32,
    pub mon_y: i32,
    pub mon_w: u32,
    pub mon_h: u32,
}

#[derive(Default)]
pub struct Inner {
    pub frozen: Option<Frozen>,
    pub capture: Option<PathBuf>,
    pub focus_title: Option<String>,
}
pub struct CaptureState(pub Mutex<Inner>);

fn monitor_under_cursor(app: &AppHandle) -> Result<xcap::Monitor, String> {
    let pos = app.cursor_position().map_err(|e| e.to_string())?;
    xcap::Monitor::from_point(pos.x as i32, pos.y as i32).map_err(|e| e.to_string())
}

fn capture_monitor(m: &xcap::Monitor) -> Result<(RgbaImage, i32, i32), String> {
    let img = m.capture_image().map_err(|e| e.to_string())?;
    Ok((img, m.x().map_err(|e| e.to_string())?, m.y().map_err(|e| e.to_string())?))
}

fn capture_path(app: &AppHandle) -> PathBuf {
    let now = chrono::Local::now();
    let dir = crate::retention::data_dir(app)
        .join("captures")
        .join(now.format("%Y-%m-%d").to_string());
    std::fs::create_dir_all(&dir).ok();
    dir.join(format!("capture-{}.png", now.format("%H%M%S-%3f")))
}

pub fn freeze_monitor_under_cursor(app: &AppHandle) -> Result<Frozen, String> {
    let m = monitor_under_cursor(app)?;
    let (image, mon_x, mon_y) = capture_monitor(&m)?;
    let dir = crate::retention::data_dir(app).join("frames");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let frame_png = dir.join("frame.png");
    image.save(&frame_png).map_err(|e| e.to_string())?;
    Ok(Frozen { mon_w: image.width(), mon_h: image.height(), image, frame_png, mon_x, mon_y })
}

pub fn save_crop(app: &AppHandle, img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> Result<PathBuf, String> {
    let x = x.min(img.width().saturating_sub(1));
    let y = y.min(img.height().saturating_sub(1));
    let w = w.min(img.width() - x).max(1);
    let h = h.min(img.height() - y).max(1);
    let out = image::imageops::crop_imm(img, x, y, w, h).to_image();
    let path = capture_path(app);
    out.save(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn save_full(app: &AppHandle) -> Result<PathBuf, String> {
    let m = monitor_under_cursor(app)?;
    let (image, _, _) = capture_monitor(&m)?;
    let path = capture_path(app);
    image.save(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Intersect a window rect (screen coords, l/t/r/b) with a monitor rect (x/y/w/h),
/// returning monitor-relative x/y/w/h — the shape save_crop expects.
pub fn relative_crop(win: (i32, i32, i32, i32), mon: (i32, i32, u32, u32)) -> Option<(u32, u32, u32, u32)> {
    let (wl, wt, wr, wb) = win;
    let (mx, my, mw, mh) = mon;
    let l = wl.max(mx);
    let t = wt.max(my);
    let r = wr.min(mx + mw as i32);
    let b = wb.min(my + mh as i32);
    if r <= l || b <= t {
        return None;
    }
    Some(((l - mx) as u32, (t - my) as u32, (r - l) as u32, (b - t) as u32))
}

/// Capture the foreground window: grab its monitor, crop to the window's frame bounds.
// ponytail: a window spanning two monitors is cropped to the one under its center;
// full spanning needs stitched captures — add if anyone actually works that way.
pub fn save_active_window(app: &AppHandle) -> Result<PathBuf, String> {
    let (l, t, r, b) = crate::sessions::foreground_rect().ok_or("no foreground window")?;
    let m = xcap::Monitor::from_point((l + r) / 2, (t + b) / 2).map_err(|e| e.to_string())?;
    let (image, mon_x, mon_y) = capture_monitor(&m)?;
    let (x, y, w, h) = relative_crop((l, t, r, b), (mon_x, mon_y, image.width(), image.height()))
        .ok_or("window is outside its monitor")?;
    let out = image::imageops::crop_imm(&image, x, y, w, h).to_image();
    let path = capture_path(app);
    out.save(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore] // needs a real display; run manually with -- --ignored
    fn captures_primary_monitor() {
        let m = xcap::Monitor::all().unwrap().into_iter().next().unwrap();
        let img = m.capture_image().unwrap();
        assert!(img.width() > 0 && img.height() > 0);
    }

    #[test]
    fn relative_crop_clamps_to_monitor() {
        // window fully inside a 1920x1080 monitor at origin
        assert_eq!(super::relative_crop((100, 50, 500, 450), (0, 0, 1920, 1080)), Some((100, 50, 400, 400)));
        // window hangs off the left/top edges: clamped to 0
        assert_eq!(super::relative_crop((-50, -20, 300, 200), (0, 0, 1920, 1080)), Some((0, 0, 300, 200)));
        // second monitor at x=1920: coordinates become monitor-relative
        assert_eq!(super::relative_crop((2000, 100, 2400, 300), (1920, 0, 1920, 1080)), Some((80, 100, 400, 200)));
        // window entirely outside the monitor: no crop
        assert_eq!(super::relative_crop((4000, 0, 4400, 300), (0, 0, 1920, 1080)), None);
    }
}
