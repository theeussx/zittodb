//! Heuristic risk classification for lines typed into the Android shell.
//!
//! IMPORTANT — what this is and is not:
//! * It is a guardrail against ACCIDENTS (typing `rm -rf /sdcard/DCIM`).
//! * It is NOT a security boundary. A shell can always hide intent (variables,
//!   `eval`, scripts already on the device). The real boundary is what the
//!   ADB shell user may do on the device.
//! * Anything the classifier does not recognise is `Modify`, never `Read`
//!   (same philosophy as the debloat risk table: unknown is never safe).
//! * Wrappers it cannot look inside (`sh -c`, `su`, `eval`, command
//!   substitution…) are `Destructive`: the user must confirm explicitly.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellRisk {
    /// Only reads state.
    Read,
    /// Changes device state in a way that is normally reversible.
    Modify,
    /// Data loss, reboot or a command that cannot be analysed.
    Destructive,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShellAssessment {
    pub risk: ShellRisk,
    /// Stable machine codes explaining the verdict (localised by the UI).
    pub reasons: Vec<String>,
}

/// Word the user must type to run a `Destructive` line.
pub const SHELL_CONFIRMATION_WORD: &str = "EXECUTAR";

const READ_COMMANDS: &[&str] = &[
    "ls",
    "cat",
    "getprop",
    "dumpsys",
    "df",
    "du",
    "uname",
    "id",
    "whoami",
    "pwd",
    "echo",
    "date",
    "uptime",
    "ps",
    "stat",
    "file",
    "wc",
    "head",
    "tail",
    "grep",
    "getenforce",
    "printenv",
    "env",
    "free",
    "lsof",
    "ip",
    "ifconfig",
    "netstat",
    "which",
    "readlink",
    "basename",
    "dirname",
    "md5sum",
    "sha1sum",
    "sha256sum",
    "cmp",
    "diff",
    "sort",
    "uniq",
    "cut",
    "tr",
    "true",
    "logcat",
    "getevent",
    "lshal",
    "service",
    "settings",
    "pm",
    "cmd",
    "wm",
    "am",
    "find",
    "mount",
];

const DESTRUCTIVE_COMMANDS: &[&str] = &[
    "rm",
    "rmdir",
    "dd",
    "mkfs",
    "mke2fs",
    "make_ext4fs",
    "format",
    "wipe",
    "reboot",
    "poweroff",
    "shutdown",
    "halt",
    "truncate",
    "shred",
    "fastboot",
    "recovery",
    "flash_image",
    "erase_image",
    "vdc",
];

/// Wrappers/interpreters whose real payload we cannot inspect.
const OPAQUE_COMMANDS: &[&str] = &[
    "sh", "bash", "ash", "mksh", "su", "eval", "exec", "source", ".", "xargs", "nohup", "busybox",
    "toybox", "timeout", "nice", "setsid", "script", "python", "python3", "perl", "sudo",
];

fn bump(risk: &mut ShellRisk, reasons: &mut Vec<String>, to: ShellRisk, why: &str) {
    if to > *risk {
        *risk = to;
    }
    if !reasons.iter().any(|r| r == why) {
        reasons.push(why.to_string());
    }
}

/// Splits on `;`, `&&`, `||`, `|` and `&` outside of quotes. Returns `None`
/// when quoting is unbalanced (treated as opaque by the caller).
fn split_segments(line: &str) -> Option<Vec<String>> {
    let mut segs = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                cur.push(c);
                if c == q {
                    quote = None;
                } else if c == '\\' && q == '"' {
                    if let Some(n) = chars.next() {
                        cur.push(n);
                    }
                }
            }
            None => match c {
                '\'' | '"' => {
                    quote = Some(c);
                    cur.push(c);
                }
                '\\' => {
                    cur.push(c);
                    if let Some(n) = chars.next() {
                        cur.push(n);
                    }
                }
                ';' | '|' | '&' => {
                    // `2>&1` and `>&2` are redirections, not separators.
                    if c == '&' && cur.ends_with('>') {
                        cur.push(c);
                        continue;
                    }
                    if matches!(chars.peek(), Some('&') | Some('|')) {
                        chars.next();
                    }
                    segs.push(std::mem::take(&mut cur));
                }
                _ => cur.push(c),
            },
        }
    }
    if quote.is_some() {
        return None;
    }
    segs.push(cur);
    Some(segs.into_iter().filter(|s| !s.trim().is_empty()).collect())
}

fn words(segment: &str) -> Vec<String> {
    segment
        .split_whitespace()
        .map(|w| w.trim_matches(|c| c == '\'' || c == '"').to_string())
        .collect()
}

fn has_write_redirect(segment: &str) -> bool {
    let mut prev = ' ';
    let mut rest = segment.chars().peekable();
    while let Some(c) = rest.next() {
        if c == '>' && prev != '>' {
            // `>&N` duplicates an fd, `>/dev/null` discards: neither writes data.
            let tail: String = rest.clone().collect();
            let t = tail.trim_start_matches('>').trim_start();
            if t.starts_with('&') || t.starts_with("/dev/null") {
                prev = c;
                continue;
            }
            return true;
        }
        prev = c;
    }
    false
}

fn classify_segment(segment: &str, risk: &mut ShellRisk, reasons: &mut Vec<String>) {
    let mut w = words(segment);
    // Leading `VAR=value` assignments do not change the command.
    while w
        .first()
        .is_some_and(|x| x.contains('=') && !x.starts_with('-'))
    {
        w.remove(0);
    }
    let Some(cmd) = w
        .first()
        .map(|s| s.rsplit('/').next().unwrap_or(s).to_string())
    else {
        return;
    };
    let sub = w.get(1).map(String::as_str).unwrap_or("");
    let args = &w[1..];

    if has_write_redirect(segment) {
        bump(risk, reasons, ShellRisk::Modify, "redirect_write");
    }

    if OPAQUE_COMMANDS.contains(&cmd.as_str()) {
        bump(risk, reasons, ShellRisk::Destructive, "opaque_wrapper");
        return;
    }
    if DESTRUCTIVE_COMMANDS.contains(&cmd.as_str()) {
        bump(risk, reasons, ShellRisk::Destructive, &format!("cmd_{cmd}"));
        return;
    }

    match cmd.as_str() {
        "pm" => match sub {
            "list" | "path" | "dump" | "get-install-location" | "resolve-activity" | "info" => {}
            "uninstall" | "clear" | "trim-caches" | "uninstall-system-updates" => {
                bump(risk, reasons, ShellRisk::Destructive, "pm_data_loss");
            }
            _ => bump(risk, reasons, ShellRisk::Modify, "pm_change"),
        },
        "cmd" => match (sub, w.get(2).map(String::as_str).unwrap_or("")) {
            ("package", "list") | ("package", "path") | ("package", "dump") => {}
            ("package", "uninstall") | ("package", "clear") => {
                bump(risk, reasons, ShellRisk::Destructive, "pm_data_loss");
            }
            _ => bump(risk, reasons, ShellRisk::Modify, "cmd_change"),
        },
        "settings" => match sub {
            "get" | "list" => {}
            _ => bump(risk, reasons, ShellRisk::Modify, "settings_change"),
        },
        "wm" => {
            // `wm size` / `wm density` print; any argument or `reset` changes.
            if args.len() > 1 || matches!(sub, "reset" | "overscan" | "scaling") {
                bump(risk, reasons, ShellRisk::Modify, "wm_change");
            }
        }
        "am" => match sub {
            "stack" | "get-config" | "get-current-user" => {}
            _ => bump(risk, reasons, ShellRisk::Modify, "am_change"),
        },
        "service" => {
            if sub != "list" && sub != "check" {
                bump(risk, reasons, ShellRisk::Modify, "service_call");
            }
        }
        "mount" => {
            if !args.is_empty() {
                bump(risk, reasons, ShellRisk::Destructive, "mount_change");
            }
        }
        "find" => {
            if args
                .iter()
                .any(|a| matches!(a.as_str(), "-delete" | "-exec" | "-execdir" | "-ok"))
            {
                bump(risk, reasons, ShellRisk::Destructive, "find_action");
            }
        }
        "ip" => {
            let changes = ["add", "del", "delete", "flush", "set", "change", "replace"];
            if args.iter().any(|a| changes.contains(&a.as_str())) {
                bump(risk, reasons, ShellRisk::Modify, "ip_change");
            }
        }
        "logcat" => {
            if args.iter().any(|a| a == "-c") {
                bump(risk, reasons, ShellRisk::Modify, "logcat_clear");
            }
        }
        c if READ_COMMANDS.contains(&c) => {}
        _ => bump(risk, reasons, ShellRisk::Modify, "unknown_command"),
    }
}

/// Classifies one shell line. Never returns `Read` for something it does not
/// positively recognise.
pub fn classify_shell_line(line: &str) -> ShellAssessment {
    let mut risk = ShellRisk::Read;
    let mut reasons: Vec<String> = Vec::new();

    // Command substitution and process substitution hide the real command.
    if line.contains("$(") || line.contains('`') || line.contains("<(") || line.contains(">(") {
        bump(
            &mut risk,
            &mut reasons,
            ShellRisk::Destructive,
            "opaque_substitution",
        );
    }

    match split_segments(line) {
        None => bump(
            &mut risk,
            &mut reasons,
            ShellRisk::Destructive,
            "unbalanced_quotes",
        ),
        Some(segs) if segs.is_empty() => {}
        Some(segs) => {
            for seg in &segs {
                classify_segment(seg, &mut risk, &mut reasons);
            }
        }
    }

    ShellAssessment { risk, reasons }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn risk(l: &str) -> ShellRisk {
        classify_shell_line(l).risk
    }

    #[test]
    fn plain_reads_are_read() {
        for l in [
            "ls -la /sdcard",
            "getprop ro.product.model",
            "dumpsys battery",
            "pm list packages -3",
            "settings get global adb_enabled",
            "cat /proc/meminfo | grep MemFree",
            "df -h /data 2>&1",
            "ls /sdcard > /dev/null",
            "wm size",
            "ip addr show",
            "find /sdcard -name '*.jpg'",
        ] {
            assert_eq!(risk(l), ShellRisk::Read, "{l}");
        }
    }

    #[test]
    fn state_changes_are_modify() {
        for l in [
            "settings put global adb_enabled 1",
            "pm disable-user --user 0 com.x.y",
            "am start -n com.x/.Main",
            "input tap 10 10",
            "setprop debug.foo 1",
            "echo hi > /sdcard/a.txt",
            "wm size 1080x1920",
            "mv /sdcard/a /sdcard/b",
            "cp /sdcard/a /sdcard/b",
            "logcat -c",
            "chmod 777 /sdcard/x",
        ] {
            assert_eq!(risk(l), ShellRisk::Modify, "{l}");
        }
    }

    #[test]
    fn data_loss_and_reboot_are_destructive() {
        for l in [
            "rm -rf /sdcard/DCIM",
            "rm /sdcard/a.txt",
            "pm uninstall --user 0 com.x.y",
            "pm clear com.x.y",
            "cmd package uninstall com.x.y",
            "reboot",
            "reboot bootloader",
            "dd if=/dev/zero of=/sdcard/x",
            "find /sdcard -name '*.tmp' -delete",
            "mount -o remount,rw /",
            "ls; rm -rf /sdcard/x",
            "ls && reboot",
            "cat x | sh",
        ] {
            assert_eq!(risk(l), ShellRisk::Destructive, "{l}");
        }
    }

    #[test]
    fn opaque_wrappers_require_confirmation() {
        for l in [
            "sh -c 'ls'",
            "su -c id",
            "su",
            "eval ls",
            "echo $(rm -rf /sdcard/x)",
            "echo `reboot`",
            "ls 'unterminated",
            "xargs rm < list",
            "FOO=1 sh -c ls",
        ] {
            assert_eq!(risk(l), ShellRisk::Destructive, "{l}");
        }
    }

    #[test]
    fn unknown_commands_are_never_read() {
        let a = classify_shell_line("frobnicate --now");
        assert_eq!(a.risk, ShellRisk::Modify);
        assert!(a.reasons.iter().any(|r| r == "unknown_command"));
    }

    #[test]
    fn quotes_do_not_hide_or_invent_separators() {
        // The `;` is inside quotes: one `echo`, so it only reads.
        assert_eq!(risk("echo 'a; rm -rf /sdcard'"), ShellRisk::Read);
        // A path with a quoted `rm` as an argument is still just `ls`.
        assert_eq!(risk("ls \"rm\""), ShellRisk::Read);
    }

    #[test]
    fn the_highest_segment_wins_and_reasons_are_reported() {
        let a = classify_shell_line("ls; settings put global x 1; rm -rf /sdcard/x");
        assert_eq!(a.risk, ShellRisk::Destructive);
        assert!(a.reasons.iter().any(|r| r == "settings_change"));
        assert!(a.reasons.iter().any(|r| r == "cmd_rm"));
    }

    #[test]
    fn absolute_paths_to_binaries_are_normalised() {
        assert_eq!(risk("/system/bin/rm /sdcard/x"), ShellRisk::Destructive);
        assert_eq!(risk("/system/bin/ls /sdcard"), ShellRisk::Read);
    }
}
