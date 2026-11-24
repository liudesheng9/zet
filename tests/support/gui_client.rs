//! The owner's GUI over HTTP, for tests that act as the owner (Locks, the owner's Connection-tree
//! writes) against a `ScratchBox` service.

#![allow(dead_code)]

use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpStream;

pub struct Gui {
    pub port: u16,
    pub token: String,
}

impl Gui {
    /// Connect to the GUI of the service that `zt gui` (run with `cmd`) names.
    pub fn connect(mut cmd: assert_cmd::Command) -> Gui {
        let out = cmd.arg("gui").output().expect("zt gui");
        assert!(out.status.success(), "zt gui failed: {out:?}");
        let text = String::from_utf8_lossy(&out.stdout);
        let port: u16 = text
            .lines()
            .find_map(|line| line.trim().strip_prefix("zt gui: http://127.0.0.1:"))
            .and_then(|rest| rest.trim_end_matches('/').parse().ok())
            .expect("zt gui prints its URL");
        let page = page(port, "/");
        let token = page
            .split("name=\"zt-token\" content=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the page carries the GUI token")
            .to_string();
        Gui { port, token }
    }

    pub fn get_with(&self, path: &str) -> (u16, Value) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to gui");
        let request = format!(
            "GET {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nX-ZT-Token: {}\r\nContent-Length: 0\r\n\r\n",
            self.port, self.token
        );
        stream.write_all(request.as_bytes()).expect("send request");
        read_reply(stream)
    }

    pub fn get(&self, path: &str) -> Value {
        let (status, body) = self.get_with(path);
        assert_eq!(status, 200, "{path} answered {status}: {body}");
        body
    }

    pub fn post_with(&self, path: &str, body: &Value) -> (u16, Value) {
        let payload = serde_json::to_string(body).expect("JSON body");
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to gui");
        let request = format!(
            "POST {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nX-ZT-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
            self.port,
            self.token,
            payload.len()
        );
        stream.write_all(request.as_bytes()).expect("send request");
        read_reply(stream)
    }

    pub fn post(&self, path: &str, body: &Value) -> Value {
        let (status, value) = self.post_with(path, body);
        assert_eq!(status, 200, "{path} answered {status}: {value}");
        value
    }

    /// Plan a Lock of the tree at `root` and confirm it: the ordinary owner action.
    pub fn lock(&self, root: &str) -> Value {
        let request = json!({ "root": root });
        let plan = self.post("/api/lock", &request);
        assert_eq!(plan["applied"], false, "the first request only plans: {plan}");
        let mut confirmed = request.clone();
        confirmed["confirm"] = json!("lock");
        confirmed["plan_hash"] = plan["plan"]["plan_hash"].clone();
        let applied = self.post("/api/lock", &confirmed);
        assert_eq!(applied["applied"], true, "the confirmation commits: {applied}");
        applied["lock"].clone()
    }

    pub fn unlock(&self, lock_id: i64) -> Value {
        let plan = self.post("/api/unlock", &json!({ "lock_id": lock_id }));
        assert_eq!(plan["applied"], false, "unlock plans first: {plan}");
        self.post(
            "/api/unlock",
            &json!({"lock_id": lock_id, "confirm": "unlock", "plan_hash": plan["plan"]["plan_hash"]}),
        )
    }

    /// One Connection-tree tool as the owner (`/api/connection`): its answer and whether it failed.
    pub fn conn_call(&self, tool: &str, args: Value) -> (Value, bool) {
        let (status, value) = self.post_with("/api/connection", &json!({ "tool": tool, "args": args }));
        (value, status != 200)
    }

    pub fn conn(&self, tool: &str, args: Value) -> Value {
        let (value, failed) = self.conn_call(tool, args.clone());
        assert!(!failed, "{tool}({args}) failed: {value}");
        value
    }

    pub fn conn_refused(&self, tool: &str, args: Value) -> Value {
        let (value, failed) = self.conn_call(tool, args.clone());
        assert!(failed, "{tool}({args}) should fail but returned {value}");
        value
    }

    /// Plan, then confirm with the plan's word and hash.
    pub fn conn_confirmed(&self, tool: &str, args: Value) -> Value {
        let plan = self.conn(tool, args.clone());
        assert_eq!(plan["applied"], false, "{tool} plans first: {plan}");
        let mut confirmed = args;
        confirmed["confirm"] = plan["confirm"].clone();
        confirmed["plan_hash"] = plan["plan_hash"].clone();
        confirmed["expected_revision"] = plan["expected_revision"].clone();
        let done = self.conn(tool, confirmed);
        assert_eq!(done["applied"], true, "{tool} applies on confirmation: {done}");
        done
    }
}

pub fn read_reply(mut stream: TcpStream) -> (u16, Value) {
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    let status = response
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("status code");
    let start = response.find("\r\n\r\n").expect("header end") + 4;
    let body = serde_json::from_str(&response[start..]).unwrap_or(Value::Null);
    (status, body)
}

pub fn page(port: u16, path: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to gui");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )
    .expect("request page");
    let mut page = String::new();
    stream.read_to_string(&mut page).expect("read page");
    page
}
