use chrono::Local;
use sysinfo::{CpuRefreshKind, Disks, Networks, RefreshKind, System};
use std::time::{Duration, Instant};

use crate::sensors;

/// Disk usage/capacity barely changes second to second, so there's no
/// reason to re-stat every disk 5x/sec just because the fast CPU/network
/// refresh loop ticks that often.
const DISK_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

pub struct SystemStats {
    sys: System,
    disks: Disks,
    cpu_history: Vec<f32>,
    cpu_history_max: usize,
    cpu_brand: String,
    gpu_name: String,
    gpu_vendor: String,
    gpu_freq_mhz: u64,
    gpu_cores: u64,
    cpu_physical_cores: u64,
    cpu_logical_cores: u64,
    networks: Networks,
    net_eth_name: Option<String>,
    net_wlo_name: Option<String>,
    net_last_refresh: Instant,
    net_up_kbps: f64,
    net_down_kbps: f64,
    last_disk_refresh: Instant,
}

impl SystemStats {
    pub fn new(net_eth_name: Option<String>, net_wlo_name: Option<String>) -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::everything().with_cpu(CpuRefreshKind::everything()),
        );
        sys.refresh_cpu_all();
        sys.refresh_memory();

        let cpu_brand = {
            let hw = sensors::get_cpu_brand();
            if !hw.is_empty() {
                hw
            } else {
                sys.cpus().first()
                    .map(|c| c.brand().trim().to_string())
                    .unwrap_or_default()
            }
        };

        let gpu_name = sensors::get_gpu_name();
        let gpu_vendor = sensors::get_gpu_vendor();
        let gpu_freq_mhz = sensors::get_gpu_frequency_mhz();
        let gpu_cores = sensors::get_gpu_cores();
        let cpu_physical_cores = sensors::get_cpu_physical_cores();
        let cpu_logical_cores = sensors::get_cpu_logical_cores();

        // Bos string ("") config'te "arayuz belirtilmedi" anlamina gelir -
        // None'a cevirip asagida "tum arayuzleri topla" moduna dusuruyoruz.
        let net_eth_name = net_eth_name.filter(|s| !s.is_empty());
        let net_wlo_name = net_wlo_name.filter(|s| !s.is_empty());

        SystemStats {
            sys,
            disks: Disks::new(),
            cpu_history: Vec::new(),
            cpu_history_max: 60,
            cpu_brand,
            gpu_name,
            gpu_vendor,
            gpu_freq_mhz,
            gpu_cores,
            cpu_physical_cores,
            cpu_logical_cores,
            networks: Networks::new_with_refreshed_list(),
            net_eth_name,
            net_wlo_name,
            net_last_refresh: Instant::now(),
            net_up_kbps: 0.0,
            net_down_kbps: 0.0,
            last_disk_refresh: Instant::now() - DISK_REFRESH_INTERVAL,
        }
    }

    /// Hizli/lokal yenileme: sysinfo (CPU/RAM/disk) mutex'i kilitliyken
    /// yapilir, cunku bu islemler zaten cok hizlidir. Yavas donanim
    /// sensorleri (WMI, LHM subprocess) icin refresh_sensors_unlocked()
    /// kullan - mutex kilitli DEGILKEN cagrilmali.
    pub fn refresh(&mut self) {
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();

        if self.last_disk_refresh.elapsed() >= DISK_REFRESH_INTERVAL {
            self.disks.refresh(true);
            self.last_disk_refresh = Instant::now();
        }

        if self.gpu_name.is_empty() {
            let gpu = sensors::get_gpu_name();
            if !gpu.is_empty() {
                self.gpu_name = gpu;
            }
        }

        self.refresh_network();
    }

    /// Yapilandirilmis ETH/WLO arayuzlerinin (ikisi de bos ise TUM
    /// arayuzlerin) yukleme/indirme hizini KB/s olarak hesaplar.
    /// networks.refresh(true) her cagrildiginda "son refresh'ten beri"
    /// gecen byte sayisini dondurur, bu yuzden gercek gecen sureyi
    /// olcup oraniyoruz (sabit bir aralik varsaymiyoruz).
    fn refresh_network(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.net_last_refresh).as_secs_f64().max(0.001);
        self.net_last_refresh = now;

        self.networks.refresh(true);

        let filter_active = self.net_eth_name.is_some() || self.net_wlo_name.is_some();
        let mut up_bytes: u64 = 0;
        let mut down_bytes: u64 = 0;
        for (name, data) in self.networks.iter() {
            let include = if filter_active {
                self.net_eth_name.as_deref() == Some(name.as_str())
                    || self.net_wlo_name.as_deref() == Some(name.as_str())
            } else {
                true
            };
            if include {
                up_bytes += data.transmitted();
                down_bytes += data.received();
            }
        }

        self.net_up_kbps = (up_bytes as f64 / 1024.0) / elapsed;
        self.net_down_kbps = (down_bytes as f64 / 1024.0) / elapsed;
    }

    pub fn net_upload_kbps(&self) -> f64 {
        self.net_up_kbps
    }

    pub fn net_download_kbps(&self) -> f64 {
        self.net_down_kbps
    }

    /// Yavas olabilecek sensor okumalarini (WMI sorgulari, LHM subprocess
    /// cagrisi) yapar. Kendi ic CACHE'ine yazar; SystemStats mutex'ini
    /// TUTMADAN cagrilmalidir, aksi halde ana render dongusu bu sure
    /// boyunca kilitlenir ("real-time gitmeme" sorununun asil kaynagi).
    pub fn refresh_sensors_unlocked() {
        sensors::refresh_all();
    }

    /// Anlik CPU yuzdesini dondurur, history'e EKLEMEZ.
    pub fn cpu_percent_now(&self) -> f32 {
        self.sys.global_cpu_usage()
    }

    /// CPU history'e bir ornek ekler. Sadece istenen ornekleme
    /// araligiyla (orn. 1 saniyede bir) cagrilmali.
    pub fn push_cpu_history(&mut self, val: f32) {
        self.cpu_history.push(val);
        if self.cpu_history.len() > self.cpu_history_max {
            self.cpu_history.remove(0);
        }
    }

    pub fn cpu_brand(&self) -> &str {
        &self.cpu_brand
    }

    pub fn cpu_physical_cores(&self) -> u64 {
        self.cpu_physical_cores
    }

    pub fn cpu_logical_cores(&self) -> u64 {
        self.cpu_logical_cores
    }

    /// NOT: Bu fonksiyon artik history'e otomatik eklemiyor (eskiden
    /// her cagrildiginda push ediyordu; ana dongu bunu her 10ms'de bir
    /// cagirdigi icin 60 elemanlik history ~600ms'de dolup tasiyordu).
    /// History'e eklemek icin push_cpu_history() kullan.
    pub fn cpu_percent(&self) -> f32 {
        self.sys.global_cpu_usage()
    }

    pub fn cpu_history(&self) -> &[f32] {
        &self.cpu_history
    }

    pub fn cpu_frequency_mhz(&self) -> u64 {
        let freqs = sensors::get_cpu_frequencies();
        if freqs.is_empty() {
            self.sys.cpus().first().map(|c| c.frequency()).unwrap_or(0)
        } else {
            (freqs.iter().sum::<i64>() / freqs.len() as i64) as u64
        }
    }

    pub fn cpu_temperature(&self) -> f32 {
        sensors::get_cpu_temperature()
    }

    pub fn memory_percent(&self) -> f32 {
        if self.sys.total_memory() == 0 {
            return 0.0;
        }
        self.sys.used_memory() as f32 / self.sys.total_memory() as f32 * 100.0
    }

    pub fn memory_used_mb(&self) -> u64 {
        self.sys.used_memory() / (1024 * 1024)
    }

    pub fn memory_total_mb(&self) -> u64 {
        self.sys.total_memory() / (1024 * 1024)
    }

    pub fn disk_used_gb(&self) -> u64 {
        self.disks
            .iter()
            .map(|d| d.total_space().saturating_sub(d.available_space()))
            .sum::<u64>()
            / (1000 * 1000 * 1000)
    }

    pub fn disk_total_gb(&self) -> u64 {
        self.disks
            .iter()
            .map(|d| d.total_space())
            .sum::<u64>()
            / (1000 * 1000 * 1000)
    }

    pub fn disk_percent(&self) -> f32 {
        let total: u64 = self.disks.iter().map(|d| d.total_space()).sum();
        let used: u64 = self
            .disks
            .iter()
            .map(|d| d.total_space().saturating_sub(d.available_space()))
            .sum();
        if total == 0 {
            return 0.0;
        }
        used as f32 / total as f32 * 100.0
    }

    pub fn uptime_seconds(&self) -> u64 {
        System::uptime()
    }

    pub fn datetime_str(&self) -> String {
        Local::now().format("%H:%M").to_string()
    }

    pub fn date_str(&self) -> String {
        Local::now().format("%a %d %b").to_string()
    }

    pub fn gpu_name(&self) -> &str {
        &self.gpu_name
    }

    pub fn gpu_vendor(&self) -> &str {
        &self.gpu_vendor
    }

    pub fn gpu_load(&self) -> f32 {
        sensors::get_gpu_load()
    }

    pub fn gpu_temperature(&self) -> f32 {
        sensors::get_gpu_temperature()
    }

    pub fn gpu_mem_junction_temperature(&self) -> f32 {
        sensors::get_gpu_mem_junction_temperature()
    }

    pub fn gpu_memory_used_mb(&self) -> f32 {
        sensors::get_gpu_memory_used_mb()
    }

    pub fn gpu_memory_total_mb(&self) -> f32 {
        sensors::get_gpu_memory_total_mb()
    }

    pub fn gpu_frequency_mhz(&self) -> u64 {
        self.gpu_freq_mhz
    }

    pub fn gpu_cores(&self) -> u64 {
        self.gpu_cores
    }
}
