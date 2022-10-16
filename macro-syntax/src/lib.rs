#[macro_export]
macro_rules! _bdd_decompose {
    ($env:ident :: $head:ident) => {
        $env.var("$head".into())
    };
    ($env:ident :: $head:tt & $($tail:tt)*) => {
        $env.and($crate::_bdd_decompose!($env :: $head), $crate::_bdd_decompose!($env :: $($tail)*))
    };
}

#[macro_export]
macro_rules! bdd {
    ($($expr:tt)+) => {{
        let new_bddenv = rsbdd::bdd::BDDEnv::<rsbdd::bdd::NamedSymbol>::new();

        let bdd_expr = $crate::_bdd_decompose!(new_bddenv :: $($expr)+);

        bdd_expr
    }};
}