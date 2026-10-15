#!/usr/bin/env python3
"""Self-tests of the movie checks: compare_mdec.py and compare_xa.py must pass a good synthetic
log and fail each kind of bad one (a picture leaning in one channel, pixel data cut short or
missing, a slice of the wrong size; a gap in the port's XA audio inside a read). The logs are
made up here (no game data). tools/sound-check.sh runs this first.

    python3 tools/oracle/test_sound_checks.py
"""
import os
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))


def fnv(data):
    h = 0xcbf29ce484222325
    for b in data:
        h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return h


def put(path, data):
    with open(path, 'wb' if isinstance(data, bytes) else 'w') as f:
        f.write(data)


def run(script, *args):
    r = subprocess.run([sys.executable, os.path.join(HERE, script), *args], capture_output=True,
                       text=True)
    return r.returncode, r.stdout + r.stderr


class Mdec(unittest.TestCase):
    """One frame: 8 bytes of codes, two slices of 6x4 halfwords (16 RGB pixels each)."""

    W, H = 6, 4

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.codes = bytes(range(8))
        n = self.W * self.H * 2
        self.slices = [bytes((40 + 7 * i + 3 * s) % 256 for i in range(n)) for s in range(2)]

    def tearDown(self):
        self.dir.cleanup()

    def write(self, native_slices, native_offsets=None, native_bin_cut=0, oracle_n=None):
        d = self.dir.name
        ora = os.path.join(d, 'ora')
        os.makedirs(ora, exist_ok=True)
        obin = self.codes + b''.join(self.slices)
        lines = [f'500 frame 25 rle 0 {len(self.codes)} 3\n']
        off = len(self.codes)
        for s in self.slices:
            lines.append(f'500 slice 25 0 0 {self.W} {self.H} {off} {oracle_n or len(s)}\n')
            off += len(s)
        put(os.path.join(ora, 'mdec.bin'), obin)
        put(os.path.join(ora, 'mdec-log.txt'), ''.join(lines))
        nbin = bytearray(self.codes)
        nlines = [f'frame 1 rle 0 {len(self.codes)} 3\n']
        for i, s in enumerate(native_slices):
            at = len(nbin) if native_offsets is None else native_offsets[i]
            nlines.append(f'slice 1 {at} {len(s)} {fnv(s):016x}\n')
            nbin += s
        if native_bin_cut:
            nbin = nbin[:-native_bin_cut]
        put(os.path.join(d, 'nat.bin'), bytes(nbin))
        put(os.path.join(d, 'nat.txt'), ''.join(nlines))
        return run('compare_mdec.py', ora, os.path.join(d, 'nat'))

    def shifted(self, pixels):
        """The slices with R raised by one in the first `pixels` pixels of each."""
        out = []
        for s in self.slices:
            b = bytearray(s)
            for p in range(pixels):
                b[3 * p] = min(255, b[3 * p] + 1)
            out.append(bytes(b))
        return out

    def test_identical_passes(self):
        code, out = self.write(self.slices)
        self.assertEqual(code, 0, out)

    def test_small_lean_passes(self):
        # 1 pixel of 16 raised: R leans 0.0625.
        code, out = self.write(self.shifted(1))
        self.assertEqual(code, 0, out)

    def test_lean_in_one_channel_fails(self):
        # 4 pixels of 16: R leans 0.25 (over all bytes it would look like 0.083, under 0.1).
        code, out = self.write(self.shifted(4))
        self.assertNotEqual(code, 0, out)
        self.assertIn('+0.250', out)

    def test_cut_pixel_data_fails(self):
        code, out = self.write(self.slices, native_bin_cut=10)
        self.assertNotEqual(code, 0, out)

    def test_missing_pixels_fail(self):
        code, out = self.write(self.shifted(1), native_offsets=[-1, -1])
        self.assertNotEqual(code, 0, out)

    def test_short_native_slice_fails(self):
        code, out = self.write([self.slices[0], self.slices[1][:-6]])
        self.assertNotEqual(code, 0, out)

    def test_emulator_slice_size_mismatch_fails(self):
        code, out = self.write(self.slices, oracle_n=len(self.slices[0]) - 6)
        self.assertNotEqual(code, 0, out)


class Xa(unittest.TestCase):
    """Four audio sectors of one read (LBAs 1000-1024), decoded at VBlanks 110-119 on both sides;
    polls every 2 VBlanks from 100; each sector is 150 frames and the port's CD input plays 50 a
    VBlank, so each is heard as it's decoded."""

    LBAS = [1000, 1008, 1016, 1024]
    VBS = [110, 113, 116, 119]

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.pcm = [bytes((i * 31 + k) % 256 for k in range(16)) for i in range(4)]

    def tearDown(self):
        self.dir.cleanup()

    def write(self, underrun_at=None):
        d = self.dir.name
        ora = os.path.join(d, 'ora')
        os.makedirs(ora, exist_ok=True)
        replay = os.path.join(d, 'replay.txt')
        put(replay, ''.join(f'{k} {100 + 2 * k} 00000000 800108d8\n' for k in range(30)))
        lines = ['100 pos 990\n']
        for i, (lba, vb) in enumerate(zip(self.LBAS, self.VBS)):
            lines.append(f'{vb} xa {lba + 2} 128 0 128 0 0 0 224 37800 1 4 {16 * i}\n')
        put(os.path.join(ora, 'xa-log.txt'), ''.join(lines))
        put(os.path.join(ora, 'xa-pcm.bin'), b''.join(self.pcm))
        nl = []
        for vb in range(100, 130):
            polls = (vb - 100) // 2 + 1
            since = (vb - 100) % 2
            if since == 0:
                nl.append(f'poll {vb} {polls}\n')
            if vb in self.VBS:
                i = self.VBS.index(vb)
                nl.append(f'sector {vb} {polls} {since} {self.LBAS[i]} {vb} 4 {16 * i} {150 * i}\n')
            if vb >= 110:
                under = 0
                if underrun_at is not None and vb >= underrun_at:
                    under = 7
                nl.append(f'pop {vb} {polls} {since} {(vb - 109) * 50} {under}\n')
        put(os.path.join(d, 'nat.txt'), ''.join(nl))
        put(os.path.join(d, 'nat.bin'), b''.join(self.pcm))
        return run('compare_xa.py', ora, os.path.join(d, 'nat'), replay)

    def test_continuous_passes(self):
        code, out = self.write()
        self.assertEqual(code, 0, out)

    def test_gap_after_the_read_passes(self):
        code, out = self.write(underrun_at=125)
        self.assertEqual(code, 0, out)

    def test_gap_inside_a_read_fails(self):
        code, out = self.write(underrun_at=115)
        self.assertNotEqual(code, 0, out)
        self.assertIn('ran dry', out)


if __name__ == '__main__':
    unittest.main()
