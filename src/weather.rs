use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// Eski Python projesindeki WEATHER_LATITUDE/LONGITUDE/UNITS config
// degerlerinin karsiligi. API anahtari gerektirmeyen Open-Meteo
// kullaniliyor (https://open-meteo.com), tipki orijinal projedeki gibi.

#[derive(Default, Clone)]
pub struct WeatherData {
    pub temperature: f32,
    pub temperature_felt: f32,
    pub humidity_pct: f32,
    pub description: String,
    pub weather_code: i64,
    pub unit: String,
}

static CACHE: Lazy<Mutex<Option<(Instant, WeatherData)>>> = Lazy::new(|| Mutex::new(None));

/// 5 dakikada bir gercek bir agdan cekim yapar (theme.yaml'deki
/// STATS.WEATHER.INTERVAL: 300 ile ayni), aradaki cagrilar cache'ten
/// doner. Bu fonksiyon AG ISTEGI YAPABILIR (saniyeler surebilir) - ana
/// render dongusunun kullandigi stats mutex'i TUTULMADAN, ayri bir
/// arka plan thread'inden cagrilmalidir (lhm.rs'deki subprocess
/// hatasinin aynisina dusmemek icin).
pub fn get(lat: f64, lon: f64, units: &str) -> WeatherData {
    {
        let cache = CACHE.lock().unwrap();
        if let Some((t, ref d)) = *cache {
            if t.elapsed() < Duration::from_secs(300) {
                return d.clone();
            }
        }
    }

    let data = match fetch(lat, lon, units) {
        Some(d) => {
            log::info!("Weather fetched: {:.1} degrees, {}", d.temperature, d.description);
            d
        }
        None => {
            log::warn!("Weather data could not be fetched (network/API issue) - lat={}, lon={}", lat, lon);
            WeatherData::default()
        }
    };

    let mut cache = CACHE.lock().unwrap();
    *cache = Some((Instant::now(), data.clone()));
    data
}

/// En son basarili sonucu, yeni bir ag istegi YAPMADAN dondurur (varsa).
/// Render dongusunda her cagrildiginda internete gitmemek icin kullanilir.
pub fn get_cached() -> Option<WeatherData> {
    CACHE.lock().unwrap().as_ref().map(|(_, d)| d.clone())
}

fn fetch(lat: f64, lon: f64, units: &str) -> Option<WeatherData> {
    let temp_unit = if units.eq_ignore_ascii_case("imperial") { "fahrenheit" } else { "celsius" };
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code&temperature_unit={}",
        lat, lon, temp_unit
    );

    let call_result = ureq::get(&url).timeout(Duration::from_secs(10)).call();
    let resp: serde_json::Value = match call_result {
        Ok(r) => match r.into_json() {
            Ok(j) => j,
            Err(e) => {
                log::warn!("Weather: JSON parse error: {}", e);
                return None;
            }
        },
        Err(e) => {
            log::warn!("Weather: request failed: {}", e);
            return None;
        }
    };

    let current = resp.get("current")?;
    let temperature = current.get("temperature_2m")?.as_f64()? as f32;
    let temperature_felt = current
        .get("apparent_temperature")
        .and_then(|v| v.as_f64())
        .unwrap_or(temperature as f64) as f32;
    let humidity_pct = current
        .get("relative_humidity_2m")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32;
    let code = current.get("weather_code").and_then(|v| v.as_i64()).unwrap_or(0);

    let unit = if units.eq_ignore_ascii_case("imperial") { "F".to_string() } else { "C".to_string() };

    Some(WeatherData {
        temperature,
        temperature_felt,
        humidity_pct,
        description: wmo_description(code).to_string(),
        weather_code: code,
        unit,
    })
}

/// Maps WMO weather code to icon filename (without extension)
pub fn wmo_icon_name(code: i64) -> &'static str {
    match code {
        0 => "sunny",
        1..=3 => "cloudy",
        45 | 48 => "foggy",
        51..=57 => "rainy",
        61..=67 => "rainy",
        71..=77 => "snowy",
        80..=82 => "rainy",
        85 | 86 => "snowy",
        95..=99 => "stormy",
        _ => "cloudy",
    }
}

/// WMO weather code -> short description
fn wmo_description(code: i64) -> &'static str {
    match code {
        0 => "Clear sky",
        1..=3 => "Partly cloudy",
        45 | 48 => "Fog",
        51..=57 => "Drizzle",
        61..=67 => "Rain",
        71..=77 => "Snow",
        80..=82 => "Rain showers",
        85 | 86 => "Snow showers",
        95..=99 => "Thunderstorm",
        _ => "Unknown",
    }
}
