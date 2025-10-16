pub const MAX_ZOOM_LEVEL: usize = 60;

pub const fn gen_xy_max() -> [u64; MAX_ZOOM_LEVEL + 1] {
    let mut arr = [0u64; MAX_ZOOM_LEVEL + 1];
    let mut i = 0;
    while i <= MAX_ZOOM_LEVEL {
        arr[i] = (1u64 << i) - 1;
        i += 1;
    }
    arr
}

pub const fn gen_f_min() -> [i64; MAX_ZOOM_LEVEL + 1] {
    let mut arr = [0i64; MAX_ZOOM_LEVEL + 1];
    let mut i = 0;
    while i <= MAX_ZOOM_LEVEL {
        arr[i] = -(1i64 << i);
        i += 1;
    }
    arr
}

pub const fn gen_f_max() -> [i64; MAX_ZOOM_LEVEL + 1] {
    let mut arr = [0i64; MAX_ZOOM_LEVEL + 1];
    let mut i = 0;
    while i <= MAX_ZOOM_LEVEL {
        arr[i] = (1i64 << i) - 1;
        i += 1;
    }
    arr
}

pub const XY_MAX: [u64; MAX_ZOOM_LEVEL + 1] = gen_xy_max();
pub const F_MIN: [i64; MAX_ZOOM_LEVEL + 1] = gen_f_min();
pub const F_MAX: [i64; MAX_ZOOM_LEVEL + 1] = gen_f_max();
