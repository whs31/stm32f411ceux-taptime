"""Chart rendering for /time output (Pillow-free; uses matplotlib only)."""
from __future__ import annotations

from io import BytesIO

import matplotlib
matplotlib.use("Agg")  # non-interactive backend — must be set before importing pyplot
import matplotlib.patches as mpatches
import matplotlib.pyplot as plt

from .workhours import DayRow, seconds_worked

# ── colour palette ────────────────────────────────────────────────────────────
_C = {
    "bg":        "#fafafa",
    "grid":      "#e9ecef",
    "text":      "#343a40",
    "axis":      "#adb5bd",
    "req":       "#fa5252",   # required-hours reference line
    "normal":    "#5b8dee",   # worked, balanced
    "overtime":  "#20c997",   # worked > required
    "undertime": "#fd7e14",   # worked < required
    "missing":   "#dee2e6",   # past workday with no record
    "dayoff":    "#adb5bd",   # day off
    "remote":    "#74c0fc",   # remote day
    "weekend":   "#cc5de8",   # weekend work
}

_LEGEND_ITEMS = [
    ("normal",    "Worked"),
    ("overtime",  "Overtime"),
    ("undertime", "Undertime"),
    ("missing",   "Missing"),
    ("dayoff",    "Day off"),
    ("remote",    "Remote"),
    ("weekend",   "Weekend"),
]


def _save(fig: plt.Figure) -> BytesIO:
    buf = BytesIO()
    fig.savefig(buf, format="png", dpi=130, bbox_inches="tight",
                facecolor=fig.get_facecolor())
    plt.close(fig)
    buf.seek(0)
    return buf


def _apply_style(ax: plt.Axes) -> None:
    ax.set_facecolor(_C["bg"])
    ax.grid(axis="y", color=_C["grid"], linewidth=0.8, zorder=0)
    ax.set_axisbelow(True)
    for spine in ("top", "right"):
        ax.spines[spine].set_visible(False)
    for spine in ("left", "bottom"):
        ax.spines[spine].set_color(_C["axis"])
    ax.tick_params(colors=_C["text"], labelsize=8)
    ax.yaxis.label.set_color(_C["text"])


def render_month(
    rows: list[DayRow],
    name: str,
    month_name: str,
    year: int,
    user_req: int,
    lunch: int = 0,
) -> BytesIO:
    """Bar chart: one bar per day, height = hours worked, colour = status."""
    N = len(rows)
    fig_w = max(8.0, N * 0.48 + 2.0)
    fig, ax = plt.subplots(figsize=(fig_w, 5.0))
    fig.patch.set_facecolor(_C["bg"])

    labels: list[str] = []
    heights: list[float] = []
    colors: list[str] = []
    req_markers: list[float | None] = []

    req_h = user_req / 3600
    lunch_h = lunch / 3600

    for r in rows:
        labels.append(f"{r.d.day}\n{r.weekday_abbr[0]}")

        day_req = (r.required_seconds / 3600) if r.required_seconds else req_h

        if r.is_day_off:
            heights.append(day_req)
            colors.append(_C["dayoff"])
            req_markers.append(None)
        elif r.is_remote:
            heights.append(day_req)
            colors.append(_C["remote"])
            req_markers.append(None)
        elif r.is_weekend and r.check_in and r.check_out:
            heights.append(seconds_worked(r.check_in, r.check_out) / 3600)
            colors.append(_C["weekend"])
            req_markers.append(None)
        elif r.check_in and r.check_out:
            worked_h = seconds_worked(r.check_in, r.check_out) / 3600
            heights.append(worked_h)
            if r.balance_seconds and r.balance_seconds > 0:
                colors.append(_C["overtime"])
            elif r.balance_seconds and r.balance_seconds < 0:
                colors.append(_C["undertime"])
            else:
                colors.append(_C["normal"])
            req_markers.append(day_req)
        elif r.check_in:
            heights.append(0.15)          # in-progress placeholder
            colors.append(_C["normal"])
            req_markers.append(day_req)
        else:
            heights.append(0.0)
            colors.append(_C["missing"])
            # Missing past workday: effective required is reduced by lunch
            req_markers.append(max(day_req - lunch_h, 0.0) if r.balance_seconds is not None else None)

    x = list(range(N))
    ax.bar(x, heights, color=colors, width=0.72, edgecolor="white", linewidth=0.5, zorder=3)

    # Required-hours reference line (baseline, full at-work threshold)
    ax.axhline(y=req_h, color=_C["req"], linestyle="--", linewidth=1.5,
               label=f"Required ({req_h:.1f}h)", zorder=5)

    # Per-day required markers — show the effective required for each day
    # (drops by lunch for missing past workdays)
    for xi, m in zip(x, req_markers):
        if m is None or abs(m - req_h) < 1e-6:
            continue
        ax.hlines(y=m, xmin=xi - 0.36, xmax=xi + 0.36,
                  color=_C["req"], linestyles=":", linewidth=1.2, zorder=5)

    # Axes
    ax.set_xticks(x)
    ax.set_xticklabels(labels, fontsize=8)
    ax.set_ylabel("Hours")
    y_top = max(req_h * 1.3, (max(heights) if heights else req_h) * 1.1, req_h + 1.0)
    ax.set_ylim(0, y_top)
    ax.set_title(f"{month_name} {year}  —  {name}",
                 color=_C["text"], fontsize=13, fontweight="bold", pad=12)

    _apply_style(ax)

    # Only show legend entries whose colour actually appears
    used = set(colors)
    handles = [
        mpatches.Patch(color=_C[k], label=lbl)
        for k, lbl in _LEGEND_ITEMS if _C[k] in used
    ]
    handles.append(plt.Line2D([0], [0], color=_C["req"], linestyle="--",
                               linewidth=1.5, label=f"Required ({req_h:.1f}h)"))
    ax.legend(handles=handles, loc="upper right", fontsize=8, framealpha=0.92)

    fig.tight_layout()
    return _save(fig)


def render_year(
    month_data: list[tuple[str, int]],
    name: str,
    year: int,
) -> BytesIO:
    """Bar chart: one bar per month, height = net balance (positive up, negative down)."""
    N = len(month_data)
    fig, ax = plt.subplots(figsize=(max(6.0, N * 0.85 + 2.0), 4.5))
    fig.patch.set_facecolor(_C["bg"])

    labels = [m[0] for m in month_data]
    values = [m[1] / 3600 for m in month_data]
    colors = [_C["overtime"] if v >= 0 else _C["undertime"] for v in values]

    x = list(range(N))
    ax.bar(x, values, color=colors, width=0.6, edgecolor="white", linewidth=0.5, zorder=3)
    ax.axhline(y=0, color=_C["axis"], linewidth=1.0, zorder=4)

    ax.set_xticks(x)
    ax.set_xticklabels(labels, fontsize=10)
    ax.set_ylabel("Balance (hours)")
    ax.set_title(f"{year}  —  {name}",
                 color=_C["text"], fontsize=13, fontweight="bold", pad=12)

    _apply_style(ax)

    handles = [
        mpatches.Patch(color=_C["overtime"],  label="Overtime"),
        mpatches.Patch(color=_C["undertime"], label="Undertime"),
    ]
    ax.legend(handles=handles, loc="upper right", fontsize=9, framealpha=0.92)

    fig.tight_layout()
    return _save(fig)
