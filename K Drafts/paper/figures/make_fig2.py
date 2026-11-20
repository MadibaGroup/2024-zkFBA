# Draws the worked order book of Section 3 as an SVG: python make_fig2.py
import csv
import os

here = os.path.dirname(os.path.abspath(__file__))
rows = list(csv.DictReader(open(os.path.join(here, "worked_book.csv"))))
bids = [int(r["Bids++"]) for r in rows]
asks = [int(r["Asks++"]) for r in rows]
n = len(rows)

# Cumulative demand from the top tick down, supply from the bottom tick up.
acc_b = [sum(bids[i:]) for i in range(n)]
acc_a = [sum(asks[: i + 1]) for i in range(n)]
vol = [min(a, b) for a, b in zip(acc_a, acc_b)]
v_max = max(vol)
c = vol.index(v_max)
d = n - 1 - vol[::-1].index(v_max)
p_star = 5

W, H, left, bottom, top = 640, 360, 60, 50, 30
plot_w, plot_h = W - left - 30, H - bottom - top
y_max = max(acc_a + acc_b) + 2
step = plot_w / n


def x(i):
    return left + i * step


def y(v):
    return top + plot_h * (1 - v / y_max)


def stairs(values):
    pts = []
    for i, v in enumerate(values):
        pts += [(x(i), y(v)), (x(i + 1), y(v))]
    return " ".join(f"{px:.1f},{py:.1f}" for px, py in pts)


out = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" font-family="sans-serif" font-size="12">']
out.append(f'<rect width="{W}" height="{H}" fill="white"/>')
# Plateau band.
out.append(f'<rect x="{x(c):.1f}" y="{top}" width="{(d - c + 1) * step:.1f}" height="{plot_h}" fill="#e8e8e8"/>')
out.append(f'<text x="{(x(c) + x(d + 1)) / 2:.1f}" y="{top + 14}" text-anchor="middle">plateau [c, d] = [{c}, {d}]</text>')
# Executable volume bars.
for i, v in enumerate(vol):
    out.append(f'<rect x="{x(i) + 6:.1f}" y="{y(v):.1f}" width="{step - 12:.1f}" height="{y(0) - y(v):.1f}" fill="#9db7d5"/>')
# Axes.
out.append(f'<line x1="{left}" y1="{y(0)}" x2="{left + plot_w}" y2="{y(0)}" stroke="black"/>')
out.append(f'<line x1="{left}" y1="{top}" x2="{left}" y2="{y(0)}" stroke="black"/>')
for i in range(n):
    out.append(f'<text x="{x(i) + step / 2:.1f}" y="{y(0) + 16:.1f}" text-anchor="middle">{i}</text>')
for v in range(0, y_max, 5):
    out.append(f'<text x="{left - 8}" y="{y(v) + 4:.1f}" text-anchor="end">{v}</text>')
    out.append(f'<line x1="{left - 4}" y1="{y(v):.1f}" x2="{left}" y2="{y(v):.1f}" stroke="black"/>')
out.append(f'<text x="{left + plot_w / 2:.1f}" y="{H - 8}" text-anchor="middle">price tick</text>')
out.append(f'<text x="16" y="{top + plot_h / 2:.1f}" text-anchor="middle" transform="rotate(-90 16 {top + plot_h / 2:.1f})">volume</text>')
# Depth curves.
out.append(f'<polyline points="{stairs(acc_b)}" fill="none" stroke="#b03030" stroke-width="2"/>')
out.append(f'<polyline points="{stairs(acc_a)}" fill="none" stroke="#2a6f3a" stroke-width="2" stroke-dasharray="6 3"/>')
# Maximum volume and clearing tick.
out.append(f'<line x1="{left}" y1="{y(v_max):.1f}" x2="{left + plot_w}" y2="{y(v_max):.1f}" stroke="#555" stroke-dasharray="2 3"/>')
out.append(f'<text x="{left + plot_w - 4}" y="{y(v_max) - 5:.1f}" text-anchor="end">V_max = {v_max}</text>')
out.append(f'<text x="{x(p_star) + step / 2:.1f}" y="{y(0) + 32:.1f}" text-anchor="middle" font-weight="bold">p*</text>')
# Legend.
lx, ly = left + 12, top + 34
out.append(f'<line x1="{lx}" y1="{ly}" x2="{lx + 26}" y2="{ly}" stroke="#b03030" stroke-width="2"/><text x="{lx + 32}" y="{ly + 4}">demand depth AccB</text>')
out.append(f'<line x1="{lx}" y1="{ly + 18}" x2="{lx + 26}" y2="{ly + 18}" stroke="#2a6f3a" stroke-width="2" stroke-dasharray="6 3"/><text x="{lx + 32}" y="{ly + 22}">supply depth AccA</text>')
out.append(f'<rect x="{lx}" y="{ly + 30}" width="26" height="10" fill="#9db7d5"/><text x="{lx + 32}" y="{ly + 40}">executable volume Min</text>')
out.append("</svg>")

open(os.path.join(here, "fig2-worked-book.svg"), "w").write("\n".join(out))
print("AccB", acc_b, "AccA", acc_a, "Min", vol, "plateau", c, d)
