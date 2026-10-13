# Task C: US-build source survey

The main porting risks are **in-place TMD relocation, pointer-bearing EFE buffers, overlay state split between resident and replaceable sections, sector-rounded overlay reads, and timing tied to update order**.

This survey was read-only; nothing under `disc/` was read.

**Scope and notation**

- The source inventory is the **120 C files** listed in `mk/version/us.mk:21`, including its subsequent unit/overlay lists.
- The active macro is **`VERSION_US`**, yielding ID 4, date 19990331, and NTSC-U: `include/dw/version.h:15`, `include/dw/version.h:35`. Counts exclude inactive regional branches and function declarations.
- Widths below are **PS1 widths**, in bits. Original `long`, addresses, and pointers are 32-bit; that is not the native macOS layout. See `include/dw/types.h:10` and `mk/platform/psx.mk:20`.
- Paths are relative to `ref/dw_decomp`, unless prefixed `ref/DW2-PC`.
- Compact `file:{a,b,c}` references mean `file:a`, `file:b`, and `file:c` individually.
- The local Psy-Q submodule is empty. SDK type definitions were checked against its pinned upstream revision, `1140be3876054886ef753cc0ca20bb915ae4ae52`; the submodule is identified in `.gitmodules:7`.

**Verification limits:** generated `asm/`, generated assembly/data, object files, and the linked ELF are absent. Consequently, the requested complete **generated-assembly/linked-symbol audit remains unverified**. The source/global-name audit below includes objects, US-generated text definitions, and the reproducible SDK BSS symbol inventory; it is not presented as an `nm` audit.

## 1. Loaded formats

### Common loading mechanisms

| Mechanism | Function and reference | Behaviour |
|---|---|---|
| Whole file | `readFile`, `src/main/file.c:80` | Looks up the compiled file table, sets CD location, reads `ceil(size/2048)` sectors, blocks on `CdReadSync(0)`, retries errors. |
| Sector subsection | `readFileSectors`, `src/main/file.c:134` | Offset and length are sector counts; blocks and retries. |
| Script subsection | `readFileSection`, `src/main/script_interp.c:2783` | Byte offset/size are shifted right by 11; caches the file’s starting sector. No general unaligned-byte read. |
| Queued whole file | `addFileReadRequestPath`, `src/main/file_queue.c:191`; `addFileReadRequestLookup`, `src/main/file_queue.c:281` | Queues a destination, completion flag, callback, and callback parameter. |
| Queued subsection | `addFileReadRequestSection`, `src/main/file_queue.c:264` | Sector offset/count. |
| Queue progression | `tickFileReadQueue`, `src/main/file_queue.c:99` | Starts the sector-rounded read at `:143`, polls with `CdReadSync(1)` at `:152`, then runs completion/continuation callbacks. |
| Card file | `tickMainMenu`, `src/main/main_menu.c:3319`; `loadSaveSlotData`, `:4792` | Whole 8192-byte save reads or 128-byte preview reads; completion is managed through `MemCardSync`. |

The compiled lookup record is `FileEntry`: `sector:s32` and `size:s32` are location/size; `filename:pointer32` is an **absolute runtime address**, not a disc-relative offset. It is compiled executable data, not a separately read table: `include/dw/file_table.h:9`, `src/main/file_table_us.c:6`.

A native loader must also address the queued-path implementation’s unconditional `slot->pos = *loc`, despite the path wrapper passing `NULL`: `src/main/file_queue.c:196`, `:250`. The location is subsequently replaced by `CdSearchFile`: `:127`.

### MMD, standalone TMD, and MTN animation data

| Format | Loader/consumer | Buffer interpretation |
|---|---|---|
| MMD | `loadMMD`, `src/main/model.c:3356`; reads at `:3414`, `:3446` | First two 32-bit words are **relative byte offsets from the MMD base**: TMD and animation table. Resolution at `:3415`, `:3416`, `:3447`, `:3448`. No named MMD-header struct. |
| Queued MMD | `loadMMDAsync`, `src/main/model.c:3580`; queue at `:3635`; `applyMMD`, `:3642` | Texture read is synchronous; MMD read is queued. `EvoModelData` holds runtime pointers/sizes, not an on-disc MMD header. Application copies the MMD, resolves its two offsets, and maps TMD data: `:3724–3727`. |
| Standalone TMD/MTN fallback | `applyMMD`, `src/main/model.c:3684`, `:3692` | Separate TMD and animation-table allocations. Extensions are `.TMD` and `.MTN`, not `.ANM`: `:455`, `:457`. **Reachability with the retail US file set is unverified**; there are no MTN entries in the US lookup table. |
| Animation table | `startAnimation`, `src/main/anim.c:201` | Each entry is `u32`, a **relative byte offset from the animation-table base**, or zero: `:216–220`. |
| Animation body | `startAnimation`, `src/main/anim.c:234`; `tickAnimation`, `:276` | Parsed as 16-bit instruction/data words, not as a serialized `EntityAnim`. |

Animation fields:

- Initial word: `u16`, frame count in low 15 bits and “has scale” flag in bit 15: `src/main/anim.c:234–240`.
- Per non-root bone: optional scale `x/y/z:s16`, rotation `x/y/z:s16`, translation `x/y/z:s16`; all plain data: `:251–267`.
- Instruction word: `u16`, opcode/flags in high bits, frame or loop count in low bits: `:295–348`.
- Momentum instruction: `u16` bone/channel mask; divisor `s16`; selected delta values `s16`: `:374–405`.
- Loop pointers and current instruction pointers are written into **runtime `EntityAnim`**, not the animation file: `:273`, `:298`. Animation-table offsets remain offsets.

`TMDModel` overlays the loaded model:

| Field | Width | Meaning |
|---|---:|---|
| `id` | s32 | Plain format identifier |
| `flags` | u32 | Plain mapping flags |
| `nobj` | s32 | Object count |
| `obj[]` | 28 bytes/object on PS1 | `TMD_STRUCT` records |

Reference: `include/dw/model.h:15`.

`TMD_STRUCT` has three 32-bit references—`vertop`, `nortop`, `primtop`—three `u32` counts—`vern`, `norn`, `primn`—and `scale:u32` plain data. Its declared pointer fields are the locations affected by model mapping. The SDK declaration is `mw_lib41/include/libgs.h:542`. [Pinned SDK definition](https://github.com/jype0/psyq_headers/blob/1140be3876054886ef753cc0ca20bb915ae4ae52/mw_lib41/include/libgs.h#L542)

**Relocation:** the intended Psy-Q operation converts unmapped TMD offsets to absolute addresses **in place**. A native implementation cannot put 64-bit pointers into the original 28-byte object records. The game subsequently dereferences the mapped `primtop` and edits primitive words: `src/main/graphics.c:178–189`. The exact US SDK relocation instructions and initial mapping flags of individual retail models are **unverified**, because generated SDK assembly and disc buffers were not available.

The game accesses primitive bodies as words. It uses the header’s 8-bit input length to advance records and edits texture coordinates/pages/CLUTs in place: `src/main/graphics.c:180–191`. SDK primitive schemas use 8-bit header/color/UV fields and 16-bit vertex/normal indices and texture-page/CLUT fields; padding varies by primitive type. Whether every SDK-declared primitive variant actually occurs in the loaded US assets is **unverified**. [Pinned primitive declarations](https://github.com/jype0/psyq_headers/blob/1140be3876054886ef753cc0ca20bb915ae4ae52/mw_lib41/include/libgs.h#L328)

**All 35 game-side TMD mapping sites**, including models embedded in overlays:

| Function(s) | References |
|---|---|
| `initializeBuffModel` | `src/main/buff_model.c:83` |
| `loadStaticTMD`, chest, warp-crystal, Toy Town box loaders | `src/main/map_object.c:{300,453,510,677,678}` |
| `loadMMD`, `applyMMD` | `src/main/model.c:{3417,3449,3685,3727}` |
| `initializePoop` | `src/main/partner_impl.c:440` |
| EFE, confusion, stun, finisher-aura setup | `src/btl/battle_effect.c:{1754,5537,5594,5984}` |
| Egg-model setup | `src/dooa/dooa.c:549` |
| Boss buildup | `src/eab/eab.c:657` |
| `FISH_loadTMDModel` | `src/fish/fish_model.c:43` |
| `KAR_start` | `src/kar/kar.c:787` |
| Arena loader | `src/std/std_setup.c:648` |
| Versus/result/champion loaders | `src/std/std_hud.c:{1145,1251,1297,1354}` |
| EFE/confusion/stun/finisher setup | `src/std/std_effect.c:{1859,5731,5788,6178}` |
| EFE/confusion/stun/finisher setup | `src/vs/vs_effect.c:{1490,5316,5372,5762}` |
| Versus/result model loaders | `src/vs/vs_hud.c:{1326,1436}` |
| Arena loader | `src/vs/vs_scene.c:1688` |

Standalone disc-model reads are at `src/main/map_object.c:{298,450,507,672,673}`, `src/main/partner_impl.c:438`, `src/fish/fish_model.c:28`, `src/kar/kar.c:786`, `src/std/std_setup.c:647`, `src/std/std_hud.c:{1144,1250,1295,1350,1352}`, `src/vs/vs_hud.c:{1325,1435}`, and `src/vs/vs_scene.c:1687`.

Fishing additionally constructs `GsDOBJ2` records **after the file bytes in the supplied buffer**, then writes linked-model pointers into them: `src/fish/fish_model.c:31`, `:47`. These are runtime records appended to a loaded buffer, not serialized model records.

### TIM images and image containers

| Container | Loader | Interpretation |
|---|---|---|
| Individual TIM | `loadTIMFile`, `src/main/file.c:98`; `loadTextureFile`, `:53` | TIM decoded through `GsGetTimInfo`; pixels and optional CLUT uploaded. |
| Fixed-size stacked entry | `loadStackedTIMEntry`, `src/main/file.c:116` | Reads a sector subsection containing a TIM. |
| Digimon texture bank `ALLTIM.TIM` | `loadDigimonTexture`, `src/main/model.c:3285`; `loadMMDAsync`, `:3625` | Nine sectors, `0x4800` bytes, per Digimon. |
| Card-image bank `CARD.ALL` | `loadCardImage`, `src/main/game_menu.c:2380` | Fourteen sectors per card: `:2383–2384`; filename at `:461`. |
| Concatenated TIMs | `loadStackedTIMFile`, `src/main/utils2.c:492` | Continues while the next magic word is `0x10`; advances by decoded image/CLUT sizes. |
| Tournament TIM packs | `STD_loadTIMToVRAM`, `src/std/std_setup.c:501`; `STD_loadArenaTIMToVRAM`, `:618` | Six images or an explicit image count. |
| VS TIM packs | `VS_loadArenaTIMToVRAM`, `src/vs/vs_scene.c:1811`; `VS_loadTIMToVRAM`, `:1831` | Counted concatenated TIMs. |
| MAP image sections | `loadMapImage1`, `src/main/overworld.c:380`; `loadMapImage2`, `:394` | `OpenTIM`/`ReadTIM`, with special CLUT placement. |

A TIM’s serialized fields are a 32-bit magic, 32-bit mode, and image/optional CLUT blocks containing a 32-bit block size followed by 16-bit `x,y,w,h`; remaining bytes are plain pixel/palette data. These contain no CPU pointers. DW2-PC’s parser confirms the block traversal: `ref/DW2-PC/psyq/libgs.c:474`.

The SDK result structures are **runtime views**, not the disc layout:

- `GsIMAGE`: `pmode:u32`; `px,py,cx,cy:s16`; `pw,ph,cw,ch:u16`; `pixel,clut:pointer32`. `mw_lib41/include/libgs.h:182`. [Pinned definition](https://github.com/jype0/psyq_headers/blob/1140be3876054886ef753cc0ca20bb915ae4ae52/mw_lib41/include/libgs.h#L182)
- `TIM_IMAGE`: `mode:u32`; `crect,caddr,prect,paddr:pointer32`. Those pointers refer into the loaded TIM. `mw_lib41/include/libgpu.h:617`. [Pinned definition](https://github.com/jype0/psyq_headers/blob/1140be3876054886ef753cc0ca20bb915ae4ae52/mw_lib41/include/libgpu.h#L617)

The game writes these decoded pointers into separate local/runtime structures; it does not replace TIM header fields with native pointers.

### MAP and TFS

`loadMap`, `src/main/map.c:4020`, reads the MAP into `GENERAL_BUFFER_PTR` at `:4036`.

There is **no single serialized MAP struct**. The loader walks a variable-length `s32` offset directory:

| Directory field, in order | Width/class | Reference |
|---|---|---|
| Setup offset | s32 relative byte offset from MAP base | `src/main/map.c:4039` |
| Each 8-bit-image section offset | s32 relative offset | `:4047` |
| Each 4-bit-image section offset | s32 relative offset | `:4054` |
| Object section offset, when images exist | s32 relative offset | `:4059` |
| Entity section offset | s32 relative offset | `:4064` |
| Collision section offset | s32 relative offset | `:4080` |

Image counts are supplied by **resident `MapEntry` records**, not read from this directory. `MapEntry` has `filename:8×10`, then `num8bppImages,num4bppImages,flags,doorsId,toiletId,loadingName:s8`: `include/dw/map.h:15`.

MAP section schemas:

| Section | Fields and classification | Consumer |
|---|---|---|
| Setup | All serialized words are s32: viewpoint `vpx,vpy,vpz,vrx,vry,vrz`; three lights × `vx,vy,vz,r,g,b`; ambient `r,g,b`; projection distance; two region arrays × four; width, height; `width×height` tile IDs; final returned word. Dimensions/counts are counts; other fields plain data. | `loadMapSetup`, `src/main/map.c:4143–4209` |
| Object definitions | `count:s16`; each definition has `texX,texY,width,height,someX,someY,someZ,clut,transparency:s16`, all plain data. Several are narrowed in runtime `LocalMapObject`. | `src/main/overworld.c:331–350`; runtime struct at `:44` |
| Object instances | `count:s16`; each instance has eight sprite IDs, eight frame durations, `x,y,flag`, **all serialized s16**. Runtime durations/flag are narrowed to bytes. | `src/main/overworld.c:353–363`; `include/dw/map.h:36` |
| Warps | Six arrays of ten s16 values: `spawnX,spawnY,spawnZ,rotation,targetMap,targetExit`; all plain positions/IDs. `MapWarps` is copied directly from the section. | Struct `src/main/tamer.c:158`; copy `:390` |
| NPCs | Count s16. Records contain s16 type/follow mode, position/rotation/tracking, NPC IDs, base/current stats, reward bits, charge mode, flags, four moves, four priorities, flee vector, waypoint count, eight AI-section IDs, and counted waypoint triples. These are values/IDs, not pointers. | `loadMapDigimon`, `src/main/overworld.c:1026–1081` |
| Collision | 10,000 signed bytes, a 100×100 grid. Plain collision/trigger values. | `loadMapCollisionData`, `src/main/map_collision.c:23`; separate arena `.ATR` reads at `src/std/std_setup.c:655`, `src/vs/vs_scene.c:1695` |

`MapTiles`, `MapTileData`, and `LocalMapObjectInstance` are **expanded runtime structures**; they are not copied wholesale from MAP files. In particular, `MapTileData.imagePtr:pointer32` is runtime-only: `include/dw/map.h:25`.

TFS loading:

- `readMapTFS`, `src/main/map.c:4217`, reads the file at `:4222`.
- `setupMap`, `:4225`, skips an initial u32 word, reads a u32 CLUT count, then consumes 512 bytes per CLUT: `:4234–4241`.
- Each present background tile has a skipped u32 word followed by `4096×u32`, or 16,384 bytes, of pixel data: `:4255–4259`.
- The meanings of those skipped words are **unverified**.
- `MAP_CLUTS[]` and each tile’s `imagePtr` retain pointers into the TFS buffer: `:4240`, `:4256`. The TFS directory/header itself is not relocated.

### SCN scripts and embedded text

| File/layout | Fields | Loader |
|---|---|---|
| `DG.SCN` directory | u32 byte offsets; adjacent offsets determine a script’s size. Relative to the file. | Initial `0x2000` bytes: `src/main/script_interp.c:2204`; indexed at `:662–665` |
| `MAPHEAD.SCN` | Script bytes loaded as a whole | `src/main/script_interp.c:2203`; filenames identified by `src/main/file_table_us.c:754`, `:755` |
| Script section directory | Initial two bytes skipped; repeating `section:u16,offset:u16`; `0xffff` section sentinel. Offset is relative to script base. | `getScriptSection`, `src/main/script_interp.c:671–687` |
| Instruction stream | Opcode u8; operands u8, u16/s16, or s32 according to opcode; branch targets are offsets or section IDs. | `include/dw/script.h:13–125`; dispatch `src/main/script_interp.c:603–615` |
| Text/selection payload | Byte stream with control bytes and paired text bytes; selection targets are u16 offsets. | `src/main/script_textbox.c:1017–1041`, `:1097–1235` |

Text parsing includes substitution controls, names/stat values, formatting controls, and line/page transitions. It copies bytes into the resident text-line buffer rather than writing native pointers into the SCN: `src/main/script_textbox.c:1100–1235`.

Other text, descriptions, names, and gameplay tables are **compiled main/overlay data**, rather than independent text/table file loads. US text generation uses `en_us` and CP932 encoding: `tools/gen_text.py:14`, `:55`, `:92`; build integration is `mk/text.mk:1–10`. Pointer tables produced by that generator contain absolute executable addresses on PS1: `tools/gen_text.py:237`.

### EFE effects: scripts, mutable records, TIM and TMD

EFE records are read from `EFEDAT.EFE` in fixed **10-sector / `0x5000`-byte slots**:

- Location computation: `src/main/efe.c:1043–1049`.
- BTL queued loader: `BTL_loadNextEFEFile`, `src/btl/battle_effect.c:1574`, read at `:1614`.
- STD equivalent: `src/std/std_effect.c:2367`.
- VS equivalent: `src/vs/vs_effect.c:1350`.

`EfeFileHeader` is **13×32-bit words**, `0x34` bytes:

| Fields | Width/class |
|---|---|
| `startScript`, `initScript` | s32 relative offsets |
| `uvAnims`, `uvAnimsEnd`, `subEffects` | 32-bit relative offsets |
| `tmdStart`, `tmdEnd`, `timStart`, `timEnd`, `dataEnd` | s32 relative section boundaries |
| `unk28` | s32, meaning unverified |
| `numSubEffects` | s32 count |
| `effectId` | s32 plain ID |

Reference: `include/dw/efe.h:155`. Offsets use the **base immediately after the 0x34-byte header**, demonstrated at `src/btl/battle_effect.c:1654`, `:1670`, `:1790`.

The following structs overlay mutable areas **inside the loaded EFE buffer**:

| Struct | Every field’s width/class | Reference |
|---|---|---|
| `EfeSubEffect` | `inst,someInst,instance,parentInstance,sourceEntity,targetEntity,boneOffset`: seven pointer32 fields; runtime addresses written during interpretation | `include/dw/efe.h:122` |
| `EfeUvAnim` | `unk0,unk2,unk4,unk6,unk8,unkA,numKeyframes,keyframe,uvFrame,frame,numFrames,pad`: twelve s16 fields; counts/timers or plain state. `uvData,uv`: pointer32 runtime addresses | `include/dw/efe.h:138` |
| `EfeUvKeyframe` | `frames:s16` duration; `deltaU,deltaV:s8` plain values | `include/dw/efe.h:132` |
| `EfeInstance` | `frame:s32`; position and rotation each three s32 values, via `EfeTransform`/`EfeVector`; plain mutable state | `include/dw/efe.h:32`, `:44`, `:49` |
| `EfeBoneOffset` | `boneId,positionX,positionY,positionZ:s16`; plain IDs/coordinates | `include/dw/efe.h:54` |
| `EfeTechBoneOffset` | `type:s16`, then `EfeBoneOffset`; plain values | `include/dw/efe.h:61` |

**Native pointers are written back into loaded data here:**

- Sub-effect script pointers: `src/btl/battle_effect.c:4618`, `:4619`.
- Instance, parent, source and target pointers: `:4726–4733`.
- Bone-offset pointer: `:4677`.
- UV-data/current-keyframe pointers: `:4647`, `:4648`; advanced at `:1883`, `:1886`.
- Additional target-pointer changes: `:2840`, `:3500`, `:4284`.
- General s32 writes to script-relative or instance-relative locations: `:4746`, `:4748`; a value may originate from a pointer-bearing interpreter stack.

STD and VS use the same overlaid schemas and setup pattern: `src/std/std_effect.c:1906–1921`, `src/vs/vs_effect.c:1537–1552`.

EFE therefore needs more than fixed-width header decoding: **the original buffer is simultaneously file content, script storage, and mutable pointer-bearing runtime storage**.

### VHB/VAB/VAG and SEQ

| Format | Loader | Schema |
|---|---|---|
| VHB bank wrapper | `readVHBFile`, `src/main/sound.c:385`; subsection version `:439` | `VhbFileHeader`: `vhOffset:u32`, `vbOffset:u32`, both relative byte offsets. `include/dw/sound.h:8` |
| Queued VHB | `loadFullVHB`, `src/main/sound_async.c:66`; `loadVHBFile`, `:105` | Same wrapper; secondary callback uploads it: `:128`. |
| Music font | `loadMusicFont`, `src/main/sound.c:738` | A `0x27`-sector FAALL record; additional words `[2],[3]:u32` delimit the SEQ container: `:743–748`. |
| SEQ container | `seqOpen`, `src/main/sound.c:475` | u32 relative byte-offset table. First offset divided by four supplies the track-table bound: `:480`; selected offset passed to `SsSeqOpen`: `:485`. |

The game copies the VH section into resident `VHB_HEADER_ADDR[]`, then uploads the VB section to SPU memory: `src/main/sound.c:400–414`. It does not turn wrapper offsets into pointers in the wrapper.

`VhbFile` overlays the copied VH: `VabHdr`, 128 `ProgAtr` records, then variable `VagAtr[16]` program groups: `src/main/sound.c:16`.

The pinned SDK layouts are:

- **`VabHdr`, 32 bytes:** `form,ver,id:s32`; `fsize:u32` size; `reserved0,ps,ts,vs:u16`, with `ps/ts/vs` counts; `mvol,pan,attr1,attr2:u8`; `reserved1:u32`.
- **`ProgAtr`, 16 bytes:** `tones:u8` count; `mvol,prior,mode,mpan:u8`; `reserved0:s8`; `attr:s16`; `reserved1,reserved2:u32`.
- **`VagAtr`, 32 bytes:** `prior,mode,vol,pan,center,shift,min,max,vibW,vibT,porW,porT,pbmin,pbmax,reserved1,reserved2:u8`; `adsr1,adsr2:u16`; `prog,vag,reserved[4]:s16`. These are sound parameters/sample indices, not CPU pointers.

References: `mw_lib41/include/libsnd.h:{72,89,102}`. [Pinned sound layouts](https://github.com/jype0/psyq_headers/blob/1140be3876054886ef753cc0ca20bb915ae4ae52/mw_lib41/include/libsnd.h#L72)

Bank subsection conventions are explicit:

- VLALL: 15 sectors/entry, `src/main/sound.c:686`.
- VBALL: 7 sectors/entry, `:703`.
- ESALL: resident `MapSoundPara` has `sectorId:u16`, `sectorCount:u16`; loader divides both by two, `include/dw/sound.h:13`, `src/main/sound.c:729–731`.

**SEQ internals are delegated to SDK code.** DW2-PC provides an applicable reference: header validation, 16-bit big-endian resolution, 24-bit big-endian tempo, followed by events at `ref/DW2-PC/psyq/libsnd.c:596–620`. Exact DW1 SDK validation/event semantics and SDK-side VH mutations remain **unverified**.

### STR/XA movies

Entry points:

- `MOV_playMovie`, `src/mov/mov.c:207`.
- `MOV_startStream`, `:111`: ring setup, stream setup, MDEC callback.
- `MOV_startRead`, `:120`: CD streaming mode.
- `MOV_readFrame`, `:175`: obtains payload and `StHEADER`.
- `MOV_decodeFrame`, `:135`: VLC-decodes the payload.
- `MOV_onSliceDecoded`, `:85`: MDEC-output completion and VRAM upload.

`StHEADER` fields are `id,type,secCount,nSectors:u16`; `frameCount,frameSize:u32`; `width,height:u16`; `dummy1,dummy2:u32`; and `loc`, a four-byte `CdlLOC` containing byte-sized BCD minute/second/sector and track. Counts/sizes/dimensions are counts; IDs/reserved/location are plain data. **No CPU addresses are embedded by this schema.** Usage is at `src/mov/mov.c:178–198`. [Pinned streaming definition](https://github.com/jype0/psyq_headers/blob/1140be3876054886ef753cc0ca20bb915ae4ae52/mw_lib41/include/libcd.h)

`MovieEntry` is resident/overlay metadata, not a stream header: `path:8×32`, `startFrame:s32`, `frameCount:s32`: `src/mov/mov.c:10`.

`MovieEnv` is mutable overlay storage: two run-level pointers, run-level index s32, two slice pointers, slice index s32, two `RECT`s, frame index s32, slice `RECT`, flip flag s32. Each `RECT` contains four s16 fields: `include/dw/mov.h:8`. Initialization writes native-equivalent pointers into this loaded overlay object: `src/mov/mov.c:68–72`.

CD mode includes streaming/real-time audio: `src/mov/mov.c:132`. XA-sector demultiplexing and compressed-audio/bitstream payload details are handled below the game C layer and are **unverified for DW1 here**. No separate game-side XA-file loader was found.

### Memory-card saves

Whole-save reads occur at **two active US sites**, `src/main/main_menu.c:3751` and `:4151`; preview reads at `:4828`. The regional branch containing `:4594` is not active for US.

`SaveFile`, `src/main/main_menu.c:140`:

| Field | Width/class |
|---|---|
| `id[2]` | 8-bit signature bytes |
| `iconDisplayFlag`, `blockNumber` | u8 plain flag/count |
| `title[64]`, `reserved[12]`, `pocketStation[16]` | 8-bit byte arrays |
| `iconClut[16]` | u16 plain palette data |
| `iconFrames[3][0x80]` | 8-bit pixel data |
| `saves[2]` | Two fixed-layout payloads |

The header is `0x200` bytes. The PS1-layout payload is `0xF00` bytes; two payloads plus header make the requested `0x2000` bytes. This size is derived from the declarations at `src/main/main_menu.c:88–150`, not a native `sizeof` assumption.

`SavegamePayload` fields, all **plain saved data**:

| Fields | Width | Reference |
|---|---|---|
| `mainState`, `money`, `partnerType` | s32 each | `src/main/main_menu.c:{89,93,94}` |
| `tamerPosition`, `partnerPosition` | Four s32 components each (`VECTOR`, including pad) | `:90`, `:91` |
| `worldPoop[100]` | Each `map,x,y:u8`, `size:s8` | `:92`; `include/dw/partner.h:24` |
| `partnerData[16]` | u8 bytes | `:95` |
| `currentFrame`, `lastHandledFrame` | u16 | `:96`, `:97` |
| `year,day,hour,minute` | s16 | `:98–101` |
| `playtimeFrames,playtimeHours,playtimeMinutes` | u16 | `:102–104` |
| `pad1DE[2]` | u8 | `:105` |
| `partnerPara` | `condition:u32`; every subsequent named field s16 | `:106`; `include/dw/params.h:34–100` |
| `merit,battlesStarted,battlesFled,tournamentsWon,tournamentWins,tournamentsLost` | s16 | `:107–112` |
| `stats` | Fixed `BaseStats` + `CurrentStats`, detailed below | `:113` |
| `scriptState[0x17C]` | u8 bytes | `:114` |
| `inventory` | Three 30-byte arrays and one u8 size | `:115`; `include/dw/item.h:39–48` |
| `playerName[20]`, `digimonName[20]` | 8-bit bytes | `:116`, `:117` |
| `partnerLives,tamerLevel` | s8 | `:118`, `:119` |
| `raisedCount` | u8 | `:120` |
| `npcMaps[8]` | s8 | `:121` |
| `currentScreen,previousScreen,currentExit,previousExit` | u8 | `:122–125` |
| `tamerWaypointX[30]`, `tamerWaypointY[30]` | s8 | `:126`, `:127` |
| `tamerPreviousTileX,tamerPreviousTileY,tamerWaypointCurrent,tamerWaypointCount,tamerStartTileX,tamerStartTileY,tamerWaypointActive` | s8 | `:128–134` |
| `unknown4E1[0x1B]` | u8 bytes | `:135` |
| `checksum` | s32 | `:136` |
| `battleRegistrationData[40]` | Forty 64-byte `RegisteredDigimon` records | `:137` |

Nested records:

- `BaseStats`: `off,def,speed,brain,hp,mp:s16`; `movesPrio[4],moves[4]:u8`. `include/dw/entity.h:63`.
- `CurrentStats`: `currentHP,currentMP,unk1,vabId:s16`; `efeSubEffect,isHit:s8`; `chargeMode,field7_0x57:u8`. `include/dw/entity.h:74`.
- `RegisteredDigimon`: `hp,mp,offense,defense,speed,brains,discipline:s16`; `name[14]:8-bit`; `digimonId,moves[3],unk20[32]:u8`. `include/dw/vs.h:20`.
- Every `PartnerPara` member after `condition` is s16, including its unknown/reserved fields: `include/dw/params.h:36–99`.

**The save contains no pointer fields.** In particular, only the first `0x17C` bytes of `ScriptState` are saved/restored, excluding its pointer-bearing return stack: `src/main/main_menu.c:5120`, `:5058`; `include/dw/script.h:158`.

Checksums sum the first `0x4E4` payload bytes plus `0xA00` registration bytes, not the entire payload: `src/main/main_menu.c:5004–5013`. Backup payload fallback is at `:3758–3768`.

### Files present in the lookup table without an identified game loader

`SYSTEM.CNF`, `PSX.EXE`, and `MMM.DAT` appear at `src/main/file_table_us.c:{7,823,824}`. Their presence does not establish a game-C load path. `SYSTEM.CNF`/executable boot handling belongs outside the surveyed C loaders; a game-side `MMM.DAT` consumer was not found. Its format/use is **unverified**.

## 2. Overlay lifetimes

### Loader and all call sites

`loadDynamicLibrary`, `src/main/utils2.c:376`, indexes the 16-entry filename/address tables at `:89` and `:144`.

- Synchronous path calls `readFile`: `:381–382`.
- Async path calls `addFileReadRequestPath`: `:384–386`.
- The flag means **running**, despite parameter names such as `isComplete`: enqueue sets it to 1 and completion clears it to 0, `src/main/file_queue.c:241–245`, `:173–175`.
- The synchronous path does **not** update that flag or invoke the callback.
- There is no generic relocation, destructor, or “reset all globals belonging to this source directory” step.

| Overlay | All US callers | Mode |
|---|---|---|
| BTL | `src/main/btl.c:379` | Queue async; caller pumps frames while loading at `:396–406` |
| STD | `src/main/utils2.c:661` | Sync |
| FISH | `src/main/fish.c:124` | Sync |
| EVL | `src/main/evl.c:108` | Sync |
| KAR | `src/main/kar.c:11` | Sync |
| VS | `src/main/main_menu.c:4194` | Sync |
| MOV | `src/main/utils2.c:397` | Sync |
| DOO2 | `src/dooa/dooa.c:365` | Queue async |
| DOOA | `src/main/partner.c:654`; `src/murd/murd.c:788` | Sync |
| TRN | `src/main/map.c:5460` | Queue async |
| SHOP | `src/main/script_common.c:{3076,3084}` | Sync / queue async respectively |
| DGET | `src/main/script_common.c:3097` | Sync |
| TRN2 | `src/main/map.c:5466` | Queue async |
| MURD | `src/main/tamer.c:{1703,1738}` | Sync |
| ENDI | `src/main/script_interp.c:1850` | Queue async |
| EAB | `src/main/script_interp.c:1844` | Sync |

### Address and overlap map

Intervals are half-open. **Payload end** comes from YAML file length. **Transfer end** includes the sector rounding performed by both loaders: `src/main/file.c:91`, `src/main/file_queue.c:143`.

| Overlay | Start | Payload end | Transfer end | Layout references |
|---|---:|---:|---:|---|
| BTL | `80052AE0` | `80075F7C` | `800762E0` | `config/us/overlay.ld:3`; `config/us/btl.yaml:{53,113}` |
| DGET | `80080800` | `80081CF8` | `80082000` | `overlay.ld:83`; `dget.yaml:{53,68}` |
| DOO2 | `80070000` | `8007B6EC` | `8007B800` | `overlay.ld:108`; `doo2.yaml:{53,68}` |
| DOOA | `80080000` | `800878AC` | `80088000` | `overlay.ld:134`; `dooa.yaml:{53,68}` |
| EAB | `80060000` | `80062054` | `80062800` | `overlay.ld:165`; `eab.yaml:{53,68}` |
| ENDI | `80060000` | `80061534` | `80061800` | `overlay.ld:194`; `endi.yaml:{53,67}` |
| EVL | `80060000` | `80069008` | `80069800` | `overlay.ld:220`; `evl.yaml:{53,69}` |
| FISH | `80070000` | `8007D96C` | `8007E000` | `overlay.ld:249`; `fish.yaml:{53,70}` |
| KAR | `80053800` | `800639EC` | `80064000` | `overlay.ld:282`; `kar.yaml:{53,69}` |
| MOV | `80010000` | `80083428` | `80083800` | `overlay.ld:307`; `mov.yaml:{53,68}` |
| MURD | `8007C000` | `8007E960` | `8007F000` | `overlay.ld:333`; `murd.yaml:{53,68}` |
| STD | `80052AE0` | `8007FEC8` | `800802E0` | `overlay.ld:358`; `std.yaml:{53,110}` |
| TRN2 | `80088800` | `8008DCC0` | `8008E000` | `overlay.ld:423`; `trn2.yaml:{53,87}` |
| TRN | `80088800` | `8008F3D4` | `8008F800` | `overlay.ld:511`; `trn.yaml:{53,86}` |
| VS | `80052AE0` | `80073268` | `800732E0` | `overlay.ld:599`; `vs.yaml:{53,114}` |
| SHOP, retail file | `80080800` | `80088610` | `80088800` | Address `src/main/utils2.c:24`; size `src/main/file_table_us.c:818` |

All numbers above have the `0x` prefix implied. SHOP has no US overlay YAML/linker output; its code is a resident unit: `mk/version/us.mk:93`. Its retail file’s internal contents are **unverified**.

Complete transfer-range overlap lists, using the referenced intervals above:

| Overlay | Overlaps |
|---|---|
| BTL | DOO2, EAB, ENDI, EVL, FISH, KAR, MOV, STD, VS |
| DGET | DOOA, MOV, SHOP |
| DOO2 | BTL, FISH, MOV, STD, VS |
| DOOA | DGET, MOV, SHOP, **STD** |
| EAB | BTL, ENDI, EVL, KAR, MOV, STD, VS |
| ENDI | BTL, EAB, EVL, KAR, MOV, STD, VS |
| EVL | BTL, EAB, ENDI, KAR, MOV, STD, VS |
| FISH | BTL, DOO2, MOV, MURD, STD, VS |
| KAR | BTL, EAB, ENDI, EVL, MOV, STD, VS |
| MOV | BTL, DGET, DOO2, DOOA, EAB, ENDI, EVL, FISH, KAR, MURD, SHOP, STD, VS |
| MURD | FISH, MOV, STD |
| SHOP | DGET, DOOA, MOV |
| STD | BTL, DOO2, DOOA, EAB, ENDI, EVL, FISH, KAR, MOV, MURD, VS |
| TRN / TRN2 | Each other only |
| VS | BTL, DOO2, EAB, ENDI, EVL, FISH, KAR, MOV, STD |

The STD/DOOA overlap is caused entirely by sector rounding: STD overwrites DOOA’s first **`0x2E0` bytes**. A reset scheme based only on equal overlay bases would miss this.

### Section placement by source

In this table, **M** means `config/us/main.ld`; **O** means `config/us/overlay.ld`. Numbers are exact selector lines. A dash means no selector for that section, not proof that a native compiler will emit an empty section.

| Source | Resident `.sdata` M | Resident `.sbss` M | Overlay `.data` O | Overlay `.bss` O |
|---|---:|---:|---:|---:|
| `src/btl/battle_setup.c` | 634 | 774 | 50 | 65 |
| `src/btl/battle_main.c` | 636 | — | 52 | 67 |
| `src/btl/command_shout.c` | 638 | — | 54 | 69 |
| `src/btl/command_menu.c` | 640 | 776 | 56 | 71 |
| `src/btl/battle_hud.c` | 641 | 778 | 58 | 73 |
| `src/btl/battle_effect.c` | 643 | 780 | 60 | — |
| `src/dget/dget.c` | — | 816 | 98 | — |
| `src/doo2/doo2.c` | 669 | 806 | 123 | 129 |
| `src/dooa/dooa.c` | 671 | 808 | 149 | 152 |
| `src/eab/eab.c` | 685 | 826 | 180 | 189 |
| `src/endi/endi.c` | 683 | — | 209 | 212 |
| `src/evl/evl.c` | 655 | 792 | 235 | 244 |
| `src/fish/fish.c` | 653 | 790 | 266 | 276 |
| `src/fish/fish_model.c` | — | — | 267 | 277 |
| `src/kar/kar.c` | 657 | 794 | 297 | 302 |
| `src/mov/mov.c` | — | 804 | 322 | 328 |
| `src/murd/murd.c` | 681 | 824 | 348 | 353 |
| `src/std/std_setup.c` | 645 | 782 | 396 | 407 |
| `src/std/std_main.c` | 647 | 784 | 398 | 409 |
| `src/std/std_hud.c` | 649 | 786 | 400 | 411 |
| `src/std/std_effect.c` | 651 | 788 | 402 | 413 |
| `src/trn2/trn2_hp_map99.c` | — | 818 | 470 | 490 |
| `src/trn2/trn2_def_map99.c` | — | 819 | 472 | 492 |
| `src/trn2/trn2_hp_map107.c` | — | — | 474 | 494 |
| `src/trn2/trn2_mp.c` | — | — | 476 | 496 |
| `src/trn2/trn2_off.c` | — | 820 | 478 | 498 |
| `src/trn2/trn2_def_map108.c` | — | — | 480 | 500 |
| `src/trn2/trn2_reward.c` | — | 821 | 482 | 502 |
| `src/trn2/trn2_hud.c` | 677 | 822 | 484 | 504 |
| `src/trn2/trn2_slots.c` | 679 | — | 486 | 506 |
| `src/trn/trn_reward.c` | — | — | 558 | 578 |
| `src/trn/trn_hp.c` | — | 810 | 560 | 580 |
| `src/trn/trn_mp.c` | — | 811 | 562 | 582 |
| `src/trn/trn_off.c` | — | 812 | 564 | 584 |
| `src/trn/trn_def.c` | — | 813 | 566 | 586 |
| `src/trn/trn_speed.c` | — | — | 568 | 588 |
| `src/trn/trn_brain.c` | — | — | 570 | 590 |
| `src/trn/trn_hud.c` | 673 | 814 | 572 | 592 |
| `src/trn/trn_slots.c` | 675 | — | 574 | 594 |
| `src/vs/vs_scene.c` | 659 | 796 | 645 | 661 |
| `src/vs/vs_camera.c` | 661 | 798 | 649 | 665 |
| `src/vs/vs_main.c` | 663 | — | 651 | 667 |
| `src/vs/vs_hud.c` | 665 | 800 | 653 | 669 |
| `src/vs/vs_effect.c` | 667 | 802 | 655 | 671 |

Additional resident placements:

| Source | Resident placement |
|---|---|
| `src/shop/script_menu.c` | `.data` M:532; `.sdata` M:628; resident code through the SHOP unit, `mk/version/us.mk:93` |
| `src/vs/vs_intro.c` | `.data` M:518; `.sdata` M:616; `.sbss` M:758; `.bss` M:913 |
| `src/vs/vs_combat.c` | `.data` M:520; `.sdata` M:618; `.sbss` M:760 |
| `src/vs/vs_select.c` | `.data` M:522; `.sdata` M:620; `.sbss` M:762; `.bss` M:915 |

The repeated VS `.data` selectors at `config/us/main.ld:{540,542,544,550,552,554}` do not create duplicate definitions; an input section is consumed once. Unselected sections are discarded by `config/us/main.ld:947`, making original section ownership significant for the native build.

### Assets inside overlays

**Explicitly identified assets:** BTL, STD, and VS each contain a TIM texture and four TMD model blocks before their code/header:

- BTL: `config/us/btl.yaml:{50,57,64,71,78}`; `config/us/overlay.ld:{8,10,12,14,16}`.
- STD: `config/us/std.yaml:50`; `config/us/overlay.ld:{362,364,366,368,370}`.
- VS: `config/us/vs.yaml:50`; `config/us/overlay.ld:{603,605,607,609,611}`.

Their fixed model/texture addresses are also exposed in game headers, for example `include/dw/vs.h:14–18`.

Other overlays contain initialized buffers and tables, including apparent retail residue. That does **not** establish that every such buffer is a usable asset. Additional embedded assets and SHOP’s internal payload are **unverified**.

### “BSS” source files with non-zero initializers

All fourteen `_bss_us.c` files are selected into overlay **`.data`**, regardless of their filenames.

| File | Non-zero initializer evidence | Overlay `.data` selector |
|---|---|---|
| `src/btl/btl_bss_us.c` | `:19` | `config/us/overlay.ld:62` |
| `src/doo2/doo2_bss_us.c` | `:30` | `:126` |
| `src/dooa/dooa_bss_us.c` | `:38` | `:155` |
| `src/eab/eab_bss_us.c` | `:24` | `:182` |
| `src/endi/endi_bss_us.c` | `:26` | `:215` |
| `src/evl/evl_bss_us.c` | `:22` | `:237` |
| `src/fish/fish_bss_us.c` | `:33` | `:269` |
| `src/kar/kar_bss_us.c` | `:22` | `:299` |
| `src/mov/mov_bss_us.c` | `:10`, `:18` | `:325` |
| `src/murd/murd_bss_us.c` | `:38` | `:350` |
| `src/std/std_bss_us.c` | `:11` | `:404` |
| `src/vs/vs_bss_us.c` | `:138` | `:657` |
| `src/trn/trn_bss_us.c` | Explicit zero initializers at `:6–18` | `:576` |
| `src/trn2/trn2_bss_us.c` | Explicit zero initializers at `:6–16` | `:488` |

Thus **12 of 14 contain non-zero initializers**. A blanket zero-on-reload policy changes the original loaded image.

Loaded overlay data also contains literal address fields:

- `DOOA_ORDERING_TABLE` has typed pointers to `0x80040000`: `src/dooa/dooa_bss_us.c:{1015,1020}`.
- `DOOA_SHARD_EFFECT.shardWrite` includes `(void *)0x80040000`: `:1032`.
- Other typed pointer initializers include non-PS1-looking values: `:1031`, `:1038`, `:1045`. They must not automatically be treated as valid live references.
- `MOV_ENV` has non-zero indices/rectangles/flags before initialization: `src/mov/mov_bss_us.c:6–14`; `MOV_initEnvironment` overwrites runtime pointers at `src/mov/mov.c:68–72`.

Raw words resembling addresses inside other dumped arrays are **unverified as pointers**.

### State that survives swaps

The section map identifies **540 externally named objects from overlay-source files in resident main addresses**: 511 explicit C objects and 29 US-generated text objects. These include immutable parameters as well as mutable state; they are not all resettable state.

The complete **511 explicit-object definition references**, grouped by file:

| File | Count | Definition lines |
|---|---:|---|
| `src/btl/battle_effect.c` | 26 | `{767,768,769,770,771,772,773,774,775,776,777,778,779,780,781,782,783,784,785,786,788,789,790,791,792,793}` |
| `src/btl/battle_hud.c` | 25 | `{154,155,156,158,159,160,161,162,163,164,165,166,167,168,169,170,171,172,173,174,175,176,177,178,179}` |
| `src/btl/battle_main.c` | 5 | `{300,301,302,303,304}` |
| `src/btl/battle_setup.c` | 5 | `{30,31,33,34,35}` |
| `src/btl/command_menu.c` | 13 | `{29,30,31,32,33,34,35,36,37,95,96,97,98}` |
| `src/dget/dget.c` | 4 | `{84,85,86,87}` |
| `src/doo2/doo2.c` | 8 | `{85,86,87,88,90,91,92,93}` |
| `src/dooa/dooa.c` | 18 | `{209,210,211,213,214,215,216,217,218,219,220,221,222,223,224,225,226,227}` |
| `src/eab/eab.c` | 7 | `{121,122,123,124,125,126,128}` |
| `src/endi/endi.c` | 3 | `{37,38,39}` |
| `src/evl/evl.c` | 19 | `{199,200,201,202,203,204,206,207,208,209,210,211,212,213,214,215,216,217,218}` |
| `src/fish/fish.c` | 20 | `{343,344,345,346,347,348,349,350,351,352,353,354,355,356,357,358,359,361,362,936}` |
| `src/kar/kar.c` | 31 | `{180,181,182,183,184,185,186,187,188,189,190,191,192,193,194,195,196,197,198,199,200,268,269,270,274,276,277,441,442,443,444}` |
| `src/mov/mov.c` | 3 | `{42,43,44}` |
| `src/murd/murd.c` | 6 | `{95,96,97,98,99,101}` |
| `src/std/std_setup.c` | 7 | `{132,134,135,136,137,138,139}` |
| `src/std/std_main.c` | 68 | `{714,715,717,719,720,721,722,723,724,725,726,727,728,729,730,731,734,736,737,738,739,741,742,743,744,745,746,747,748,749,750,751,752,753,754,755,756,757,758,759,760,761,762,763,764,765,766,767,768,769,770,771,772,773,774,775,776,777,778,779,780,781,782,783,784,785,1509,1510}` |
| `src/std/std_hud.c` | 19 | `{160,161,162,163,164,165,166,167,168,169,170,171,172,173,422,423,424,425,426}` |
| `src/std/std_effect.c` | 25 | `{765,766,767,768,769,770,771,772,773,774,775,776,777,778,779,780,781,783,784,785,786,787,788,789,790}` |
| `src/trn2/trn2_hp_map99.c` | 6 | `{28,29,30,31,32,33}` |
| `src/trn2/trn2_def_map99.c` | 4 | `{31,32,33,34}` |
| `src/trn2/trn2_off.c` | 1 | `38` |
| `src/trn2/trn2_reward.c` | 2 | `{45,46}` |
| `src/trn2/trn2_hud.c` | 7 | `{68,69,70,71,72,73,74}` |
| `src/trn2/trn2_slots.c` | 3 | `{45,46,47}` |
| `src/trn/trn_hp.c` | 4 | `{37,38,39,40}` |
| `src/trn/trn_mp.c` | 6 | `{37,38,39,40,41,42}` |
| `src/trn/trn_off.c` | 1 | `41` |
| `src/trn/trn_def.c` | 7 | `{39,40,41,42,43,44,45}` |
| `src/trn/trn_hud.c` | 4 | `{61,62,63,64}` |
| `src/trn/trn_slots.c` | 3 | `{45,46,47}` |
| `src/vs/vs_camera.c` | 13 | `{117,118,125,126,127,128,129,130,131,132,133,134,135}` |
| `src/vs/vs_combat.c` | 12 | `{224,231,234,242,248,249,250,251,252,253,254,255}` |
| `src/vs/vs_effect.c` | 25 | `{699,700,701,702,703,704,705,706,707,708,709,710,711,712,713,714,715,717,718,719,720,721,722,723,724}` |
| `src/vs/vs_hud.c` | 27 | `{169,170,171,172,173,174,175,176,177,178,179,180,181,182,183,184,185,186,187,188,438,439,440,441,442,443,444}` |
| `src/vs/vs_intro.c` | 14 | `{29,90,94,98,100,102,106,121,122,123,124,125,126,127}` |
| `src/vs/vs_main.c` | 4 | `{160,161,162,163}` |
| `src/vs/vs_scene.c` | 25 | `{165,166,168,169,170,171,172,173,174,175,176,177,178,179,180,181,182,183,184,185,186,187,188,189,190}` |
| `src/vs/vs_select.c` | 31 | `{49,113,117,121,125,129,133,140,143,148,151,153,155,157,159,163,175,179,188,191,206,213,224,236,241,243,244,245,246,247,248}` |

The additional **29 generated resident objects** originate at:

- Seven HUD strings: `src/btl/battle_hud.c:152`.
- Six shout/command strings: `src/btl/command_shout.c:{19,31}`.
- One fishing string: `src/fish/fish.c:524`.
- Three curling dialogue strings: `src/kar/kar.c:279`.
- Four STD command strings: `src/std/std_main.c:1496`.
- Four VS command strings: `src/vs/vs_main.c:165`.
- Two VS result strings: `src/vs/vs_scene.c:163`.
- Two VS selection strings: `src/vs/vs_select.c:161`.

Concrete lifetime relationships also need preserving:

- Resident world-object slots retain overlay **function pointers**, so an overlapping load must follow the appropriate teardown: `src/main/world_object.c:42–45`, `:74–77`.
- Resident fishing pointer refers into replaceable `FISHING_DATA`; teardown clears it before loading/reinitializing: `src/main/fish.c:109–137`.
- EFE buffers, stacks, slot pointers, and loading continuation state are resident, while their interpreters/callbacks reside in BTL/STD/VS: `src/main/efe.c:253–314`, `src/btl/battle_effect.c:1869`.
- Tournament results flow back into resident script/game state after the STD call returns: `src/main/utils2.c:662–681`.
- DOOA can coexist with DOO2; MURD can coexist with DOOA. These explicit transitions occur at `src/dooa/dooa.c:365` and `src/murd/murd.c:788`, and their intervals above are disjoint.

### Duplicate-symbol audit

**No duplicate external definition names were detected in the available expanded US C inventory.** This included global variables, initialized tables, function-pointer objects, functions, and **753 text definitions generated in memory**, rather than only function definitions. The inventory contained **5,594 external definition names**.

The checked-in US symbol declarations also contain **7,584 distinct names and zero repeated names**, across `config/us/symbols.txt` and the per-overlay symbol files referenced by each YAML, e.g. `config/us/btl.yaml:26–28`.

The reproducible generated SDK BSS inventory contains **153 global symbols**, with no collisions against the expanded C inventory:

| Library | Count | `config/us/symbols.txt` references |
|---|---:|---|
| libapi | 10 | `5042–5051` |
| libetc | 2 | `5052–5053` |
| libgpu | 5 | `5054–5058` |
| libgs | 34 | `5059–5092` |
| libgte | 1 | `5093` |
| libcd | 29 | `5125–5153` |
| libspu | 1 | `5188` |
| libsnd | 41 | `5189–5229` |
| libds | 1 | `5264` |
| libmcrd | 29 | `5266–5294` |

Generation emits explicit `.global` directives: `tools/gen_bss.py:8–12`; membership is determined from `config/us/bss.yaml:1`.

**The requested real audit over generated assembly/data and linked symbols is still unverified.** Symbol configuration files are declarations, not proof of emitted labels, linkage, aliases, or linker survival. Assembly acquisition requires the absent generation inputs/workflow at `mk/platform/psx.mk:85–87`; none was run.

## 3. Frame pacing

### Loop structures

- `main`, `src/main/main.c:3371`, initializes services, repeatedly runs movies/landing/menu, initializes a game, then calls `gameLoop` until `MAIN_STATE == 3`: `:3411`, `:3522–3527`.
- Landing screen has a countdown loop and a separate fade loop: `:3604`, `:3666`.
- Main menu runs its own tick/render loop: `:3701–3722`.
- New-game scene runs input, dialogue, world ticks, rendering, and presentation: `:3740–3770`.
- `gameLoop`, `:3933`, polls input, processes script state, ticks objects, renders, waits, swaps, and submits the ordering table: `:3935–3962`.
- BTL has its own main/frame routines: `src/btl/battle_main.c:1832`, `:2595`.
- STD’s tournament loop pumps frames through bracket/match/result states: `src/std/std_setup.c:521–605`; frame helpers `src/std/std_main.c:6185`, `:6194`.
- VS frame helper: `src/vs/vs_main.c:996`.
- Movies have an independent decode/read/output loop: `src/mov/mov.c:244–265`.

World ticks execute callbacks in **slot order**, once per `tickObjects` call: `src/main/world_object.c:82–89`. Clock/care semantics therefore depend on update ordering, not just elapsed VBlank time.

### Every `VSync` call: 15 sites

| Reference | Argument | Context |
|---|---:|---|
| `src/main/btl.c:153` | 3 | Battle-loading frame pump |
| `src/main/main.c:3656` | 0 | Landing screen |
| `src/main/main.c:3675` | 0 | Landing-screen fade |
| `src/main/main.c:3712` | 0 | Main menu |
| `src/main/main.c:3761` | 3 | New-game scene |
| `src/main/main.c:3958` | 3 | Gameplay |
| `src/main/utils.c:86` | 3 | `pauseFrame` |
| `src/btl/battle_main.c:2606` | 3 | Battle frame |
| `src/fish/fish.c:2975` | -1 | Population update gate: `/3 %20` |
| `src/fish/fish.c:3067` | -1 | Bite selection gate: `/3 %8` |
| `src/fish/fish.c:3073` | -1 | Alternate bite gate: `/3 %20` |
| `src/mov/mov.c:131` | 3 | Stream-start retry |
| `src/mov/mov.c:257` | 0 | Movie presentation |
| `src/std/std_main.c:6205` | 3 | Tournament battle frame |
| `src/vs/vs_main.c:1008` | 3 | VS frame |

DW2-PC implements `VSync(0)` as the next VBlank, positive multi-field waits against the previous wait, and negative calls as counter reads: `ref/DW2-PC/psyq/libetc.c:25–44`. That implementation is a reference; exact DW1 SDK return/counter/reset semantics are **unverified**.

There are **zero game-C `VSyncCallback` registrations/handlers** in the active US source inventory. The SDK contains those functions: `config/us/symbols.txt:88–89`; their internal use is outside the verified C surface.

### DrawSync: 134 sites, all `DrawSync(0)`

These synchronize GPU work before presentation, reuse of uploaded/stored image buffers, or dependent image operations.

Complete site inventory:

| File | Count | Lines |
|---|---:|---|
| `src/main/battle_ui.c` | 1 | `528` |
| `src/main/btl.c` | 1 | `152` |
| `src/main/efe.c` | 2 | `{983,1012}` |
| `src/main/effect_common.c` | 4 | `{359,404,417,453}` |
| `src/main/game_menu.c` | 3 | `{4077,4095,4109}` |
| `src/main/inventory.c` | 2 | `{315,844}` |
| `src/main/main.c` | 5 | `{3655,3674,3711,3760,3957}` |
| `src/main/main_menu.c` | 69 | `{2892,2898,2901,2906,2913,2920,2922,2927,2929,2931,2933,2935,2940,2958,2962,2964,2966,2982,2984,2986,2988,2990,2992,2994,2996,3004,3017,3026,3028,3031,3044,3046,3052,3054,3056,3058,3067,3070,3073,3076,3079,3082,3085,3088,3091,3094,3101,3107,3110,3113,3115,3117,3119,3121,3123,3129,3131,3137,3146,3148,3153,3155,3161,3163,3165,3195,3197,3199,3283}` |
| `src/main/map.c` | 6 | `{4419,4429,4441,4601,5229,5270}` |
| `src/main/map_object.c` | 1 | `1202` |
| `src/main/model.c` | 1 | `3577` |
| `src/main/overworld.c` | 3 | `{387,390,404}` |
| `src/main/utils.c` | 1 | `85` |
| `src/btl/battle_effect.c` | 1 | `2770` |
| `src/btl/battle_main.c` | 1 | `2605` |
| `src/doo2/doo2.c` | 3 | `{360,369,407}` |
| `src/dooa/dooa.c` | 5 | `{671,680,689,1264,1302}` |
| `src/endi/endi.c` | 3 | `{102,112,277}` |
| `src/evl/evl.c` | 3 | `{280,289,298}` |
| `src/fish/fish.c` | 5 | `{1242,1357,1518,1523,1525}` |
| `src/mov/mov.c` | 2 | `{242,271}` |
| `src/murd/murd.c` | 1 | `356` |
| `src/std/std_setup.c` | 1 | `449` |
| `src/std/std_main.c` | 3 | `{1619,4709,6204}` |
| `src/std/std_effect.c` | 1 | `2928` |
| `src/trn2/trn2_hud.c` | 1 | `392` |
| `src/trn/trn_hud.c` | 1 | `189` |
| `src/vs/vs_combat.c` | 1 | `1521` |
| `src/vs/vs_effect.c` | 1 | `2511` |
| `src/vs/vs_intro.c` | 1 | `193` |
| `src/vs/vs_main.c` | 1 | `1007` |

Of the menu sites, 65 belong to `drawMainMenuStrings`; three to save-slot text; one to registered-Digimon text: `src/main/main_menu.c:2892`, `:3195`, `:3283`.

### Game clock and care timers

`addClock` installs separate game-clock and playtime objects: `src/main/clock.c:90–93`.

`tickGameClock`, `:96`:

- Records `LAST_HANDLED_FRAME = CURRENT_FRAME` before its gates: `:100`.
- Stops during other game states, fishing, disabled map layer, or stopped game time: `:102–106`.
- Normal speed adds one; fast speed adds two and aligns to an even frame; slow speed adds one every second subframe; speed 3 stops: `:114–130`.
- **20 game ticks per game minute**, 1200 per hour: `:133–139`.
- Hourly evolution timer increments and lifetime decreases: `:140–141`.
- Midnight increments age/day and executes daily triggers; 30 days advance the year: `:143–158`.
- `advanceToTime` changes date/frame state directly; it does not simulate every omitted care tick: `:194–209`.

`tickPlaytime` increments once per object update, independently of the game-clock stop gates. It records a minute every **1200 updates**, then hours, capped at 999: `src/main/clock.c:174–190`.

Normal partner processing invokes care and poop processing: `src/main/partner.c:268–274`. `tickConditions` rejects updates when game/partner state is unsuitable, game time is stopped, no game frame changed, or fade protection is active: `src/main/partner_impl.c:2072–2074`.

| Timer/mechanic | Advancement | Reference |
|---|---|---|
| Sleep tiredness | Every 20 changed game ticks | `src/main/partner_impl.c:923–926` |
| Sleep adjustment | Every 200 changed ticks, subject to threshold | `:931–934` |
| Missed-sleep penalties | 200/300/1200 changed-tick gates; hourly missed-sleep counters | `:955–976` |
| Unhappiness | Chance every 200; recovery every 15; stat loss every 300 changed ticks | `:1050`, `:1062`, `:1067` |
| Tiredness hunger accumulation | Every eligible care update | `:1411` |
| Tiredness happiness loss | Every 100 changed ticks | `:1417` |
| Starvation | Decrements every 10 changed ticks | `:1500–1502` |
| Food | Decrements every 20 changed ticks | `:1503–1505` |
| Energy | Decrements every 1200 changed ticks | `:1508–1510` |
| Empty stomach | Increments every 20 changed ticks | `:1537–1539` |
| Weight loss | `%200` gate without an internal `LAST_HANDLED_FRAME` check | `:1532–1533` |
| Poop level | Decrements every 200 changed ticks | `:1666–1668` |
| Pooping timeout | Decrements once per changed game tick while poopy | `:1677–1678` |
| Sickness/injury | Hourly chance/penalty/timer advancement | `:1841–1882` |
| Area-effect timer | Once per eligible care update; wraps above 28800 | `:2082–2085` |
| Training boost | Decrements once per eligible care update | `:2087–2090` |
| Unhappy lifetime penalty | Every 4800 game ticks | `:2092–2094` |

Fast clock speed does not generally execute care processing twice. Preserve the original counter gates and object-update ordering.

### Root counters, waits, and busy loops

No direct active-US game-C calls to `SetRCnt`, `GetRCnt`, `ResetRCnt`, `StartRCnt`, or `StopRCnt` were found. SDK root-counter symbols exist: `config/us/symbols.txt:35–36`.

Music explicitly selects **`SS_TICK240`** and starts the SDK scheduler: `src/main/sound.c:524`, `:534`. DW2-PC implements that mode using root counter 2 and an interrupt callback: `ref/DW2-PC/psyq/libsnd.c:98–159`. DW1’s exact implementation is **unverified**; a VBlank-only sequencer replacement must not assume the game requested VSync ticking.

Important CPU-counted or polling waits:

| Wait | Reference |
|---|---|
| Movie frame-acquisition retry, 2000 iterations | `src/mov/mov.c:182–187` |
| Movie decode retry, 2000 iterations | `src/mov/mov.c:140–145` |
| Movie flip timeout, `0x800000` iterations, forcibly changes frame state | `src/mov/mov.c:159–169` |
| Stream command/read retries, with `VSync(3)` | `src/mov/mov.c:126–132` |
| Movie restart/recovery loops | `src/mov/mov.c:234–237`, `:247–254` |
| Whole-file/subsection CD command/read waits | `src/main/file.c:86–94`, `:142–149` |
| Script CD command/read waits | `src/main/script_interp.c:2801–2818` |
| EFE CD/search waits | `src/main/efe.c:1023`, `:1030` |
| EFEX lookup waits | `src/main/efe_table.c:31`, `:38` |
| Queue-capacity wait pumps the queue directly | `src/main/file_queue.c:209–225` |
| Sound-loading wait pumps the queue directly | `src/main/sound_async.c:314–315` |
| Pause wait polls pad continuously, without a VSync inside that loop | `src/main/utils2.c:425–431` |
| Blocking card synchronization | For example `src/main/main_menu.c:3749`, `:3756`, `:4833` |

Movie timeout duration therefore changes with CPU speed if ported literally.

## 4. Script engines

### Main SCN engine

Entry points:

- Initialization: `initializeScripts`, `src/main/script_interp.c:2190`.
- Map-head execution: `runMapHeadScript`, `:2235`.
- Section activation: `callScriptSection`, `:2244`.
- Per-update progression: `tickScript`, `:390`; called from gameplay at `src/main/main.c:3944`.

Dispatch is an opcode-range chain, then switches in six handlers:

| Range | Handler reference |
|---|---|
| `FB–FF` | `src/main/script_interp.c:2833` |
| `10–27` | `:692` |
| `28–3F` | `:1082` |
| `46–58` | `:1300` |
| `5A–5F` | `:1553` |
| `64–7E` | `:1588` |

The dispatch chain itself is `src/main/script_interp.c:603–618`. Unsupported opcodes pause the interpreter and jump out.

Addressing:

- Global MAPHEAD and one replaceable map-script buffer: `src/main/script_interp.c:{349,355}`.
- Offset directory: `:351`.
- Script selection/file read: `getScript`, `:647–667`.
- Section offsets are resolved against the current script base: `:676–680`.
- Jumps/switches use u16 offsets: `:719–720`, `:740–741`, `:755–757`.
- Cross-file calls save a runtime return pointer and script ID; return reloads the appropriate script buffer and restores the pointer: `:724–736`.
- No SCN offset directory is relocated in place.

Stacks/state:

- Eight `StackEntry` return records: `include/dw/script.h:152–164`.
- Each entry is `scriptPtr:pointer32`, `scriptId:u16`, two control bytes.
- Push has no explicit overflow check: `src/main/script_interp.c:2368–2374`.
- Pop decrements the index; empty pop sets only the control type: `:2383–2396`.
- Other resident state includes 22 scripted-movement slots, `0x540` text-line bytes, and `jmp_buf`: `src/main/script_interp.c:{361,363,364}`.
- Selection state retains runtime pointers into script data: `src/main/script_textbox.c:1028`, `:1041`.
- Return-stack pointers are excluded from the card-save prefix: `src/main/main_menu.c:5120`.

`setjmp`/`longjmp`:

- One `setjmp` site: `src/main/script_interp.c:601`.
- Jump result 1 retries dispatch; result 2 yields/stops; result 3 initiates map-change handling: `:620–630`.
- There are **37 source `longjmp` sites**, including one matching-only garbage function at `:1076`.
- A notable native-ABI/lifetime concern: `tickScript` contains a `longjmp` at `:520` **before** the current invocation reaches `setjmp` at `:601`. Whether that path can target an environment saved by a previously returned invocation requires verification.

All `longjmp` references:

`src/main/script_interp.c:{520,618,708,770,840,843,921,948,958,1076,1293,1401,1412,1543,1546,1581,1681,1729,1752,1756,1759,1764,1773,1799,1832,1837,1864,1868,1940,1957,1961,2137,2178,2706,2715,2719,2854}`.

### EFE interpreter

This is a separate script engine, duplicated in BTL/STD/VS:

- BTL execution/dispatch: `src/btl/battle_effect.c:5336`, `:5341`.
- Initialization can execute four instructions per continuation callback: `:1830–1838`.
- Instructions are 16-bit words; low eight bits select the opcode: `:5348–5349`.
- Relative addresses are added to the EFE data base or current instance: `:4693`, `:4746–4748`, `:4788–4792`.
- Subroutine calls push context and return pointers: `:4776–4779`.

Resident stacks are **32 s32 operand slots** and **16 s32 call-stack slots**: `src/main/efe.c:309`, `:314`. `EFE_PUSH` explicitly narrows values—including pointers—to s32: `include/dw/efe.h:14–22`. Stack reset is at `src/btl/battle_effect.c:5344–5346`.

This interpreter uses its explicit stacks, not `setjmp`/`longjmp`.

## 5. Randomness

### Sources and seeding

| Source | Behaviour | Reference |
|---|---|---|
| `customRandom` | Two u32 states each advance by `state×0x41C650AD + 0x3039`, wrapping at 32 bits. Combines halves and applies modulo to an inclusive range. Equal endpoints return without advancing. | `src/main/rng.c:13–31` |
| Custom initial state | `CUSTOM_RNG_VAL2 = 0x0013CC25`; `CUSTOM_RNG_VAL1` starts as resident zero-initialized state | `src/main/rng.c:8`, `:11`; startup clearing `src/main/_psstart.c:87–108` |
| SDK `rand()` | Called directly and through `randomLimit`; SDK implementation/state is absent | SDK symbol `config/us/symbols.txt:71`; linked SDK section `config/us/main.ld:23` |
| `randomLimit(limit)` | `(limit * rand()) >> 15`; assumes the SDK’s 15-bit random result | `src/main/math.c:5–10` |
| Fixed wireframe table | A constant permutation, not a third evolving RNG | `src/main/model.c:2501–2504`; uses `:3177`, `:3217`, `src/doo2/doo2.c:{459,501}`, `src/dooa/dooa.c:1611` |

**Seeding sites found:** zero `srand()` calls; zero explicit reseeding assignments to custom state outside its initializers and recurrence. No timer/VSync-derived RNG seed was found in active US C.

Fishing does depend on VSync counter values when deciding **when to make random decisions**, which changes RNG consumption timing: `src/fish/fish.c:{2975,3067,3073}`.

The initial state and exact recurrence of **DW1’s SDK `rand()` remain unverified**. Using macOS libc’s `rand()` would not establish equivalence.

### Counts

- **154 direct `rand()` call sites**, including the one inside `randomLimit`.
- **168 `randomLimit()` caller sites**.
- **57 `customRandom()` caller sites**.
- These are static source-site counts, not execution counts.
- The wrapper’s `rand()` site is not an independent additional stream.

### Every direct `rand()` site

| File | Count | Lines |
|---|---:|---|
| `src/main/efe.c` | 21 | `{359,360,361,363,388,389,390,391,392,393,394,395,402,403,404,405,522,523,626,651,653}` |
| `src/main/math.c` | 1 | `9` |
| `src/main/model.c` | 1 | `3156` |
| `src/main/particle.c` | 4 | `{136,148,150,153}` |
| `src/btl/battle_effect.c` | 28 | `{1931,1932,1933,2822,4480,4481,4482,4930,5382,5383,5561,5562,5563,5909,5999,6001,6002,6003,6209,6210,6369,6374,6376,6688,6690,6691,6692,6693}` |
| `src/doo2/doo2.c` | 2 | `{433,583}` |
| `src/dooa/dooa.c` | 17 | `{514,515,516,1092,1093,1094,1199,1200,1336,1337,1338,1357,1361,1363,1392,1611,1764}` |
| `src/endi/endi.c` | 7 | `{295,296,297,397,398,399,401}` |
| `src/evl/evl.c` | 15 | `{439,522,523,524,611,612,613,653,654,655,973,974,975,1260,1313}` |
| `src/kar/kar.c` | 14 | `{701,708,719,755,757,2375,2377,2412,2414,2417,2435,2437,2653,2817}` |
| `src/std/std_effect.c` | 18 | `{2036,2037,2038,2980,4669,4670,4671,5119,5576,5577,5755,5756,5757,6103,6193,6195,6196,6197}` |
| `src/trn2/trn2_slots.c` | 4 | `{342,362,407,409}` |
| `src/trn/trn_slots.c` | 4 | `{342,362,407,409}` |
| `src/vs/vs_effect.c` | 18 | `{1667,1668,1669,2563,4252,4253,4254,4704,5161,5162,5340,5341,5342,5687,5777,5779,5780,5781}` |

### Every `randomLimit()` caller

| File | Count | Lines |
|---|---:|---|
| `src/main/battle_ui.c` | 6 | `{159,188,200,217,264,273}` |
| `src/main/btl.c` | 2 | `{305,335}` |
| `src/main/evolution.c` | 1 | `879` |
| `src/main/item.c` | 3 | `{2941,2945,3045}` |
| `src/main/overworld.c` | 8 | `{368,370,688,716,784,785,813,814}` |
| `src/main/partner.c` | 3 | `{907,925,931}` |
| `src/main/partner_impl.c` | 11 | `{633,1056,1265,1361,1369,1443,1475,1477,1749,1812,1846}` |
| `src/main/script_common.c` | 3 | `{1368,1413,2512}` |
| `src/main/script_interp.c` | 1 | `822` |
| `src/main/tamer.c` | 1 | `1316` |
| `src/main/utils2.c` | 2 | `{634,646}` |
| `src/shop/script_menu.c` | 2 | `{1422,1445}` |
| `src/btl/battle_hud.c` | 8 | `{538,556,563,569,571,573,575,577}` |
| `src/btl/battle_main.c` | 27 | `{333,440,548,642,1053,1590,1592,1820,1979,2019,2027,2029,2140,2170,2194,2798,2882,3030,3162,3174,3182,3551,3621,3625,3700,3730,3857}` |
| `src/std/std_setup.c` | 9 | `{315,316,317,318,319,320,370,385,387}` |
| `src/std/std_main.c` | 37 | `{2462,2471,2493,3471,3475,3487,3500,3506,3763,3871,4335,4422,4425,4494,4497,4835,5370,5372,5570,5764,5770,5799,5823,5998,6006,6008,6088,6100,6108,6311,6315,6390,6545,6807,6866,6945,7164}` |
| `src/std/std_hud.c` | 8 | `{572,590,597,603,605,607,609,611}` |
| `src/std/std_effect.c` | 1 | `1411` |
| `src/trn/trn_reward.c` | 2 | `{140,863}` |
| `src/vs/vs_camera.c` | 3 | `{366,375,392}` |
| `src/vs/vs_combat.c` | 7 | `{996,1206,1209,1658,2220,2226,2415}` |
| `src/vs/vs_hud.c` | 8 | `{768,786,793,799,801,803,805,807}` |
| `src/vs/vs_main.c` | 15 | `{374,383,465,477,485,613,617,641,665,984,1178,1257,1337,1669,1739}` |

### Every `customRandom()` caller

| File | Count | Lines |
|---|---:|---|
| `src/btl/battle_effect.c` | 4 | `{2572,2594,2595,3587}` |
| `src/doo2/doo2.c` | 4 | `{568,574,575,576}` |
| `src/dooa/dooa.c` | 1 | `1198` |
| `src/evl/evl.c` | 4 | `{1246,1251,1252,1253}` |
| `src/fish/fish.c` | 32 | `{1580,1594,1597,1962,1963,2387,2388,2389,2391,2623,2624,2699,2700,2735,3089,3139,3142,3184,3190,3252,3256,3260,3263,3312,3317,3331,3332,3333,3343,3427,3993,3994}` |
| `src/std/std_main.c` | 2 | `{2828,2843}` |
| `src/std/std_effect.c` | 4 | `{2730,2752,2753,3769}` |
| `src/vs/vs_camera.c` | 2 | `{729,744}` |
| `src/vs/vs_effect.c` | 4 | `{2313,2335,2336,3352}` |