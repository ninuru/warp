use std::io::{self, Read, Write};
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use warp_util::path::LineAndColumnArg;

use crate::terminal::model::terminal_model::ShellProcessInfo;

const RPC_TIMEOUT: Duration = Duration::from_secs(2);
const REQUEST_ID: u32 = 1;
const MSGPACK_NIL: u8 = 0xc0;

pub fn is_neovim(editor: &str) -> bool {
    let base = Path::new(editor)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(editor)
        .to_lowercase();
    matches!(base.as_str(), "nvim" | "neovim")
}

pub fn foreground_pid(shell: &ShellProcessInfo) -> Option<u32> {
    let fd = shell.pty_leader_fd?;
    nix::unistd::tcgetpgrp(fd)
        .ok()
        .map(|pid| pid.as_raw() as u32)
        .filter(|&pid| pid > 0)
}

pub fn server_socket_for_pid(pid: u32) -> Option<PathBuf> {
    server_socket_in_dirs(&candidate_server_dirs(), pid)
}

pub(super) fn server_socket_in_dirs(dirs: &[PathBuf], pid: u32) -> Option<PathBuf> {
    let prefix = format!("nvim.{pid}.");
    dirs.iter().find_map(|dir| {
        std::fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
            let is_match = entry.file_name().to_str()?.starts_with(&prefix)
                && entry.file_type().ok()?.is_socket();
            is_match.then(|| entry.path())
        })
    })
}

fn candidate_server_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .into_iter()
        .collect();
    let Some(user) = std::env::var_os("USER").or_else(|| std::env::var_os("LOGNAME")) else {
        return dirs;
    };
    let user = user.to_string_lossy();
    let mut tmp_roots = vec![std::env::temp_dir(), PathBuf::from("/tmp")];
    tmp_roots.dedup();
    for root in tmp_roots {
        let Ok(entries) = std::fs::read_dir(root.join(format!("nvim.{user}"))) else {
            continue;
        };
        dirs.extend(
            entries
                .flatten()
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                .map(|entry| entry.path()),
        );
    }
    dirs
}

pub fn open_in_server(
    socket: &Path,
    file: &Path,
    line_col: Option<LineAndColumnArg>,
) -> io::Result<()> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(RPC_TIMEOUT))?;
    stream.set_write_timeout(Some(RPC_TIMEOUT))?;
    stream.write_all(&encode_command_request(
        REQUEST_ID,
        &drop_command(file, line_col),
    ))?;

    let mut response = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => response.extend_from_slice(&chunk[..read]),
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => {
                if response.is_empty() {
                    return Err(err);
                }
                break;
            }
        }
        if response.len() >= 4 && response[3] == MSGPACK_NIL {
            return Ok(());
        }
    }
    Err(io::Error::other(response_error_message(&response)))
}

pub(super) fn drop_command(file: &Path, line_col: Option<LineAndColumnArg>) -> String {
    let path = fnameescape(&file.to_string_lossy());
    match line_col {
        None => format!("drop {path}"),
        Some(LineAndColumnArg {
            line_num,
            column_num: None,
        }) => format!("drop +{line_num} {path}"),
        Some(LineAndColumnArg {
            line_num,
            column_num: Some(column),
        }) => format!("drop +call\\ cursor({line_num},{column}) {path}"),
    }
}

pub(super) fn fnameescape(name: &str) -> String {
    const SPECIAL: &str = " \t\n*?[{`$\\%#'\"|!<";
    let mut escaped = String::with_capacity(name.len() + 8);
    for (index, ch) in name.chars().enumerate() {
        let leading_special = index == 0 && matches!(ch, '+' | '>' | '-');
        if SPECIAL.contains(ch) || leading_special {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

pub(super) fn encode_command_request(msgid: u32, command: &str) -> Vec<u8> {
    let mut buf = vec![0x94, 0x00];
    encode_uint(&mut buf, msgid);
    encode_str(&mut buf, "nvim_command");
    buf.push(0x91);
    encode_str(&mut buf, command);
    buf
}

fn encode_uint(buf: &mut Vec<u8>, value: u32) {
    match value {
        0..=0x7f => buf.push(value as u8),
        0x80..=0xff => buf.extend_from_slice(&[0xcc, value as u8]),
        0x100..=0xffff => {
            buf.push(0xcd);
            buf.extend_from_slice(&(value as u16).to_be_bytes());
        }
        _ => {
            buf.push(0xce);
            buf.extend_from_slice(&value.to_be_bytes());
        }
    }
}

fn encode_str(buf: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    match bytes.len() {
        0..=0x1f => buf.push(0xa0 | bytes.len() as u8),
        0x20..=0xff => buf.extend_from_slice(&[0xd9, bytes.len() as u8]),
        0x100..=0xffff => {
            buf.push(0xda);
            buf.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
        }
        _ => {
            buf.push(0xdb);
            buf.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        }
    }
    buf.extend_from_slice(bytes);
}

fn response_error_message(response: &[u8]) -> String {
    let fallback = || format!("unexpected Neovim RPC response: {response:02x?}");
    if response.len() < 6 || response[3] != 0x92 {
        return fallback();
    }
    let (header, rest) = match response[5] {
        0xa0..=0xbf => (1, (response[5] & 0x1f) as usize),
        0xd9 if response.len() > 6 => (2, response[6] as usize),
        0xda if response.len() > 7 => (3, u16::from_be_bytes([response[6], response[7]]) as usize),
        _ => return fallback(),
    };
    let start = 5 + header;
    response
        .get(start..start + rest)
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .unwrap_or_else(fallback)
}

#[cfg(test)]
#[path = "neovim_tests.rs"]
mod tests;
