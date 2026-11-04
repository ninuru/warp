use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use warp_util::path::LineAndColumnArg;

use super::{
    drop_command, encode_command_request, fnameescape, is_neovim, open_in_server,
    server_socket_in_dirs,
};

#[test]
fn recognizes_neovim_by_basename() {
    assert!(is_neovim("nvim"));
    assert!(is_neovim("/usr/bin/nvim"));
    assert!(is_neovim("NeoVim"));
    assert!(!is_neovim("vim"));
    assert!(!is_neovim("code"));
}

#[test]
fn escapes_special_characters_for_drop() {
    assert_eq!(fnameescape("a b.txt"), "a\\ b.txt");
    assert_eq!(fnameescape("%x#y|z"), "\\%x\\#y\\|z");
    assert_eq!(fnameescape("+leading"), "\\+leading");
    assert_eq!(fnameescape("/plain/path.rs"), "/plain/path.rs");
}

#[test]
fn builds_drop_command_with_position() {
    let file = Path::new("/src/my file.rs");
    assert_eq!(drop_command(file, None), "drop /src/my\\ file.rs");
    let line = Some(LineAndColumnArg {
        line_num: 12,
        column_num: None,
    });
    assert_eq!(drop_command(file, line), "drop +12 /src/my\\ file.rs");
    let line_col = Some(LineAndColumnArg {
        line_num: 12,
        column_num: Some(4),
    });
    assert_eq!(
        drop_command(file, line_col),
        "drop +call\\ cursor(12,4) /src/my\\ file.rs"
    );
}

#[test]
fn encodes_msgpack_request() {
    let bytes = encode_command_request(1, "drop a");
    let mut expected = vec![0x94, 0x00, 0x01, 0xac];
    expected.extend_from_slice(b"nvim_command");
    expected.extend_from_slice(&[0x91, 0xa6]);
    expected.extend_from_slice(b"drop a");
    assert_eq!(bytes, expected);

    let long = "x".repeat(40);
    let bytes = encode_command_request(300, &long);
    assert_eq!(&bytes[2..5], &[0xcd, 0x01, 0x2c]);
    assert_eq!(bytes[bytes.len() - 42..bytes.len() - 40], [0xd9, 40]);
}

#[test]
fn finds_socket_matching_pid() {
    let dir = TempDir::new().unwrap();
    let socket = dir.path().join("nvim.123.0");
    let _listener = UnixListener::bind(&socket).unwrap();
    std::fs::write(dir.path().join("nvim.456.0"), b"").unwrap();
    let dirs = vec![PathBuf::from("/nonexistent"), dir.path().to_path_buf()];

    assert_eq!(server_socket_in_dirs(&dirs, 123), Some(socket));
    assert_eq!(server_socket_in_dirs(&dirs, 456), None);
    assert_eq!(server_socket_in_dirs(&dirs, 12), None);
}

fn fake_server(reply: Vec<u8>) -> (TempDir, PathBuf, std::thread::JoinHandle<Vec<u8>>) {
    let dir = TempDir::new().unwrap();
    let socket = dir.path().join("nvim.1.0");
    let listener = UnixListener::bind(&socket).unwrap();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = vec![0u8; 256];
        let read = stream.read(&mut request).unwrap();
        request.truncate(read);
        stream.write_all(&reply).unwrap();
        request
    });
    (dir, socket, handle)
}

#[test]
fn sends_drop_command_and_accepts_nil_error() {
    let (_dir, socket, server) = fake_server(vec![0x94, 0x01, 0x01, 0xc0, 0xc0]);
    open_in_server(&socket, Path::new("/tmp/a.rs"), None).unwrap();
    let request = server.join().unwrap();
    assert_eq!(request, encode_command_request(1, "drop /tmp/a.rs"));
}

#[test]
fn reports_server_error_message() {
    let mut reply = vec![0x94, 0x01, 0x01, 0x92, 0x00, 0xa9];
    reply.extend_from_slice(b"E37: fail");
    reply.push(0xc0);
    let (_dir, socket, server) = fake_server(reply);
    let err = open_in_server(&socket, Path::new("/tmp/a.rs"), None).unwrap_err();
    server.join().unwrap();
    assert_eq!(err.to_string(), "E37: fail");
}
