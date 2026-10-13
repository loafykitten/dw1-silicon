//! Every SDK function the game C calls that has no Rust implementation yet, by library. Each
//! one traps with its name (`trap::stubs!`). Implementing one means deleting it here and
//! writing it in its library's module.
//!
//! The list is what linking main and every overlay needs (Phase 1, 2026-10-13); a call the
//! game gains later shows up as an undefined symbol at link time.

use crate::trap::stubs;

// libc: process exit (returns to the BIOS on the PS1). Renamed `dw1_exit` by dw1_native.h.
stubs! { dw1_exit }

// libgpu
stubs! { LoadClut OpenTIM ReadTIM SetDrawLoad }

// libgte
stubs! {
    ApplyMatrix ApplyMatrixLV ApplyMatrixSV ApplyTransposeMatrixLV CompMatrix MulMatrix
    MulMatrix0 NormalColorCol NormalColorCol3 PopMatrix PushMatrix RotMatrix RotMatrixYXZ
    RotMatrixZYX RotNclip3 RotNclip4 RotTransPers RotTransPers3 RotTransPers4 ScaleMatrix
    SetLightMatrix SetRotMatrix SetTransMatrix SquareRoot0 TransMatrix TransposeMatrix ratan2
    rcos rsin
}

// libgs (its data globals are in `globals`).
stubs! {
    GsGetLs GsGetLw GsGetLws

    GsMulCoord0 GsMulCoord3 GsSetAmbient GsSetFlatLight GsSetLightMatrix GsSetLightMode
    GsSetLsMatrix GsSetView2
    GsSortBoxFill GsSortFastSprite GsSortObject4J GsSortOt
    GsSortPoly GsSortSprite
}

// libgs: the TMD primitive handlers GsSortObject4J dispatches to through GsFCALL4.
stubs! {
    GsTMDdivTF3NL GsTMDdivTF4L GsTMDdivTF4NL GsTMDdivTG3NL GsTMDdivTG4NL GsTMDdivTNF3
    GsTMDdivTNF4 GsTMDdivTNG3 GsTMDdivTNG4 GsTMDfastF3L GsTMDfastF4L GsTMDfastF4NL
    GsTMDfastG3L GsTMDfastG4L GsTMDfastNF4 GsTMDfastTF3L GsTMDfastTF3NL GsTMDfastTF4L
    GsTMDfastTF4NL GsTMDfastTG3L GsTMDfastTG3NL GsTMDfastTG4L GsTMDfastTG4NL GsTMDfastTNF3
    GsTMDfastTNF4 GsTMDfastTNG3 GsTMDfastTNG4
}

// libcd's movie streaming calls (the rest is in `libcd`; its data global is in `globals`).
stubs! {
    CdRead2
    StCdInterrupt StFreeRing StGetBackloc StGetNext StSetRing StSetStream StUnSetRing
}

// libpress: the MDEC.
stubs! { DecDCTin DecDCTout DecDCToutCallback DecDCTReset DecDCTvlc2 DecDCTvlcBuild }
