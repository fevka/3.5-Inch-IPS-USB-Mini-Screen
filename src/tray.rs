// Sistem tepsisi simgesi ve sag-tik menusu ("Ayarlar" / "Cikis").
// tray-icon crate'i Windows'ta calismasi icin bir Win32 mesaj
// dongusunun PUMPLANMASINI gerektirir (winit kullanmiyoruz, o yuzden
// bunu kendimiz, ana render dongusunun her iterasyonunda, elle yapiyoruz).

use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

pub struct AppTray {
    // _tray drop edilirse simge kaybolur - AppTray yasadigi surece
    // (yani program calistigi surece) canli tutulmasi SART.
    _tray: TrayIcon,
    settings_id: MenuId,
    exit_id: MenuId,
}

impl AppTray {
    pub fn new() -> Result<Self> {
        let menu = Menu::new();
        let settings_item = MenuItem::new("Settings", true, None);
        let exit_item = MenuItem::new("Exit", true, None);
        menu.append(&settings_item)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&exit_item)?;

        let settings_id = settings_item.id().clone();
        let exit_id = exit_item.id().clone();

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Mini System Monitor")
            .with_icon(build_icon())
            .build()?;

        Ok(AppTray {
            _tray: tray,
            settings_id,
            exit_id,
        })
    }

    /// Ana render dongusunde HER ITERASYONDA cagrilmali (10ms'lik
    /// sleep'in yaninda, ucretsize yakin bir islem). Win32 mesaj
    /// kuyrugunu isler (tray simgesinin ve menusunun calismasi icin
    /// GEREKLI - aksi halde simge tikanir/tepki vermez) ve bekleyen
    /// menu tiklamalarini isler.
    ///
    /// Donus degeri true ise "Cikis" secilmis demektir, cagiran taraf
    /// ana donguyu sonlandirmali.
    pub fn poll(&self, display: &crate::lcd_comm::LcdComm) -> bool {
        pump_windows_messages();

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.settings_id {
                launch_configure();
            } else if event.id == self.exit_id {
                // Orijinal Python projesindeki clean_stop() gibi -
                // cikmadan once ekrani kapat.
                let _ = display.screen_off();
                return true;
            }
        }
        false
    }
}

fn launch_configure() {
    let exe_name = if cfg!(windows) { "configure-app.exe" } else { "configure-app" };
    let path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(exe_name)))
        .unwrap_or_else(|| PathBuf::from(exe_name));

    if let Err(e) = Command::new(&path).spawn() {
        log::error!("Settings window could not be launched ({}): {}", path.display(), e);
    }
}

/// Basit, prosedurel olarak uretilmis yuvarlak bir simge (tema LED
/// rengiyle - theme.yaml'deki DISPLAY_RGB_LED: 180,80,255 - uyumlu).
/// Harici bir .ico dosyasina bagimli olmamak icin kod icinde ciziliyor.
fn build_icon() -> Icon {
    let size: u32 = 32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let cx = size as f32 / 2.0;
    let cy = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx + 0.5;
            let dy = y as f32 - cy + 0.5;
            let d = (dx * dx + dy * dy).sqrt();
            if d <= cx {
                rgba.extend_from_slice(&[180, 80, 255, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Icon::from_rgba(rgba, size, size).expect("could not create tray icon")
}

#[cfg(windows)]
fn pump_windows_messages() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

#[cfg(not(windows))]
fn pump_windows_messages() {}
