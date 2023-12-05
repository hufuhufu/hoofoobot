macro_rules! re_export {
    ($name:ident) => {
        pub mod $name;
        pub use $name::*;
    };
}

// Public commands
re_export!(graveyard);
re_export!(hello);
re_export!(rank);
re_export!(register);
re_export!(set_afk_channel);

// Owners only commands
re_export!(gtfo);
re_export!(incr_score);
