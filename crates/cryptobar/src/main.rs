//! Entry point: parse the arguments and dispatch to the commands.
//!
//! Exit codes follow `sysexits.h`. A failure is written once on stderr as
//! `cryptobar: outer: cause: root`, and the code says which family it belongs to.
//!
//! | Code | Name | Meaning |
//! |---|---|---|
//! | 0 | success | the command completed |
//! | 65 | `EX_DATAERR` | an argument is invalid, such as a malformed cryptocurrency id |
//! | 69 | `EX_UNAVAILABLE` | an upstream service or a required program is unreachable |
//! | 70 | `EX_SOFTWARE` | the graphical window failed, or an internal output could not be encoded |
//! | 74 | `EX_IOERR` | the runtime state, the output or the executable could not be read or written |
//! | 78 | `EX_CONFIG` | the configuration or the XDG directories are unusable |
//!
//! Code 2 is the usage error reported by `clap` before `main` runs. When stderr
//! itself cannot be written, the failure cannot be reported and the code is 74.

mod exit;
mod failure;

use std::process::ExitCode;

use clap::Parser as _;

use cryptobar::app::App;
use cryptobar::cli::{Cli, Command};
use cryptobar::commands::{bar, chart, coins, config_cmd, nav, popup, wallet};
use cryptobar::output::{Output, ProcessOutput};
use cryptobar::render::waybar::Markup;
use cryptobar::report::Report;

use exit::Exit;
use failure::Failure;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut output = Output::process();
    match run(cli, &mut output) {
        Ok(()) => Exit::Success.into(),
        Err(failure) => report(&mut output, &failure),
    }
}

fn run(cli: Cli, output: &mut ProcessOutput) -> Result<(), Failure> {
    let Cli { config, command } = cli;
    let command = command.unwrap_or(Command::Bar {
        watch: false,
        markup: Markup::default(),
    });
    match command {
        Command::Config { action } => config_cmd::run(&action, config, output)?,
        Command::Bar { watch, markup } => bar::run(App::new(config)?, watch, markup, output)?,
        Command::Next => nav::step(&App::new(config)?, output, 1)?,
        Command::Prev => nav::step(&App::new(config)?, output, -1)?,
        Command::Select { index } => nav::select(&App::new(config)?, output, index)?,
        Command::Refresh => nav::refresh(&App::new(config)?)?,
        Command::Chart {
            coin,
            range,
            window: true,
        } => chart::spawn_terminal(&App::new(config)?, coin.as_deref(), range)?,
        Command::Chart {
            coin,
            range,
            window: false,
        } => chart::run(&App::new(config)?, coin.as_deref(), range, output)?,
        Command::Coins { term } => coins::run(&App::new(config)?, &term, output)?,
        Command::Popup => popup::run(App::new(config)?)?,
        Command::Portfolio => wallet::run(&App::new(config)?, output)?,
    }
    Ok(())
}

fn report(output: &mut ProcessOutput, failure: &Failure) -> ExitCode {
    let exit = Exit::from(failure);
    match output.warning(format_args!("cryptobar: {}", Report(failure))) {
        Ok(()) => exit.into(),
        Err(_) => Exit::IoError.into(),
    }
}
