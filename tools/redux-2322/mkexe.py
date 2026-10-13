# Writes gte-vz0-delay.exe, a PS-X EXE: for N = 0..3, write VZ0 = 0x100, 4 nops, write
# VZ0 = 0x200, N nops, MVMVA (sf=1, RT * V0, no translation) with RT = identity, mfc2 IR3.
# IR3 is the VZ0 the MVMVA used. Results are stored at 0x80020000 + 4 * N, then 0x600d at +16.
import struct
import sys

T0, T1, T2, S0, ZERO = 8, 9, 10, 16, 0
VXY0, VZ0, IR3 = 0, 1, 11
NOP = 0
MVMVA = 0x4A486012  # cop2 0x486012


def itype(op, rs, rt, imm):
    return op << 26 | rs << 21 | rt << 16 | (imm & 0xFFFF)


def mfc0(rt, rd): return 0x40000000 | rt << 16 | rd << 11
def mtc0(rt, rd): return 0x40800000 | rt << 16 | rd << 11
def mfc2(rt, rd): return 0x48000000 | rt << 16 | rd << 11
def mtc2(rt, rd): return 0x48800000 | rt << 16 | rd << 11
def ctc2(rt, rd): return 0x48C00000 | rt << 16 | rd << 11
def ori(rt, rs, imm): return itype(0x0D, rs, rt, imm)
def lui(rt, imm): return itype(0x0F, 0, rt, imm)
def addiu(rt, rs, imm): return itype(0x09, rs, rt, imm)
def sw(rt, off, base): return itype(0x2B, base, rt, off)
def and_(rd, rs, rt): return rs << 21 | rt << 16 | rd << 11 | 0x24
def or_(rd, rs, rt): return rs << 21 | rt << 16 | rd << 11 | 0x25


BASE = 0x80010000
code = [
    mfc0(T1, 12), NOP,                       # SR: interrupts off, COP2 on
    addiu(T2, ZERO, -2), and_(T1, T1, T2),
    lui(T2, 0x4000), or_(T1, T1, T2),
    mtc0(T1, 12), NOP, NOP,
    lui(S0, 0x8002),
    ori(T0, ZERO, 0x1000),                   # RT = identity (1.0 = 0x1000)
    ctc2(T0, 0), ctc2(ZERO, 1), ctc2(T0, 2), ctc2(ZERO, 3), ctc2(T0, 4),
    mtc2(ZERO, VXY0), NOP, NOP, NOP, NOP,
]
for n in range(4):
    code += [ori(T1, ZERO, 0x100), mtc2(T1, VZ0), NOP, NOP, NOP, NOP]  # old VZ0, settled
    code += [ori(T1, ZERO, 0x200), mtc2(T1, VZ0)] + [NOP] * n          # new VZ0, N nops
    code += [MVMVA, mfc2(T2, IR3), NOP, sw(T2, 4 * n, S0)]
code += [ori(T1, ZERO, 0x600D), sw(T1, 16, S0)]
loop = BASE + 4 * len(code)
code += [2 << 26 | (loop >> 2) & 0x3FFFFFF, NOP]  # j loop

text = b''.join(struct.pack('<I', w) for w in code)
text += b'\0' * (-len(text) % 2048)
header = bytearray(2048)
header[0:8] = b'PS-X EXE'
struct.pack_into('<IIII', header, 0x10, BASE, 0, BASE, len(text))  # pc0, gp0, t_addr, t_size
struct.pack_into('<I', header, 0x30, 0x801FFF00)                    # stack
header[0x4C:0x4C + 55] = b'Sony Computer Entertainment Inc. for North America area'
with open(sys.argv[1] if len(sys.argv) > 1 else 'gte-vz0-delay.exe', 'wb') as f:
    f.write(bytes(header) + text)
