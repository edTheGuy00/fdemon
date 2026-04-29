//! Debug test for settings E2E
#[cfg(test)]
mod debug_settings_test {
    use crate::e2e::pty_utils::{FdemonSession, TestFixture};
    use serial_test::serial;
    use std::time::Duration;

    #[tokio::test]
    #[serial]
    #[ignore = "Debug test - run manually"]
    async fn debug_settings_rendering() {
        let fixture = TestFixture::simple_app();
        let mut session = FdemonSession::spawn(&fixture.path()).expect("spawn fdemon");

        // Wait for header
        session.expect_header().expect("header should appear");
        eprintln!("Header found!");
        
        tokio::time::sleep(Duration::from_millis(500)).await;
        eprintln!("After 500ms sleep");
        
        // Capture what's currently in the stream
        let current = session.capture_for_snapshot().unwrap_or_default();
        eprintln!("Current content (500 chars): {}", &current[..current.len().min(500)]);
        
        // Send comma
        session.send_key(',').expect("send comma");
        eprintln!("Sent comma!");
        
        tokio::time::sleep(Duration::from_millis(200)).await;
        eprintln!("After 200ms sleep");
        
        // Try to get ANY output
        let content = session.capture_for_snapshot().unwrap_or_default();
        eprintln!("Post-comma content (1000 chars): {}", &content[..content.len().min(1000)]);
        
        // Now try the actual expect
        let result = session.expect_timeout(
            "System Settings|Auto Start|Confirm Quit",
            Duration::from_secs(5),
        );
        eprintln!("Expect result: {:?}", result.is_ok());
        
        let _ = session.quit();
    }
}
