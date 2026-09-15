#!/usr/bin/env python3
"""Plot a gain-source light curve from one or more fivepoints text reports.

Example:
    python3 scripts/plot_gain_lightcurve.py \
        five_point_result/I26191F_c/*.txt \
        five_point_result/I26204F_c/*.txt \
        --source J1041+536 \
        --fit 1d \
        --output five_point_result/J1041+536_c_lightcurve.png
"""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
from pathlib import Path
import re
import sys
from collections import defaultdict

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt


GAIN_HEADER = re.compile(
    r"^gain source=(?P<source>.+?) pair=(?P<pair>\d+) reference flux calibrator="
)
FREQUENCY = re.compile(r"^frequency:\s*(?P<frequency>\S+)", re.MULTILINE)
CENTER_TIME = re.compile(
    r"^gain five-point center time mean = (?P<timestamp>\S+) "
    r"MJD=(?P<mjd>[+-]?[0-9]+(?:\.[0-9]+)?)$"
)


@dataclass(frozen=True)
class GainPoint:
    report: Path
    source: str
    frequency: str
    pair: int
    mjd: float
    timestamp: str
    flux_1d: float
    error_1d: float
    flux_2d: float
    error_2d: float


def source_key(source: str) -> str:
    """Normalize 1041+536 and J1041+536 to the same key."""
    key = re.sub(r"\s+", "", source).lower()
    if key.startswith("j"):
        key = key[1:]
    return key


def number_from_line(line: str, prefix: str, report: Path) -> float | None:
    if not line.startswith(prefix):
        return None
    value = line[len(prefix) :].strip()
    try:
        return float(value.split()[0])
    except ValueError as exc:
        raise ValueError(f"{report}: invalid number in: {line}") from exc


def parse_report(report: Path) -> list[GainPoint]:
    text = report.read_text(encoding="utf-8")
    frequency_match = FREQUENCY.search(text)
    if frequency_match is None:
        raise ValueError(f"{report}: frequency: line was not found")
    frequency = frequency_match.group("frequency").upper()

    points: list[GainPoint] = []
    current: dict[str, object] | None = None

    def finish_current() -> None:
        if current is None:
            return
        required = (
            "source",
            "pair",
            "mjd",
            "timestamp",
            "flux_1d",
            "error_1d",
            "flux_2d",
            "error_2d",
        )
        missing = [name for name in required if name not in current]
        if missing:
            raise ValueError(
                f"{report}: incomplete gain pair {current.get('pair')}; "
                f"missing {', '.join(missing)}"
            )
        points.append(
            GainPoint(
                report=report,
                source=str(current["source"]),
                frequency=frequency,
                pair=int(current["pair"]),
                mjd=float(current["mjd"]),
                timestamp=str(current["timestamp"]),
                flux_1d=float(current["flux_1d"]),
                error_1d=float(current["error_1d"]),
                flux_2d=float(current["flux_2d"]),
                error_2d=float(current["error_2d"]),
            )
        )

    for line in text.splitlines():
        header = GAIN_HEADER.match(line)
        if header:
            finish_current()
            current = {
                "source": header.group("source"),
                "pair": int(header.group("pair")),
            }
            continue
        if current is None:
            continue

        match = CENTER_TIME.match(line)
        if match:
            current["timestamp"] = match.group("timestamp")
            current["mjd"] = float(match.group("mjd"))
            continue

        fields = (
            ("flux_1d", "gain flux density from 1D = "),
            ("error_1d", "gain flux density thermal error from 1D (1-sigma) = "),
            ("flux_2d", "gain flux density from 2D = "),
            ("error_2d", "gain flux density thermal error from 2D (1-sigma) = "),
        )
        for name, prefix in fields:
            value = number_from_line(line, prefix, report)
            if value is not None:
                current[name] = value
                break

    finish_current()
    return points


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Plot a gain-source light curve from fivepoints text reports."
    )
    parser.add_argument(
        "reports",
        nargs="+",
        type=Path,
        help="fivepoints text report files",
    )
    parser.add_argument(
        "--source",
        default="J1041+536",
        help="gain source to plot; 1041+536 and J1041+536 are equivalent",
    )
    parser.add_argument(
        "--frequency",
        help="plot only this frequency, for example C or X; default: plot each frequency separately",
    )
    parser.add_argument(
        "--fit",
        choices=("1d", "2d", "both"),
        default="1d",
        help="Gaussian result to plot (default: 1d)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("five_point_result/gain_lightcurve.png"),
        help="output PNG path",
    )
    parser.add_argument(
        "--data-output",
        type=Path,
        help="TSV path for the data used in the plot; default: <PNG stem>_data.tsv",
    )
    parser.add_argument("--dpi", type=int, default=150, help="PNG resolution")
    return parser.parse_args()


def write_plot_data(
    output: Path,
    points: list[GainPoint],
    selected_fits: tuple[str, ...],
) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.writer(handle, delimiter="\t", lineterminator="\n")
        writer.writerow(
            [
                "source",
                "frequency",
                "pair",
                "timestamp",
                "mjd",
                "fit",
                "flux_density_jy",
                "thermal_error_jy",
                "source_report",
            ]
        )
        for point in sorted(points, key=lambda item: (item.mjd, item.frequency, item.pair)):
            for fit_name in selected_fits:
                if fit_name == "1d":
                    flux, error = point.flux_1d, point.error_1d
                else:
                    flux, error = point.flux_2d, point.error_2d
                writer.writerow(
                    [
                        point.source,
                        point.frequency,
                        point.pair,
                        point.timestamp,
                        f"{point.mjd:.8f}",
                        fit_name,
                        f"{flux:.9f}",
                        f"{error:.9f}",
                        str(point.report),
                    ]
                )



def main() -> int:
    args = parse_args()
    wanted_source = source_key(args.source)
    wanted_frequency = args.frequency.upper() if args.frequency else None

    points: list[GainPoint] = []
    for report in args.reports:
        if not report.is_file():
            print(f"error: report does not exist: {report}", file=sys.stderr)
            return 2
        try:
            points.extend(parse_report(report))
        except ValueError as exc:
            print(f"error: {exc}", file=sys.stderr)
            return 2

    points = [
        point
        for point in points
        if source_key(point.source) == wanted_source
        and (wanted_frequency is None or point.frequency == wanted_frequency)
    ]
    if not points:
        print(
            f"error: no gain pairs found for source {args.source}"
            + (f" at frequency {wanted_frequency}" if wanted_frequency else ""),
            file=sys.stderr,
        )
        return 2

    groups: dict[str, list[GainPoint]] = defaultdict(list)
    for point in points:
        groups[point.frequency].append(point)

    fig, ax = plt.subplots(figsize=(8.0, 5.0))
    colors = plt.get_cmap("tab10")
    fit_specs = {
        "1d": ("1D", "o", "-"),
        "2d": ("2D", "s", "--"),
    }
    selected_fits = ("1d", "2d") if args.fit == "both" else (args.fit,)
    data_output = args.data_output or args.output.with_name(
        f"{args.output.stem}_data.tsv"
    )
    write_plot_data(data_output, points, selected_fits)

    for color_index, frequency in enumerate(sorted(groups)):
        color = colors(color_index % 10)
        series = sorted(groups[frequency], key=lambda point: (point.mjd, point.pair))
        x = [point.mjd for point in series]
        for fit_name in selected_fits:
            label, marker, linestyle = fit_specs[fit_name]
            if fit_name == "1d":
                y = [point.flux_1d for point in series]
                yerr = [point.error_1d for point in series]
            else:
                y = [point.flux_2d for point in series]
                yerr = [point.error_2d for point in series]
            ax.errorbar(
                x,
                y,
                yerr=yerr,
                color=color,
                marker=marker,
                linestyle=linestyle,
                linewidth=1.0,
                markersize=5,
                capsize=3,
                label=f"{frequency} {label}",
            )

    ax.set_xlabel("MJD")
    ax.set_ylabel("Gain flux density (Jy)")
    ax.set_title(f"{args.source} gain-source light curve")
    ax.grid(False)
    ax.legend()
    fig.tight_layout()

    args.output.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(args.output, dpi=args.dpi)
    plt.close(fig)

    for point in sorted(points, key=lambda item: (item.mjd, item.frequency, item.pair)):
        print(
            f"{point.frequency} pair={point.pair} "
            f"time={point.timestamp} MJD={point.mjd:.5f} "
            f"flux1d={point.flux_1d:.9f}+-{point.error_1d:.9f} Jy "
            f"flux2d={point.flux_2d:.9f}+-{point.error_2d:.9f} Jy "
            f"source={point.source} report={point.report}"
        )
    print(f"saved plot: {args.output}")
    print(f"saved plot data: {data_output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
