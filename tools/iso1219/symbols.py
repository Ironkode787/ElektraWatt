# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Draws the shipped ISO 1219 symbols of content/symbols on one module (doc 11 §1 rule 6).

A valve position is a square of 12 mm with its ports on the 4 mm grid 4 mm apart and 2 mm leads, its paths drawn
from its function definition's switching model and its actuators 6 mm (the manual override 3 mm); every other
component is a body of 12 mm or less centred between ports 8 mm from its centre. The script writes the graphics, the
connection points, the slot positions and the anchors of the symbols it names (and the documents and function
definitions it adds), keeping each document's other members and its version: raise the version of a symbol you
change by hand, then run `cargo xtask symbols-qet10` to seal the documents and generate the QET edition.

Run from the repository root: `python tools/iso1219/symbols.py .`. Coordinates in mm, in 0.5 mm steps.
"""

import copy, json, os, sys

ROOT = sys.argv[1] if len(sys.argv) > 1 else '.'
SYM = os.path.join(ROOT, 'content', 'symbols')
FND = os.path.join(ROOT, 'content', 'fndefs')
H = 500000


def q(v):
    n = round(v * 2)
    assert abs(v * 2 - n) < 1e-9, v
    return n * H


def pt(x, y):
    return [q(x), q(y)]


def S(pen, dash=None):
    s = {'pen': pen}
    if dash:
        s['dash'] = dash
    return s


def line(x1, y1, x2, y2, pen='p026', dash=None):
    return {'line': {'from': pt(x1, y1), 'to': pt(x2, y2), 'stroke': S(pen, dash)}}


def rect(x1, y1, x2, y2, pen='p026'):
    return {'rect': {'rect': [pt(x1, y1), pt(x2, y2)], 'stroke': S(pen)}}


def pl(pts, pen='p026', dash=None):
    return {'polyline': {'points': [pt(*p) for p in pts], 'stroke': S(pen, dash)}}


def pg(pts, pen='p013', fill=True):
    d = {'points': [pt(*p) for p in pts], 'closed': True, 'stroke': S(pen)}
    if fill:
        d['fill'] = 'by_layer'
    return {'polygon': d}


def circ(cx, cy, r, pen='p026'):
    return {'circle': {'centre': pt(cx, cy), 'r': q(r), 'stroke': S(pen)}}


def r05(v):
    return round(v * 2) / 2


def arrow(x1, y1, x2, y2, pen='p013'):
    """A flow arrow from (x1, y1) to (x2, y2): the line and a filled head 2 mm long at the end."""
    out = [line(x1, y1, x2, y2, pen)]
    if abs(y2 - y1) >= abs(x2 - x1):
        s = 1 if y2 > y1 else -1
        by = y2 - 2 * s
        bx = r05(x2 - (x2 - x1) * 2 / abs(y2 - y1))
        out.append(pg([(bx - 0.5, by), (bx + 0.5, by), (x2, y2)], pen))
    else:
        s = 1 if x2 > x1 else -1
        bx = x2 - 2 * s
        by = r05(y2 - (y2 - y1) * 2 / abs(x2 - x1))
        out.append(pg([(bx, by - 0.5), (bx, by + 0.5), (x2, y2)], pen))
    return out


def stub(cp):
    """The 2 mm lead of a connection point, from the body's edge to the point."""
    (x, y), d = cp
    dx, dy = {'left': (2, 0), 'right': (-2, 0), 'up': (0, 2), 'down': (0, -2)}[d]
    return line(x + dx, y + dy, x, y)


def zigzag_h(x, y, w=6, a=1.5):
    """A spring drawn left to right from (x, y), w mm long."""
    pts = [(x, y)]
    n = int(w)
    for i in range(1, n):
        pts.append((x + i, y - a if i % 2 else y + a))
    pts.append((x + w, y))
    return pl(pts)


def zigzag_v(x, y, h=6, a=1.5, pen='p013'):
    pts = [(x, y)]
    n = int(h)
    for i in range(1, n):
        pts.append((x - a if i % 2 else x + a, y + i))
    pts.append((x, y + h))
    return pl(pts, pen)


def load(key):
    with open(os.path.join(SYM, key + '.json'), encoding='utf-8') as f:
        return json.load(f)


def fndef(key):
    with open(os.path.join(FND, key + '.json'), encoding='utf-8') as f:
        return json.load(f)




def save(key, d):
    with open(os.path.join(SYM, key + '.json'), 'w', encoding='utf-8', newline='\n') as f:
        f.write(json.dumps(d, indent=2, ensure_ascii=False) + '\n')


def slot(kind, x, y, align, **extra):
    s = {'kind': kind, 'pos': pt(x, y), 'align': align, 'visible': True}
    s.update(extra)
    return s


def apply(key, graphics, cps, slots, anchor=None, links=None, new=None, version=None):
    """Writes a redrawn symbol: `cps` maps a CP index to ((x, y), dir); `slots` maps a slot kind (or
    ('cp', i)) to ((x, y), align). The document's slot order and arrangements are kept."""
    d = new if new is not None else load(key)
    d['graphics'] = graphics
    for c in d['cps']:
        (x, y), dr = cps[c['idx']]
        c['pos'] = pt(x, y)
        c['dir'] = dr
    for s in d['text_slots']:
        k = s['kind']
        k = ('cp', k['cp_designation']) if isinstance(k, dict) else k
        (x, y), al = slots[k]
        s['pos'] = pt(x, y)
        s['align'] = al
    if anchor is not None:
        d['anchor'] = {'named': pt(*anchor)}
    if links is not None:
        for l in d['link_points']:
            l['pos'] = pt(*links[l['name']])
    if version is not None:
        d['version'] = version
    save(key, d)


# ------------------------------------------------------------------------------------------------------------------
# Valves: the positions are squares of 12 mm (y 2 to 14), the ports on the square of index 1 (x 18 to 30), the
# columns at 20, 24 and 28, top ports at y 0, bottom ports at y 16 (2 mm leads). The paths come from the definition's
# switching model; the actuators from its actuation (position 0 on the left, the last on the right).

PORT_COL = {
    5: {'4': (2, 'up'), '2': (10, 'up'), '5': (2, 'down'), '1': (6, 'down'), '3': (10, 'down')},
    3: {'2': (6, 'up'), '1': (2, 'down'), '3': (10, 'down')},
    2: {'2': (6, 'up'), '1': (6, 'down')},
}
TOP, BOT = 2, 14


def actuator(kind, side, x, detent=False):
    """Draws actuator `kind` outward from x on `side` (-1 left, +1 right); returns (graphics, new x, port)."""
    g = []
    port = None
    if kind == 'spring':
        x2 = x + 6 * side
        g.append(zigzag_h(min(x, x2), 8))
        return g, x2, port
    if kind == 'solenoid':
        x2 = x + 6 * side
        a, b = min(x, x2), max(x, x2)
        g.append(rect(a, 5, b, 11))
        g.append(line(a, 11, b, 5))
        return g, x2, port
    if kind == 'manual_override':
        x2 = x + 3 * side
        a, b = min(x, x2), max(x, x2)
        g.append(rect(a, 6.5, b, 9.5))
        return g, x2, port
    if kind == 'pilot':
        x2 = x + 6 * side
        tip, base = x, x + 2 * side
        g.append(pg([(base, 7), (base, 9), (tip, 8)], 'p026', fill=False))
        g.append(line(base, 8, x2, 8, 'p026', 'dash42'))
        return g, x2, x2
    if kind == 'lever':
        x2 = x + 3 * side
        g.append(line(x, 8, x2, 8))
        g.append(line(x2, 8, x2 + 1.5 * side, 5))
        return g, x2 + 1.5 * side, port
    if kind == 'pushbutton':
        x2 = x + 3 * side
        g.append(line(x, 8, x2, 8))
        e = x2 + 1.5 * side
        g.append(pl([(x2, 6.5), (x2 + 1 * side, 7), (e, 8), (x2 + 1 * side, 9), (x2, 9.5)]))
        return g, e, port
    if kind == 'roller':
        x2 = x + 3 * side
        g.append(line(x, 8, x2, 8))
        g.append(circ(x2 + 1 * side, 8, 1))
        return g, x2 + 2 * side, port
    if kind == 'detent':
        x2 = x + 3 * side
        a, b = min(x, x2), max(x, x2)
        g.append(line(a, 8, b, 8))
        g.append(pl([(x + 1.5 * side, 9), (x + 1.5 * side + 0.5, 8)] if False else [(a + 1, 9.5), (a + 1.5, 8), (a + 2, 9.5)]))
        return g, x2, port
    raise ValueError(kind)


ORDER = ['spring', 'pilot', 'solenoid', 'manual_override', 'detent', 'lever', 'pushbutton', 'roller']


def valve(key, fn_key, version=None, new=None):
    fd = fndef(fn_key)
    des = {c['idx']: c['designation'] for c in fd['cps']}
    roles = {c['idx']: c.get('port_role') for c in fd['cps']}
    fluid = [i for i in des if roles[i] not in ('pilot14', 'pilot12', 'pilot10')]
    layout = PORT_COL[len(fluid)]
    sections = fd['valve_model']['sections']
    n = len(sections[0]['positions'])
    k = 1
    x0 = lambda i: 18 + 12 * (i - k)
    g = []
    for i in range(n):
        g.append(rect(x0(i), TOP, x0(i) + 12, BOT))
    # ports and leads
    cp = {}
    for i in fluid:
        col, d = layout[des[i]]
        x = 18 + col
        y = 0 if d == 'up' else 16
        cp[i] = ((x, y), d)
    for i in fluid:
        g.append(stub(cp[i]))
    # paths
    for s in sections:
        for i, p in enumerate(s['positions']):
            dx = x0(i) - 18
            for path in p['paths']:
                if isinstance(path, dict) and 'flow' in path:
                    a, b = path['flow']['from'], path['flow']['to']
                    (xa, ya), da = cp[a]
                    (xb, yb), db = cp[b]
                    assert da != db, (key, path)
                    ya = TOP if da == 'up' else BOT
                    yb = TOP if db == 'up' else BOT
                    g += arrow(xa + dx, ya, xb + dx, yb)
                elif isinstance(path, dict) and 'open' in path:
                    a, b = path['open']['a'], path['open']['b']
                    (xa, _), da = cp[a]
                    (xb, _), db = cp[b]
                    g.append(line(xa + dx, TOP if da == 'up' else BOT, xb + dx, TOP if db == 'up' else BOT, 'p013'))
                elif isinstance(path, dict) and 'closed' in path:
                    (xc, _), dc = cp[path['closed']]
                    xc += dx
                    if dc == 'up':
                        g.append(line(xc, TOP, xc, TOP + 2, 'p013'))
                        g.append(line(xc - 1, TOP + 2, xc + 1, TOP + 2, 'p013'))
                    else:
                        g.append(line(xc, BOT, xc, BOT - 2, 'p013'))
                        g.append(line(xc - 1, BOT - 2, xc + 1, BOT - 2, 'p013'))
    # actuators by side, from the body outward
    sides = {-1: [], 1: []}
    for s in sections:
        for a in s['actuation']:
            act = a['actuator']
            kind = act if isinstance(act, str) else next(iter(act))
            sel = a['selects']
            targets = [-1, 1] if 0 < sel < n - 1 else [-1 if sel == 0 else 1]
            for t in targets:
                pilot_port = act['pilot']['port'] if kind == 'pilot' else None
                if (kind, pilot_port) not in [(x[0], x[1]) for x in sides[t]]:
                    sides[t].append((kind, pilot_port))
    xl, xr = x0(0), x0(n - 1) + 12
    for side, x in ((-1, xl), (1, xr)):
        for kind, port in sorted(sides[side], key=lambda a: ORDER.index(a[0])):
            gg, x, p = actuator(kind, side, x)
            g += gg
            if p is not None:
                cp[port] = ((p, 8), 'left' if side < 0 else 'right')
        if side < 0:
            xl = x
        else:
            xr = x
    left = int(xl) if xl == int(xl) else int(xl) - 1
    slots = {
        'tag': ((17, -1), 'bottom_right'),
        'function_text': ((left, 15), 'top_left'),
        'plc_address': ((left, 3), 'bottom_left'),
    }
    for i in des:
        (x, y), d = cp[i]
        slots[('cp', i)] = des_slot(cp[i])
    anchor = (16, 8) if n == 2 else (24, 8)
    apply(key, g, cp, slots, anchor=anchor, new=new, version=version)


VALVES = [
    ('ew.iso.valve-2-2-nc-sol-spring', 'ew.fn.valve-2-2-nc-sol-spring'),
    ('ew.iso.valve-2x3-2-nc', 'ew.fn.valve-2x3-2-nc'),
    ('ew.iso.valve-3-2-no-sol-spring', 'ew.fn.valve-3-2-no-sol-spring'),
    ('ew.iso.valve-3-2-sol-spring', 'ew.fn.valve-3-2-sol-spring'),
    ('ew.iso.valve-5-2-pilot-spring', 'ew.fn.valve-5-2-pilot-spring'),
    ('ew.iso.valve-5-2-sol-sol', 'ew.fn.valve-5-2-sol-sol'),
    ('ew.iso.valve-5-2-sol-spring', 'ew.fn.valve-5-2-sol-spring'),
    ('ew.iso.valve-5-3-cc', 'ew.fn.valve-5-3-cc'),
    ('ew.iso.valve-5-3-ec', 'ew.fn.valve-5-3-ec'),
    ('ew.iso.valve-5-3-pc', 'ew.fn.valve-5-3-pc'),
    ('ew.iso.valve-on-off', 'ew.fn.valve-on-off'),
]

# ------------------------------------------------------------------------------------------------------------------
# Components: a body of 12 mm centred on (8, 8) (x and y 2 to 14), ports 8 mm from the centre with 2 mm leads:
# left (0, 8), right (16, 8), up (8, 0), down (8, 16).

L, R, U, D = ((0, 8), 'left'), ((16, 8), 'right'), ((8, 0), 'up'), ((8, 16), 'down')
DES = {'left': ((1, 6), 'middle_left'), 'right': ((15, 6), 'middle_right'),
       'up': ((9, 1), 'middle_left'), 'down': ((9, 15), 'middle_left')}


def des_slot(c):
    """A port's designation beside its tube, just beyond the port: above a horizontal tube, right of a vertical one."""
    (x, y), d = c
    if d == 'left':
        return ((x - 1, y - 3), 'middle_right')
    if d == 'right':
        return ((x + 1, y - 3), 'middle_left')
    if d == 'up':
        return ((x + 1, y - 1), 'middle_left')
    return ((x + 1, y + 1), 'middle_left')


def comp(key, body, cps, tag=None, ftext=None, anchor=(8, 8), links=None, version=None, new=None, extra=None):
    g = list(body) + [stub(c) for c in cps.values()]
    slots = {('cp', i): des_slot(c) for i, c in cps.items()}
    dirs = {c[1] for c in cps.values()}
    if tag is None:
        if 'up' in dirs:
            tag = ((1, 5), 'middle_right')
        else:
            tag = ((2, 1), 'bottom_left')
    if ftext is None:
        if 'down' in dirs:
            ftext = ((14, 15), 'top_left')
        else:
            ftext = ((2, 15), 'top_left')
    slots['tag'] = tag
    slots['function_text'] = ftext
    if extra:
        slots.update(extra)
    apply(key, g, cps, slots, anchor=anchor, links=links, version=version, new=new)


def diamond():
    return pl([(2, 8), (8, 2), (14, 8), (8, 14), (2, 8)])


def one_down_slots():
    return dict(tag=((1, 6), 'middle_right'), ftext=((1, 10), 'middle_right'))


def components():
    comp('ew.iso.air-supply', [circ(8, 8, 4.5), pg([(8, 5), (5.5, 10), (10.5, 10)], 'p026', fill=False)],
         {0: ((8, 16), 'down')}, tag=((3, 6), 'middle_right'), ftext=((3, 10), 'middle_right'))
    # the circle's lead is 3.5 mm: from its bottom (12.5) to the port
    d = load('ew.iso.air-supply')
    d['graphics'][-1] = line(8, 12.5, 8, 16)
    save('ew.iso.air-supply', d)
    comp('ew.iso.bulkhead', [line(2, 8, 14, 8), line(8, 3.5, 8, 12.5)], {0: L, 1: R})
    comp('ew.iso.check-valve', [line(2, 8, 14, 8, 'p013'), circ(7, 8, 1.5), pl([(10.5, 5), (8.5, 8), (10.5, 11)])],
         {0: L, 1: R})
    comp('ew.iso.throttle', [line(2, 8, 14, 8, 'p013'), pl([(6, 5), (8, 7), (10, 5)], 'p013'),
                             pl([(6, 11), (8, 9), (10, 11)], 'p013')], {0: L, 1: R})
    comp('ew.iso.flow-control-one-way', [rect(2, 2, 14, 14), line(2, 8, 14, 8, 'p013'),
                                         pl([(6.5, 5), (8, 6.5), (9.5, 5)], 'p013'), circ(8, 11, 1, 'p013'),
                                         line(4, 13, 12, 3, 'p013')] + [pg([(11, 3), (11.5, 4), (12, 3)])],
         {0: L, 1: R})
    comp('ew.iso.quick-coupling', [line(2, 8, 6.5, 8), line(9.5, 8, 14, 8), line(6.5, 5.5, 6.5, 10.5),
                                   line(9.5, 5.5, 9.5, 10.5)], {0: L, 1: R})
    comp('ew.iso.filter', [diamond(), line(8, 2, 8, 14, 'p013', 'dash42')], {0: L, 1: R})
    comp('ew.iso.lubricator', [diamond(), line(5.5, 6.5, 10.5, 6.5, 'p013'), line(8, 6.5, 8, 11, 'p013')],
         {0: L, 1: R})
    comp('ew.iso.frl', [rect(2, 2, 14, 14), pl([(3, 8), (5.5, 5.5), (8, 8), (5.5, 10.5), (3, 8)], 'p013'),
                        pl([(8, 8), (10.5, 5.5), (13, 8), (10.5, 10.5), (8, 8)], 'p013')], {0: L, 1: R, 2: D})
    comp('ew.iso.regulator', [rect(2, 2, 14, 14), line(5, 11, 11, 5, 'p013'), pg([(10, 5), (11, 6), (11, 5)]),
                              line(8, 2, 8, 0.5, 'p013'), pl([(8, 0.5), (6.5, -0.5), (9.5, -1.5), (6.5, -2.5),
                                                              (9.5, -3.5), (8, -4)], 'p013')],
         {0: L, 1: R, 2: D}, tag=((2, -5), 'bottom_left'))
    comp('ew.iso.shuttle-valve', [rect(2, 2, 14, 14), circ(8, 8, 1.5), pl([(4, 6), (5.5, 8), (4, 10)], 'p013'),
                                  pl([(12, 6), (10.5, 8), (12, 10)], 'p013'), line(8, 9.5, 8, 14, 'p013')],
         {0: L, 1: R, 2: D})
    comp('ew.iso.two-pressure-valve', [rect(2, 2, 14, 14), rect(6.5, 6.5, 9.5, 9.5, 'p013'),
                                       line(8, 9.5, 8, 14, 'p013')], {0: L, 1: R, 2: D})
    comp('ew.iso.quick-exhaust', [rect(2, 2, 14, 14), circ(8, 8, 1.5), pl([(4, 6), (5.5, 8), (4, 10)], 'p013'),
                                  line(8, 9.5, 8, 11.5, 'p013'), pg([(6.5, 11.5), (9.5, 11.5), (8, 14)], 'p013', fill=False)],
         {0: L, 1: R, 2: D})
    comp('ew.iso.ejector', [rect(2, 2, 14, 14), pl([(5, 11), (8, 5), (11, 11)], 'p013'),
                            pl([(8, 5), (8, 3.5), (12, 3.5), (12, 2)], 'p013')],
         {0: ((4, 16), 'down'), 1: ((12, 0), 'up'), 2: ((12, 16), 'down')}, tag=((1, 1), 'bottom_right'),
         ftext=((1, 17), 'top_right'))
    comp('ew.iso.pressure-switch', [rect(2, 2, 14, 14), pl([(5, 11), (8, 5), (11, 11)], 'p013')], {0: D},
         **one_down_slots())
    comp('ew.iso.vacuum-switch', [rect(2, 2, 14, 14), line(5, 11, 11, 5, 'p013')], {0: D}, **one_down_slots())
    comp('ew.iso.pressure-gauge', [circ(8, 8, 5), line(8, 8, 11, 5, 'p013'), pg([(10, 5), (11, 6), (11, 5)])],
         {0: ((8, 16), 'down')}, tag=((2, 6), 'middle_right'), ftext=((2, 10), 'middle_right'))
    d = load('ew.iso.pressure-gauge')
    d['graphics'][-1] = line(8, 13, 8, 16)
    save('ew.iso.pressure-gauge', d)
    comp('ew.iso.silencer', [rect(5, 5, 11, 14), line(5, 8, 11, 8, 'p013'), line(5, 11, 11, 11, 'p013')], {0: D},
         tag=((4, 8), 'middle_right'), ftext=((4, 12), 'middle_right'))
    comp('ew.iso.plug-fluid', [line(5, 14, 11, 14), line(6, 12.5, 10, 12.5, 'p013')], {0: D},
         tag=((4, 12), 'middle_right'), ftext=((12, 12), 'middle_left'))
    comp('ew.iso.suction-cup', [pl([(2, 7), (5, 13), (11, 13), (14, 7)]), line(8, 13, 8, 14)], {0: D},
         tag=((1, 9), 'middle_right'), ftext=((15, 9), 'middle_left'))
    comp('ew.iso.semi-rotary-drive', [circ(8, 8, 4.5), pl([(5, 6), (8, 4.5), (11, 6)], 'p013'),
                                      line(4, 10, 4, 14), line(12, 10, 12, 14)],
         {0: ((4, 16), 'down'), 1: ((12, 16), 'down')}, tag=((3, 3), 'bottom_right'),
         ftext=((14, 18), 'top_left'))
    comp('ew.iso.gripper-parallel', [rect(2, 5, 14, 11), line(3.5, 5, 3.5, 2), line(12.5, 5, 12.5, 2),
                                     line(4, 11, 4, 14), line(12, 11, 12, 14)],
         {0: ((4, 16), 'down'), 1: ((12, 16), 'down')}, tag=((1, 5), 'middle_right'),
         ftext=((14, 18), 'top_left'))
    # Cylinders. Double-acting: a barrel 6 × 16 mm standing (x 2 to 8, y 2 to 18), ports on the left at y 4 and 16,
    # the piston at y 10 and the rod down to 24, its end the link point.
    comp('ew.iso.cylinder-double-acting', [rect(2, 2, 8, 18), line(2, 10, 8, 10), line(5, 10, 5, 24)],
         {0: ((0, 4), 'left'), 1: ((0, 16), 'left')}, tag=((10, 7), 'middle_left'),
         ftext=((10, 13), 'middle_left'), anchor=(4, 12), links={'rod': (5, 24)},
         extra={('cp', 0): ((-1, 3), 'bottom_right'), ('cp', 1): ((-1, 15), 'bottom_right')})
    # Single-acting, spring return: lying (x 2 to 20, y 5 to 11), the port under the cap end.
    comp('ew.iso.cylinder-single-acting', [rect(2, 5, 20, 11), line(8, 5, 8, 11), line(8, 8, 23, 8),
                                           zigzag_h(9, 8, 6), line(4, 11, 4, 14)],
         {0: ((4, 16), 'down')}, tag=((2, 4), 'bottom_left'), ftext=((6, 13), 'top_left'))



def fndef_from(base, key, category, cps=None, model='keep', version='0.1.0'):
    d = fndef(base)
    d['version'] = version
    d['meta']['key'] = key
    d['category'] = category
    if cps is not None:
        d['cps'] = cps
    if model is None:
        d.pop('valve_model', None)
    elif model != 'keep':
        d['valve_model'] = model
    with open(os.path.join(FND, key + '.json'), 'w', encoding='utf-8', newline='\n') as f:
        f.write(json.dumps(d, indent=2, ensure_ascii=False) + '\n')
    return d


def symbol_from(base, key, fn_key, en, nl, keywords):
    d = copy.deepcopy(load(base))
    d['version'] = '0.1.0'
    d['meta']['key'] = key
    d['name'] = {'en': en, 'nl': nl}
    d['keywords'] = sorted(keywords)
    d['fndef'] = {'library': d['fndef']['library'], 'item': d['fndef']['item'], 'version_req': '^0.1',
                  'key': fn_key}
    return d


def actuation_model(base_fn, actuation):
    m = copy.deepcopy(fndef(base_fn)['valve_model'])
    m['sections'][0]['actuation'] = actuation
    return m


def port(i, d, role, des):
    return {'idx': i, 'designation': des, 'discipline': 'pneumatic', 'port_role': role}


def cp(i, x, y, d):
    return {'idx': i, 'fn_cp': i, 'pos': pt(x, y), 'dir': d, 'discipline': 'pneumatic'}


def additions():
    """The symbols and function definitions added for the basic pneumatic parts."""
    # --- air preparation ------------------------------------------------------------------------------------------
    fndef_from('ew.fn.filter', 'ew.fn.filter-manual-drain', ['AirPreparation', 'Filter', 'ManualDrain'])
    d = symbol_from('ew.iso.filter', 'ew.iso.filter-manual-drain', 'ew.fn.filter-manual-drain',
                    'Filter with manual drain', 'Filter met handmatige aftap', ['drain', 'filter', 'manual', 'pneumatic'])
    comp('ew.iso.filter-manual-drain', [diamond(), line(8, 2, 8, 11, 'p013', 'dash42'), line(8, 14, 8, 16),
                                        line(6.5, 16, 9.5, 16)], {0: L, 1: R}, new=d,
         ftext=((10, 15), 'top_left'))
    fndef_from('ew.fn.filter', 'ew.fn.filter-auto-drain', ['AirPreparation', 'Filter', 'AutoDrain'])
    d = symbol_from('ew.iso.filter', 'ew.iso.filter-auto-drain', 'ew.fn.filter-auto-drain',
                    'Filter with automatic drain', 'Filter met automatische aftap',
                    ['automatic', 'drain', 'filter', 'pneumatic'])
    comp('ew.iso.filter-auto-drain', [diamond(), line(8, 2, 8, 11, 'p013', 'dash42'), line(8, 14, 8, 15.5),
                                      pg([(6.5, 15.5), (9.5, 15.5), (8, 17.5)], 'p013')], {0: L, 1: R}, new=d,
         ftext=((10, 15), 'top_left'))
    fndef_from('ew.fn.regulator', 'ew.fn.regulator-gauge', ['AirPreparation', 'Regulator', 'Gauge'])
    d = symbol_from('ew.iso.regulator', 'ew.iso.regulator-gauge', 'ew.fn.regulator-gauge',
                    'Pressure regulator with gauge', 'Drukregelaar met manometer',
                    ['gauge', 'pneumatic', 'pressure', 'regulator'])
    comp('ew.iso.regulator-gauge', [rect(2, 2, 14, 14), line(5, 11, 11, 5, 'p013'), pg([(10, 5), (11, 6), (11, 5)]),
                                    line(6, 2, 6, 0.5, 'p013'),
                                    pl([(6, 0.5), (4.5, -0.5), (7.5, -1.5), (4.5, -2.5), (7.5, -3.5), (6, -4)], 'p013'),
                                    line(12, 2, 12, 0.5), circ(12, -1.5, 2), line(12, -1.5, 13, -2.5, 'p013')],
         {0: L, 1: R, 2: D}, new=d, tag=((2, -5), 'bottom_left'))
    fndef_from('ew.fn.frl', 'ew.fn.frl-detailed', ['AirPreparation', 'ServiceUnit', 'FrlDetailed'])
    d = symbol_from('ew.iso.frl', 'ew.iso.frl-detailed', 'ew.fn.frl-detailed',
                    'Service unit, detailed (filter, regulator, lubricator)',
                    'Verzorgingseenheid, uitgebreid (filter, drukregelaar, oliënevelaar)',
                    ['filter', 'lubricator', 'pneumatic', 'regulator', 'service unit'])
    d['cps'][1]['pos'] = pt(32, 8)
    comp('ew.iso.frl-detailed', [rect(2, 1, 30, 15, 'p013'),
                                 pl([(3, 8), (6, 5), (9, 8), (6, 11), (3, 8)]), line(6, 5, 6, 11, 'p013', 'dash42'),
                                 line(6, 11, 6, 14), line(9, 8, 11, 8),
                                 rect(11, 3, 21, 13), line(13, 11, 19, 5, 'p013'), pg([(18, 5), (19, 6), (19, 5)]),
                                 line(21, 8, 23, 8),
                                 pl([(23, 8), (26, 5), (29, 8), (26, 11), (23, 8)]), line(24.5, 6.5, 27.5, 6.5, 'p013'),
                                 line(26, 6.5, 26, 9.5, 'p013'),
                                 line(2, 8, 3, 8), line(29, 8, 30, 8)],
         {0: L, 1: ((32, 8), 'right'), 2: ((8, 16), 'down')}, new=d, ftext=((18, 17), 'top_left'))
    d = load('ew.iso.frl-detailed')
    # the drain's lead from the filter's bottom (x 6) to the port at x 8 crosses below the box
    d['graphics'][-1] = line(8, 15, 8, 16)
    d['graphics'].insert(-1, line(6, 14, 6, 15))
    d['graphics'].insert(-1, line(6, 15, 8, 15))
    save('ew.iso.frl-detailed', d)
    fndef_from('ew.fn.regulator', 'ew.fn.soft-start', ['AirPreparation', 'SoftStart', 'Standard'])
    d = symbol_from('ew.iso.quick-exhaust', 'ew.iso.soft-start', 'ew.fn.soft-start',
                    'Soft-start valve', 'Langzaamstartventiel', ['pneumatic', 'soft start', 'start', 'valve'])
    comp('ew.iso.soft-start', [rect(2, 2, 14, 14), line(2, 8, 14, 8, 'p013'), pl([(6.5, 5.5), (8, 7), (9.5, 5.5)], 'p013'),
                               pl([(6.5, 10.5), (8, 9), (9.5, 10.5)], 'p013'), line(4, 13, 12, 3, 'p013'),
                               pg([(11, 3), (11.5, 4), (12, 3)])], {0: L, 1: R, 2: D}, new=d)
    fndef_from('ew.fn.silencer', 'ew.fn.condensate-drain', ['AirPreparation', 'Drain', 'Automatic'],
               cps=[port(0, 'up', 'generic', '1')])
    d = symbol_from('ew.iso.silencer', 'ew.iso.condensate-drain', 'ew.fn.condensate-drain',
                    'Condensate drain, automatic', 'Condensaataftap, automatisch',
                    ['condensate', 'drain', 'pneumatic', 'water'])
    comp('ew.iso.condensate-drain', [diamond(), pg([(6.5, 10), (9.5, 10), (8, 12.5)], 'p013')], {0: U}, new=d,
         ftext=((15, 10), 'middle_left'))
    fndef_from('ew.fn.silencer', 'ew.fn.air-reservoir', ['AirPreparation', 'Reservoir', 'Standard'],
               cps=[port(0, 'down', 'generic', '1')])
    d = symbol_from('ew.iso.silencer', 'ew.iso.air-reservoir', 'ew.fn.air-reservoir',
                    'Air reservoir', 'Luchtketel', ['air', 'pneumatic', 'receiver', 'reservoir', 'tank'])
    r = {'rect': {'rect': [pt(1, 4), pt(15, 12)], 'rx': q(2), 'ry': q(2), 'stroke': S('p026')}}
    comp('ew.iso.air-reservoir', [r, line(8, 12, 8, 14)], {0: D}, new=d, tag=((0, 8), 'middle_right'),
         ftext=((16, 8), 'middle_left'))
    fndef_from('ew.fn.silencer', 'ew.fn.exhaust', ['FluidFitting', 'Exhaust', 'Open'],
               cps=[port(0, 'up', 'generic', '1')])
    d = symbol_from('ew.iso.silencer', 'ew.iso.exhaust', 'ew.fn.exhaust',
                    'Exhaust port, open', 'Ontluchting, open', ['exhaust', 'pneumatic', 'port'])
    comp('ew.iso.exhaust', [pg([(5.5, 2), (10.5, 2), (8, 5.5)], 'p026', fill=False)], {0: U}, new=d,
         tag=((4, 4), 'middle_right'), ftext=((12, 4), 'middle_left'))
    # --- hand- and roller-operated valves ----------------------------------------------------------------------------
    spring1 = {'actuator': 'spring', 'selects': 1}
    for key, base_fn, base_sym, kind, en, nl, kw in [
        ('valve-3-2-pushbutton', 'ew.fn.valve-3-2-sol-spring', 'ew.iso.valve-3-2-sol-spring', 'pushbutton',
         '3/2 valve, push button and spring', '3/2-ventiel, drukknop en veer', ['3/2', 'manual', 'push button']),
        ('valve-3-2-lever', 'ew.fn.valve-3-2-sol-spring', 'ew.iso.valve-3-2-sol-spring', 'lever',
         '3/2 valve, lever and spring', '3/2-ventiel, hendel en veer', ['3/2', 'lever', 'manual']),
        ('valve-3-2-roller', 'ew.fn.valve-3-2-sol-spring', 'ew.iso.valve-3-2-sol-spring', 'roller',
         '3/2 valve, roller and spring', '3/2-ventiel, rol en veer', ['3/2', 'limit', 'roller']),
        ('valve-5-2-pushbutton', 'ew.fn.valve-5-2-sol-sol', 'ew.iso.valve-5-2-sol-sol', 'pushbutton',
         '5/2 valve, push button and spring', '5/2-ventiel, drukknop en veer', ['5/2', 'manual', 'push button']),
        ('valve-5-2-lever', 'ew.fn.valve-5-2-sol-sol', 'ew.iso.valve-5-2-sol-sol', 'lever',
         '5/2 valve, lever with detent', '5/2-ventiel, hendel met vergrendeling', ['5/2', 'lever', 'manual']),
    ]:
        if kind == 'lever' and '5-2' in key:
            act = [{'actuator': 'lever', 'selects': 0}, {'actuator': 'detent', 'selects': 1}]
            cat_last = 'LeverDetent'
        else:
            act = [{'actuator': kind, 'selects': 0}, spring1]
            cat_last = {'pushbutton': 'PushbuttonSpring', 'lever': 'LeverSpring', 'roller': 'RollerSpring'}[kind]
        fd = fndef(base_fn)
        group = fd['category'][1]
        model = actuation_model(base_fn, act)
        fndef_from(base_fn, 'ew.fn.' + key, ['ValveDirectional', group, cat_last], model=model)
        d = symbol_from(base_sym, 'ew.iso.' + key, 'ew.fn.' + key, en, nl, ['pneumatic', 'valve'] + kw)
        valve('ew.iso.' + key, 'ew.fn.' + key, new=d)
    # --- cylinders (the double-acting one keeps its drawing: Golden A holds it) ----------------------------------
    for key, cat, en, nl, kw, extra in [
        ('cylinder-double-acting-magnetic', ['CylinderFluid', 'DoubleActing', 'Magnetic'],
         'Cylinder, double-acting, magnetic piston', 'Cilinder, dubbelwerkend, magnetische zuiger',
         ['cylinder', 'double-acting', 'magnetic', 'pneumatic', 'sensor'],
         [pg([(2.5, 9), (7.5, 9), (7.5, 11), (2.5, 11)], 'p013')]),
        ('cylinder-double-acting-cushioned', ['CylinderFluid', 'DoubleActing', 'Cushioned'],
         'Cylinder, double-acting, cushioned', 'Cilinder, dubbelwerkend, met eindstanddemping',
         ['cushioned', 'cylinder', 'double-acting', 'pneumatic'],
         [rect(3.5, 2, 6.5, 4, 'p013'), rect(3.5, 16, 6.5, 18, 'p013')]),
    ]:
        fndef_from('ew.fn.cylinder-double-acting', 'ew.fn.' + key, cat)
        d = symbol_from('ew.iso.cylinder-double-acting', 'ew.iso.' + key, 'ew.fn.' + key, en, nl, kw)
        comp('ew.iso.' + key, [rect(2, 2, 8, 18), line(2, 10, 8, 10), line(5, 10, 5, 24)] + extra,
             {0: ((0, 4), 'left'), 1: ((0, 16), 'left')}, tag=((10, 7), 'middle_left'),
             ftext=((10, 13), 'middle_left'), anchor=(4, 12), links={'rod': (5, 24)}, new=d,
             extra={('cp', 0): ((-1, 3), 'bottom_right'), ('cp', 1): ((-1, 15), 'bottom_right')})
    fndef_from('ew.fn.cylinder-double-acting', 'ew.fn.cylinder-rodless', ['CylinderFluid', 'Rodless', 'DoubleActing'])
    d = symbol_from('ew.iso.cylinder-double-acting', 'ew.iso.cylinder-rodless', 'ew.fn.cylinder-rodless',
                    'Cylinder, rodless, double-acting', 'Cilinder, stangloos, dubbelwerkend',
                    ['cylinder', 'double-acting', 'pneumatic', 'rodless', 'slide'])
    comp('ew.iso.cylinder-rodless', [rect(2, 5, 30, 11), line(16, 5, 16, 11), rect(13, 1, 19, 5)],
         {0: L, 1: ((32, 8), 'right')}, tag=((2, 4), 'bottom_left'), ftext=((2, 12), 'top_left'),
         anchor=(16, 8), links={'rod': (16, 1)}, new=d)
    # --- the distributor (manifold block): inlet 1 on the left, four outlets 2 on top --------------------------------
    outlets = [8, 12, 16, 20]
    fndef_from('ew.fn.filter', 'ew.fn.distributor', ['FluidFitting', 'Distributor', 'Manifold'],
               cps=[port(0, 'left', 'supply1', '1')] + [port(i + 1, 'up', 'working2', '2') for i in range(4)],
               model={'sections': [{'positions': [{'label': '0', 'paths': [
                   {'flow': {'from': 0, 'to': i + 1}} for i in range(4)]}], 'rest': 0}]})
    d = symbol_from('ew.iso.ejector', 'ew.iso.distributor', 'ew.fn.distributor',
                    'Distributor, manifold block with four outlets', 'Verdeler, verdeelblok met vier uitgangen',
                    ['distributor', 'manifold', 'pneumatic'])
    d['cps'] = [cp(0, 0, 8, 'left')] + [cp(i + 1, x, 0, 'up') for i, x in enumerate(outlets)]
    slots = [{'kind': 'tag', 'pos': [0, 0], 'align': 'bottom_left', 'visible': True},
             {'kind': 'function_text', 'pos': [0, 0], 'align': 'top_left', 'visible': True}]
    slots += [{'kind': {'cp_designation': i}, 'pos': [0, 0], 'align': 'middle_left', 'visible': True}
              for i in range(5)]
    d['text_slots'] = slots
    d['arrangements'] = [{'name': 'full', 'slots': list(range(7))}, {'name': 'tag', 'slots': [0]},
                         {'name': 'ports', 'slots': list(range(1, 7))}]
    body = [rect(2, 2, 26, 14), line(2, 8, 20, 8, 'p013')] + [line(x, 8, x, 2, 'p013') for x in outlets]
    comp('ew.iso.distributor', body, {0: L, **{i + 1: ((x, 0), 'up') for i, x in enumerate(outlets)}}, new=d,
         tag=((27, 4), 'middle_left'), ftext=((2, 15), 'top_left'), anchor=(12, 8))


def valve_solenoid():
    """ew.iec.valve-solenoid: the internal coil of a valve (ew.fn.valve-solenoid), no connection point."""
    d = load('ew.iec.valve-coil')
    d['version'] = '0.1.0'
    d['meta']['key'] = 'ew.iec.valve-solenoid'
    d['name'] = {'en': 'Solenoid of a valve, internal coil', 'nl': 'Magneetspoel van een ventiel, interne spoel'}
    d['keywords'] = ['coil', 'internal', 'solenoid', 'valve']
    d['fndef'] = {'library': d['fndef']['library'], 'item': d['fndef']['item'], 'version_req': '^0.1',
                  'key': 'ew.fn.valve-solenoid'}
    # The coil's box with the winding's diagonal; no connection point: the coil is internal, a PLC output is bound to
    # it, not wired (doc 11 §3.1).
    d['graphics'] = [rect(-2, 4, 2, 12), line(-2, 12, 2, 4)]
    d.pop('cps', None)
    d['text_slots'] = [s for s in d['text_slots'] if not isinstance(s['kind'], dict)]
    n = len(d['text_slots'])
    d['arrangements'] = [{'name': 'full', 'slots': list(range(n))},
                         {'name': 'tag', 'slots': [i for i, s in enumerate(d['text_slots']) if s['kind'] == 'tag']}]
    save('ew.iec.valve-solenoid', d)


if __name__ == '__main__':
    for k, f in VALVES:
        valve(k, f)
    components()
    additions()
    valve_solenoid()
    print('drawn; now run `cargo xtask symbols-qet10`')
