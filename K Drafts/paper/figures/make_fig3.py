# Draws prover time, verifier time and proof size against ticks: python make_fig3.py
import csv
import math
import os
import statistics

here = os.path.dirname(os.path.abspath(__file__))
results = os.path.join(here, "..", "..", "results")


def rust(threads, metric, kin="32"):
    # Median of one metric per tick count from the Rust scaling run.
    path = os.path.join(results, "final", f"scaling_{threads}thr.csv")
    out = {}
    for r in csv.DictReader(open(path)):
        if r["k_in"] == kin and r["metric"] == metric:
            out[int(r["n_ticks"])] = float(r["median_ms"])
    return out


def noir(threads, fields):
    # Median over all runs of the sum of the given fields, per tick count.
    path = os.path.join(results, "final_noir", f"synthetic_{threads}thr_raw.csv")
    groups = {}
    for r in csv.DictReader(open(path)):
        groups.setdefault(int(r["ticks"]), []).append(sum(float(r[f]) for f in fields))
    return {t: statistics.median(v) for t, v in groups.items()}


def add(a, b):
    return {t: a[t] + b[t] for t in a}


panels = [
    ("(a) Prover time (ms)", True, [
        ("This work, 1 thread", rust(1, "prove"), "#1f4e79", ""),
        ("This work, 8 threads", rust(8, "prove"), "#1f4e79", "5 3"),
        ("Noir, 1 thread", noir(1, ["witness_ms", "prove_ms"]), "#b03030", ""),
        ("Noir, 8 threads", noir(8, ["witness_ms", "prove_ms"]), "#b03030", "5 3"),
    ]),
    ("(b) Verifier time (ms), 1 thread", False, [
        ("This work", add(rust(1, "verify"), rust(1, "decode")), "#1f4e79", ""),
        ("Noir", noir(1, ["verify_ms"]), "#b03030", ""),
    ]),
    ("(c) Proof size (bytes)", False, [
        ("This work, 32-bit", rust(1, "proof_bytes"), "#1f4e79", ""),
        ("This work, 16-bit", rust(1, "proof_bytes", "16"), "#1f4e79", "5 3"),
        ("Noir", noir(1, ["proof_bytes"]), "#b03030", ""),
    ]),
]

PW, PH, GAP, LEFT, TOP, BOTTOM = 250, 190, 70, 60, 34, 92
W = LEFT + 3 * PW + 2 * GAP + 20
H = TOP + PH + BOTTOM
x_lo, x_hi = math.log10(80), math.log10(10000)
out = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" font-family="sans-serif" font-size="11">',
       f'<rect width="{W}" height="{H}" fill="white"/>']

for k, (title, log_y, series) in enumerate(panels):
    x0 = LEFT + k * (PW + GAP)
    values = [v for _, s, _, _ in series for v in s.values()]
    if log_y:
        y_lo, y_hi = math.floor(math.log10(min(values))), math.ceil(math.log10(max(values)))
        ticks_y = [10 ** e for e in range(y_lo, y_hi + 1)]
        fy = lambda v: TOP + PH * (1 - (math.log10(v) - y_lo) / (y_hi - y_lo))
    else:
        top_v = max(values) * 1.15
        step = 10 ** math.floor(math.log10(top_v)) / 2
        ticks_y = [i * step for i in range(int(top_v / step) + 1)]
        fy = lambda v, top_v=top_v: TOP + PH * (1 - v / top_v)
    fx = lambda t: x0 + PW * (math.log10(t) - x_lo) / (x_hi - x_lo)

    out.append(f'<text x="{x0 + PW / 2}" y="{TOP - 12}" text-anchor="middle" font-size="12">{title}</text>')
    out.append(f'<rect x="{x0}" y="{TOP}" width="{PW}" height="{PH}" fill="none" stroke="black"/>')
    for t in (100, 1000, 10000):
        out.append(f'<line x1="{fx(t):.1f}" y1="{TOP + PH}" x2="{fx(t):.1f}" y2="{TOP + PH + 4}" stroke="black"/>')
        out.append(f'<text x="{fx(t):.1f}" y="{TOP + PH + 16}" text-anchor="middle">{t:,}</text>')
    for v in ticks_y:
        out.append(f'<line x1="{x0}" y1="{fy(v):.1f}" x2="{x0 + PW}" y2="{fy(v):.1f}" stroke="#dddddd"/>')
        out.append(f'<text x="{x0 - 5}" y="{fy(v) + 4:.1f}" text-anchor="end">{v:,.0f}</text>')
    out.append(f'<text x="{x0 + PW / 2}" y="{TOP + PH + 32}" text-anchor="middle">price ticks N</text>')
    for j, (label, s, colour, dash) in enumerate(series):
        pts = " ".join(f"{fx(t):.1f},{fy(s[t]):.1f}" for t in sorted(s))
        d = f' stroke-dasharray="{dash}"' if dash else ""
        out.append(f'<polyline points="{pts}" fill="none" stroke="{colour}" stroke-width="1.8"{d}/>')
        for t in sorted(s):
            out.append(f'<circle cx="{fx(t):.1f}" cy="{fy(s[t]):.1f}" r="2.3" fill="{colour}"/>')
        ly = TOP + PH + 46 + 12 * j
        out.append(f'<line x1="{x0 + 10}" y1="{ly}" x2="{x0 + 36}" y2="{ly}" stroke="{colour}" stroke-width="1.8"{d}/>')
        out.append(f'<text x="{x0 + 42}" y="{ly + 4}">{label}</text>')

out.append("</svg>")
open(os.path.join(here, "fig3-performance.svg"), "w").write("\n".join(out))
for title, _, series in panels:
    print(title)
    for label, s, _, _ in series:
        print("  ", label, {t: round(s[t], 1) for t in sorted(s)})
