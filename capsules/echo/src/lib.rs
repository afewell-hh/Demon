/// Minimal echo capsule. In later milestones this will be a WASM component
/// with capability-scoped interfaces. For M0 we keep it as a native lib.
pub fn echo(message: String) -> String {
    println!("{message}");
    message
}

#[cfg(test)]
mod tests {
    #[test]
    fn echoes() {
        let msg = "hi".to_string();
        let out = super::echo(msg.clone());
        assert_eq!(out, msg);
    }
}
// guard: third no-op
