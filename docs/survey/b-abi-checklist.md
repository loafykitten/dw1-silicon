# Task B: native ABI and compiler checklist

The largest risks are the effect interpreters’ 32-bit addresses, pointer-bearing records accessed directly inside loaded files, and Psy-Q’s 32-bit `long` layouts. A native pointer window alone does not solve the record-layout problems.

## Scope and counting

- Surveyed the **120 C files** listed in `mk/version/us.mk:20–212`, plus local headers, at decomp revision `09c174c`.
- Selected `VERSION_US`: version ID 4, date 19990331, NTSC-U. The actual predicates are `VERSION_IS`, `VERSION_REGION_IS`, and the date comparisons; there is no `VER_US` selector here. References: `include/dw/version.h:8,15,20,35–38,51–54`.
- Considered the original CodeWarrior branches and the alternative GCC branches separately.
- No files were modified. Nothing under `disc/` was read.
- Paths below are relative to `ref/dw_decomp`, except explicitly identified DW2-PC paths. Ranges are inclusive.
- Counts generally count **distinct source lines**, not individual casts or macro expansions on the same line. Categories overlap.

**Verification limitation:** `external/psyq_headers` is empty in this checkout. Its declaration is at `.gitmodules:7–9`; the US include selection is at `mk/version/us.mk:3`. Consequently, SDK typedefs, SDK prototype compatibility, implicit declarations involving SDK functions, and exact SDK-dependent layouts remain **unverified**. A syntax check stopped at the missing `libcd.h` in `src/main/file_queue.c:1`.

The large inventories below are complete for their stated source patterns. They are deliberately broader than confirmed defects: a shift, cast, or `long` declaration is not automatically a bug. This is not an exhaustive proof that the program contains no other undefined behavior.

| Category | Counted inventory |
|---|---:|
| `long` / `u_long` | 521 game/local-header lines, plus 13 lines in used CodeWarrior GTE macros |
| Pointer conversions | 464 explicit EFE conversion lines; 353 pointer stack macro sites; 159 additional EFE address-restoration lines; 175 related sites outside those interpreters |
| Literal pointer initializers | 335 US BSS lines; 24 fixed-address macro definitions; 5 ordinary null initializers; 3 offset calculations using a null base |
| Copy / size expressions | 89 lines, including two `memcpy` declarations |
| Shifts | 805 lines |
| Cast-dereference / loaded-buffer cast candidates | 554 lines |
| Union definitions | 9 |
| K&R definitions | 233 |
| Empty parameter-list declarations/definitions | 197 lines |
| Written function-signature discrepancies | 145 names among ordinary declarations; 339 including K&R parameter declarations, at 1,212 reference lines |
| Nonlocal jumps | 1 `setjmp`, 37 `longjmp`, 1 game `jmp_buf` definition |
| Direct GTE macro calls | 181 lines, using 11 locally implemented macros |
| Floating-point declarations / literals | 0 / 0 |

## 1. `long`, `unsigned long`, `u_long`, and 4-byte assumptions

### Confirmed width-sensitive mechanisms

| Mechanism | References | Native consequence |
|---|---|---|
| MMD header offsets read through `long *` | `src/main/model.c:3415,3416,3447,3448,3725,3726` | Reads 8-byte fields and uses an 8-byte table stride. |
| Animation table / vector access through `long *` | `include/dw/entity.h:55`; `include/dw/model.h:25`; `src/main/anim.c:135,136`; `src/main/tamer.c:1203` | Animation offsets and vector components must retain their original widths. |
| TFS word traversal | `src/main/map.c:4227,4234` | `data++`, `data += 128`, and `data += 4096` become twice the intended byte distances. |
| TIM header skipping with `u_long * + 1` | `src/main/file.c:105,123`; `src/main/model.c:3562,3713`; `src/dooa/dooa.c:549` | Skips 8 bytes instead of 4. |
| Packed screen coordinates passed as `long *` | `src/main/aabb.c:30`; `src/main/drop_shadow.c:64,65`; `src/main/effect_common.c:475–477,493,494`; full inventory below | A shim using native `long` can overwrite adjacent coordinate/packet fields. |
| Packed radius reinterpreted as `long` | `src/btl/battle_effect.c:2632,2634,2635`; `src/std/std_effect.c:2790,2792,2793`; `src/vs/vs_effect.c:2373,2375,2376` | Reads neighboring bytes as part of the value. |
| Initialized word arrays | `src/doo2/doo2_bss_us.c:15,27,79`; `src/evl/evl_bss_us.c:399,451,1594`; `src/kar/kar_bss_us.c:349`; `src/mov/mov_bss_us.c:17,10260,20503,21226,21949` | Doubles the arrays’ byte lengths and changes their serialized representation. |
| Scratch/runtime struct fields declared `long` | `include/dw/btl.h:14`; `include/dw/dooa.h:19`; `include/dw/efe.h:198,202–205,212–214,242`; `include/dw/item.h:30`; `src/fish/fish.c:38,40,41,54` | Changes field offsets and struct alignment. |
| Card API output words | `src/main/main_menu.c:58,3324,4761,4796` | `MemCardSyncWord` is `unsigned long` for US. A fixed-width shim must not leave an 8-byte local half initialized. |
| RNG bound arithmetic | `src/main/rng.c:13,30`; `include/dw/rng.h:6` | `long` changes the arithmetic conversions and range arithmetic; the RNG state itself remains `uint32_t`. |
| Linker/runtime address words | `include/__rts_info_t__.h:19–39,42–45,50,55`; `src/main/_psstart.c:29`; `src/main/main.c:3534,3535,3539` | These describe the original runtime and should not become native linker/address schemas unchanged. |

There are **249 lines with `long`/`u_long` pointer or word-array exposure** within the 521-line inventory. Ordinary bounded loop indices and small counters are not established 32-bit-wrap dependencies; their width risk is **unverified**, and many can safely become `int32_t`.

**Typedefs:** no active typedef mapping to `long` was found in the available local `include/` headers. The one local source typedef is `MemCardSyncWord` at `src/main/main_menu.c:58`. SDK `u_long` and other SDK typedef definitions are **unverified** because the submodule is absent.

`include/dw/types.h:10–19` uses `int`, rather than `long`, for the CodeWarrior 32-bit integer and pointer-integer typedefs. The non-CodeWarrior branch uses `<stdint.h>` at `include/dw/types.h:23–24`.

### Complete `long` / `u_long` inventory

This includes declarations, API crossings, layout fields, ordinary arithmetic uses, and integer-address uses. It excludes comments and the intentional `long long` at `src/main/utils.c:270`.

```text
src/main/_psstart.c:29
src/main/aabb.c:19,20,30
src/main/anim.c:135,136
src/main/battle_ui.c:132,301
src/main/btl.c:225
src/main/buff_model.c:83
src/main/drop_shadow.c:25,26,64,65
src/main/efe.c:165,256,259,306,978,986,1011
src/main/effect_common.c:25-27,358,364,403,407,420,446,449,452,470,471,475-477,487,488,493,494,592
src/main/entity_text.c:164
src/main/file.c:55,59,62,92,105,123
src/main/file_queue.c:144
src/main/font.c:1510,1697
src/main/game_menu.c:247,2383
src/main/inventory.c:35,71,306,949,996
src/main/main.c:75,132,133,3534,3535,3539
src/main/main_menu.c:58,171,279,3233,3324,4761,4796
src/main/map.c:37,90-92,174,176,2910,3070,3078,3135,3664,3745,3746,3895,3962,4023-4028,4227,4234,4295,4416,4426,4438,4560,4600,4814,4882,4885,5047,5206,5226,5248,5267
src/main/map_object.c:300,301,453,459,510,516,666,667,674,675,687,692,1060
src/main/model.c:2839,3133-3135,3181,3182,3199,3200,3222,3241,3415-3417,3447-3449,3562,3685,3713,3725-3727
src/main/overworld.c:59,151,152,380,394,398,970,977,983
src/main/particle.c:183,192
src/main/partner_impl.c:440,441
src/main/rng.c:13
src/main/script_textbox.c:981
src/main/sound.c:45,390,445,485
src/main/sound_async.c:132,174
src/main/tamer.c:1124,1200,1203,1282
src/main/utils.c:22,125,191
src/main/utils2.c:46,248,501
src/btl/battle_effect.c:45,243,295,1738,1744,1754,2366,2632,2634,2635,2658,2659,2738,2769,2797,2958,2960,3798,3986,3988,4604,5207,5286,5287,5537,5594,5962,5963,5972,5984,6343
src/btl/battle_hud.c:548
src/btl/battle_main.c:138,189,517,650,708,744,1992,2086,2087,2244,2773,3617
src/btl/battle_setup.c:34,211
src/btl/command_menu.c:518
src/doo2/doo2.c:41,165,354,359,363,368,406,420-422,462,488,504,505,533,534,597,601,602
src/doo2/doo2_bss_us.c:15,27,79
src/dooa/dooa.c:105,107,109,133,138,306,320,321,327,547,549,550,602,670,679,688,791,861,1144,1183,1263,1301,1559
src/eab/eab.c:657
src/endi/endi.c:20,21,26,37,38,71,100,105,110,234,423,424
src/evl/evl.c:54,55,278,288,297,634,676,998
src/evl/evl_bss_us.c:399,451,1594
src/fish/fish.c:38,40,41,54,118,123,136,964,1086,1686,1687
src/fish/fish_model.c:43,47
src/kar/kar.c:61,192,733,850,1529,1961
src/kar/kar_bss_us.c:349
src/mov/mov.c:24,138,175,177,184
src/mov/mov_bss_us.c:17,10260,20503,21226,21949
src/murd/murd.c:48,345,354,793
src/std/std_setup.c:68,511,527,528,628,648,650
src/std/std_main.c:106,286,3216,3468,3947,3948,4303,4433,4437,5713,5716,5971,6307,6701,6841
src/std/std_hud.c:582,1145,1148,1229,1251,1253,1275,1297,1305,1354,1355
src/std/std_bss_us.c:225,231,237,243
src/std/std_effect.c:82,235,304,306,307,1426,1843,1849,1859,2534,2790,2792,2793,2816,2817,2896,2927,2955,3116,3118,3980,4168,4170,4793,5401,5480,5481,5731,5788,6156,6157,6166,6178
src/vs/vs_bss_us.c:212,218,224,230
src/vs/vs_combat.c:285,788,789,1096,1145,1149,1570,2663
src/vs/vs_effect.c:43,209,259,268,270,271,1474,1480,1490,2117,2373,2375,2376,2399,2400,2479,2510,2538,2699,2701,3563,3751,3753,4378,4986,5065,5066,5316,5372,5740,5741,5750,5762
src/vs/vs_hud.c:778,1326,1329,1412,1436,1438
src/vs/vs_main.c:90,352,567,1153
src/vs/vs_scene.c:43,1688,1690,1821,1841
src/vs/vs_select.c:483,659,660
include/__rts_info_t__.h:19-39,42-45,50,55
include/dw/btl.h:14,108
include/dw/doo2.h:35-37,43,50,51
include/dw/dooa.h:19
include/dw/efe.h:198,202-205,212-214,242,342,344
include/dw/endi.h:33,34
include/dw/entity.h:55
include/dw/item.h:30
include/dw/kar.h:155,172
include/dw/model.h:25
include/dw/mov.h:9,11,24-28
include/dw/rng.h:6
include/dw/sound.h:19
```

The **13 additional `long` conversions in used CodeWarrior GTE macros** are:

```text
include/mwcc/inline_n.h:46,67-69,291,374,437,1083,1089-1091,1273,1512
```

DW2-PC provides a useful width convention: it substitutes `psyq_u_long` and defines it as `unsigned int`, explicitly separating it from host `u_long`. References: `ref/DW2-PC/psyq/psyq_types.h:4–18`. Its header/API coverage is a separate question.

## 2. Pointer ↔ integer casts, narrowing, and interpreter stacks

Classification:

- **S — stored-then-restored:** a real address enters an integer representation and is subsequently used as an address. Retaining a 32-bit representation requires a handle or a window with explicit encode/decode.
- **C — compared/hashed only:** no dereference through the narrowed value is established.
- **A — address arithmetic:** arithmetic or offsets are applied to integer representations of addresses.

A native `long` cast itself no longer narrows on this Mac, but an unchanged Psy-Q address API or subsequent 32-bit field can still narrow it. Those SDK crossings are **unverified** until the shim signatures are fixed.

### Effect interpreter representations

The storage is explicitly 32-bit:

```text
include/dw/efe.h:14,15,17,18,21,22,175,337,350,357,362
include/dw/doo2.h:23-25
include/dw/model.h:41
```

`EFE_PUSH` at `include/dw/efe.h:15` casts the value to `int32_t`; `EFE_POP` at line 14 casts the stored integer back to the requested type.

### S: 353 pointer-valued stack macro sites

These include pointers to **automatic locals**, particularly vectors and colors. Moving only game heap/global storage into a window would not cover them.

```text
src/main/efe.c:771,772,824,825,830
src/dooa/dooa.c:509,518,519,1085,1096,1097
src/btl/battle_effect.c:2213,2230,2273,2274,2316,2372,2373,2443,2483-2485,2487,2568,2591,2612,2696,2710,2724,2755,2776,2809,2853-2856,2872,2884,2896,2928-2932,3002-3004,3092-3094,3153,3225,3226,3282,3306,3307,3321,3322,3337,3338,3355,3356,3413,3414,3425,3426,3437,3438,3449,3450,3461,3462,3473,3474,3483,3484,3493,3494,3511,3512,3527,3529,3564,3565,3586,3605,3633,3645,3657,3684,3688,3787,3805-3807,3837,3882,3938,3939,4079,4108,4112,4134,4181-4184,4244,4255,4298,4337,4362,4383,4434,4438,4439,4462,4585,4665,4756,4758,4777,4778,5232,5233
src/std/std_effect.c:2374,2391,2441,2442,2484,2540,2541,2611,2651-2653,2655,2726,2749,2770,2854,2868,2882,2913,2934,2967,3011-3014,3030,3042,3054,3086-3090,3160-3162,3255-3257,3316,3407,3408,3464,3488,3489,3503,3504,3519,3520,3537,3538,3595,3596,3607,3608,3619,3620,3631,3632,3643,3644,3655,3656,3665,3666,3675,3676,3693,3694,3709,3711,3746,3747,3768,3787,3815,3827,3839,3866,3870,3969,3987-3989,4019,4064,4120,4121,4261,4290,4294,4316,4363-4366,4433,4444,4487,4526,4551,4572,4623,4627,4628,4651,4774,4854,4945,4947,4966,4967,5426,5427
src/vs/vs_effect.c:1957,1974,2024,2025,2067,2123,2124,2194,2234-2236,2238,2309,2332,2353,2437,2451,2465,2496,2517,2550,2594-2597,2613,2625,2637,2669-2673,2743-2745,2838-2840,2899,2990,2991,3047,3071,3072,3086,3087,3102,3103,3120,3121,3178,3179,3190,3191,3202,3203,3214,3215,3226,3227,3238,3239,3248,3249,3258,3259,3276,3277,3292,3294,3329,3330,3351,3370,3398,3410,3422,3449,3453,3552,3570-3572,3602,3647,3703,3704,3844,3873,3877,3899,3946-3949,4016,4027,4070,4109,4134,4155,4206,4210,4211,4234,4357,4439,4530,4532,4551,4552,5011,5012
```

Additional direct stack stores outside those macros are S:

```text
src/eab/eab.c:696,703,704
src/evl/evl.c:445,452,453,510,526,527
```

### Explicit EFE conversions: 464 lines

**C: 12 comparison-only lines.** The ordered comparisons still depend on the signed interpretation of a truncated address.

```text
src/btl/battle_effect.c:1487,1533,4506,4718
src/std/std_effect.c:1640,1686,4695,4907
src/vs/vs_effect.c:1223,1269,4278,4492
```

**S: 69 stores, address-valued arguments, or round trips.**

```text
src/btl/battle_effect.c:1489-1491,1535,1536,1584,1645,1792,2738,2797,4604,4653,4724,4744,4760,4786,4803,4825,4834,4881,4966,5536,5983
src/std/std_effect.c:1642-1644,1688,1689,1750,1897,2337,2896,2955,4793,4842,4913,4933,4949,4975,4992,5014,5023,5070,5155,5730,6177
src/vs/vs_effect.c:1225-1227,1271,1272,1320,1381,1528,2479,2538,4378,4427,4498,4518,4534,4560,4577,4599,4608,4655,4740,5315,5761
```

**A: 383 arithmetic/access lines.**

```text
src/btl/battle_effect.c:1654,1670,1801,1807,1814-1816,1890,1891,1939-1941,2048,2126,2133,2134,2136,2139,2144,2280-2282,2338,2339,2375,2376,2401,2411,2613-2618,2726,2729-2731,2778-2780,2783-2785,2960,3007-3009,3358-3362,3378,3382-3384,3531-3533,3690,3695,3720,3724,3742,3744,3745,3765,3771,3809,3810,3838,3843,3855,3988,4363,4384,4386,4414-4417,4468,4502-4504,4532,4568,4574,4587,4590-4592,4673,4699,4737,4746,4751,4767,4790,4795,4805,4818,4827,4852,4858,4864,4873,4883,4888,4899,4905,4911,4925,4931,4941,4947,4953,4959,4968,5262,5265-5267,5837,6127,6129
src/std/std_effect.c:1759,1775,1906,1912,1919-1921,1995,1996,2044-2046,2153,2231,2238,2239,2241,2244,2249,2448-2450,2506,2507,2543,2544,2569,2579,2771-2776,2884,2887-2889,2936-2938,2941-2943,3118,3165-3167,3540-3544,3560,3564-3566,3713-3715,3872,3877,3902,3906,3924,3926,3927,3947,3953,3991,3992,4020,4025,4037,4170,4552,4573,4575,4603-4606,4657,4691-4693,4721,4757,4763,4776,4779-4781,4862,4888,4926,4935,4940,4956,4979,4984,4994,5007,5016,5041,5047,5053,5062,5072,5077,5088,5094,5100,5114,5120,5130,5136,5142,5148,5157,5456,5459-5461,6031,6321,6323
src/vs/vs_effect.c:1390,1406,1537,1543,1550-1552,1626,1627,1675-1677,1784,1862,1869,1870,1872,1875,1880,2031-2033,2089,2090,2126,2127,2152,2162,2354-2359,2467,2470-2472,2519-2521,2524-2526,2701,2748-2750,3123-3127,3143,3147-3149,3296-3298,3455,3460,3485,3489,3507,3509,3510,3530,3536,3574,3575,3603,3608,3620,3753,4135,4156,4158,4186-4189,4240,4274-4276,4304,4340,4346,4359-4361,4364-4366,4447,4473,4511,4520,4525,4541,4564,4569,4579,4592,4601,4626,4632,4638,4647,4657,4662,4673,4679,4685,4699,4705,4715,4721,4727,4733,4742,5041,5044-5046,5615,5905,5907
```

### Additional EFE integer-address restoration: 159 lines

These consume integer-address variables or `EFE_SCRIPT_HEAD`. Direct restoration is S; expressions adding script/data offsets are A. The 72 variable-operator accesses are S endpoints.

```text
src/btl/battle_effect.c:3249,3254,3257,3261,3264,4055,4060,4063,4066,4748,4779,4811,4813,4815,4835,4854,4860,4866,4871,4885,4901,4907,4913,4918,4920,4922,4943,4949,4955,4973,4978,4983,4988,4993,4998,5003,5008,5013,5018,5023,5028,5033,5038,5043,5048,5053,5058,5063,5068,5073,5078,5083,5088
src/std/std_effect.c:3431,3436,3439,3443,3446,4237,4242,4245,4248,4937,4968,5000,5002,5004,5024,5043,5049,5055,5060,5074,5090,5096,5102,5107,5109,5111,5132,5138,5144,5162,5167,5172,5177,5182,5187,5192,5197,5202,5207,5212,5217,5222,5227,5232,5237,5242,5247,5252,5257,5262,5267,5272,5277
src/vs/vs_effect.c:3014,3019,3022,3026,3029,3820,3825,3828,3831,4522,4553,4585,4587,4589,4609,4628,4634,4640,4645,4659,4675,4681,4687,4692,4694,4696,4717,4723,4729,4747,4752,4757,4762,4767,4772,4777,4782,4787,4792,4797,4802,4807,4812,4817,4822,4827,4832,4837,4842,4847,4852,4857,4862
```

The variable-address parameter declarations and definitions also need widening or explicit handle decoding:

```text
src/btl/battle_effect.c:99-122,245,263,4971,4976,4981,4986,4991,4996,5001,5006,5011,5016,5021,5026,5031,5036,5041,5046,5051,5056,5061,5066,5071,5076,5081,5086,5341,5829
src/std/std_effect.c:204-227,238,263,5160,5165,5170,5175,5180,5185,5190,5195,5200,5205,5210,5215,5220,5225,5230,5235,5240,5245,5250,5255,5260,5265,5270,5275,5535,6023
src/vs/vs_effect.c:177-200,212,231,4745,4750,4755,4760,4765,4770,4775,4780,4785,4790,4795,4800,4805,4810,4815,4820,4825,4830,4835,4840,4845,4850,4855,4860,5120,5607
```

### Other pointer/integer sites

The following inventory has **175 reference sites**, including declarations and address propagation. Classifications are attached to each group.

| Classification | References |
|---|---|
| A: animation offset addition | `src/main/anim.c:220` |
| S: model address argument/restoration | `src/main/buff_model.c:40,57` |
| A: stride/table address arithmetic; S: SDK object address | `src/main/effect_common.c:574,664,668,669`; S at `:592` |
| S: address returned as `int32_t` | `src/main/evl.c:112` |
| S: integer declaration versus pointer callee parameter | `src/main/game_menu.c:247,2383` |
| A | `src/main/graphics.c:170` |
| S: SDK object-address crossings | `src/main/map_object.c:301,459,516,687,692` |
| S: SDK object address; A: packet traversal; S/A: integer-address loader | `src/main/model.c:2839`; A at `:3176,3179,3193,3197,3211,3214,3219,3235,3261`; loader at `:3580,3622,3623,3625,3626,3628,3632,3633,3635,3636,3638,3639` |
| S | `src/main/partner_impl.c:441` |
| A: script address arithmetic/round trips; S: effect-buffer argument | `src/main/script_interp.c:720,730,741,747,757,946,956,1084,1926,2254`; S at `:1843` |
| A | `src/main/script_textbox.c:963` |
| S: integer ID encoded in a callback `void *`, then decoded | `src/main/sound_async.c:83,125,134,140,144–147` |
| A: textbox arithmetic and integer passed to `strlen` | `src/shop/script_menu.c:2949,3005` |
| S: integer scratch address and restored shard addresses; A: vertex/shard construction | `src/doo2/doo2.c:172,186,187,195,200,359,368,626,642–644,657,671–677,679,694–700,702` |
| S/A: CLUT/model arguments, shard buffer arithmetic and full-width `intptr_t` conversions | `src/dooa/dooa.c:370–372,547,550,1144,1183,1342,1413,1417,1449,1582,1594,1606,1608,1617,1649,1741` |
| C: pointer cast passed as an unused scalar argument | `src/dooa/dooa.c:1103` |
| S: effect stack stores | `src/eab/eab.c:696,703,704` |
| S: integer CLUT addresses | `src/endi/endi.c:100,110,423,424` |
| C: numeric timer punned through a pointer, then compared/used numerically | `src/evl/evl.c:457,458,462,466,476,485` |
| S/A: shard arena, integer addresses, stack stores and restored buffers | `src/evl/evl.c:309,324,445,452,453,510,526,527,540,893,894,901,905,1035,1053–1055,1062,1065,1066,1070,1072,1075,1085–1087,1093,1105–1107,1297–1301` |
| A | `src/fish/fish.c:3057` |
| S: SDK object address | `src/fish/fish_model.c:47` |
| S: SDK object addresses | `src/kar/kar.c:733,850,1961` |
| S | `src/murd/murd.c:354,793` |
| S | `src/std/std_setup.c:650` |
| C: narrowed address arguments are unused by `initializeSomeArenaArrays` | `src/std/std_main.c:2830`; `src/vs/vs_camera.c:731`; callee at `src/main/effect_common.c:613` |
| A: TMD header arithmetic; S: SDK object addresses | `src/std/std_hud.c:1116,1148,1253,1305,1355` |
| S | `src/vs/vs_hud.c:1329,1438`; `src/vs/vs_scene.c:1690` |

The `void *` callback tokens in `sound_async.c` carry **IDs**, not dereferenceable host addresses; they do not require placing callback locals in a 4 GB arena.

Full-width arithmetic already present in DOOA is useful but does not make its records layout-independent:

```text
src/dooa/dooa.c:1342,1413,1417,1449,1582,1594,1606,1608,1741
```

The heap setup is another A/fixed-address crossing:

```text
src/main/main.c:3537-3539
```

### Literal addresses and pointer-bearing initialized data

**24 fixed-address macro definitions — A/fixed PS1 address translation:**

```text
src/main/btl.c:23-27
src/main/utils2.c:24
src/dooa/dooa.c:36,37
src/murd/murd.c:28
src/std/std_setup.c:27-31
src/std/std_hud.c:29
src/vs/vs_hud.c:20
include/dw/main.h:8-10
include/dw/vs.h:14-18
```

The numeric buffer address at `src/evl/evl.c:28` and heap setup above also require translation.

**335 US BSS lines — S/data-layout candidates, runtime interpretation unverified.** Many values visibly resemble script/text bytes rather than usable pointers. Preserve these as original bytes or fixed-width records; do not blindly translate every value as an address.

```text
src/btl/btl_bss_us.c:1001-1012,1016-1019,1028,1035,1042,1049,1054-1058,1062-1086,1090,1091,2154,2155,2168,2169,2267
src/dooa/dooa_bss_us.c:223,534,1015,1016,1019,1020,1031,1032,1038,1045,1046
src/eab/eab_bss_us.c:9-11,239
src/endi/endi_bss_us.c:30,35,40,45,60,65,70,75,80,85,90,95,100,105,110,115,120,125,130,135,140,145,150,155,160,165,170,175,180,185,190,195,200,205,210,215,220,230,235,240,245,250,255,260,265,270,275,280,285,290,295,300,305,310,315,320,325,330,335,340,345,350,355,360,365,370,375,380,385,390,395,400,411
src/evl/evl_bss_us.c:1846,1859,2671-2686
src/fish/fish_bss_us.c:800,856,857,897-899,903-905,909,1122,1123,1306,1308
src/kar/kar_bss_us.c:2343,2346,2349,2374-2384
src/murd/murd_bss_us.c:290,293,297,300
src/std/std_bss_us.c:26,29,33,36,43,46,50,53,78,81,85,88,224,225,230,231,236,237,242,243,256,257,3005-3016,3020-3023,3032,3039,3046,3053,3058-3062,3066-3090,3094,3095
src/vs/vs_bss_us.c:211,212,217,218,223,224,229,230,240-245,1394-1405,1409-1412,1421,1428,1435,1442,1447-1451,1455-1479,1483,1484
```

Five ordinary null initializers are not address-zero reads:

```text
src/std/std_main.c:1329,1339,1347,1506
src/vs/vs_main.c:176
```

Three null-base member-offset calculations are A; use `offsetof` rather than treating them as real null dereferences:

```text
src/btl/battle_effect.c:3843
src/std/std_effect.c:4025
src/vs/vs_effect.c:3608
```

## 3. `sizeof`, hardcoded sizes, offsets, and raw struct treatment

### High-risk layout dependencies

| Dependency | References | Assessment |
|---|---|---|
| EFE slot table reserves `0x280` for 16 entries | `src/btl/battle_effect.c:2127,2133`; `src/std/std_effect.c:2232,2238`; `src/vs/vs_effect.c:1863,1869`; `include/dw/efe.h:172–185` | `EfeSlot` has six pointers. Its ordinary layout grows from 40 to 80 bytes on LP64. |
| Loaded EFE subeffect records accessed as native structs | `src/btl/battle_effect.c:1801–1803`; `src/std/std_effect.c:1906–1908`; `src/vs/vs_effect.c:1537–1539`; `include/dw/efe.h:125–133` | Seven pointers: 28 → 56 bytes. In-place array traversal changes. |
| Loaded EFE UV animation records accessed as native structs | `src/btl/battle_effect.c:1807–1810`; `src/std/std_effect.c:1912–1915`; `src/vs/vs_effect.c:1543–1546`; `include/dw/efe.h:138–152` | Two pointers after 24 bytes of scalars: 32 → 40 bytes. |
| EFE header size `0x34` | `src/btl/battle_effect.c:1789,1790`; `src/std/std_effect.c:1894,1895`; `src/vs/vs_effect.c:1525,1526`; `include/dw/efe.h:155–169` | Header is fixed-width and pointer-free; preserve it as a disk schema. |
| Entity/position fields accessed at byte offsets and stride 136 | `src/btl/battle_effect.c:3358–3362,4363,4468`; `src/std/std_effect.c:3540–3544,4552,4657`; `src/vs/vs_effect.c:3123–3127,4135,4240` | Assumes original `Entity`, `PositionData`, Gs coordinate and matrix layouts. |
| Raw layout unions | `include/dw/entity.h:20–23`; `include/dw/graphics.h:13–22` | Explicit raw widths 136, 32 and 80. A union grows if its native member grows. |
| TMD table viewed as pointer-bearing `Doo2ModelDesc` | `src/doo2/doo2.c:29–37,626,627` | The locally declared descriptor grows from 28 to 40 bytes; it cannot index the original table unchanged. |
| EVL shard table reserves `0x2d0` for 30 records | `src/evl/evl.c:30–39,309–312` | `EvlShardSet` includes a pointer; 24 → 32 bytes. |
| Fish iteration uses literal 56-byte stride | `src/fish/fish.c:3057` | Replace with validated representation-specific traversal; native size is SDK-dependent and unverified. |
| Map tile table accessed at literal offset `0x1bb8` | `src/main/map.c:4405` | Offset does not follow native pointer-bearing tile layout. |
| Shard records contain primitive pointers | `src/dooa/dooa.c:1413,1417,1499,1553,1608,1643,1678` | `sizeof` follows expanded native records, while surrounding allocation/buffer assumptions must be checked together. |
| Pointer-bearing runtime struct assignment | `src/doo2/doo2.c:182`; `src/main/model.c:3340,3348`; `src/main/script_interp.c:2373,2391,2396` | Native copying is appropriate only when both source and destination are native objects. |
| Initialized pointer-bearing overlay records | All 335 BSS references in category 2 | Literal bytes and native initializer layouts must be separated. |

### Saves

The save payload is declared at `src/main/main_menu.c:88–138`, and the outer file at `:140–152`.

Important byte assumptions:

```text
src/main/main_menu.c:3623,3751,3999,4151,4358,4828
src/main/main_menu.c:4160,4164,4921,5010,5011
include/dw/vs.h:20-32
```

These include `0x2000` card-file transfers, a `0xF00` payload clear, `0x4E4` and `0xA00` checksum regions, and 40 registered Digimon records of `0x40` bytes.

**The save does not dump the pointer-bearing `ScriptState` stack.** Its `0x17C` copies are a prefix; the stack follows the scalar arrays in `include/dw/script.h:158–164`. References: `src/main/main_menu.c:5058,5120`. Expanding `ScriptState` must still preserve the saved prefix separately.

SDK-dependent `VECTOR` fields in the save schema are a width risk. Their exact original definitions are **unverified** in this checkout.

The three 28-byte copies into `SAVE_FILE.reserved` intentionally span the declared 12-byte `reserved` array and the adjacent 16-byte PocketStation field:

```text
src/main/main_menu.c:3618,3994,4353
src/main/main_menu.c:145,146
```

Represent that combined disk region explicitly; do not accidentally “correct” its byte count.

### Complete copy / size-expression inventory: 89 lines

Two entries are `memcpy` declarations, not copies. No active US `memcmp` or `memmove` call was found.

```text
src/main/anim.c:68-73
src/main/bubble.c:436
src/main/efe.c:794
src/main/fish.c:132
src/main/main.c:3789,3867,3868
src/main/main_menu.c:3586,3587,3592,3593,3618,3962,3963,3968,3969,3994,4160,4164,4321,4322,4327,4328,4353,5024,5037,5044,5045,5058,5060,5061,5088,5101,5107,5108,5120-5122,5243,5244,5312
src/main/model.c:2829,2830,3193,3211,3235,3251,3258,3724
src/main/script_interp.c:2205
src/main/sound.c:400,455,748
src/main/sound_async.c:140,179
src/main/tamer.c:233,390,391
src/btl/battle_effect.c:2718
src/dget/dget.c:677
src/doo2/doo2.c:546,646,678,679,701,702
src/dooa/dooa.c:1413,1417,1499,1553,1603,1608,1643,1678,1685,1689
src/evl/evl.c:1056,1088,1089,1108,1109
src/fish/fish_model.c:33
src/std/std_effect.c:2876
src/vs/vs_effect.c:2459
```

## 4. Signedness, `char`, shifts, and truncation

### Compiler flags and `char`

Neither checked-in toolchain explicitly selects signed or unsigned plain `char`:

```text
mk/toolchain/gcc.mk:1-5
mk/toolchain/mwcc.mk:3-6
mk/platform/psx.mk:20-26
```

The original CodeWarrior default is **unverified** from these flags alone. Do not infer that DW1 requires globally unsigned `char`.

There is affirmative source evidence requiring signed byte semantics in animation:

- `MomentumData.subValue` is plain `char[9]`: `include/dw/entity.h:30`.
- It is accessed through `int8_t *`: `src/main/anim.c:141,163,366,382,445,453`.
- The instruction decoder stores `-1`: `src/main/anim.c:412`.
- Those values participate in momentum arithmetic: `src/main/anim.c:172,177,185,467`.

For the original CodeWarrior branch, `int8_t` is itself plain `char`: `include/dw/types.h:10`. That makes the compiler’s default significant.

**Seven plain-`char` fields in local game headers:**

```text
include/dw/entity.h:30,116
include/dw/fish.h:251
include/dw/item.h:13
include/dw/map.h:16
include/dw/params.h:20
include/dw/vs.h:28
```

Most are strings; `entity.h:30` is numeric. No active US ctype-library calls were found.

### Integer behavior

- Native `int` remains 32-bit; ordinary `int32_t` → `int` operations do not become 64-bit merely because the target is arm64.
- `long`, native pointers, `intptr_t`, `size_t`, and SDK types spelled with `long` do change width.
- GCC already uses `-fwrapv -fno-strict-aliasing`: `mk/toolchain/gcc.mk:4`.
- `-fwrapv` does not make arbitrary signed left shifts, invalid shift counts, or division overflow safe.

Examples requiring preserved signed behavior:

```text
src/main/anim.c:467,472,495-497
src/main/map.c:4154-4159
src/main/vecmath.c:157-159,173,207,211,246,248,250,252
src/btl/battle_effect.c:4973,4978,5013,5018,5053,5058
src/std/std_effect.c:5162,5167,5202,5207,5242,5247
src/vs/vs_effect.c:4747,4752,4787,4792,4827,4832
```

The interpreter shift counts come from script state; whether all retail inputs stay within valid ranges is **unverified**.

### Complete shift inventory: 805 lines

This includes unsigned packet packing and safe shifts as well as signed candidates.

```text
src/main/aabb.c:61,64-66
src/main/anim.c:242-244,327,329,332,337,391,467,472,495-497
src/main/btl.c:571
src/main/bubble.c:497,503,507,508,537,555,561,566,574,583,645
src/main/buff_model.c:62
src/main/butterfly.c:128
src/main/drop_shadow.c:72,76
src/main/efe.c:359-361,520,524,525,573,588,590-592,945-947,951,956,1002-1004,1006-1008,1086-1088,1097,1102
src/main/effect_common.c:380-382,384-386,393-395,397-399,436-438,440-442,479,496,513,530,545,548,557
src/main/evl.c:100
src/main/evolution.c:709,744,809,1045
src/main/file.c:72,91,110,128
src/main/file_queue.c:143,275
src/main/font.c:1530,1540,1592,1756
src/main/game_menu.c:3538,3539,3541,3542,4269,4270
src/main/graphics.c:173-175,180,184
src/main/item.c:2487,2488,2498,2499
src/main/line.c:36,54
src/main/main.c:3538
src/main/main_menu.c:3434,4671,4872
src/main/map.c:3330,3331,3519,3644,3645,3696,3698,3701,3714,3716,4154-4159,4585,4586,4588,4589,4595
src/main/math.c:9
src/main/model.c:2980-2982,3191,3209,3233,3249,3401,3403,3564-3567,3572,3674,3676,3714-3717,3720
src/main/overworld.c:449,711,764,768,769,772
src/main/particle.c:149,151,185
src/main/rng.c:30
src/main/script_common.c:1240,1241,1246,1631,1822,1992,2019,2618,2716,2921,3108
src/main/script_interp.c:755,885,888,934,940,949,959,1790,2471,2493,2509,2543,2671,2810,2815
src/main/script_textbox.c:218,475,633,776,1115,1121,1130,1140,1150,1184,1194
src/main/script_value.c:122
src/main/sjis.c:55,111,119
src/main/sound.c:401,456,480,575,622,634,658,748
src/main/sound_async.c:141,180
src/main/utils2.c:313,329,504,536
src/main/vecmath.c:157-159,167,169,170,173,207,211,246,248,250,252
src/shop/script_menu.c:902,2256,2306,2760
src/btl/battle_effect.c:1734,1736,1739,1740,1745-1747,1931-1933,1991,1992,1995,1996,1999,2000,2003,2004,2010-2015,2017,2026,2028,2054,2450,2502,2504,2506,2524,2639,2676,2716,2719,2764-2766,2768,2948,3175,3291,3294,3415-3417,3636,3648,3764,3775-3777,3823-3825,3863-3866,3895,3896,3907,3942,3949,4080,4081,4093,4094,4098,4099,4119-4121,4167,4187,4206,4220,4222,4227,4344,4420-4422,4480-4482,4540-4543,4768,4852,4854,4858,4860,4864,4866,4883,4885,4973,4978,5013,5018,5053,5058,5255-5260,5313,5422,5842,5977,6257,6260-6262,6301-6303
src/btl/battle_hud.c:569
src/btl/battle_main.c:2677,2679,2681,2683,2685,2687,2689,2691,2693
src/btl/command_menu.c:556
src/btl/command_shout.c:131
src/doo2/doo2.c:367,387-389,400-402,465,494,508,538
src/dooa/dooa.c:476,477,484,542,620,626,669,908,915,940,1244-1246,1257-1259,1282-1284,1295-1297,1382
src/eab/eab.c:174,631
src/endi/endi.c:83-85,94-96,251-253,269-271
src/evl/evl.c:277,468,469,616-618,628-630,658-660,670-672,978-980,992-994,997,1199
src/fish/fish.c:1003,1004,1595,1596,1598,1599,1628,1630,1631,1633,1652,1655,1673,1675,1676,1678,1698,1701,1702,1898-1900,2284,2493-2495,2804,2869,3265,3317,3347,3426,3500,3512,3513,3519-3521,3674
src/kar/kar.c:1274,1280,1290,1598
src/murd/murd.c:353,374,382,578,585,610
src/std/std_setup.c:514,631
src/std/std_main.c:1686-1688,2308,2309,7002,7004,7006,7008,7010,7012,7014,7016,7018
src/std/std_hud.c:603,1119,1123,1127
src/std/std_effect.c:1839,1841,1844,1845,1850-1852,2036-2038,2096,2097,2100,2101,2104,2105,2108,2109,2115-2120,2122,2131,2133,2159,2618,2670,2672,2674,2692,2797,2834,2874,2877,2922-2924,2926,3106,3338,3473,3476,3597-3599,3818,3830,3946,3957-3959,4005-4007,4045-4048,4077,4078,4089,4124,4131,4262,4263,4275,4276,4280,4281,4301-4303,4349,4369,4388,4409,4411,4416,4533,4609-4611,4669-4671,4729-4732,4957,5041,5043,5047,5049,5053,5055,5072,5074,5162,5167,5202,5207,5242,5247,5449-5454,5507,5616,6036,6171
src/trn2/trn2_slots.c:129,159,236,237,245
src/trn/trn_slots.c:129,159,236,237,245
src/vs/vs_combat.c:1355,1356,2613,2614
src/vs/vs_effect.c:1470,1472,1475,1476,1481-1483,1667-1669,1727,1728,1731,1732,1735,1736,1739,1740,1746-1751,1753,1762,1764,1790,2201,2253,2255,2257,2275,2380,2417,2457,2460,2505-2507,2509,2689,2921,3056,3059,3180-3182,3401,3413,3529,3540-3542,3588-3590,3628-3631,3660,3661,3672,3707,3714,3845,3846,3858,3859,3863,3864,3884-3886,3932,3952,3971,3992,3994,3999,4116,4192-4194,4252-4254,4312-4315,4542,4626,4628,4632,4634,4638,4640,4657,4659,4747,4752,4787,4792,4827,4832,5034-5039,5092,5201,5620,5755
src/vs/vs_hud.c:661,662,799
src/vs/vs_intro.c:260-262,419,420
src/vs/vs_main.c:1083,1085,1087,1089,1091,1093,1095,1097,1099
src/vs/vs_scene.c:1349,1350,1374,1824,1844
src/vs/vs_select.c:392,395,500,501,601,631,768,787,788,818,859,973,974
```

### Long-suffixed constants: 52 lines

These can promote expressions to native 64-bit `long`. Most comparisons to small constants are harmless; alignment/index arithmetic deserves review.

```text
src/main/btl.c:367
src/main/map.c:5584
src/main/model.c:3622,3632
src/main/partner_impl.c:2128
src/main/utils2.c:338
src/btl/battle_effect.c:1481,1508,1528,1562,1845,3744,4569
src/btl/battle_main.c:2588,2729
src/dooa/dooa.c:642,643,967
src/evl/evl.c:306,321,426,429,430,1204,1205,1310,1325,1358
src/fish/fish.c:1675,1678
src/murd/murd.c:637
src/std/std_main.c:3487,5758,6812,7054
src/std/std_effect.c:1634,1661,1681,1715,1950,3926,4758
src/vs/vs_effect.c:1217,1244,1264,1298,1581,3509,4341
src/vs/vs_main.c:607,989,1135
```

## 5. Unaligned access and type punning

### Specific mechanisms

- **Bytecode halfword reads after byte-sized advancement:** `src/main/script_interp.c:2399–2427,2574–2577`. `SCRIPT_POINTER` has byte alignment; use explicit little-endian reads. Actual retail alignment is **unverified**.
- **Loaded script section offsets:** `src/main/script_common.c:1189,1215`; `src/main/tournament.c:163`; `src/main/script_interp.c:755,2185`; `src/main/script_textbox.c:964`.
- **Pointer-sized read from a 32-bit numeric timer:** `src/evl/evl.c:457,458,462,466,476,485`. On LP64 this reads eight bytes, then truncates.
- **Packed primitive/color/UV access through wider aliases:** the EFE cast-dereference inventory below, especially `src/btl/battle_effect.c:2632,2668–2671,5292–5308`, with STD/VS counterparts.
- **Typed loaded headers:** MMD, VHB, TIM, SEQ and EFE casts in the inventory below. Alignment, bounds and object-lifetime assumptions need explicit loader handling.
- **Arena alignment of only four bytes:** `src/btl/battle_effect.c:2126,2134,2136,2139`; `src/std/std_effect.c:2231,2239,2241,2244`; `src/vs/vs_effect.c:1862,1870,1872,1875`. Native pointer-bearing objects require their native alignment.

An arm64 load may physically support an unaligned address while the C access still violates alignment or aliasing rules. Original PS1 runtime tolerance is **unverified** here.

### All 9 union definitions

```text
src/kar/kar.c:33
include/dw/btl.h:95
include/dw/entity.h:20
include/dw/fish.h:145
include/dw/graphics.h:13,19
include/dw/kar.h:46,61
include/dw/vs.h:41
```

The raw-layout unions are identified in category 3. Other union use is not automatically invalid; preserve member widths and distinguish intentional overlapping views from serialized native structs.

### Complete cast-dereference / loaded-buffer cast candidate inventory: 554 lines

This broad inventory includes harmless byte aliases and API casts. It is not a count of 554 confirmed unaligned accesses.

```text
src/main/btl.c:455
src/main/buff_model.c:62,64
src/main/efe.c:1011
src/main/effect_common.c:358,403
src/main/file.c:92,105,123
src/main/game_menu.c:1553,1568
src/main/graphics.c:172
src/main/inventory.c:559
src/main/line.c:33,51
src/main/main_menu.c:4828
src/main/map.c:4033,4234,4256,4405
src/main/model.c:3158-3160,3415,3416,3447,3448,3625,3626,3635,3636,3725,3726
src/main/script_common.c:1189,1215,3109
src/main/script_interp.c:755,2185,2413,2576
src/main/script_textbox.c:964
src/main/sound.c:401,402,410,456,457,465,742
src/main/sound_async.c:141,142,152,180,181,190
src/main/tournament.c:163
src/main/utils.c:281
src/main/utils2.c:499
src/btl/battle_effect.c:1614,1684,1718,1760,1789,1790,1890,1891,1939-1941,2054,2281,2282,2322,2338,2339,2349,2376,2380,2401,2411,2632,2634,2635,2668-2671,3007-3009,3249-3251,3254,3257,3258,3261,3264,3382-3384,3532,3533,3695,3720,3724,3742,3744,3745,3765,3771,3810,3843,4055-4057,4060,4063,4066,4118,4344,4348-4350,4386,4414-4417,4468,4503,4504,4646,4655,4746,4748,4811,4813,4815,4852,4854,4858,4860,4864,4866,4899,4901,4905,4907,4911,4913,4918,4920,4922,4941,4943,4947,4949,4953,4955,4973,4978,4983,4988,4993,4998,5003,5008,5013,5018,5023,5028,5033,5038,5043,5048,5053,5058,5063,5068,5073,5078,5083,5088,5292,5293,5295,5296,5299,5300,5302,5303,5305-5308,5348,5842,5846-5848,5907,5908,5911,5972
src/btl/battle_main.c:1350
src/dget/dget.c:319,485,542
src/doo2/doo2.c:436-438
src/dooa/dooa.c:651,670,679,688,1452,1453,1456,1503
src/eab/eab.c:174,178-180,636,637
src/endi/endi.c:37
src/evl/evl.c:278,288,297,457,458,462,466,476,485
src/fish/fish.c:1292,1622,1648-1651,1693,2115,2276,3672
src/murd/murd.c:354
src/std/std_setup.c:507,624
src/std/std_main.c:2776,3966,4092,5129
src/std/std_hud.c:1118
src/std/std_effect.c:1789,1823,1865,1894,1895,1995,1996,2044-2046,2159,2367,2449,2450,2490,2506,2507,2517,2544,2548,2569,2579,2790,2792,2793,2826-2829,3165-3167,3431-3433,3436,3439,3440,3443,3446,3564-3566,3714,3715,3877,3902,3906,3924,3926,3927,3947,3953,3992,4025,4237-4239,4242,4245,4248,4300,4533,4537-4539,4575,4603-4606,4657,4692,4693,4835,4844,4935,4937,5000,5002,5004,5041,5043,5047,5049,5053,5055,5088,5090,5094,5096,5100,5102,5107,5109,5111,5130,5132,5136,5138,5142,5144,5162,5167,5172,5177,5182,5187,5192,5197,5202,5207,5212,5217,5222,5227,5232,5237,5242,5247,5252,5257,5262,5267,5272,5277,5486,5487,5489,5490,5493,5494,5496,5497,5499-5502,5542,6036,6040-6042,6101,6102,6105,6166
src/vs/vs_camera.c:677
src/vs/vs_combat.c:799,927,928,1971
src/vs/vs_effect.c:1350,1420,1454,1496,1525,1526,1626,1627,1675-1677,1790,2032,2033,2073,2089,2090,2100,2127,2131,2152,2162,2373,2375,2376,2409-2412,2748-2750,3014-3016,3019,3022,3023,3026,3029,3147-3149,3297,3298,3460,3485,3489,3507,3509,3510,3530,3536,3575,3608,3820-3822,3825,3828,3831,3883,4116,4120-4122,4158,4186-4189,4240,4275,4276,4420,4429,4520,4522,4585,4587,4589,4626,4628,4632,4634,4638,4640,4673,4675,4679,4681,4685,4687,4692,4694,4696,4715,4717,4721,4723,4727,4729,4747,4752,4757,4762,4767,4772,4777,4782,4787,4792,4797,4802,4807,4812,4817,4822,4827,4832,4837,4842,4847,4852,4857,4862,5071,5072,5074,5075,5078,5079,5081,5082,5084-5087,5127,5620,5624-5626,5685,5686,5689,5750
src/vs/vs_scene.c:1817,1837
```

## 6. K&R definitions, declarations, prototypes, callbacks, and varargs

### K&R definitions: 233

Clang can accept these in older C language modes, with deprecation diagnostics; they are not valid C23 definitions. Their narrow parameters undergo default argument promotions. Rewriting them mechanically with narrow prototypes can change the calling contract.

```text
src/main/anim.c:201
src/main/battle_tick.c:235,301
src/main/battle_ui.c:415,712
src/main/btl.c:161,183,210
src/main/clock.c:293
src/main/effect_common.c:579
src/main/entity_text.c:156
src/main/evl.c:77
src/main/evolution.c:852,1054
src/main/font.c:1549,1728
src/main/game_menu.c:1411,2380,3899,3953
src/main/item.c:3025
src/main/main.c:4221,4293
src/main/map.c:4101,4500,4525,4844,4855,5054,5080,5100,5114,5168,5195,5237,5322,5426
src/main/map_collision.c:57,103,190
src/main/map_object.c:586,1130,1148,1156,1186
src/main/model.c:2881,2986
src/main/overworld.c:1424,1465,1723
src/main/partner.c:1087,1094,1133,1139
src/main/partner_impl.c:848,1113,2153
src/main/script_common.c:1204,1741,1778,1875,2960,3019,3063,3088
src/main/script_interp.c:671,692,1030,1090,1297,1550,1640,2235,2244,2475,2483,2498,2514,2587,2659,2677,2833
src/main/script_textbox.c:149,207,332,527,562,589,603,814,881
src/main/script_value.c:60,208
src/main/sjis.c:63
src/main/tamer.c:379,608,885,968,983,991,1114,1192,1309,1359
src/main/ui.c:120,234
src/main/utils.c:176,196
src/main/utils2.c:462
src/main/world_object.c:19,51
src/btl/battle_effect.c:5582,5779,6702
src/btl/battle_hud.c:383,516
src/btl/battle_main.c:2229,3034,3063,3202
src/btl/command_menu.c:216
src/dooa/dooa.c:1699,1745
src/eab/eab.c:670
src/endi/endi.c:407
src/evl/evl.c:806
src/kar/kar.c:1888,1916,1997,2009,2576,2663,2686,2704
src/murd/murd.c:712,756
src/std/std_setup.c:639
src/std/std_main.c:1626,1695,1703,1714,1970,2010,2033,2093,2715,3393,3406,3603,6163,7168,7197,7228
src/std/std_hud.c:551,559,1002,1016,1098
src/std/std_effect.c:2263,5776,5973
src/trn2/trn2_hp_map99.c:53
src/trn2/trn2_def_map99.c:48
src/trn2/trn2_hp_map107.c:39,73
src/trn2/trn2_mp.c:33,52
src/trn2/trn2_off.c:64,95
src/trn2/trn2_def_map108.c:39
src/trn2/trn2_reward.c:50,169,360,409
src/trn2/trn2_hud.c:158,540
src/trn2/trn2_slots.c:100,214
src/trn/trn_reward.c:151,234,308,374,422,474
src/trn/trn_hp.c:58,79
src/trn/trn_mp.c:60,150
src/trn/trn_off.c:43
src/trn/trn_def.c:67,100
src/trn/trn_speed.c:35
src/trn/trn_brain.c:37
src/trn/trn_hud.c:337
src/trn/trn_slots.c:100,214
src/vs/vs_camera.c:357,620,769
src/vs/vs_combat.c:962,1430,2552
src/vs/vs_effect.c:1894,5361,5557
src/vs/vs_hud.c:610,1214,1233,1313,1490
src/vs/vs_intro.c:148,166,200,269,277,288,360,441,481,510,570
src/vs/vs_main.c:540,1341,1370,1401,1757,1778
src/vs/vs_scene.c:1401,1440
src/vs/vs_select.c:478,652,1110,1206
```

### Empty parameter lists: 197 lines

These are not necessarily implicit declarations. They suppress checking in pre-C23 C and are especially relevant to callbacks and dispatch tables.

```text
src/main/_psstart.c:16,19
src/main/battle_ui.c:21,44,46-50,54-56
src/main/btl.c:64
src/main/clock.c:16,17,19
src/main/efe.c:134,145,148,150,153,155,160,164,168,169
src/main/entity_text.c:51,71
src/main/evl.c:16-18
src/main/fade.c:50
src/main/game_menu.c:175,182
src/main/graphics.c:10-36
src/main/inventory.c:43,44,46,48,51,53,56,57,59-61,63-69,72
src/main/main_menu.c:250,254,255,257,259,260,262,264-266,268,276,281,296,297,305,306
src/main/map.c:70
src/main/model.c:41
src/main/particle.c:13,201
src/main/partner.c:69,71,73-75,78-84,86-90,93,96-100,103-105
src/main/script_common.c:695
src/main/tamer.c:195,245,275
src/btl/battle_effect.c:1114
src/btl/battle_main.c:139
src/kar/kar.c:113
src/mov/mov.c:20,111
src/std/std_setup.c:71
src/std/std_main.c:100,110
src/std/std_effect.c:81,309,310,930
src/trn2/trn2_hp_map99.c:16-19,21
src/trn2/trn2_def_map99.c:16,17,25
src/trn2/trn2_hp_map107.c:19-21,23
src/trn2/trn2_mp.c:19
src/trn2/trn2_off.c:17,26
src/trn2/trn2_def_map108.c:18,25
src/trn2/trn2_reward.c:25
src/trn2/trn2_hud.c:31
src/trn/trn_hp.c:19-21,23
src/trn/trn_mp.c:19,20,25
src/trn/trn_off.c:18,19,28
src/trn/trn_def.c:19,20,27
src/trn/trn_speed.c:19
src/trn/trn_brain.c:22
src/trn/trn_hud.c:29
src/vs/vs_combat.c:128
src/vs/vs_effect.c:42,273,274,864
src/vs/vs_main.c:47,91
src/vs/vs_select.c:70
include/dw/anim.h:9
include/dw/font.h:24,26
include/dw/model.h:46
include/dw/particle.h:38
include/dw/sjis.h:8
include/dw/utils.h:28
include/dw/vs.h:181,213,214
include/dw/world_object.h:23
```

### Concrete prototype hazards

| Function / mechanism | Conflicting sites |
|---|---|
| `memcpy`: `void(uint8_t *, uint8_t *, uint32_t)` versus libc’s pointer return, `const void *` and `size_t` | `src/main/main.c:3789`; `src/main/tamer.c:233` |
| `addFileReadRequestPath`: pointer buffer versus `int32_t` buffer | `src/main/file_queue.c:191`; `include/dw/file_queue.h:16`; `src/main/script_interp.c:83` |
| `loadStackedTIMEntry`: `void *` versus `u_long` | `src/main/file.c:116`; `include/dw/file.h:22`; `src/main/game_menu.c:247` |
| `addFileReadRequest`: typed function pointer versus `void *` callback | `src/main/file_queue.c:200`; `include/dw/file_queue.h:20`; `src/btl/battle_effect.c:51`; `src/std/std_effect.c:285`; `src/vs/vs_effect.c:44` |
| `setFileReadCallback2`: typed callback versus object pointer | `src/main/file_queue.c:184`; `include/dw/file_queue.h:14`; `src/btl/battle_effect.c:81`; `src/std/std_effect.c:73`; `src/vs/vs_effect.c:255` |
| `addObject`: differing return types, integer widths and callback types | `src/main/world_object.c:19`; `include/dw/world_object.h:21`; `src/main/fade.c:18`; `src/main/map.c:39`; `src/btl/battle_main.c:48`; `src/vs/vs_hud.c:71` |
| `fadeoutCLUT1`: two versus three arguments | `src/main/effect_common.c:25,364`; `src/main/map.c:90`; `src/dooa/dooa.c:146`; `src/evl/evl.c:77`; `src/murd/murd.c:43` |
| `fadeoutCLUT2`: two versus three arguments | `src/main/effect_common.c:27,420`; `src/main/map.c:92`; `src/evl/evl.c:78` |
| `checkFishingMap`: one versus two arguments | `src/main/fish.c:117`; `include/dw/fish.h:299`; `src/main/map.c:42` |
| `setActiveAnim`: one versus two arguments | `src/main/script_interp.c:46`; `src/main/overworld.c:147,1783` |
| `activateMedalTexture`: one versus two arguments | `src/main/game_menu.c:230`; `src/main/map_object.c:1193`; `include/dw/map_object.h:65` |
| `STD_addWinScene`: no arguments versus one | `src/std/std_main.c:120`; `src/std/std_effect.c:297,1516` |
| `BTL_initializeFinisherChargeup`: no arguments versus one | `src/btl/battle_hud.c:71`; `src/btl/command_menu.c:23,360`; `src/btl/battle_main.c:80` |
| `triggerSeadramonCutscene`: no arguments versus pointer argument | `src/main/fish.c:10`; `src/main/script_common.c:112,3057` |
| `worldPosToScreenPos`: incompatible pointer argument meanings and return type | `src/main/effect_common.c:31,504`; `src/trn2/trn2_hud.c:27`; `src/trn/trn_hud.c:28` |
| `setEntityPosition`: `int32_t` versus `long` coordinates | `src/main/model.c:2971`; `include/dw/model.h:45`; `src/main/tamer.c:1124,1200`; `src/dooa/dooa.c:306` |

Callback-specific casts include:

```text
src/main/game_menu.c:1325
src/btl/battle_effect.c:1614,2141
src/std/std_effect.c:2246,2367
src/vs/vs_effect.c:1350,1877
include/dw/file_queue.h:8
include/dw/utils.h:28
include/dw/world_object.h:8,9
```

The **339-name discrepancy inventory** includes K&R promotion-compatible cases; it is **not 339 confirmed ABI failures**. SDK conflicts and the full set of implicit declarations remain **unverified** without the SDK/generated headers and a complete compiler pass.

### All 1,212 reference lines participating in written signature discrepancies

```text
src/main/battle_tick.c:34,40,41,235,301
src/main/map.c:39,40,42-45,47,49,52-57,60,62,64-68,79,81-84,89-92,95,99,100,102-104,106,109,110,114,116,120,126,127,130,132,133,135,136,139,140,144,146,147,150,151,153,154,156,157,164,165,167,168,3133,3157,3270,3375,3400,3530,3871,3905,4020,4101,4212,4225,4398,4500,4525,4659,4844,4855,5054,5080,5100,5114,5168,5195,5237,5277,5322,5331,5400,5407,5426,5592
src/btl/battle_setup.c:23,26,27
src/main/utils2.c:30,31,33,37,40,42,45,48-52,54,56,63-67,183,207,232,247,269,288,333,344,354,462,487,511
src/main/tamer.c:183-185,188-190,193,195,201,202,208,217,220,221,224,226,228,229,232,235,243,263,266,267,379,608,885,968,977,983,991,1124,1200,1309,1359
src/main/partner.c:48-50,68-70,72,77,85,88,92,101,102,106,109-115,1087,1094,1133,1139
src/main/battle_ui.c:22,24,26,28,29,34,39,42,51,58,415,712
src/main/game_menu.c:182,186,189,190,199,204,206,214,217,220,221,223,225,228,230,231,237,239,246,247,249,260,269,270,1411,1684,2223,2245,2380,3899,3953,4276
src/trn2/trn2_hud.c:26-30,32,33,158,540
src/trn/trn_hud.c:23-28,337
src/vs/vs_scene.c:44-46,55,57,79,93,1401,1440
src/main/utils.c:28,31,34-36,176,196,217,264,297,302
src/main/inventory.c:28,29,31,33,36,37
src/main/script_common.c:65-69,74,75,91,95,106,108,109,112-114,117,123,127,137,141,142,1294,1741,1778,1875,2221,2431,2905,2960,3011,3019,3057,3063,3088,3115
src/main/script_textbox.c:24-29,31,34,36,39,41,45,49,52,149,207,332,527,562,589,603,814,881,1275
src/shop/script_menu.c:41,44,45,47,49,52,103,106,108,109
src/btl/battle_hud.c:68,69,71,75,77-80,88,90,245,383,516
src/btl/command_menu.c:16,17,19,21-23,216,360
src/fish/fish.c:118-120,123,126,128,231
src/kar/kar.c:46,53,54,64-67,78,100,101,104,108,112,113,781,1888,1916,1997,2009,2576,2663,2686,2704
src/std/std_main.c:98-100,105,106,111,116,120,132,167,188,189,207,209,210,219,237,249,256,258,263,264,269,280,284,287,290,292,294-296,303,310,311,313,324,325,330,347,354,1626,1695,1703,1714,1885,1970,2010,2033,2093,2715,3393,3406,3544,3566,3603,5582,6163,6185,7168,7197,7228
src/trn/trn_reward.c:19-22,27,151,234,308,374,422,474
src/vs/vs_combat.c:48,50,52,56,57,63,68,77,127,962,1430,2552
src/vs/vs_intro.c:33,35,37,39,43,44,46,48-50,55-57,60,148,166,200,269,277,288,441,481,510,570
src/vs/vs_main.c:45,47,48,62,77-79,89,90,97,99,540,898,1341,1370,1401,1828
src/vs/vs_select.c:52,54,56,58,69,75,77,478,1110,1206
src/dget/dget.c:30-32,34,39,43,44,47-49,51,59,281
src/main/item.c:34,35,37,41,52,57,2549,3025
src/main/overworld.c:81,82,84,88,90,109,115,120,123,129,133,138,141-144,147-149,151,152,162,163,165,166,170,171,173,331,380,394,844,920,930,952,992,1002,1026,1088,1093,1142,1151,1424,1465,1503,1723,1783,1802,1821,1829
src/main/ui.c:11,12,16,17,20,22,24,120,234
src/std/std_hud.c:56-59,72,76,79,85,551,559,1002,1016,1098
src/vs/vs_camera.c:28-31,56,357,620,769
src/vs/vs_hud.c:38,43,58,63,66-69,71,72,74,679,1214,1233,1313,1490
src/main/fade.c:18-21
src/main/btl.c:46,47,50,52,65,68-70,161,183,189,210
src/main/butterfly.c:18,21,22
src/main/efe.c:134,137,139-142,146,162,370,965
src/main/effect_common.c:20,24-27,31-35,39-41,278,353,364,407,420,504,518,537,567,579,749,756,761
src/btl/battle_effect.c:44,46,49,51,63,67,69,80,81,261,271,275,290,294,5779,6137,6523,6702
src/dooa/dooa.c:107,108,122,130,135,141-143,145-147,306,1699,1745
src/eab/eab.c:29-31,33,670
src/evl/evl.c:74,77-79,83,84,89,93-95,113,806
src/murd/murd.c:33-35,39,40,42,43,58,712,756
src/std/std_effect.c:53,56,64,66,68,72,73,75,77,260,285,297,1516,5973
src/vs/vs_effect.c:44,54,59,240,243,245,255,256,258,260,266,5557
src/main/bubble.c:42,44,46,48
src/btl/battle_main.c:47,48,50,60,64,66,67,69,80,88,92,93,98,115,118,119,132-134,138,140,142,186,2229,3034,3063,3202
src/btl/command_shout.c:13
src/std/std_setup.c:49,63,65,66,69,74,90,91,94,104,521,639
src/main/buff_model.c:11,12,88,95
src/main/particle.c:11,12
src/endi/endi.c:16,17,407
src/main/clock.c:10,18,20,293
src/main/partner_impl.c:305,306,309,310,316,327,338,343,367,664,848,1113,2062-2066,2153
src/trn2/trn2_def_map99.c:15,18,19,21,22,24,25,48
src/trn2/trn2_off.c:16,18,21-23,25,26,64,95
src/trn2/trn2_def_map108.c:17,19,21,22,24,25,39
src/trn/trn_off.c:17,20,23-25,27,28,43
src/trn/trn_def.c:17,21,24-27,67,100
src/main/entity_text.c:41,42,47,49,52,54,96,156
include/dw/script.h:290,294,299,306,316,319,324,326,331,332,346-351,358,359,362-364,378-380,389,391,392
src/main/script_interp.c:38,39,45,46,49-51,56-60,65-69,73,74,79-81,83,671,692,1030,1090,1297,1550,1640,2235,2244,2475,2483,2498,2514,2587,2659,2677,2722,2763,2775,2833
src/main/evl.c:16,17,77
src/main/sound_async.c:233,248
include/dw/sound_async.h:9,10
src/main/evolution.c:16,17,674,690,724,789,852,1054
src/main/main.c:38,86-89,103,109,126,128,137,139,151,153,154,172,3776,3784,3788,4221,4293
include/dw/evolution.h:8-13
include/dw/world_object.h:21,23
src/main/world_object.c:19,51
src/main/file.c:80,116
src/main/map_object.c:117,119,124,134,309,586,1130,1148,1156,1186,1193
include/dw/file.h:20,22
src/main/file_queue.c:184,191,200
include/dw/file_queue.h:14,16,20
src/main/fish.c:10,11,117
src/main/tournament.c:26,27,29,31,32
include/dw/fish.h:299
src/main/line.c:26,44
include/dw/line.h:6,8
include/dw/map_object.h:39,60-62,64,65
include/dw/font.h:26
src/main/font.c:1728
src/main/graphics.c:39,156
src/main/model.c:26,2971
src/main/sound.c:517,641,721
include/dw/sound.h:25,28,33
include/dw/item.h:68,78
src/main/main_menu.c:244,282,296,3319,5153
include/dw/btl.h:167,168
src/trn2/trn2_hp_map99.c:14,18-21,53
src/trn2/trn2_hp_map107.c:17,21-23,39,73
src/trn2/trn2_mp.c:17-19,33,52
src/trn2/trn2_reward.c:18,19,22,24,25,50,169,360,409
src/trn/trn_hp.c:17,21-23,25,58,79
src/trn/trn_mp.c:18,21,22,24,25,60,150
src/trn/trn_speed.c:17-19,35
src/trn/trn_brain.c:19,21,22,37
src/main/map_collision.c:11,13-15,23,57,103,190
include/dw/model.h:45
include/dw/tamer.h:11,18,20,21,23
src/main/script_value.c:14,15,60
src/main/vecmath.c:117,139
include/dw/vecmath.h:11,12
include/dw/vs.h:178,206,217,220
src/trn2/trn2_slots.c:25,26,100,214
src/trn/trn_slots.c:25,26,100,214
include/dw/clock.h:24
include/dw/dooa.h:65,66
include/dw/eab.h:49
include/dw/endi.h:41
include/dw/evl.h:89
include/dw/murd.h:37
include/dw/partner.h:36,39,40
include/dw/std.h:157,158
include/dw/training.h:37
include/dw/trn.h:48,50-60,64
include/dw/trn2.h:42-45,47-50,52,55,59,60,65
```

### Variadic calls: 10 active US sites

No `va_start`, `va_arg`, or `va_end` machinery was found. Format compatibility for externally supplied/generated format strings is **unverified**.

```text
src/main/game_menu.c:4143
src/main/item.c:3064
src/main/main_menu.c:3228
src/main/utils.c:275
src/btl/battle_effect.c:3575
src/fish/fish.c:4281,4286,4290
src/std/std_effect.c:3757
src/vs/vs_effect.c:3340
```

No implicit-int function definition was found by the definition scan. That does not establish zero implicit function declarations.

## 7. Null reads, bounds, indeterminate locals, and address zero

These are **observed source findings**, not an exhaustive runtime UB count.

| Count | Finding | References |
|---:|---|---|
| 1 | Definite null dereference path: path wrapper passes `NULL`, callee unconditionally copies `*loc` | `src/main/file_queue.c:197,250` |
| 1 | Empty script-stack pop initializes only `entry.smth[0]`, then copies the whole local struct | `src/main/script_interp.c:2386,2393,2396` |
| 2 | `ModelComponent.pad` is not initialized before copying the local template | `include/dw/model.h:33`; `src/main/model.c:3326,3329–3337,3340,3348` |
| 1 | Script stack has eight entries, but push has no capacity check | `include/dw/script.h:163`; `src/main/script_interp.c:2372–2374` |
| 3 | EFE call stacks push context/return addresses without a capacity check | `include/dw/efe.h:362`; `src/btl/battle_effect.c:4777,4778`; `src/std/std_effect.c:4966,4967`; `src/vs/vs_effect.c:4551,4552` |
| 1 | Coordinate hierarchy collection uses a 100-pointer local array without a depth/cycle guard | `src/main/vecmath.c:261,267,268` |
| 1 | Eight-byte numeric-format buffer can overflow for wider/negative formatted values | `src/main/utils.c:267,275`; reachability **unverified** |
| 1 | Integer address passed to `strlen`; truncation can cause an invalid read | `src/shop/script_menu.c:3005` |

For the null `CdlLOC` copy, the value read from original address zero and whether the original compiler optimized or emitted that access are **unverified**. Native behavior must be explicit; mapping a fake page zero is not an appropriate substitute.

The three null-base offset calculations listed in category 2 are not actual reads from address zero. The five ordinary null initializers are also not null reads.

The fixed PS1 addresses and 335 pointer-literal BSS lines are catalogued in category 2; their runtime dereference behavior is **unverified** without execution.

## 8. MIPS assembly, compiler extensions, placement, and Clang rejection

### Startup and placement

| Item | Count | References |
|---|---:|---|
| CodeWarrior `asm` declaration/definition | 2 lines, one startup body | `src/main/_psstart.c:24,31` |
| GCC top-level MIPS/ELF assembly | 1 block | `src/main/_psstart.c:222–242` |
| CodeWarrior data-placement keyword | 1 macro | `include/__rts_info_t__.h:13` |
| GCC section-placement attribute | 1 macro | `include/__rts_info_t__.h:15` |
| Active game `register` declarations | 0 found | — |
| Active `#pragma` directives in surveyed game/local headers | 0 found | — |

The apparent pragma at `src/main/_psstart.c:110` is a comment. The startup manipulates MIPS `sp`, `gp`, CP0 and return-address registers; it must be replaced by native startup, not assembled for arm64.

The GCC section spelling `section(".data")` is ELF-oriented. Mach-O placement requires its own section naming or a different mechanism.

### Used GTE implementations: 11 macros in each toolchain header

```text
include/gcc/inline_n.h:9,30,228,294,362,498,504,976,978,1152,1382
include/mwcc/inline_n.h:45,66,290,373,436,540,547,1082,1088,1272,1511
```

CodeWarrior uses `__evaluate`, `__asm_start`, and `__asm_end`; GCC uses MIPS inline assembly. Both implementations need replacement.

### All 181 direct macro-call lines

```text
src/main/btl.c:576-578
src/main/effect_common.c:509-512
src/main/graphics.c:56,57
src/main/item.c:2483-2486,2538-2544
src/main/map.c:3068-3070,3076-3078,3662-3664,3893-3895,4367-4369,4371-4373,4482-4484,4486-4488,4880-4885,5139-5144
src/main/map_object.c:1058-1060
src/main/overworld.c:445-448,945-947
src/main/tamer.c:1280-1282
src/main/utils.c:189-191
src/main/vecmath.c:149,154,162,165
src/btl/battle_effect.c:2022-2025,3034-3036
src/btl/battle_hud.c:664-670,701-707,809-815
src/btl/battle_main.c:636-638
src/btl/command_menu.c:378-380
src/fish/fish.c:977-979
src/kar/kar.c:1052-1055
src/std/std_main.c:5690-5693
src/std/std_hud.c:698-704,735-741,843-849
src/std/std_effect.c:2127-2130,3197-3199
src/vs/vs_combat.c:2570-2573
src/vs/vs_effect.c:1758-1761,2780-2782
src/vs/vs_hud.c:898-904,935-941,1051-1057
```

SDK assembly reached indirectly by SDK functions is outside this count and **unverified**.

### Build flags that cannot carry over unchanged

```text
mk/platform/psx.mk:20-25,28-30
mk/toolchain/gcc.mk:1-5
mk/toolchain/mwcc.mk:3-6,14-18
```

These include MIPS architecture/ABI flags, small-data options, GNU ELF linker scripts, and CodeWarrior options.

**Directly verified compiler rejection:** Apple Clang rejects `-fexec-charset=CP932`, located at `mk/toolchain/gcc.mk:5`. The native build needs explicit handling of game text encoding.

Other source constructs needing compiler-mode review include the zero-length array in `include/dw/model.h:19`, K&R definitions, incompatible callback conversions, and the libc redeclarations in category 6.

## 9. `setjmp`, `longjmp`, stack tricks, and recursion

### Complete nonlocal-jump inventory

- One game buffer: `src/main/script_interp.c:364`; declaration at `include/dw/script.h:241`.
- One save point: `src/main/script_interp.c:601`.
- **37 jump sites:**

```text
src/main/script_interp.c:520,618,708,770,840,843,921,948,958,1076,1293,1401,1412,1543,1546,1581,1681,1729,1752,1756,1759,1764,1773,1799,1832,1837,1864,1868,1940,1957,1961,2137,2178,2706,2715,2719,2854
```

Use native libc’s `jmp_buf`; do not reproduce the PS1 buffer layout. The retry path uses `goto setjmp_retry` at `src/main/script_interp.c:623`. No persistent serialization of `jmp_buf` was found.

### Recursion/re-entry concern

A direct-call cycle exists:

```text
src/main/script_interp.c:406  tickScript -> scriptTickChangeMap
src/main/map.c:5291           scriptTickChangeMap -> changeMap
src/main/map.c:5397           changeMap -> runMapHeadScript
src/main/script_interp.c:2240 runMapHeadScript -> tickScript
```

Actual recursion depth is **unverified** because state guards may bound it. Re-entry is particularly important because all invocations share `SCRIPT_JMP_BUF`; a nested invocation can replace the outer saved context.

The other stack mechanisms are:

```text
include/dw/script.h:153-164
src/main/script_interp.c:2368-2396
include/dw/efe.h:14-22,362
src/main/vecmath.c:259-276
src/main/_psstart.c:178-202
```

These cover the script’s eight-entry stack, EFE integer stacks, the coordinate hierarchy array, and MIPS startup’s manual stack handling. There is no evidence here establishing an unbounded ordinary self-recursive game function.

## 10. Floating point and overflow-sensitive fixed point

**Zero active US game/local-header `float` or `double` declarations and zero floating-point literals were found.** The original target explicitly uses soft-float flags: `mk/platform/psx.mk:20`.

The important arithmetic is integer/fixed point:

| Algorithm | References | Behavior to preserve |
|---|---|---|
| Custom RNG | `src/main/rng.c:13,27,28,30` | Both `uint32_t` state streams wrap at 32 bits; normalize the `long` bounds. |
| SDK-rand scaling | `src/main/math.c:5,9` | Assumes the original `rand()` range, with `>> 15`. Host libc `rand()` changes results. |
| Trig lookup helpers | `src/main/math.c:13,27,32` | Integer lookup, quadrant handling, multiplication, negation and truncation. |
| Linear interpolation | `src/main/effect_common.c:278,289–291` | Multiplication before division; do not silently widen intermediate results. |
| Curve interpolation | `src/main/effect_common.c:622,663–675` | Nested products/divisions and 32-bit address arithmetic. |
| Momentum integration | `src/main/anim.c:420,467,472,491–497` | Signed steps, Q15 shifts, accumulated integer state and arithmetic right shift. |
| Distance approximation | `src/main/vecmath.c:139,145–174` | Overflow sentinel, leading-zero normalization, squaring and shifts. |
| World-position projection | `src/main/vecmath.c:64,97–104` | Products/divisions using original vector widths. |
| Matrix-to-Euler conversion | `src/main/vecmath.c:177,207,211,217,246,248,250,252` | Signed shifts and integer division. |
| Clamped effect interpolation | `src/btl/battle_effect.c:5207`; `src/std/std_effect.c:5401`; `src/vs/vs_effect.c:4986` | `long t` changes promotions on LP64. |
| EFE variable arithmetic | `src/btl/battle_effect.c:4971–5088`; `src/std/std_effect.c:5160–5277`; `src/vs/vs_effect.c:4745–4862` | 72 helpers perform arithmetic and narrow stores through integer addresses. |

The complete shift inventory is in category 4. Multiplications without shifts are not exhaustively proven overflow-free by this survey. Retain `-fwrapv`, fixed-width intermediates, and oracle checks rather than “improving” arithmetic by promoting everything to 64 bits.

## Ranked 15 riskiest individual sites

| Rank | Site | Why it is risky |
|---:|---|---|
| 1 | `include/dw/efe.h:15` | Truncates every pointer pushed through `EFE_PUSH`, including addresses of stack locals. |
| 2 | `src/btl/battle_effect.c:2133` | Reserves 640 bytes for an EFE slot table whose native layout requires 1,280 bytes. STD/VS repeat it. |
| 3 | `src/btl/battle_effect.c:1801` | Treats loaded file bytes as an array of seven-pointer native subeffect records. |
| 4 | `src/main/model.c:3415` | Reads an MMD 32-bit disk offset as an 8-byte native `long`. |
| 5 | `src/main/map.c:4234` | TFS parser advances through loaded words using host-width `u_long`. |
| 6 | `src/main/drop_shadow.c:64` | Packed 4-byte screen-coordinate fields are exposed as `long *`; a native-width shim can corrupt the packet. |
| 7 | `src/main/main_menu.c:5010` | Save checksum covers an exact original byte schema; SDK type growth silently changes offsets and bytes. |
| 8 | `src/btl/battle_effect.c:3359` | Hardcodes a 136-byte position stride and pointer-field offsets in native objects. |
| 9 | `src/doo2/doo2.c:626` | Interprets TMD bytes as a pointer-bearing descriptor whose native size grows. |
| 10 | `src/btl/btl_bss_us.c:1001` | Initialized non-address bytes occupy pointer fields; native compilation changes their representation. Runtime meaning is unverified. |
| 11 | `src/evl/evl.c:457` | Reads a numeric timer through an 8-byte pointer-sized alias, then narrows it. |
| 12 | `src/main/file_queue.c:250` | Unconditionally dereferences `loc`, while the normal path wrapper passes `NULL`. |
| 13 | `src/main/script_interp.c:2396` | Copies an incompletely initialized stack entry on an empty pop; the function also has a non-void return type without a return value. |
| 14 | `src/main/script_interp.c:601` | One global nonlocal-jump context is shared across a possible script re-entry cycle. Runtime depth is unverified. |
| 15 | `src/shop/script_menu.c:3005` | Truncates a string address and passes the resulting integer to `strlen`; both compile-time and runtime hazards. |