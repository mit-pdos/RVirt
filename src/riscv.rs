

#[macro_export]
macro_rules! reg {
    ( $r:ident ) => {
        {
            let value: usize;
            #[allow(unused_unsafe)]
            unsafe { asm!(concat!("mv $0, ", stringify!($r)) : "=r"(value)) };
            value
        }
    };
}

#[macro_export]
macro_rules! csrr {
    ( $r:ident ) => {
        {
            let value: usize;
            #[allow(unused_unsafe)]
            unsafe { asm!("csrr $0, $1" : "=r"(value) : "i"(crate::csr::$r)) };
            value
        }
    };
}

#[macro_export]
macro_rules! csrw {
    ( $r:ident, $x:expr ) => {
        {
            let x: usize = $x;
            #[allow(unused_unsafe)]
            unsafe { asm!("csrw $0, $1" :: "i"(crate::csr::$r), "r"(x)) };
        }
    };
}

#[macro_export]
macro_rules! csrwi {
    ( $r:ident, $x:expr ) => {
        {
            const X: usize = $x;
            #[allow(unused_unsafe)]
            unsafe { asm!("li t0, $1
                            csrw $0, t0"
                          :
                          : "i"(crate::csr::$r), "i"(X)
                          : "t0"
                          : "volatile") };
        }
    };
}

#[macro_export]
macro_rules! csrs {
    ( $r:ident, $x:expr ) => {
        {
            let x: usize = $x;
            #[allow(unused_unsafe)]
            unsafe { asm!("csrs $0, $1" :: "i"(crate::csr::$r), "r"(x)) };
        }
    };
}

#[macro_export]
macro_rules! csrsi {
    ( $r:ident, $x:expr ) => {
        {
            const X: usize = $x;
            #[allow(unused_unsafe)]
            unsafe { asm!("li t0, $1
                           csrs $0, t0"
                          :
                          : "i"(crate::csr::$r), "i"(X)
                          : "t0"
                          : "volatile") };
        }
    };
}

#[macro_export]
macro_rules! csrc {
    ( $r:ident, $x:expr ) => {
        {
            let x: usize = $x;
            #[allow(unused_unsafe)]
            unsafe { asm!("csrc $0, $1" :: "i"(crate::csr::$r), "r"(x)) };
        }
    };
}

// #[macro_export]
// macro_rules! csrci {
//     ( $r:ident, $x:expr ) => {
//         {
//             const X: usize = $x;
//             #[allow(unused_unsafe)]
//             unsafe { asm!("csrci $0, $1" :: "i"(crate::csr::$r), "i"(X)) };
//         }
//     };
// }
