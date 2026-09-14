# Five Points

Rust program for five-point beam analysis. It matches observation schedules with observation data, fits AZ/EL Gaussian beams, and saves the results.

## Usage

Specify the observation data file, the 32m and 34m schedules, and the frequency label.

```bash
cargo run -- \
  --ifile data/i26191f_frinZ_len0s_c_time.tsv \
  --skd32m data/I26191F32.skd \
  --skd34m data/I26191F34.skd \
  --frequency c
```

With a built binary:

```bash
fivepoints_calc \
  --ifile observation.tsv \
  --skd32m I26191F32.skd \
  --skd34m I26191F34.skd \
  --frequency c
```

The value of `--frequency` is normally `c`, `x`, or `k`. The default output directory is `./five_point_result`; use `--output-dir` to change it.

```bash
fivepoints_calc \
  --ifile observation.tsv \
  --skd32m I26191F32.skd \
  --skd34m I26191F34.skd \
  --frequency x \
  --output-dir ./result
```

The command prints the schedule/data comparison and fit results. It also saves a text report and PNG plots.

## Schedule/data matching

The program detects five-point scans in the `$SKED` section of each schedule:

1. AZ offset +2
2. Center point (0, 0)
3. AZ offset -2
4. EL offset +2
5. EL offset -2

Each schedule entry is matched to the observation data by timestamp and source name. Each point in the report contains the correlation amplitude, SNR, and MJD.

Output is grouped by experiment and frequency:

```text
five_point_result/
+-- I26191F_c/
    +-- I26191F_i26191f_frinZ_len0s_c_time_five_point_result.txt
    +-- ..._AZ_1d.png
    +-- ..._EL_1d.png
    +-- ..._2d.png
```

## Gaussian fits

The AZ and EL scans are fitted with one-dimensional Gaussian functions:

```text
G(x) = A * exp(-(x - mu)^2 / (2 * sigma^2))
```

The report gives the peak `amp`, center `mu`, and width `sigma`.

The five points also define an axis-aligned, separable two-dimensional Gaussian. It is allowed to be elliptical because sigma_AZ and sigma_EL are fitted independently:

```text
I(x,y) = A
         * exp(-(x - mu_AZ)^2 / (2 * sigma_AZ^2))
         * exp(-(y - mu_EL)^2 / (2 * sigma_EL^2))
```

The plots do not display grid lines. The 2D PNG is a square image with equal AZ/EL pixel scales, so an observed circular beam is drawn as a circle. An ellipse in the figure therefore represents the fitted difference between sigma_AZ and sigma_EL, rather than display distortion. Every graph PNG is passed through imagequant 4.4.1 with its default attributes before it is saved.

## Gain calibrator flux density

The repository includes the Perley & Butler (2017) flux calibrator table in
`data/perley_butler_2017.tsv`. If a catalog calibrator and a non-catalog gain
source are both present in completed five-point scans, the program estimates the
gain-source flux density from the YI true-amplitude ratio:

`S_gain = (YI_gain / YI_reference) * S_reference`

Here `S_reference` is calculated from the catalog polynomial at the selected observing
frequency. The report prints the catalog value, the reference and gain YI true-amplitude
means, and the resulting gain flux density from both the 1D and 2D analyses.

The report also records the calibrator flux densities at 6.856 GHz and 8.448 GHz.

The gain-source five-point center SNR and time are also reported as the arithmetic means
over all completed 32m and 34m five-point scans for that source. The time line includes
both the schedule timestamp and its MJD.

## Derivation of antenna true amplitude

Let:

- `A_AZ` be the peak from the AZ one-dimensional fit.
- `A_EL` be the peak from the EL one-dimensional fit.
- `I(0,0)` be the measured amplitude at the center point.
- `A` be the peak of the separable two-dimensional Gaussian.

The AZ scan varies x (AZ offset) while fixing y=0 (EL offset):

```text
I_AZ(x) = I(x,0)
         = A
           * exp(-(x - mu_AZ)^2 / (2 * sigma_AZ^2))
           * exp(-(0 - mu_EL)^2 / (2 * sigma_EL^2))
```

Its peak occurs at x=mu_AZ, so

```text
A_AZ = I_AZ(mu_AZ)
      = A * exp(-(0 - mu_EL)^2 / (2 * sigma_EL^2))
      = A * exp(-mu_EL^2 / (2 * sigma_EL^2))
```

The EL scan varies y (EL offset) while fixing x=0 (AZ offset):

```text
I_EL(y) = I(0,y)
         = A
           * exp(-(0 - mu_AZ)^2 / (2 * sigma_AZ^2))
           * exp(-(y - mu_EL)^2 / (2 * sigma_EL^2))
```

Its peak occurs at y=mu_EL, so

```text
A_EL = I_EL(mu_EL)
      = A * exp(-(0 - mu_AZ)^2 / (2 * sigma_AZ^2))
      = A * exp(-mu_AZ^2 / (2 * sigma_AZ^2))
```

Therefore, the -mu_AZ term belongs to the fixed AZ factor of the EL slice, while the -mu_EL term belongs to the fixed EL factor of the AZ slice.

The center point is

```text
I(0,0) = A
         * exp(-mu_AZ^2 / (2 * sigma_AZ^2)
               -mu_EL^2 / (2 * sigma_EL^2))
```

Therefore,

```text
A_true = A_AZ * A_EL / I(0,0)
```

For example:

```text
A_AZ  = 0.047631273
A_EL  = 0.046162930
I(0,0) = 0.046160000

A_true = 0.047631273 * 0.046162930 / 0.046160000
       = 0.047634296
```

The current two-dimensional model is axis-aligned and has no AZ/EL correlation term. With these five points, its peak `A` is algebraically identical to the true amplitude calculated above. A rotated or correlated two-dimensional Gaussian requires additional observation points.

## Verification

```bash
cargo fmt -- --check
cargo check
cargo test
```
