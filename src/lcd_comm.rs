use anyhow::{anyhow, Result};
use image::RgbImage;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const BAUD_RATE: u32 = 115200;

enum LcdCommand {
    DisplayImage {
        rgb565: Vec<u8>,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
    },
    Reset,
    ScreenOff,
    ScreenOn,
    SetBrightness(u8),
    SetOrientation(u8, u16, u16),
    Quit,
}

pub struct LcdComm {
    tx: mpsc::Sender<LcdCommand>,
}

impl LcdComm {
    pub fn new(com_port: &str, on_hard_failure: Box<dyn Fn() + Send + Sync>) -> Result<Self> {
        let (tx, rx) = mpsc::channel::<LcdCommand>();

        // Portu SENKRON ac. Eski kod portu ayri bir thread icinde
        // sessizce aciyordu; port acilamazsa program yine de "baglandim"
        // saniyor, ekran bos kaliyordu. Artik acilamazsa Err doner ve
        // cagiran taraf (main.rs) retry/exit kararini kendisi verir.
        let mut port = serialport::new(com_port, BAUD_RATE)
            .timeout(Duration::from_millis(5000))
            .open()
            .map_err(|e| anyhow!("LCD serial port acilamadi: {}", e))?;

        // Bekcim: ust uste "failed to write whole buffer" gibi dogalayici
        // yazma hatalari (port aciliyor ama cihaz veri almadan donuyor)
        // surekli tekrarlanip programin baslamasini engelliyor. Cok fazla
        // ard arda basarisizliktan sonra on_hard_failure cagrilir ve
        // cagiran taraf (main.rs) tum sureci 5 saniye sonra yeniden baslatir.
        const FAILURE_LIMIT: u32 = 3;
        let mut consecutive_failures: u32 = 0;
        let mut report_failure = move |e: &dyn std::fmt::Display, consecutive_failures: &mut u32| {
            log::error!("LCD write hatasi: {}", e);
            *consecutive_failures += 1;
            if *consecutive_failures >= FAILURE_LIMIT {
                log::error!(
                    "LCD ard arda {} kez yazamadi, program 5 sn sonra yeniden baslatiliyor",
                    *consecutive_failures
                );
                on_hard_failure();
            }
        };

        thread::spawn(move || {
            loop {
                match rx.recv() {
                    Ok(LcdCommand::DisplayImage { rgb565, x, y, w, h }) => {
                        let x1 = x + w - 1;
                        let y1 = y + h - 1;
                        let header = encode_header(x, y, x1, y1, 197);
                        if let Err(e) = port.write_all(&header) {
                            report_failure(&e, &mut consecutive_failures);
                            continue;
                        }
                        let mut ok = true;
                        for chunk in rgb565.chunks(320 * 8) {
                            if let Err(e) = port.write_all(chunk) {
                                report_failure(&e, &mut consecutive_failures);
                                ok = false;
                                break;
                            }
                        }
                        if ok {
                            consecutive_failures = 0;
                        }
                        let _ = port.flush();
                    }
                    Ok(LcdCommand::Reset) => {
                        if let Err(e) = port.write_all(&encode_header(0, 0, 0, 0, 101)) {
                            report_failure(&e, &mut consecutive_failures);
                            continue;
                        }
                        consecutive_failures = 0;
                        let _ = port.flush();
                    }
                    Ok(LcdCommand::ScreenOff) => {
                        if let Err(e) = port.write_all(&encode_header(0, 0, 0, 0, 108)) {
                            report_failure(&e, &mut consecutive_failures);
                            continue;
                        }
                        consecutive_failures = 0;
                        let _ = port.flush();
                    }
                    Ok(LcdCommand::ScreenOn) => {
                        if let Err(e) = port.write_all(&encode_header(0, 0, 0, 0, 109)) {
                            report_failure(&e, &mut consecutive_failures);
                            continue;
                        }
                        consecutive_failures = 0;
                        let _ = port.flush();
                    }
                    Ok(LcdCommand::SetBrightness(level)) => {
                        let level_abs = 255u16 - ((level as u16 * 255) / 100);
                        if let Err(e) = port.write_all(&encode_header(level_abs, 0, 0, 0, 110)) {
                            report_failure(&e, &mut consecutive_failures);
                            continue;
                        }
                        consecutive_failures = 0;
                        let _ = port.flush();
                    }
                    Ok(LcdCommand::SetOrientation(orientation, width, height)) => {
                        let mut buf = [0u8; 11];
                        buf[..6].copy_from_slice(&encode_header(0, 0, 0, 0, 121));
                        buf[6] = orientation + 100;
                        buf[7] = (width >> 8) as u8;
                        buf[8] = (width & 0xff) as u8;
                        buf[9] = (height >> 8) as u8;
                        buf[10] = (height & 0xff) as u8;
                        if let Err(e) = port.write_all(&buf) {
                            report_failure(&e, &mut consecutive_failures);
                            continue;
                        }
                        consecutive_failures = 0;
                        let _ = port.flush();
                    }
                    Ok(LcdCommand::Quit) => break,
                    Err(mpsc::RecvError) => break,
                }
            }
        });

        Ok(LcdComm { tx })
    }

    pub fn auto_detect() -> Result<String> {
        let ports = serialport::available_ports()
            .map_err(|e| anyhow!("No ports: {}", e))?;
        for port in &ports {
            if let serialport::SerialPortType::UsbPort(info) = &port.port_type {
                if info.serial_number.as_deref() == Some("USB35INCHIPSV2") {
                    return Ok(port.port_name.clone());
                }
                if info.vid == 0x1a86 && info.pid == 0x5722 {
                    return Ok(port.port_name.clone());
                }
            }
        }
        if !ports.is_empty() {
            log::info!("USB ekrani taninamadi, sirasiyla portlar deneniyor...");
            for port in &ports {
                log::info!("  {} deneniyor...", port.port_name);
                if serialport::new(&port.port_name, BAUD_RATE)
                    .timeout(Duration::from_millis(500))
                    .open()
                    .is_ok()
                {
                    return Ok(port.port_name.clone());
                }
            }
        }
        Err(anyhow!("LCD ekran bulunamadi. Portlar: {:?}",
            ports.iter().map(|p| p.port_name.clone()).collect::<Vec<_>>()))
    }

    pub fn display_image(&self, img: &RgbImage, x: u16, y: u16) -> Result<()> {
        let (w, h) = (img.width() as u16, img.height() as u16);
        let mut rgb565 = Vec::with_capacity(w as usize * h as usize * 2);
        for py in 0..h {
            for px in 0..w {
                let p = img.get_pixel(px as u32, py as u32);
                let r = p[0] as u16 >> 3;
                let g = p[1] as u16 >> 2;
                let b = p[2] as u16 >> 3;
                let val = (r << 11) | (g << 5) | b;
                rgb565.extend_from_slice(&val.to_le_bytes());
            }
        }
        self.tx.send(LcdCommand::DisplayImage { rgb565, x, y, w, h })?;
        Ok(())
    }

    pub fn initialize(&self, brightness: u8) -> Result<()> {
        self.set_orientation(0)?;
        self.screen_on()?;
        self.set_brightness(brightness)?;
        Ok(())
    }

    fn send_cmd(&self, cmd: LcdCommand) -> Result<()> {
        self.tx.send(cmd)?;
        Ok(())
    }

    pub fn reset(&self) -> Result<()> { self.send_cmd(LcdCommand::Reset) }
    pub fn screen_off(&self) -> Result<()> { self.send_cmd(LcdCommand::ScreenOff) }
    pub fn screen_on(&self) -> Result<()> { self.send_cmd(LcdCommand::ScreenOn) }

    pub fn set_brightness(&self, level: u8) -> Result<()> {
        self.send_cmd(LcdCommand::SetBrightness(level))
    }

    pub fn set_orientation(&self, orientation: u8) -> Result<()> {
        self.send_cmd(LcdCommand::SetOrientation(orientation, 320, 480))
    }
}

impl Drop for LcdComm {
    fn drop(&mut self) {
        let _ = self.tx.send(LcdCommand::Quit);
    }
}

fn encode_header(x: u16, y: u16, ex: u16, ey: u16, cmd: u8) -> [u8; 6] {
    [
        (x >> 2) as u8,
        (((x & 3) << 6) + (y >> 4)) as u8,
        (((y & 15) << 4) + (ex >> 6)) as u8,
        (((ex & 63) << 2) + (ey >> 8)) as u8,
        (ey & 255) as u8,
        cmd,
    ]
}
