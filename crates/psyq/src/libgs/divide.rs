//! libgs: the dividing TMD handlers (`GsDIV1`).

use super::{GsOt, Vert};
use crate::trap::unimplemented;

macro_rules! div_handlers {
    ($($name:ident)*) => {
        $(
            /// A dividing handler (placeholder).
            ///
            /// # Safety
            /// As `GsSortObject4J`'s.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn $name(
                _op: *const u32,
                _vp: *const Vert,
                _np: *const Vert,
                _pk: *mut u32,
                _n: i32,
                _shift: i32,
                _ot: *mut GsOt,
                _scratch: *mut u32,
            ) -> *mut u32 {
                unimplemented(stringify!($name))
            }
        )*
    };
}

div_handlers! { GsTMDdivTNF3 GsTMDdivTNF4 GsTMDdivTNG3 GsTMDdivTNG4 }
