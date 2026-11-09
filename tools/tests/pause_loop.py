#!/usr/bin/env python3
"""Exercise actual C pause/input bodies with an event-driven native pad.

No disc/saves/assets. The input stub is deliberately cached until VSync runs, just like
Window: this fails boundedly on the old infinite loop. Non-native checks instead model
asynchronous pad updates, and reject newly inserted waits in the retail source branch.
"""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]


def function(path, signature):
    text = (ROOT / path).read_text()
    start = text.index(signature + "\n{")
    brace = text.index("{", start)
    depth = 1
    end = brace + 1
    while depth:
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    return text[start:end] + "\n"


PREAMBLE = r'''
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
uint32_t PAUSE_INPUT, PAUSE_INPUT_PREVIOUS, VS__CURRENT_INPUT, VS__PREVIOUS_INPUT;
uint8_t PAUSE_STATE, PAUSE_BOX_VISIBLE, VS__PAUSING_PLAYER;
int IS_GAMETIME_RUNNING, GAME_STATE, FADE_PROTECTION;
static int pstat, fields, polls, waits, creates, removes, frames, scenario;
static uint32_t script[40];
static void check(int ok, const char *what) {
    if (!ok) { fprintf(stderr, "scenario %d: %s\n", scenario, what); exit(1); }
}
int readPStat(int x) { return pstat; }
void removePauseBox(void) { removes++; }
void createPauseBox(void) { creates++; }
int VSync(int mode) {
#ifndef DW1_NATIVE
    check(frames < 2, "retail branch gained a polling wait");
#endif
    check(mode >= 0, "wait must advance time");
    fields += mode ? mode : 1; waits++;
    check(fields < 40, "pause did not exit in bounded fields");
    return 0;
}
void pauseFrame(void) { VSync(3); frames++; }
uint32_t PadRead(int id) {
    check(id == 1, "changed pad id");
    check(++polls < 1000, "stuck polling cached pad: no host event service");
#ifdef DW1_NATIVE
    return script[fields];
#else
    /* Physical PS1 input changes asynchronously, even without a VSync call. */
    return script[polls < 39 ? polls-1 : 39];
#endif
}
'''
HARNESS = r'''
static void reset(void) {
    PAUSE_INPUT=PAUSE_INPUT_PREVIOUS=VS__CURRENT_INPUT=VS__PREVIOUS_INPUT=0;
    PAUSE_STATE=PAUSE_BOX_VISIBLE=VS__PAUSING_PLAYER=0;
    fields=polls=waits=creates=removes=frames=pstat=FADE_PROTECTION=0;
    IS_GAMETIME_RUNNING=1; GAME_STATE=0;
    for(int i=0;i<40;i++) script[i]=0;
}
static void general_pause(int battle, int second) {
    reset(); GAME_STATE=battle;
    /* Battle still allows pause while game time is stopped / partner unavailable. */
    if(battle) { IS_GAMETIME_RUNNING=0; pstat=3; }
#ifdef DW1_NATIVE
    for(int i=0;i<8;i++) script[i]=0x800;
    script[9]=0x40; /* unrelated button must not resume */
    for(int i=12;i<40;i++) script[i]=0x800;
#else
    script[0]=script[1]=script[2]=0x800;
    script[4]=0x40;
    for(int i=6;i<40;i++) script[i]=0x800;
#endif
    handlePause();
    check(PAUSE_STATE==0 && PAUSE_BOX_VISIBLE==1, "did not resume on second Start edge");
    check(creates==1 && frames==2, "pause box/frame setup changed");
#ifdef DW1_NATIVE
    check(fields==12 && waits==8, "must wait while held, released or unrelated key");
#else
    check(waits==2 && fields==6, "retail waits changed");
#endif
    if(second) {
        int before=waits;
        handlePause(); /* still held: no repeated toggle, old box is removed */
        check(PAUSE_STATE==0 && PAUSE_BOX_VISIBLE==0, "held Start re-paused");
        check(creates==1 && removes==1 && waits==before, "resume cleanup changed");
    }
}
static void versus_pause(int owner) {
    reset();
    uint32_t start=0x800u<<((owner-1)*16), other=0x800u<<((2-owner)*16);
    /* First two updates draw the pause box; only the third update blocks. */
    script[0]=script[1]=start;
    script[3]=other; /* wrong player cannot resume */
    for(int i=6;i<40;i++) script[i]=start;
    VS__handlePause(); VS__handlePause();
    check(PAUSE_STATE==1 && PAUSE_BOX_VISIBLE==2 && waits==0, "VS setup changed");
    VS__handlePause();
    check(PAUSE_STATE==0 && VS__PAUSING_PLAYER==0 && PAUSE_BOX_VISIBLE==0,
          "VS did not resume/clean up");
    check(creates==2 && removes==1, "VS box lifecycle changed");
#ifdef DW1_NATIVE
    check(fields==6 && waits==6, "VS resumed before owner edge or did not wait");
#else
    check(waits==0, "retail VS gained waits");
#endif
}
int main(void) {
    for(int battle=0;battle<=3;battle++) {
        scenario++; general_pause(battle,0);
        scenario++; general_pause(battle,1);
    }
    scenario++; reset(); handlePause();
    check(waits==0 && polls==1 && creates==0, "nonpaused path changed");
    scenario++; reset(); IS_GAMETIME_RUNNING=0; handlePause();
    check(polls==0 && waits==0, "stopped field-time guard changed");
    scenario++; reset(); pstat=3; handlePause();
    check(polls==0 && waits==0, "partner guard changed");
    scenario++; reset(); FADE_PROTECTION=1; handlePause();
    check(polls==0 && waits==0, "fade guard changed");
    scenario++; versus_pause(1);
    scenario++; versus_pause(2);
    printf("PASS: %d pause scenarios\n",scenario);
    return 0;
}
'''


def main():
    cc = shutil.which(os.environ.get("CC", "clang"))
    if not cc:
        raise SystemExit("clang is required")
    bodies = function("decomp/src/main/utils2.c", "void handlePause(void)")
    bodies += function("decomp/src/vs/vs_combat.c", "uint8_t VS__isButtonsPressed(int32_t buttons)")
    bodies += function("decomp/src/vs/vs_combat.c", "void VS__handlePause(void)")
    with tempfile.TemporaryDirectory(prefix="dw1-pause-") as folder:
        for native in [True, False]:
            name = "native" if native else "retail-branch"
            c = Path(folder) / (name + ".c")
            exe = Path(folder) / name
            c.write_text(("#define DW1_NATIVE 1\n" if native else "") + PREAMBLE + bodies + HARNESS)
            subprocess.run([cc, "-std=c99", "-Wall", "-Wextra", "-Wno-unused-parameter",
                            str(c), "-o", str(exe)], check=True)
            print(name, flush=True)
            subprocess.run([str(exe)], check=True, timeout=5)


if __name__ == "__main__":
    main()
