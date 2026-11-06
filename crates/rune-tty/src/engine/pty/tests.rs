//! The PTY's own tests: the shell the pane picks on each platform, the integration scripts it
//! hands that shell, and the queue the reader thread fills.

use super::*;

/// The environment the shell would inherit: what the integration has to preserve.
fn lookup(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
    move |key| {
        pairs
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| OsString::from(*value))
    }
}

fn value(integration: &Integration, key: &str) -> Option<OsString> {
    integration
        .env
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, value)| value.clone())
}

const RESOURCES: &str = "/usr/share/ghostty";

/// bash is started in POSIX mode so it reads `ENV` instead of its startup files; the script
/// then recreates that sequence itself. `GHOSTTY_BASH_INJECT` is what tells it it was
/// injected, so it knows to leave POSIX mode and restore the user's history file.
#[test]
fn bash_loads_the_script_from_env_in_posix_mode() {
    let integration = integration(Path::new("/bin/bash"), Path::new(RESOURCES), &lookup(&[]))
        .expect("bash has an integration");
    assert_eq!(integration.args, vec!["--posix"]);
    assert_eq!(
        value(&integration, "GHOSTTY_BASH_INJECT").as_deref(),
        Some("1".as_ref())
    );
    assert_eq!(
        value(&integration, "ENV"),
        Some(OsString::from(
            "/usr/share/ghostty/shell-integration/bash/ghostty.bash"
        ))
    );
    assert_eq!(
        value(&integration, "GHOSTTY_BASH_ENV"),
        None,
        "there was no ENV of the user's to preserve"
    );
}

/// An ENV the user already had is handed back to the script, which restores it once it has
/// finished, so the shell after startup sees what it would have seen without the pane.
#[test]
fn bash_hands_the_users_own_env_back_to_the_script() {
    let integration = integration(
        Path::new("/bin/bash"),
        Path::new(RESOURCES),
        &lookup(&[("ENV", "/home/me/.env")]),
    )
    .expect("bash has an integration");
    assert_eq!(
        value(&integration, "GHOSTTY_BASH_ENV"),
        Some(OsString::from("/home/me/.env"))
    );
}

/// zsh reads `$ZDOTDIR/.zshenv` first, so the pane points it at the integration directory and
/// hands the user's own directory back for the script to restore.
#[test]
fn zsh_points_zdotdir_at_the_scripts() {
    let integration = integration(
        Path::new("/usr/bin/zsh"),
        Path::new(RESOURCES),
        &lookup(&[("ZDOTDIR", "/home/me/.config/zsh")]),
    )
    .expect("zsh has an integration");
    assert!(integration.args.is_empty(), "zsh needs no arguments");
    assert_eq!(
        value(&integration, "ZDOTDIR"),
        Some(OsString::from("/usr/share/ghostty/shell-integration/zsh"))
    );
    assert_eq!(
        value(&integration, "GHOSTTY_ZSH_ZDOTDIR"),
        Some(OsString::from("/home/me/.config/zsh"))
    );
}

/// fish and elvish read vendor directories from XDG_DATA_DIRS; the script takes the pane's
/// directory back out of it once it has loaded, and the rest of the list is untouched.
#[test]
fn fish_prepends_the_vendor_directory_to_xdg_data_dirs() {
    let integration = integration(
        Path::new("/usr/bin/fish"),
        Path::new(RESOURCES),
        &lookup(&[("XDG_DATA_DIRS", "/home/me/.local/share")]),
    )
    .expect("fish has an integration");
    assert!(integration.args.is_empty());
    assert_eq!(
        value(&integration, "XDG_DATA_DIRS"),
        Some(OsString::from("/usr/share/ghostty:/home/me/.local/share"))
    );
    assert_eq!(
        value(&integration, "GHOSTTY_SHELL_INTEGRATION_XDG_DIR"),
        Some(OsString::from(RESOURCES)),
        "the script needs to know which entry to remove again"
    );
}

/// A shell without a script is started exactly as it would have been.
#[test]
fn a_shell_without_a_script_is_started_untouched() {
    for shell in ["/bin/sh", "/bin/dash", "/bin/ksh", "/usr/bin/nu"] {
        assert!(
            integration(Path::new(shell), Path::new(RESOURCES), &lookup(&[])).is_none(),
            "{shell} has no integration script"
        );
    }
}

/// A Windows pane starts a shell Windows has: `/bin/sh`, the Unix fallback, is not a program
/// there, and naming it left the pane on "Starting the shell…" forever.
#[test]
fn windows_starts_powershell_and_falls_back_to_the_command_processor() {
    let none = |_: &str| None;
    assert_eq!(
        shell(true, &none, &|_| false, &|program| program == "pwsh.exe"),
        OsString::from("pwsh.exe"),
        "PowerShell 7 first"
    );
    assert_eq!(
        shell(true, &none, &|_| false, &|program| program
            == "powershell.exe"),
        OsString::from("powershell.exe"),
        "then the one Windows ships"
    );
    assert_eq!(
        shell(
            true,
            &|key: &str| {
                (key == "COMSPEC").then(|| OsString::from("C:\\Windows\\System32\\cmd.exe"))
            },
            &|_| false,
            &|_| false
        ),
        OsString::from("C:\\Windows\\System32\\cmd.exe"),
        "no PowerShell: the command processor COMSPEC names"
    );
    assert_eq!(
        shell(true, &none, &|_| false, &|_| false),
        OsString::from("cmd.exe"),
        "and cmd.exe when even COMSPEC is missing"
    );
}

/// A shell the user chose is the one that starts, on either platform, as long as Windows can
/// find it: Git Bash exports a POSIX path the Windows loader cannot start.
#[test]
fn the_users_own_shell_wins_when_it_can_start() {
    let chosen = |key: &str| (key == "SHELL").then(|| OsString::from("/usr/bin/fish"));
    assert_eq!(
        shell(true, &chosen, &|_| true, &|_| true),
        OsString::from("/usr/bin/fish")
    );
    assert_eq!(
        shell(true, &chosen, &|_| false, &|program| program == "pwsh.exe"),
        OsString::from("pwsh.exe"),
        "a shell Windows cannot start falls through to the default"
    );
}

/// Unix keeps its own rule: `SHELL`, else `/bin/sh`, with no existence check to get in the
/// way of the shell it has always started.
#[test]
fn unix_keeps_shell_and_bin_sh() {
    let none = |_: &str| None;
    assert_eq!(
        shell(false, &none, &|_| true, &|_| true),
        OsString::from("/bin/sh")
    );
    assert_eq!(
        shell(
            false,
            &|key: &str| (key == "SHELL").then(|| OsString::from("/bin/zsh")),
            &|_| false,
            &|_| false
        ),
        OsString::from("/bin/zsh"),
        "a shell Unix cannot find is still the one it tries"
    );
}

#[test]
fn the_queue_gives_one_budget_at_a_time_and_keeps_the_rest() {
    let incoming = Incoming::new();
    assert_eq!(incoming.len(), 0);
    assert!(incoming.take(64).is_empty());
    incoming.push(b"0123456789");
    assert_eq!(incoming.len(), 10);
    assert_eq!(incoming.take(4), b"0123");
    assert_eq!(incoming.len(), 6);
    assert!(
        incoming.lag() > Duration::ZERO,
        "the rest of the chunk is still waiting"
    );
    assert_eq!(incoming.take(64), b"456789");
    assert_eq!(incoming.len(), 0);
    assert_eq!(incoming.lag(), Duration::ZERO, "nothing is waiting now");
    incoming.push(b"x");
    assert!(incoming.lag() > Duration::ZERO, "a queued byte is late");
    incoming.close();
}
