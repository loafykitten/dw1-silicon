#!/usr/bin/env python3
"""Checks bone matrices in a DW1STATE dump (GsSortObject4J entry) against "MVMVA used the previous
column's VZ0" (PCSX-Redux PR #2283, issue #2322). Usage: stalevz.py callNN_in.bin"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from dumpstate import load, matrix, rd
def clamp(v): return max(-0x8000, min(0x7FFF, v))
def mul(W, C, stale_vz0):
    out = [[0]*3 for _ in range(3)]
    vz = stale_vz0
    for j in range(3):
        col = [C[0][j], C[1][j], C[2][j]]
        v = [col[0], col[1], vz if stale_vz0 is not None else col[2]]
        if stale_vz0 is not None: vz = col[2]
        for i in range(3):
            out[i][j] = clamp(sum(W[i][k] * v[k] for k in range(3)) >> 12)
    return out
st = load(sys.argv[1])
coord2 = rd(st, st['gpr'][4], '4I')[1]
chain = []
c = coord2
while c:
    chain.append(c); c = rd(st, c + 0x48, 'I')[0]
chain.reverse()  # root first
for parent, child in zip(chain, chain[1:]):
    W, _ = matrix(st, parent + 0x24); C, _ = matrix(st, child + 4); got, _ = matrix(st, child + 0x24)
    exact = mul(W, C, None)
    # VZ0 left over from the previous multiply is unknown for the first column: solve for it from IR1
    match = [vz for vz in range(-4096, 4097) if mul(W, C, vz) == got]
    print(f'{child:08x}: exact_ok={exact == got} stale_vz0_fits={bool(match)} vz0={match[:3]}')
