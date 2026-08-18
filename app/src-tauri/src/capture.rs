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

#[cfg(test)]
mod tests {
    #[test]
    #[ignore] // needs a real display; run manually with -- --ignored
    fn captures_primary_monitor() {
        let m = xcap::Monitor::all().unwrap().into_iter().next().unwrap();
        let img = m.capture_image().unwrap();
        assert!(img.width() > 0 && img.height() > 0);
    }
}
