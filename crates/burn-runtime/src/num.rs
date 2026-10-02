#[cfg(not(burn_core))]
mod imp {
    pub fn sqrt(x: f64) -> f64 {
        x.sqrt()
    }
    pub fn floor(x: f64) -> f64 {
        x.floor()
    }
    pub fn ceil(x: f64) -> f64 {
        x.ceil()
    }
    pub fn round(x: f64) -> f64 {
        x.round()
    }
    pub fn trunc(x: f64) -> f64 {
        x.trunc()
    }
    pub fn abs(x: f64) -> f64 {
        x.abs()
    }
    pub fn sin(x: f64) -> f64 {
        x.sin()
    }
    pub fn cos(x: f64) -> f64 {
        x.cos()
    }
    pub fn tan(x: f64) -> f64 {
        x.tan()
    }
    pub fn ln(x: f64) -> f64 {
        x.ln()
    }
    pub fn log10(x: f64) -> f64 {
        x.log10()
    }
    pub fn exp(x: f64) -> f64 {
        x.exp()
    }
    pub fn asin(x: f64) -> f64 {
        x.asin()
    }
    pub fn acos(x: f64) -> f64 {
        x.acos()
    }
    pub fn atan(x: f64) -> f64 {
        x.atan()
    }
    pub fn atan2(y: f64, x: f64) -> f64 {
        y.atan2(x)
    }
    pub fn pow(x: f64, y: f64) -> f64 {
        x.powf(y)
    }
}

#[cfg(burn_core)]
mod imp {
    extern "C" {
        #[link_name = "sqrt"]
        fn c_sqrt(x: f64) -> f64;
        #[link_name = "floor"]
        fn c_floor(x: f64) -> f64;
        #[link_name = "ceil"]
        fn c_ceil(x: f64) -> f64;
        #[link_name = "round"]
        fn c_round(x: f64) -> f64;
        #[link_name = "trunc"]
        fn c_trunc(x: f64) -> f64;
        #[link_name = "fabs"]
        fn c_fabs(x: f64) -> f64;
        #[link_name = "sin"]
        fn c_sin(x: f64) -> f64;
        #[link_name = "cos"]
        fn c_cos(x: f64) -> f64;
        #[link_name = "tan"]
        fn c_tan(x: f64) -> f64;
        #[link_name = "log"]
        fn c_log(x: f64) -> f64;
        #[link_name = "log10"]
        fn c_log10(x: f64) -> f64;
        #[link_name = "exp"]
        fn c_exp(x: f64) -> f64;
        #[link_name = "asin"]
        fn c_asin(x: f64) -> f64;
        #[link_name = "acos"]
        fn c_acos(x: f64) -> f64;
        #[link_name = "atan"]
        fn c_atan(x: f64) -> f64;
        #[link_name = "atan2"]
        fn c_atan2(y: f64, x: f64) -> f64;
        #[link_name = "pow"]
        fn c_pow(x: f64, y: f64) -> f64;
    }
    pub fn sqrt(x: f64) -> f64 {
        unsafe { c_sqrt(x) }
    }
    pub fn floor(x: f64) -> f64 {
        unsafe { c_floor(x) }
    }
    pub fn ceil(x: f64) -> f64 {
        unsafe { c_ceil(x) }
    }
    pub fn round(x: f64) -> f64 {
        unsafe { c_round(x) }
    }
    pub fn trunc(x: f64) -> f64 {
        unsafe { c_trunc(x) }
    }
    pub fn abs(x: f64) -> f64 {
        unsafe { c_fabs(x) }
    }
    pub fn sin(x: f64) -> f64 {
        unsafe { c_sin(x) }
    }
    pub fn cos(x: f64) -> f64 {
        unsafe { c_cos(x) }
    }
    pub fn tan(x: f64) -> f64 {
        unsafe { c_tan(x) }
    }
    pub fn ln(x: f64) -> f64 {
        unsafe { c_log(x) }
    }
    pub fn log10(x: f64) -> f64 {
        unsafe { c_log10(x) }
    }
    pub fn exp(x: f64) -> f64 {
        unsafe { c_exp(x) }
    }
    pub fn asin(x: f64) -> f64 {
        unsafe { c_asin(x) }
    }
    pub fn acos(x: f64) -> f64 {
        unsafe { c_acos(x) }
    }
    pub fn atan(x: f64) -> f64 {
        unsafe { c_atan(x) }
    }
    pub fn atan2(y: f64, x: f64) -> f64 {
        unsafe { c_atan2(y, x) }
    }
    pub fn pow(x: f64, y: f64) -> f64 {
        unsafe { c_pow(x, y) }
    }
}

pub use imp::*;

pub fn fract(x: f64) -> f64 {
    x - trunc(x)
}
