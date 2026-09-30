//! Hand the terminal to a shell command, then take it back exactly where we left off.
//! tb never exits: the tree, cursor and camera stay in memory while the child runs.

use std::io::{self, stdout, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use ratatui::crossterm::cursor::Show;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::DefaultTerminal;

/// One-liners that finish faster than this wait for a key, so `ls` output stays readable.
/// Anything longer (claude, vim, a full shell) was interactive: straight back to the tree.
const QUICK: Duration = Duration::from_secs(3);

pub enum Run {
    /// `!` prompt: `$SHELL -ic LINE`, so aliases and functions from the rc file work.
    Cmd(String),
    /// `s`: a full interactive shell; exit / ctrl-d returns.
    Shell,
}

/// A handler, not SIG_IGN: handlers reset to default across exec, so the child still
/// dies on ctrl-c while tb (same foreground group) shrugs it off.
extern "C" fn shrug(_: libc::c_int) {}

pub fn run(term: &mut DefaultTerminal, dir: &Path, sel: &Path, what: &Run) -> io::Result<()> {
    execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen, Show)?;
    disable_raw_mode()?;
    // Cooked-mode settings as tb left them; a child that crashes mid-raw can't leak its mode back.
    let saved = unsafe {
        let mut t = std::mem::zeroed::<libc::termios>();
        (libc::tcgetattr(0, &mut t) == 0).then_some(t)
    };

    let shell = std::env::var_os("SHELL").unwrap_or_else(|| "bash".into());
    let mut cmd = Command::new(&shell);
    cmd.current_dir(dir).env("f", sel);
    let dim = |s: String| println!("\x1b[2m{s}\x1b[0m");
    match what {
        Run::Cmd(line) => {
            dim(format!("{} $ {line}", crate::ui::tilde(dir)));
            cmd.arg("-ic").arg(line);
        }
        Run::Shell => dim(format!("{} · exit or ctrl-d returns to tb", crate::ui::tilde(dir))),
    }

    let (int, quit) = unsafe {
        (
            libc::signal(libc::SIGINT, shrug as *const () as libc::sighandler_t),
            libc::signal(libc::SIGQUIT, shrug as *const () as libc::sighandler_t),
        )
    };
    let start = Instant::now();
    let status = cmd.status();
    let took = start.elapsed();
    unsafe {
        // An interactive shell moves the terminal to its own process group and normally
        // hands it back on exit; take it back ourselves in case the child died hard.
        let ttou = libc::signal(libc::SIGTTOU, libc::SIG_IGN);
        libc::tcsetpgrp(0, libc::getpgrp());
        if let Some(t) = &saved {
            libc::tcsetattr(0, libc::TCSANOW, t);
        }
        libc::signal(libc::SIGTTOU, ttou);
    }

    enable_raw_mode()?;
    let failed = !matches!(status, Ok(s) if s.success());
    if matches!(what, Run::Cmd(_)) && (took < QUICK || status.is_err()) {
        let tag = match &status {
            Ok(s) if s.success() => "done".into(),
            Ok(s) => match (s.code(), s.signal()) {
                (Some(c), _) => format!("exit {c}"),
                (_, Some(libc::SIGINT)) => "interrupted".into(),
                (_, sig) => format!("signal {}", sig.unwrap_or(0)),
            },
            Err(e) => format!("{}: {e}", shell.to_string_lossy()),
        };
        let color = if failed { "31" } else { "2" };
        print!("\r\n\x1b[{color}m[{tag}]\x1b[0m\x1b[2m any key returns to tb\x1b[0m");
        stdout().flush()?;
        loop {
            if let Event::Key(k) = event::read()?
                && k.kind == KeyEventKind::Press
            {
                break;
            }
        }
        // End the line, so the next command's output starts on its own.
        print!("\r\n");
    }
    unsafe {
        libc::signal(libc::SIGINT, int);
        libc::signal(libc::SIGQUIT, quit);
    }
    execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    term.clear()
}
