//
// Vidoxide - Image acquisition for amateur astronomy
// Copyright (c) 2023 Filip Szczerek <ga.software@yahoo.com>
//
// This project is licensed under the terms of the MIT license
// (see the LICENSE file for details).
//

//!
//! Command-line argument parsing.
//!

mod cmdline {
    pub const ENABLE_LOGGING: &str = "log";
}

pub struct Args {
    pub logging: bool
}

impl Default for Args {
    fn default() -> Args {
        Args{
            logging: false
        }
    }
}

pub fn parse_command_line<I: Iterator<Item=String>>(stream: I) -> Args {
    let allowed_options = vec![
        cmdline::ENABLE_LOGGING
    ];

    // key: option name
    let mut option_values = std::collections::HashMap::<String, Vec<String>>::new();

    let mut current: Option<&mut Vec<String>> = None;

    for arg in stream.skip(1) /*skip the binary name*/ {
        if arg.starts_with("--") {
            match &arg[2..] {
                x if !allowed_options.contains(&x) => {
                    eprintln!("Unknown command-line option: {}.", x);
                    return Args::default();
                },

                opt => current = Some(option_values.entry(opt.to_string()).or_insert(vec![])),
            }
        } else {
            if current.is_none() {
                eprintln!("Unexpected value: {}.", arg);
                return Args::default();
            } else {
                (*(*current.as_mut().unwrap())).push(arg);
            }
        }
    }

    Args{
        logging: option_values.contains_key(cmdline::ENABLE_LOGGING)
    }
}
