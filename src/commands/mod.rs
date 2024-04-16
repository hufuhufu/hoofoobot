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
re_export!(set_afk_channel);

// Admins only commands
re_export!(settings);

// Owners only commands
re_export!(gtfo);
re_export!(incr_score);
re_export!(voice_state);
re_export!(register);
