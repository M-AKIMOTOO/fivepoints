use plotters::prelude::*;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const PERLEY_BUTLER_2017_CATALOG: &str = include_str!("../data/perley_butler_2017.tsv");

#[derive(Debug, Clone, Copy)]
struct GaussianParams {
    amp: f64,
    mu: f64,
    sigma: f64,
}

impl GaussianParams {
    fn value_at(&self, x: f64) -> f64 {
        let n = (x - self.mu) / self.sigma;
        self.amp * (-0.5 * n * n).exp()
    }
}

#[derive(Debug, Clone, Copy)]
struct Gaussian2DParams {
    amp: f64,
    mu_az: f64,
    mu_el: f64,
    sigma_az: f64,
    sigma_el: f64,
}

impl Gaussian2DParams {
    fn value_at(&self, az: f64, el: f64) -> f64 {
        let az_n = (az - self.mu_az) / self.sigma_az;
        let el_n = (el - self.mu_el) / self.sigma_el;
        self.amp * (-0.5 * (az_n * az_n + el_n * el_n)).exp()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Timestamp {
    year: u16,
    doy: u16,
    seconds: u32,
}

impl Timestamp {
    fn new(year: u16, doy: u16, hour: u32, minute: u32, second: u32) -> Result<Self, String> {
        if doy == 0 || doy > 366 || hour >= 24 || minute >= 60 || second >= 60 {
            return Err(format!(
                "invalid timestamp fields: {year}/{doy} {hour}:{minute}:{second}"
            ));
        }
        Ok(Self {
            year: year % 100,
            doy,
            seconds: hour * 3600 + minute * 60 + second,
        })
    }

    fn hms(self) -> (u32, u32, u32) {
        (
            self.seconds / 3600,
            (self.seconds % 3600) / 60,
            self.seconds % 60,
        )
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (h, m, s) = self.hms();
        write!(
            f,
            "20{:02}/{:03}T{:02}:{:02}:{:02}",
            self.year, self.doy, h, m, s
        )
    }
}

#[derive(Debug, Clone)]
struct DataRow {
    time: Timestamp,
    source: String,
    amplitude: f64,
    snr: f64,
    mjd: f64,
}

type DataMap = HashMap<(Timestamp, String), Vec<DataRow>>;

#[derive(Debug, Clone)]
struct FluxCalibrator {
    primary_name: String,
    aliases: Vec<String>,
    c_ghz: f64,
    c_jy: f64,
    x_ghz: f64,
    x_jy: f64,
    k_ghz: f64,
    k_jy: f64,
    coefficients: [f64; 6],
}

impl FluxCalibrator {
    fn matches_source(&self, source: &str) -> bool {
        self.aliases
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(source))
    }

    fn frequency_info(&self, frequency: &str) -> Option<(f64, f64)> {
        match frequency.to_ascii_lowercase().as_str() {
            "c" => Some((self.c_ghz, self.c_jy)),
            "x" => Some((self.x_ghz, self.x_jy)),
            "k" => Some((self.k_ghz, self.k_jy)),
            _ => None,
        }
    }

    fn model_flux_jy(&self, frequency_ghz: f64) -> f64 {
        let log_frequency = frequency_ghz.log10();
        let mut log_flux = 0.0;
        for coefficient in self.coefficients.iter().rev() {
            log_flux = log_flux * log_frequency + coefficient;
        }
        10.0_f64.powf(log_flux)
    }
}

fn parse_flux_calibrators(text: &str) -> Result<Vec<FluxCalibrator>, Box<dyn Error>> {
    let mut calibrators = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("source_name\t") {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 17 {
            return Err(format!(
                "flux catalog line {} has {} fields; expected 17",
                line_number + 1,
                fields.len()
            )
            .into());
        }
        let parse = |index: usize| -> Result<f64, Box<dyn Error>> {
            fields[index].parse::<f64>().map_err(|_| {
                format!(
                    "flux catalog line {}: invalid number {}",
                    line_number + 1,
                    fields[index]
                )
                .into()
            })
        };
        let aliases = fields[0..3]
            .iter()
            .filter(|value| !value.is_empty())
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>();
        let mut coefficients = [0.0; 6];
        for (i, coefficient) in coefficients.iter_mut().enumerate() {
            *coefficient = parse(9 + i)?;
        }
        calibrators.push(FluxCalibrator {
            primary_name: fields[0].to_string(),
            aliases,
            c_ghz: parse(3)?,
            c_jy: parse(4)?,
            x_ghz: parse(5)?,
            x_jy: parse(6)?,
            k_ghz: parse(7)?,
            k_jy: parse(8)?,
            coefficients,
        });
    }
    Ok(calibrators)
}

fn find_flux_calibrator<'a>(
    catalog: &'a [FluxCalibrator],
    source: &str,
) -> Option<&'a FluxCalibrator> {
    catalog.iter().find(|entry| entry.matches_source(source))
}

#[derive(Debug, Clone)]
struct ScheduleEntry {
    source: String,
    time: Timestamp,
    az_offset: f64,
    el_offset: f64,
}

#[derive(Debug, Clone)]
struct FivePointScan {
    entries: [ScheduleEntry; 5],
}

impl FivePointScan {
    fn source(&self) -> &str {
        &self.entries[0].source
    }
}

#[derive(Debug)]
struct ScheduleFile {
    path: PathBuf,
    experiment: String,
    entries: Vec<ScheduleEntry>,
    scans: Vec<FivePointScan>,
}

#[derive(Debug, Clone)]
struct FitResult {
    az: GaussianParams,
    el: GaussianParams,
    two_d: Gaussian2DParams,
    true_amp: f64,
}

#[derive(Debug, Clone)]
struct ScanResult {
    number: usize,
    source: String,
    center_time: Timestamp,
    center_amplitude: Option<f64>,
    center_snr: Option<f64>,
    center_mjd: Option<f64>,
    fit: Option<FitResult>,
}

#[derive(Debug, Clone)]
struct PairResult {
    source: String,
    yi_1d: f64,
    yi_2d: f64,
}

struct Cli {
    ifile: PathBuf,
    skd32m: PathBuf,
    skd34m: PathBuf,
    frequency: String,
    output_dir: PathBuf,
}

fn usage(program: &str) -> String {
    format!(
        "Usage: {program} --ifile DATA --skd32m SCHEDULE --skd34m SCHEDULE --frequency C|X|K [--output-dir DIR]\n\n\
         Match schedule timestamps and offsets to observation data, fit 1D and 2D Gaussian beams,\n         and estimate gain-source flux density from Perley & Butler 2017 calibrators.\n\
         print the comparison and save a text report and PNG plots.\n\
         Default output directory: ./five_point_result"
    )
}

fn parse_cli() -> Result<Option<Cli>, String> {
    let args: Vec<String> = env::args().collect();
    if args.len() == 1 {
        eprintln!("{}", usage(&args[0]));
        return Ok(None);
    }

    let mut ifile = None;
    let mut skd32m = None;
    let mut skd34m = None;
    let mut frequency = None;
    let mut output_dir = PathBuf::from("five_point_result");
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];
        if arg == "--help" || arg == "-h" {
            println!("{}", usage(&args[0]));
            return Ok(None);
        }

        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) if name.starts_with("--") => (name, Some(value.to_string())),
            _ => (arg.as_str(), None),
        };
        let next_value = |index: &mut usize| -> Result<String, String> {
            if let Some(value) = &inline {
                return Ok(value.clone());
            }
            *index += 1;
            args.get(*index)
                .cloned()
                .ok_or_else(|| format!("missing value for {name}"))
        };

        match name {
            "--ifile" => ifile = Some(PathBuf::from(next_value(&mut i)?)),
            "--skd32m" => skd32m = Some(PathBuf::from(next_value(&mut i)?)),
            "--skd34m" => skd34m = Some(PathBuf::from(next_value(&mut i)?)),
            "--frequency" => frequency = Some(next_value(&mut i)?),
            "--output-dir" => output_dir = PathBuf::from(next_value(&mut i)?),
            _ => return Err(format!("unknown argument: {arg}\n\n{}", usage(&args[0]))),
        }
        i += 1;
    }

    Ok(Some(Cli {
        ifile: ifile.ok_or_else(|| "--ifile is required".to_string())?,
        skd32m: skd32m.ok_or_else(|| "--skd32m is required".to_string())?,
        skd34m: skd34m.ok_or_else(|| "--skd34m is required".to_string())?,
        frequency: frequency.ok_or_else(|| "--frequency is required".to_string())?,
        output_dir,
    }))
}

fn parse_date_and_time(date: &str, time: &str) -> Result<Timestamp, String> {
    let mut d = date.split('/');
    let year = d
        .next()
        .ok_or_else(|| format!("invalid date: {date}"))?
        .parse::<u16>()
        .map_err(|_| format!("invalid year: {date}"))?;
    let doy = d
        .next()
        .ok_or_else(|| format!("invalid date: {date}"))?
        .parse::<u16>()
        .map_err(|_| format!("invalid day of year: {date}"))?;
    if d.next().is_some() {
        return Err(format!("invalid date: {date}"));
    }

    let mut t = time.split(':');
    let hour = t
        .next()
        .ok_or_else(|| format!("invalid time: {time}"))?
        .parse::<u32>()
        .map_err(|_| format!("invalid hour: {time}"))?;
    let minute = t
        .next()
        .ok_or_else(|| format!("invalid time: {time}"))?
        .parse::<u32>()
        .map_err(|_| format!("invalid minute: {time}"))?;
    let second = t
        .next()
        .ok_or_else(|| format!("invalid time: {time}"))?
        .parse::<u32>()
        .map_err(|_| format!("invalid second: {time}"))?;
    if t.next().is_some() {
        return Err(format!("invalid time: {time}"));
    }
    Timestamp::new(year, doy, hour, minute, second)
}

fn parse_data_timestamp(tokens: &[&str], fi: usize) -> Result<Timestamp, String> {
    if fi == 0 {
        return Err("frequency has no preceding timestamp".into());
    }
    let previous = tokens[fi - 1];
    if let Some((date, time)) = previous.split_once('T') {
        return parse_date_and_time(date, time);
    }
    if fi < 2 {
        return Err("timestamp needs date and time tokens".into());
    }
    parse_date_and_time(tokens[fi - 2], previous)
}

fn parse_data_file(path: &Path, frequency: &str) -> Result<DataMap, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let wanted = frequency.to_ascii_lowercase();
    let mut data = DataMap::new();
    let mut count = 0usize;

    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        let Some(fi) = tokens
            .iter()
            .position(|token| token.to_ascii_lowercase() == wanted)
        else {
            continue;
        };
        if fi + 15 >= tokens.len() {
            return Err(format!("{}:{}: short data row", path.display(), line_number + 1).into());
        }
        let time = parse_data_timestamp(&tokens, fi)
            .map_err(|e| format!("{}:{}: {e}", path.display(), line_number + 1))?;
        let source = tokens[fi + 1].to_string();
        let amplitude = tokens[fi + 3].parse::<f64>().map_err(|_| {
            format!(
                "{}:{}: invalid amplitude {}",
                path.display(),
                line_number + 1,
                tokens[fi + 3]
            )
        })?;
        let snr = tokens[fi + 4].parse::<f64>().map_err(|_| {
            format!(
                "{}:{}: invalid SNR {}",
                path.display(),
                line_number + 1,
                tokens[fi + 4]
            )
        })?;
        let mjd = tokens[fi + 15].parse::<f64>().map_err(|_| {
            format!(
                "{}:{}: invalid MJD {}",
                path.display(),
                line_number + 1,
                tokens[fi + 15]
            )
        })?;
        if !amplitude.is_finite() || !snr.is_finite() || !mjd.is_finite() {
            return Err(format!(
                "{}:{}: amplitude, SNR, or MJD is not finite",
                path.display(),
                line_number + 1
            )
            .into());
        }
        let row = DataRow {
            time,
            source: source.clone(),
            amplitude,
            snr,
            mjd,
        };
        data.entry((time, source)).or_default().push(row);
        count += 1;
    }
    if count == 0 {
        return Err(format!(
            "no frequency '{}' rows found in {}",
            frequency,
            path.display()
        )
        .into());
    }
    Ok(data)
}

fn parse_schedule_timestamp(token: &str) -> Result<Timestamp, String> {
    if token.len() != 11 || !token.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("invalid schedule timestamp: {token}"));
    }
    let year = token[0..2]
        .parse::<u16>()
        .map_err(|_| format!("invalid year: {token}"))?;
    let doy = token[2..5]
        .parse::<u16>()
        .map_err(|_| format!("invalid doy: {token}"))?;
    let hour = token[5..7]
        .parse::<u32>()
        .map_err(|_| format!("invalid hour: {token}"))?;
    let minute = token[7..9]
        .parse::<u32>()
        .map_err(|_| format!("invalid minute: {token}"))?;
    let second = token[9..11]
        .parse::<u32>()
        .map_err(|_| format!("invalid second: {token}"))?;
    Timestamp::new(year, doy, hour, minute, second)
}

fn parse_schedule(path: &Path) -> Result<ScheduleFile, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let mut experiment = None;
    let mut in_sked = false;
    let mut entries = Vec::new();

    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("$EXPER") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                experiment = Some(parts[1].to_string());
            }
            continue;
        }
        if trimmed == "$SKED" {
            in_sked = true;
            continue;
        }
        if in_sked && trimmed.starts_with('$') {
            break;
        }
        if !in_sked || trimmed.is_empty() || trimmed.starts_with('*') {
            continue;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 6 {
            continue;
        }
        let Ok(time) = parse_schedule_timestamp(parts[1]) else {
            continue;
        };
        let az_offset = parts[3].parse::<f64>().map_err(|_| {
            format!(
                "{}:{}: invalid AZ offset {}",
                path.display(),
                line_number + 1,
                parts[3]
            )
        })?;
        let el_offset = parts[4].parse::<f64>().map_err(|_| {
            format!(
                "{}:{}: invalid EL offset {}",
                path.display(),
                line_number + 1,
                parts[4]
            )
        })?;
        entries.push(ScheduleEntry {
            source: parts[0].to_string(),
            time,
            az_offset,
            el_offset,
        });
    }

    if entries.is_empty() {
        return Err(format!("no $SKED entries found in {}", path.display()).into());
    }
    let scans = find_scans(&entries);
    if scans.is_empty() {
        return Err(format!("no five-point scan pattern found in {}", path.display()).into());
    }
    Ok(ScheduleFile {
        path: path.to_path_buf(),
        experiment: experiment.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("experiment")
                .to_string()
        }),
        entries,
        scans,
    })
}

fn zero(value: f64) -> bool {
    value.abs() < 1e-9
}

fn find_scans(entries: &[ScheduleEntry]) -> Vec<FivePointScan> {
    let mut scans = Vec::new();
    for window in entries.windows(5) {
        let same_source = window.iter().all(|entry| entry.source == window[0].source);
        let cross_pattern = !zero(window[0].az_offset)
            && zero(window[0].el_offset)
            && zero(window[1].az_offset)
            && zero(window[1].el_offset)
            && !zero(window[2].az_offset)
            && zero(window[2].el_offset)
            && zero(window[3].az_offset)
            && !zero(window[3].el_offset)
            && zero(window[4].az_offset)
            && !zero(window[4].el_offset)
            && window[0].az_offset * window[2].az_offset < 0.0
            && window[3].el_offset * window[4].el_offset < 0.0;
        if same_source && cross_pattern {
            scans.push(FivePointScan {
                entries: window.to_vec().try_into().expect("five point window"),
            });
        }
    }
    scans
}

fn solve_axis(
    point1: (f64, f64),
    center: f64,
    point2: (f64, f64),
) -> Result<GaussianParams, String> {
    if center <= 0.0 || point1.1 <= 0.0 || point2.1 <= 0.0 {
        return Err("all Gaussian amplitudes must be positive".into());
    }
    let (x1, y1) = point1;
    let (x2, y2) = point2;
    if x1.abs() < 1e-12 || x2.abs() < 1e-12 || (x1 - x2).abs() < 1e-12 {
        return Err("offset positions must be distinct and non-zero".into());
    }

    let l1 = (y1 / center).ln();
    let l2 = (y2 / center).ln();
    let a1 = l1 / x1;
    let a2 = l2 / x2;
    let inv_sigma_sq = 2.0 * (a2 - a1) / (x1 - x2);
    if !inv_sigma_sq.is_finite() || inv_sigma_sq <= 0.0 {
        return Err("computed non-positive Gaussian inverse sigma^2".into());
    }
    let sigma_sq = 1.0 / inv_sigma_sq;
    let mu = (a1 + 0.5 * x1 * inv_sigma_sq) / inv_sigma_sq;
    let sigma = sigma_sq.sqrt();
    let amp = center * (0.5 * mu * mu * inv_sigma_sq).exp();
    if !amp.is_finite() || !mu.is_finite() || !sigma.is_finite() {
        return Err("computed non-finite Gaussian parameters".into());
    }
    Ok(GaussianParams { amp, mu, sigma })
}

fn fit_scan(scan: &FivePointScan, amplitudes: &[f64; 5]) -> Result<FitResult, String> {
    let center = amplitudes[1];
    let az = solve_axis(
        (scan.entries[0].az_offset, amplitudes[0]),
        center,
        (scan.entries[2].az_offset, amplitudes[2]),
    )?;
    let el = solve_axis(
        (scan.entries[3].el_offset, amplitudes[3]),
        center,
        (scan.entries[4].el_offset, amplitudes[4]),
    )?;
    let true_amp = az.amp * el.amp / center;
    Ok(FitResult {
        az,
        el,
        two_d: Gaussian2DParams {
            amp: true_amp,
            mu_az: az.mu,
            mu_el: el.mu,
            sigma_az: az.sigma,
            sigma_el: el.sigma,
        },
        true_amp,
    })
}

fn plot_gaussian(
    filename: &Path,
    params: &GaussianParams,
    samples: &[(f64, f64)],
    caption: &str,
    x_label: &str,
) -> Result<(), Box<dyn Error>> {
    let (xmin, xmax) = (-10.0, 10.0);
    let xs: Vec<f64> = (0..=1000)
        .map(|i| xmin + (xmax - xmin) * i as f64 / 1000.0)
        .collect();
    let curve: Vec<(f64, f64)> = xs.iter().map(|x| (*x, params.value_at(*x))).collect();
    let mut ymax = curve.iter().map(|(_, y)| *y).fold(0.0, f64::max);
    for (_, y) in samples {
        ymax = ymax.max(*y);
    }
    if ymax <= 0.0 {
        ymax = 1.0;
    }

    let root = BitMapBackend::new(filename, (900, 600)).into_drawing_area();
    root.fill(&WHITE)?;
    let mut chart = ChartBuilder::on(&root)
        .caption(caption, ("sans-serif", 26))
        .margin(20)
        .x_label_area_size(60)
        .y_label_area_size(70)
        .build_cartesian_2d(xmin..xmax, 0.0..ymax * 1.1)?;
    chart
        .configure_mesh()
        .x_desc(x_label)
        .y_desc("amplitude (%)")
        .label_style(("sans-serif", 22))
        .axis_desc_style(("sans-serif", 20))
        .disable_mesh()
        .draw()?;
    chart.draw_series(LineSeries::new(curve, &RED))?;
    chart.draw_series(
        samples
            .iter()
            .map(|(x, y)| Circle::new((*x, *y), 5, ShapeStyle::from(&BLUE).filled())),
    )?;
    root.present()?;
    quantize_png_with_imagequant(filename)?;
    Ok(())
}

fn plot_gaussian_2d(
    filename: &Path,
    params: &Gaussian2DParams,
    samples: &[(f64, f64, f64)],
    caption: &str,
) -> Result<(), Box<dyn Error>> {
    let (xmin, xmax, ymin, ymax) = (-10.0, 10.0, -10.0, 10.0);
    let max_value = params
        .amp
        .max(samples.iter().map(|(_, _, z)| *z).fold(0.0, f64::max))
        .max(1e-12);
    // Keep the image and the AZ/EL plotting area square so one arcmin has
    // the same pixel scale on both axes.
    let root = BitMapBackend::new(filename, (900, 900)).into_drawing_area();
    root.fill(&WHITE)?;
    root.draw(&Text::new(
        caption.to_string(),
        (20, 10),
        ("sans-serif", 26),
    ))?;
    let mut chart = ChartBuilder::on(&root)
        .margin(35)
        .x_label_area_size(70)
        .y_label_area_size(70)
        .build_cartesian_2d(xmin..xmax, ymin..ymax)?;
    chart
        .configure_mesh()
        .x_desc("AZ offset (arcmin)")
        .y_desc("EL offset (arcmin)")
        .label_style(("sans-serif", 22))
        .axis_desc_style(("sans-serif", 20))
        .disable_mesh()
        .draw()?;

    let grid = 80usize;
    for ix in 0..grid {
        for iy in 0..grid {
            let x0 = xmin + (xmax - xmin) * ix as f64 / grid as f64;
            let x1 = xmin + (xmax - xmin) * (ix + 1) as f64 / grid as f64;
            let y0 = ymin + (ymax - ymin) * iy as f64 / grid as f64;
            let y1 = ymin + (ymax - ymin) * (iy + 1) as f64 / grid as f64;
            let value = params.value_at((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            let ratio = (value / max_value).clamp(0.0, 1.0);
            chart.draw_series(std::iter::once(Rectangle::new(
                [(x0, y0), (x1, y1)],
                HSLColor(0.66 * (1.0 - ratio), 0.9, 0.5).filled(),
            )))?;
        }
    }
    chart.draw_series(
        samples
            .iter()
            .map(|(x, y, _)| Circle::new((*x, *y), 6, ShapeStyle::from(&BLACK).filled())),
    )?;
    root.present()?;
    quantize_png_with_imagequant(filename)?;
    Ok(())
}

fn quantize_png_with_imagequant(filename: &Path) -> Result<(), Box<dyn Error>> {
    let rgba_image = image::open(filename)?.to_rgba8();
    let (width, height) = rgba_image.dimensions();
    let pixels: Vec<imagequant::RGBA> = rgba_image
        .pixels()
        .map(|pixel| imagequant::RGBA::new(pixel[0], pixel[1], pixel[2], pixel[3]))
        .collect();

    // Keep imagequant's default attributes and default remapping behavior.
    let attr = imagequant::new();
    let mut image = attr.new_image(pixels, width as usize, height as usize, 0.0)?;
    let mut quantization = attr.quantize(&mut image)?;
    let (palette, indices) = quantization.remapped(&mut image)?;

    let mut palette_bytes = Vec::with_capacity(palette.len() * 3);
    let mut transparency = Vec::with_capacity(palette.len());
    let mut has_transparency = false;
    for color in &palette {
        palette_bytes.extend_from_slice(&[color.r, color.g, color.b]);
        transparency.push(color.a);
        has_transparency |= color.a < 255;
    }

    let output = fs::File::create(filename)?;
    let mut encoder = png::Encoder::new(output, width, height);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_palette(palette_bytes);
    if has_transparency {
        encoder.set_trns(transparency);
    }
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&indices)?;
    Ok(())
}

fn safe_component(value: &str) -> String {
    let result: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if result.is_empty() {
        "unknown".into()
    } else {
        result
    }
}

fn lookup(data: &DataMap, entry: &ScheduleEntry) -> Option<DataRow> {
    data.get(&(entry.time, entry.source.clone()))
        .and_then(|rows| rows.first().cloned())
}

fn append_comparison(
    report: &mut String,
    ant: &str,
    number: usize,
    scan: &FivePointScan,
    rows: &[Option<DataRow>; 5],
) {
    report.push_str(&format!(
        "\n### {ant} scan {number}: source={} center={}\n",
        scan.source(),
        scan.entries[1].time
    ));
    for (i, (entry, row)) in scan.entries.iter().zip(rows.iter()).enumerate() {
        report.push_str(&format!(
            "point {} schedule={} offset=({:+.3}, {:+.3})",
            i + 1,
            entry.time,
            entry.az_offset,
            entry.el_offset
        ));
        if let Some(row) = row {
            report.push_str(&format!(
                " -> data={} source={} amplitude={:.9} SNR={:.3} MJD={:.5}\n",
                row.time, row.source, row.amplitude, row.snr, row.mjd
            ));
        } else {
            report.push_str(" -> MISSING\n");
        }
    }
}

fn process_scan(
    ant: &str,
    scan: &FivePointScan,
    number: usize,
    data: &DataMap,
    save_dir: &Path,
    frequency: &str,
    experiment: &str,
    report: &mut String,
) -> Result<ScanResult, Box<dyn Error>> {
    let rows: [Option<DataRow>; 5] = std::array::from_fn(|i| lookup(data, &scan.entries[i]));
    append_comparison(report, ant, number, scan, &rows);
    let center_amplitude = rows[1].as_ref().map(|row| row.amplitude);
    let center_snr = rows[1].as_ref().map(|row| row.snr);
    let center_mjd = rows[1].as_ref().map(|row| row.mjd);
    let incomplete = || ScanResult {
        number,
        source: scan.source().to_string(),
        center_time: scan.entries[1].time,
        center_amplitude,
        center_snr,
        center_mjd,
        fit: None,
    };

    if rows.iter().any(Option::is_none) {
        report.push_str("status: incomplete; fit skipped\n");
        return Ok(incomplete());
    }

    let amplitudes = [
        rows[0].as_ref().unwrap().amplitude,
        rows[1].as_ref().unwrap().amplitude,
        rows[2].as_ref().unwrap().amplitude,
        rows[3].as_ref().unwrap().amplitude,
        rows[4].as_ref().unwrap().amplitude,
    ];
    let fit = match fit_scan(scan, &amplitudes) {
        Ok(fit) => fit,
        Err(error) => {
            report.push_str(&format!("status: fit failed: {error}\n"));
            return Ok(incomplete());
        }
    };

    report.push_str(&format!(
        "status: matched 5/5\n\
         AZ 1D (amp, mu, sigma) = ({:.9}, {:.6}, {:.6})\n\
         EL 1D (amp, mu, sigma) = ({:.9}, {:.6}, {:.6})\n\
         2D Gaussian (A, mu_AZ, mu_EL, sigma_AZ, sigma_EL) = ({:.9}, {:.6}, {:.6}, {:.6}, {:.6})\n\
         center amplitude I(0,0) = {:.9}\n\
         true amplitude derivation: A_true = A_AZ * A_EL / I(0,0)\n\
           = {:.9} * {:.9} / {:.9}\n\
           = {:.9}\n\
         2D Gaussian peak A = {:.9}\n",
        fit.az.amp,
        fit.az.mu,
        fit.az.sigma,
        fit.el.amp,
        fit.el.mu,
        fit.el.sigma,
        fit.two_d.amp,
        fit.two_d.mu_az,
        fit.two_d.mu_el,
        fit.two_d.sigma_az,
        fit.two_d.sigma_el,
        amplitudes[1],
        fit.az.amp,
        fit.el.amp,
        amplitudes[1],
        fit.true_amp,
        fit.two_d.amp
    ));

    let stem = format!(
        "{}_{}_{}_{}_{}",
        safe_component(experiment),
        safe_component(frequency),
        safe_component(scan.source()),
        ant,
        number
    );
    plot_gaussian(
        &save_dir.join(format!("{stem}_AZ_1d.png")),
        &fit.az,
        &[
            (scan.entries[0].az_offset, amplitudes[0]),
            (scan.entries[1].az_offset, amplitudes[1]),
            (scan.entries[2].az_offset, amplitudes[2]),
        ],
        &format!("{ant} {} AZ", scan.source()),
        "AZ offset (arcmin)",
    )?;
    plot_gaussian(
        &save_dir.join(format!("{stem}_EL_1d.png")),
        &fit.el,
        &[
            (scan.entries[3].el_offset, amplitudes[3]),
            (scan.entries[1].el_offset, amplitudes[1]),
            (scan.entries[4].el_offset, amplitudes[4]),
        ],
        &format!("{ant} {} EL", scan.source()),
        "EL offset (arcmin)",
    )?;
    plot_gaussian_2d(
        &save_dir.join(format!("{stem}_2d.png")),
        &fit.two_d,
        &[
            (
                scan.entries[0].az_offset,
                scan.entries[0].el_offset,
                amplitudes[0],
            ),
            (
                scan.entries[1].az_offset,
                scan.entries[1].el_offset,
                amplitudes[1],
            ),
            (
                scan.entries[2].az_offset,
                scan.entries[2].el_offset,
                amplitudes[2],
            ),
            (
                scan.entries[3].az_offset,
                scan.entries[3].el_offset,
                amplitudes[3],
            ),
            (
                scan.entries[4].az_offset,
                scan.entries[4].el_offset,
                amplitudes[4],
            ),
        ],
        &format!("{ant} {} 2D Gaussian", scan.source()),
    )?;

    Ok(ScanResult {
        number,
        source: scan.source().to_string(),
        center_time: scan.entries[1].time,
        center_amplitude,
        center_snr,
        center_mjd,
        fit: Some(fit),
    })
}

fn append_pairs(
    report: &mut String,
    scans32: &[ScanResult],
    scans34: &[ScanResult],
) -> Vec<PairResult> {
    report.push_str("\n\n## Paired 32m/34m results\n");
    let mut sources = Vec::<String>::new();
    for scan in scans32.iter().chain(scans34.iter()) {
        if !sources.iter().any(|source| source == &scan.source) {
            sources.push(scan.source.clone());
        }
    }
    let mut pairs = Vec::new();
    for source in sources {
        let left: Vec<&ScanResult> = scans32
            .iter()
            .filter(|scan| scan.source == source)
            .collect();
        let right: Vec<&ScanResult> = scans34
            .iter()
            .filter(|scan| scan.source == source)
            .collect();
        report.push_str(&format!(
            "\nsource={source}: 32m scans={}, 34m scans={}\n",
            left.len(),
            right.len()
        ));
        for i in 0..left.len().max(right.len()) {
            let (Some(a), Some(b)) = (left.get(i), right.get(i)) else {
                report.push_str(&format!("pair {}: schedule count mismatch\n", i + 1));
                continue;
            };
            let (Some(af), Some(bf), Some(ac), Some(bc)) =
                (&a.fit, &b.fit, a.center_amplitude, b.center_amplitude)
            else {
                report.push_str(&format!("pair {}: incomplete fit\n", i + 1));
                continue;
            };
            let center_mean = (ac + bc) / 2.0;
            let yi_1d = af.true_amp * bf.true_amp / center_mean;
            let yi_2d = af.two_d.amp * bf.two_d.amp / center_mean;
            report.push_str(&format!(
                "pair {}: 32m #{} ({}) + 34m #{} ({})\n\
                 YI true amplitude from 1D = {:.9}\n\
                 YI true amplitude from 2D = {:.9}\n",
                i + 1,
                a.number,
                a.center_time,
                b.number,
                b.center_time,
                yi_1d,
                yi_2d
            ));
            pairs.push(PairResult {
                source: source.clone(),
                yi_1d,
                yi_2d,
            });
        }
    }
    pairs
}

fn average(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn timestamp_tick(timestamp: Timestamp) -> i64 {
    let seconds_per_day = 86_400_i64;
    let days_per_year = 366_i64;
    timestamp.year as i64 * days_per_year * seconds_per_day
        + (timestamp.doy as i64 - 1) * seconds_per_day
        + timestamp.seconds as i64
}

fn timestamp_from_tick(tick: f64) -> Timestamp {
    let seconds_per_day = 86_400_i64;
    let days_per_year = 366_i64;
    let rounded = tick.round() as i64;
    let year = rounded.div_euclid(days_per_year * seconds_per_day);
    let within_year = rounded.rem_euclid(days_per_year * seconds_per_day);
    let doy = within_year.div_euclid(seconds_per_day) + 1;
    let seconds = within_year.rem_euclid(seconds_per_day);
    Timestamp {
        year: year as u16,
        doy: doy as u16,
        seconds: seconds as u32,
    }
}

fn average_timestamp(values: &[Timestamp]) -> Option<Timestamp> {
    if values.is_empty() {
        None
    } else {
        let sum = values
            .iter()
            .map(|timestamp| timestamp_tick(*timestamp) as f64)
            .sum::<f64>();
        Some(timestamp_from_tick(sum / values.len() as f64))
    }
}

fn append_gain_flux_calibration(
    report: &mut String,
    scans32: &[ScanResult],
    scans34: &[ScanResult],
    pairs: &[PairResult],
    catalog: &[FluxCalibrator],
    frequency: &str,
) {
    report.push_str("\n\n## Gain calibrator flux density\n");
    let Some((_, _)) = catalog
        .iter()
        .find_map(|entry| entry.frequency_info(frequency))
    else {
        report.push_str(&format!(
            "Perley & Butler 2017 flux calibration skipped: frequency '{}' is not C, X, or K\n",
            frequency
        ));
        return;
    };

    let mut reference_sources = Vec::<String>::new();
    let mut gain_sources = Vec::<String>::new();
    for pair in pairs {
        if find_flux_calibrator(catalog, &pair.source).is_some() {
            if !reference_sources
                .iter()
                .any(|source| source == &pair.source)
            {
                reference_sources.push(pair.source.clone());
            }
        } else if !gain_sources.iter().any(|source| source == &pair.source) {
            gain_sources.push(pair.source.clone());
        }
    }

    if reference_sources.is_empty() {
        report.push_str("No Perley & Butler 2017 flux calibrator was found in the completed five-point scans.\n");
        return;
    }
    if gain_sources.is_empty() {
        report
            .push_str("No non-catalog gain source was found in the completed five-point scans.\n");
        return;
    }

    let all_scans: Vec<&ScanResult> = scans32.iter().chain(scans34.iter()).collect();
    for gain_source in gain_sources {
        let gain_pairs: Vec<&PairResult> = pairs
            .iter()
            .filter(|pair| pair.source == gain_source)
            .collect();
        let gain_scans: Vec<&ScanResult> = all_scans
            .iter()
            .copied()
            .filter(|scan| scan.source == gain_source)
            .collect();
        let gain_yi_1d = average(&gain_pairs.iter().map(|pair| pair.yi_1d).collect::<Vec<_>>());
        let gain_yi_2d = average(&gain_pairs.iter().map(|pair| pair.yi_2d).collect::<Vec<_>>());
        let center_snrs = gain_scans
            .iter()
            .filter_map(|scan| scan.center_snr)
            .collect::<Vec<_>>();
        let center_mjds = gain_scans
            .iter()
            .filter_map(|scan| scan.center_mjd)
            .collect::<Vec<_>>();
        let center_times = gain_scans
            .iter()
            .map(|scan| scan.center_time)
            .collect::<Vec<_>>();
        let (
            Some(gain_yi_1d),
            Some(gain_yi_2d),
            Some(center_snr),
            Some(center_mjd),
            Some(center_time),
        ) = (
            gain_yi_1d,
            gain_yi_2d,
            average(&center_snrs),
            average(&center_mjds),
            average_timestamp(&center_times),
        )
        else {
            continue;
        };

        for reference_source in &reference_sources {
            let Some(reference) = find_flux_calibrator(catalog, reference_source) else {
                continue;
            };
            let Some((frequency_ghz, tabulated_flux_jy)) = reference.frequency_info(frequency)
            else {
                continue;
            };
            let reference_pairs: Vec<&PairResult> = pairs
                .iter()
                .filter(|pair| pair.source == *reference_source)
                .collect();
            let reference_yi_1d = average(
                &reference_pairs
                    .iter()
                    .map(|pair| pair.yi_1d)
                    .collect::<Vec<_>>(),
            );
            let reference_yi_2d = average(
                &reference_pairs
                    .iter()
                    .map(|pair| pair.yi_2d)
                    .collect::<Vec<_>>(),
            );
            let (Some(reference_yi_1d), Some(reference_yi_2d)) = (reference_yi_1d, reference_yi_2d)
            else {
                continue;
            };
            let catalog_flux_jy = reference.model_flux_jy(frequency_ghz);
            let gain_flux_1d = gain_yi_1d / reference_yi_1d * catalog_flux_jy;
            let gain_flux_2d = gain_yi_2d / reference_yi_2d * catalog_flux_jy;
            let gain_flux_error_1d = gain_flux_1d / center_snr;
            let gain_flux_error_2d = gain_flux_2d / center_snr;
            report.push_str(&format!(
                "\ngain source={gain_source} reference flux calibrator={reference_source} ({})\n\
                 flux calibrator flux density at {:.3} GHz = {:.9} Jy\n\
                 flux calibrator flux density at {:.3} GHz = {:.9} Jy\n\
                 catalog frequency = {:.3} GHz\n\
                 catalog flux density (Perley & Butler polynomial) = {:.9} Jy\n\
                 catalog tabulated flux density = {:.9} Jy\n\
                 gain YI mean from 1D = {:.9}\n\
                 gain YI mean from 2D = {:.9}\n\
                 reference YI mean from 1D = {:.9}\n\
                 reference YI mean from 2D = {:.9}\n\
                 gain flux density from 1D = {:.9} Jy\n\
                 gain flux density thermal error from 1D (1-sigma) = {:.9} Jy\n\
                 gain flux density from 2D = {:.9} Jy\n\
                 gain flux density thermal error from 2D (1-sigma) = {:.9} Jy\n\
                 gain five-point center SNR mean = {:.3}\n\
                 gain five-point center time mean = {} MJD={:.5}\n\
                 calibration formula: S_gain = (YI_gain / YI_reference) * S_reference\n\
                 thermal error formula: sigma_S = S_gain / mean center SNR\n",
                reference.primary_name,
                reference.c_ghz,
                reference.c_jy,
                reference.x_ghz,
                reference.x_jy,
                frequency_ghz,
                catalog_flux_jy,
                tabulated_flux_jy,
                gain_yi_1d,
                gain_yi_2d,
                reference_yi_1d,
                reference_yi_2d,
                gain_flux_1d,
                gain_flux_error_1d,
                gain_flux_2d,
                gain_flux_error_2d,
                center_snr,
                center_time,
                center_mjd
            ));
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    if cli.frequency.trim().is_empty() {
        return Err("--frequency must not be empty".into());
    }
    let schedule32 = parse_schedule(&cli.skd32m)?;
    let schedule34 = parse_schedule(&cli.skd34m)?;
    if schedule32.experiment != schedule34.experiment {
        eprintln!(
            "warning: schedule experiment names differ: {} vs {}",
            schedule32.experiment, schedule34.experiment
        );
    }
    let data = parse_data_file(&cli.ifile, &cli.frequency)?;
    let experiment = schedule32.experiment.clone();
    let save_dir = cli.output_dir.join(format!(
        "{}_{}",
        safe_component(&experiment),
        safe_component(&cli.frequency)
    ));
    fs::create_dir_all(&save_dir)?;

    let mut report = String::new();
    report.push_str("five-point analysis\n");
    report.push_str(&format!("input file: {}\n", cli.ifile.display()));
    report.push_str(&format!("frequency: {}\n", cli.frequency));
    report.push_str(&format!("32m schedule: {}\n", schedule32.path.display()));
    report.push_str(&format!("34m schedule: {}\n", schedule34.path.display()));
    report.push_str(&format!(
        "schedule entries: 32m={}, 34m={}; detected scans: 32m={}, 34m={}\n",
        schedule32.entries.len(),
        schedule34.entries.len(),
        schedule32.scans.len(),
        schedule34.scans.len()
    ));
    report.push_str(&format!(
        "parsed data rows at frequency: {}\n",
        data.values().map(Vec::len).sum::<usize>()
    ));
    report.push_str("fit equations (AZ and EL offsets are in arcmin)\n");
    report.push_str("AZ 1D: I_AZ(x) = A_AZ * exp(-(x - mu_AZ)^2 / (2 * sigma_AZ^2))\n");
    report.push_str("EL 1D: I_EL(y) = A_EL * exp(-(y - mu_EL)^2 / (2 * sigma_EL^2))\n");
    report.push_str("2D: I(x,y) = A * exp(-(x - mu_AZ)^2 / (2 * sigma_AZ^2)) * exp(-(y - mu_EL)^2 / (2 * sigma_EL^2))\n");
    report.push_str("parameters: A=peak, mu_AZ/mu_EL=center, sigma_AZ/sigma_EL=width\n");

    report.push_str(
        "true amplitude derivation: the separable 2D Gaussian has peak A, while the AZ and EL 1D fits have peaks A_AZ and A_EL; therefore I(0,0) = A_AZ * A_EL / A and A_true = A_AZ * A_EL / I(0,0).\n",
    );

    let mut scans32 = Vec::new();
    for (i, scan) in schedule32.scans.iter().enumerate() {
        scans32.push(process_scan(
            "32m",
            scan,
            i + 1,
            &data,
            &save_dir,
            &cli.frequency,
            &experiment,
            &mut report,
        )?);
    }
    let mut scans34 = Vec::new();
    for (i, scan) in schedule34.scans.iter().enumerate() {
        scans34.push(process_scan(
            "34m",
            scan,
            i + 1,
            &data,
            &save_dir,
            &cli.frequency,
            &experiment,
            &mut report,
        )?);
    }
    let pairs = append_pairs(&mut report, &scans32, &scans34);
    let catalog = parse_flux_calibrators(PERLEY_BUTLER_2017_CATALOG)?;
    append_gain_flux_calibration(
        &mut report,
        &scans32,
        &scans34,
        &pairs,
        &catalog,
        &cli.frequency,
    );

    let input_stem = cli
        .ifile
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("input");
    let result_path = save_dir.join(format!(
        "{}_{}_five_point_result.txt",
        safe_component(&experiment),
        safe_component(input_stem)
    ));
    fs::write(&result_path, &report)?;
    println!("{report}");
    println!("saved result: {}", result_path.display());
    println!("saved plots: {}", save_dir.display());
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    match parse_cli()? {
        Some(cli) => run(cli),
        None => Ok(()),
    }
}
