//! Deliberately deterministic CLI protocol fixture, not a real model.
use std::io::{Read, Write};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "--fixture-child") {
        let file = args.get(2).unwrap();
        loop {
            std::fs::write(file, "heartbeat").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    if args.iter().any(|a| a == "--version") {
        println!("fixture-claude-windows");
        return;
    }
    let session = args
        .windows(2)
        .find(|a| a[0] == "--session-id" || a[0] == "--resume")
        .map(|a| a[1].as_str())
        .unwrap_or("");
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    if input.contains("wait-for-cancel") {
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--fixture-child", "task-child-heartbeat"])
            .spawn()
            .unwrap();
        std::fs::write("task-child.pid", child.id().to_string()).unwrap();
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    assert!(std::path::Path::new("README.md").exists());
    let followup = args.iter().any(|a| a == "--resume");
    println!(
        "{{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"{session}\",\"model\":\"fixture-windows-model\"}}"
    );
    println!(
        "{{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"session_id\":\"{session}\",\"result\":\"Windows fixture {}\"}}",
        if followup { "resumed" } else { "started" }
    );
    std::io::stdout().flush().unwrap();
}
