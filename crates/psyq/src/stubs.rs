//! Every SDK function the game C calls that has no Rust implementation yet, by library. Each
//! one traps with its name (`trap::stubs!`). Implementing one means deleting it here and
//! writing it in its library's module.
//!
//! The list is what linking main and every overlay needs (Phase 1, 2026-10-13); a call the
//! game gains later shows up as an undefined symbol at link time.

use crate::trap::stubs;

// libapi: the PS1 heap.
stubs! { InitHeap3 malloc3 free3 }

// libc: process exit (returns to the BIOS on the PS1). Renamed `dw1_exit` by dw1_native.h.
stubs! { dw1_exit }

// libetc: callbacks, VSync, pad.
stubs! { ResetCallback VSync PadInit PadRead }

// libgpu
stubs! {
    AddPrim ClearImage DrawSync GetClut GetTPage LoadClut LoadImage MoveImage OpenTIM
    PutDispEnv PutDrawEnv ReadTIM ResetGraph SetDefDispEnv SetDispMask SetDrawLoad
    SetDrawOffset SetGraphDebug SetLineF2 SetLineF3 SetLineF4 SetPolyF4 SetPolyFT3 SetPolyFT4
    SetPolyG4 SetPolyGT4 SetSemiTrans StoreImage
}

// libgte
stubs! {
    ApplyMatrix ApplyMatrixLV ApplyMatrixSV ApplyTransposeMatrixLV CompMatrix MulMatrix
    MulMatrix0 NormalColorCol NormalColorCol3 PopMatrix PushMatrix RotMatrix RotMatrixYXZ
    RotMatrixZYX RotNclip3 RotNclip4 RotTransPers RotTransPers3 RotTransPers4 ScaleMatrix
    SetLightMatrix SetRotMatrix SetTransMatrix SquareRoot0 TransMatrix TransposeMatrix ratan2
    rcos rsin
}

// libgs (data globals are in `libgs`).
stubs! {
    GsClearOt GsDefDispBuff GsDrawOt GsGetActiveBuff GsGetLs GsGetLw GsGetLws GsGetTimInfo
    GsGetWorkBase GsInit3D GsInitCoordinate2 GsInitGraph GsLinkObject4 GsMapModelingData
    GsMulCoord0 GsMulCoord3 GsSetAmbient GsSetFlatLight GsSetLightMatrix GsSetLightMode
    GsSetLsMatrix GsSetNearClip GsSetOrign GsSetProjection GsSetRefView2 GsSetView2
    GsSetWorkBase GsSortBoxFill GsSortClear GsSortFastSprite GsSortObject4J GsSortOt
    GsSortPoly GsSortSprite GsSwapDispBuff
}

// libgs: the TMD primitive handlers GsSortObject4J dispatches to through GsFCALL4.
stubs! {
    GsTMDdivTF3NL GsTMDdivTF4L GsTMDdivTF4NL GsTMDdivTG3NL GsTMDdivTG4NL GsTMDdivTNF3
    GsTMDdivTNF4 GsTMDdivTNG3 GsTMDdivTNG4 GsTMDfastF3L GsTMDfastF4L GsTMDfastF4NL
    GsTMDfastG3L GsTMDfastG4L GsTMDfastNF4 GsTMDfastTF3L GsTMDfastTF3NL GsTMDfastTF4L
    GsTMDfastTF4NL GsTMDfastTG3L GsTMDfastTG3NL GsTMDfastTG4L GsTMDfastTG4NL GsTMDfastTNF3
    GsTMDfastTNF4 GsTMDfastTNG3 GsTMDfastTNG4
}

// libcd, with the movie streaming calls (data globals are in `libcd`).
stubs! {
    CdControl CdControlB CdInit CdIntToPos CdPosToInt CdRead CdRead2 CdReadSync CdSearchFile
    StCdInterrupt StFreeRing StGetBackloc StGetNext StSetRing StSetStream StUnSetRing
}

// libpress: the MDEC.
stubs! { DecDCTin DecDCTout DecDCToutCallback DecDCTReset DecDCTvlc2 DecDCTvlcBuild }

// libspu
stubs! {
    SpuClearReverbWorkArea SpuGetKeyStatus SpuSetReverb SpuSetReverbDepth
    SpuSetReverbModeParam SpuSetReverbVoice
}

// libsnd
stubs! {
    SsEnd SsInit SsQuit SsSeqClose SsSeqOpen SsSeqPlay SsSeqSetVol SsSeqStop SsSetMVol
    SsSetTableSize SsSetTickMode SsStart SsUtAllKeyOff SsUtAutoVol SsUtGetVBaddrInSB
    SsUtKeyOffV SsUtKeyOnV SsVabClose SsVabOpenHeadSticky SsVabTransBody SsVabTransCompleted
}

// libmcrd
stubs! {
    MemCardAccept MemCardCreateFile MemCardDeleteFile MemCardExist MemCardFormat
    MemCardGetDirentry MemCardInit MemCardReadFile MemCardStart MemCardSync MemCardWriteFile
}
