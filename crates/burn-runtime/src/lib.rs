#![allow(clippy::missing_safety_doc)]

pub mod api;
pub mod fmt;
pub mod fx;
pub mod http;
pub mod io;
pub mod json;
pub mod meta;
pub mod obj;
pub mod rc;
pub mod signal;
pub mod task;
pub mod time;

macro_rules! runtime_fns {
    ($( $variant:ident => $sym:ident = $path:path [$($arg:ident),*] ; )*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum RtFn { $($variant),* }

        impl RtFn {
            pub fn symbol(self) -> &'static str {
                match self { $(RtFn::$variant => stringify!($sym)),* }
            }

            pub fn argc(self) -> usize {
                match self { $(RtFn::$variant => { let v: &[&str] = &[$(stringify!($arg)),*]; v.len() }),* }
            }

            #[inline]
            pub fn call(self, a: &[u64]) -> u64 {
                match self {
                    $(RtFn::$variant => {
                        let mut _i = 0usize;
                        $path($({ let _ = stringify!($arg); let v = a[_i]; _i += 1; v }),*)
                    }),*
                }
            }

            pub fn all() -> &'static [RtFn] {
                &[$(RtFn::$variant),*]
            }
        }

        pub mod ffi {
            $(
                #[no_mangle]
                pub extern "C" fn $sym($($arg: u64),*) -> u64 {
                    $path($($arg),*)
                }
            )*
        }
    };
}

runtime_fns! {
    Init => burn_rt_init = crate::api::rt_init [a, b, c, d, e, f];
    SetArgs => burn_rt_set_args = crate::api::set_args_native [a, b];
    Exit => burn_rt_exit = crate::api::rt_exit [a];
    StrConcat => burn_str_concat = crate::api::str_concat [a, b];
    StrEq => burn_str_eq = crate::api::str_eq [a, b];
    StrCmp => burn_str_cmp = crate::api::str_cmp [a, b];
    StrLen => burn_str_len = crate::api::str_len [a];
    StrIndex => burn_str_index = crate::api::str_index [a, b, c];
    StrSub => burn_str_sub = crate::api::str_sub [a, b, c];
    StrFind => burn_str_find = crate::api::str_find [a, b];
    StrContains => burn_str_contains = crate::api::str_contains [a, b];
    StrReplace => burn_str_replace = crate::api::str_replace [a, b, c];
    StrSplit => burn_str_split = crate::api::str_split [a, b];
    StrTrim => burn_str_trim = crate::api::str_trim [a];
    StrUpper => burn_str_upper = crate::api::str_upper [a];
    StrLower => burn_str_lower = crate::api::str_lower [a];
    StrStarts => burn_str_starts = crate::api::str_starts [a, b];
    StrEnds => burn_str_ends = crate::api::str_ends [a, b];
    StrRepeat => burn_str_repeat = crate::api::str_repeat [a, b];
    StrChars => burn_str_chars = crate::api::str_chars [a];
    StrCode => burn_str_code = crate::api::str_code [a];
    CharClass => burn_char_class = crate::api::char_class [a, b];
    StrFromCode => burn_str_from_code = crate::api::str_from_code [a];
    ToStr => burn_to_str = crate::api::to_str [a, b];
    ParseInt => burn_parse_int = crate::api::parse_int [a, b];
    ParseFloat => burn_parse_float = crate::api::parse_float [a, b];
    IsIntStr => burn_is_int_str = crate::api::is_int_str [a];
    IsFloatStr => burn_is_float_str = crate::api::is_float_str [a];
    Print => burn_print = crate::api::print [a];
    PrintRaw => burn_print_raw = crate::api::print_raw [a];
    Input => burn_input = crate::api::input [a];
    PrintErr => burn_print_err = crate::api::print_err [a];
    ReadStdin => burn_read_stdin = crate::api::read_stdin [];
    ArrNew => burn_arr_new = crate::api::arr_new [a, b];
    ArrLen => burn_arr_len = crate::api::arr_len [a];
    ArrGet => burn_arr_get = crate::api::arr_get [a, b, c];
    ArrSet => burn_arr_set = crate::api::arr_set [a, b, c, d];
    ArrPush => burn_arr_push = crate::api::arr_push [a, b];
    ArrPop => burn_arr_pop = crate::api::arr_pop [a, b];
    ArrInsert => burn_arr_insert = crate::api::arr_insert [a, b, c, d];
    ArrRemove => burn_arr_remove = crate::api::arr_remove [a, b, c];
    ArrConcat => burn_arr_concat = crate::api::arr_concat [a, b];
    ArrSlice => burn_arr_slice = crate::api::arr_slice [a, b, c];
    ArrCopy => burn_arr_copy = crate::api::arr_copy [a];
    ArrIndexOf => burn_arr_index_of = crate::api::arr_index_of [a, b, c];
    ArrContains => burn_arr_contains = crate::api::arr_contains [a, b, c];
    ArrJoin => burn_arr_join = crate::api::arr_join [a, b, c];
    ArrReverse => burn_arr_reverse = crate::api::arr_reverse [a];
    ArrSort => burn_arr_sort = crate::api::arr_sort [a, b];
    ArrClear => burn_arr_clear = crate::api::arr_clear [a];
    MapNew => burn_map_new = crate::api::map_new_obj [a];
    MapSet => burn_map_set = crate::api::map_set [a, b, c];
    MapGet => burn_map_get = crate::api::map_get [a, b, c];
    MapGetOr => burn_map_get_or = crate::api::map_get_or [a, b, c];
    MapFind => burn_map_find = crate::api::map_find [a, b, c];
    MapHas => burn_map_has = crate::api::map_has [a, b];
    MapRemove => burn_map_remove = crate::api::map_remove [a, b];
    MapKeys => burn_map_keys = crate::api::map_keys [a, b];
    MapValues => burn_map_values = crate::api::map_values [a, b];
    MapLen => burn_map_len = crate::api::map_len [a];
    StructNew => burn_struct_new = crate::api::struct_alloc [a, b];
    Box => burn_box = crate::api::box_value [a, b];
    IsType => burn_is_type = crate::api::is_type [a, b, c];
    Cast => burn_cast = crate::api::cast [a, b, c, d];
    Unwrap => burn_unwrap = crate::api::unwrap [a, b, c];
    Destroy => burn_destroy = crate::api::destroy [a, b];
    Alive => burn_alive = crate::api::alive [a, b];
    Eq => burn_eq = crate::api::eq [a, b, c];
    Cmp => burn_cmp = crate::api::cmp [a, b, c];
    TypeName => burn_type_name = crate::api::type_name [a, b];
    AnyIndex => burn_any_index = crate::api::any_index [a, b, c, d];
    AnyLen => burn_any_len = crate::api::any_len [a, b];
    FMath => burn_fmath = crate::api::fmath [a, b];
    FPow => burn_fpow = crate::api::fpow [a, b];
    FAtan2 => burn_fatan2 = crate::api::fatan2 [a, b];
    FMod => burn_fmod = crate::api::fmod [a, b];
    IPow => burn_ipow = crate::api::ipow [a, b];
    IAbs => burn_iabs = crate::api::iabs [a];
    IMin => burn_imin = crate::api::imin [a, b];
    IMax => burn_imax = crate::api::imax [a, b];
    FMin => burn_fmin = crate::api::fmin [a, b];
    FMax => burn_fmax = crate::api::fmax [a, b];
    Random => burn_random = crate::api::random [];
    RandomInt => burn_random_int = crate::api::random_int [a, b];
    Seed => burn_seed = crate::api::seed [a];
    NowMs => burn_now_ms = crate::api::now_ms [];
    NowSec => burn_now_sec = crate::api::now_sec [];
    ClockNs => burn_clock_ns = crate::api::clock_ns [];
    Sleep => burn_sleep = crate::api::sleep_ms [a];
    LocalTime => burn_local_time = crate::api::local_time [];
    HttpRequest => burn_http_request = crate::api::http_request [a, b, c, d];
    JsonParse => burn_json_parse = crate::api::json_parse [a, b];
    JsonStringify => burn_json_stringify = crate::api::json_stringify [a, b];
    ReadFile => burn_read_file = crate::api::read_file [a, b];
    WriteFile => burn_write_file = crate::api::write_file [a, b];
    AppendFile => burn_append_file = crate::api::append_file [a, b];
    FileExists => burn_file_exists = crate::api::file_exists [a];
    Env => burn_env = crate::api::env_var [a];
    Args => burn_args = crate::api::args [];
    Exec => burn_exec = crate::api::exec [a, b, c];
    FsOp => burn_fs_op = crate::api::fs_op [a, b, c];
    ListDir => burn_list_dir = crate::api::list_dir [a];
    Cwd => burn_cwd = crate::api::cwd [];
    ExitNow => burn_exit_now = crate::api::exit_now [a];
    Panic => burn_panic = crate::api::panic [a, b];
    Assert => burn_assert = crate::api::assert [a, b, c];
    ErrIndex => burn_err_index = crate::api::err_index [a, b, c];
    ErrDivZero => burn_err_divzero = crate::api::err_divzero [a];
    ShiftCheck => burn_shift_check = crate::api::shift_check [a, b];
    ErrShift => burn_err_shift = crate::api::err_shift [a, b];
    ErrOverflow => burn_err_overflow = crate::api::err_overflow [a];
    ErrNull => burn_err_null = crate::api::err_null [a];
    ErrReturn => burn_err_return = crate::api::err_return [a];
    Spawn => burn_spawn = crate::api::spawn_native [a, b, c, d];
    Await => burn_await = crate::api::await_future [a];
    GcCollect => burn_gc_collect = crate::api::gc_collect [];
    Retain => burn_retain = crate::api::rc_retain [a];
    Release => burn_release = crate::api::rc_release [a];
    ReleaseZero => burn_release_zero = crate::api::rc_release_zero [a];
    PossibleRoot => burn_possible_root = crate::api::rc_possible_root [a];
}
