#!/usr/bin/env python3
"""Minimal R3000 disassembler for a DW1STATE dump's RAM (the instructions this project has needed:
loads/stores, ALU, branches, COP0/COP2). Usage: r3000_dis.py dump.bin 0x8009ADC0 64"""
import sys,struct
R=['zero','at','v0','v1','a0','a1','a2','a3','t0','t1','t2','t3','t4','t5','t6','t7','s0','s1','s2','s3','s4','s5','s6','s7','t8','t9','k0','k1','gp','sp','fp','ra']
GD=['VXY0','VZ0','VXY1','VZ1','VXY2','VZ2','RGBC','OTZ','IR0','IR1','IR2','IR3','SXY0','SXY1','SXY2','SXYP','SZ0','SZ1','SZ2','SZ3','RGB0','RGB1','RGB2','RES1','MAC0','MAC1','MAC2','MAC3','IRGB','ORGB','LZCS','LZCR']
GC=['RT11RT12','RT13RT21','RT22RT23','RT31RT32','RT33','TRX','TRY','TRZ']+['C%d'%i for i in range(8,32)]
f=open(sys.argv[1],'rb').read(); base=8+131*4+1024
a0=int(sys.argv[2],16); n=int(sys.argv[3])
for k in range(n):
    a=a0+4*k; w=struct.unpack_from('<I',f,base+(a&0x1fffff))[0]
    op=w>>26; rs=(w>>21)&31; rt=(w>>16)&31; rd=(w>>11)&31; sh=(w>>6)&31; fn=w&63; imm=w&0xffff; simm=imm-0x10000 if imm&0x8000 else imm
    s='?'
    if w==0: s='nop'
    elif op==0:
        s={0:f'sll {R[rd]},{R[rt]},{sh}',2:f'srl {R[rd]},{R[rt]},{sh}',3:f'sra {R[rd]},{R[rt]},{sh}',8:f'jr {R[rs]}',9:f'jalr {R[rs]}',0x21:f'addu {R[rd]},{R[rs]},{R[rt]}',0x23:f'subu {R[rd]},{R[rs]},{R[rt]}',0x24:f'and {R[rd]},{R[rs]},{R[rt]}',0x25:f'or {R[rd]},{R[rs]},{R[rt]}'}.get(fn,f'special fn={fn:#x}')
    elif op==0x10 and rs in (0,4):
        s=f"{'mfc0' if rs==0 else 'mtc0'} {R[rt]},${rd}"
    elif op==0x12:
        if rs==0: s=f'mfc2 {R[rt]},{GD[rd]}'
        elif rs==2: s=f'cfc2 {R[rt]},{GC[rd]}'
        elif rs==4: s=f'mtc2 {R[rt]},{GD[rd]}'
        elif rs==6: s=f'ctc2 {R[rt]},{GC[rd]}'
        elif w&0x02000000: s=f'cop2 {w&0x1ffffff:#x}'+(' MVMVA' if fn==0x12 else '')
    elif op in (0x20,0x21,0x23,0x24,0x25,0x28,0x29,0x2b): s=f"{ {0x20:'lb',0x21:'lh',0x23:'lw',0x24:'lbu',0x25:'lhu',0x28:'sb',0x29:'sh',0x2b:'sw'}[op]} {R[rt]},{simm}({R[rs]})"
    elif op==0x32: s=f'lwc2 {GD[rt]},{simm}({R[rs]})'
    elif op==0x3a: s=f'swc2 {GD[rt]},{simm}({R[rs]})'
    elif op==9: s=f'addiu {R[rt]},{R[rs]},{simm}'
    elif op==0xc: s=f'andi {R[rt]},{R[rs]},{imm:#x}'
    elif op==0xd: s=f'ori {R[rt]},{R[rs]},{imm:#x}'
    elif op==0xf: s=f'lui {R[rt]},{imm:#x}'
    elif op in (4,5): s=f"{'beq' if op==4 else 'bne'} {R[rs]},{R[rt]},{a+4+simm*4:#x}"
    print(f'{a:08x}: {w:08x}  {s}')
