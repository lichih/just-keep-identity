#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl, runtime::Object};
#[cfg(target_os = "macos")]
use block::ConcreteBlock;
#[cfg(target_os = "macos")]
use std::sync::mpsc::channel;

/// Verifies user identity using system biometric authentication (e.g., TouchID on macOS).
#[cfg(target_os = "macos")]
pub fn verify_biometric(reason: &str) -> Result<(), String> {
    use objc::runtime::Class;
    
    let cls = Class::get("LAContext").ok_or("Failed to get LAContext class. Is LocalAuthentication framework linked?")?;
    let context: *mut Object = unsafe { msg_send![cls, new] };
    
    let policy: i64 = 1; // LAPolicyDeviceOwnerAuthenticationWithBiometrics
    let mut error: *mut Object = std::ptr::null_mut();
    
    let can_evaluate: bool = unsafe { msg_send![context, canEvaluatePolicy:policy error:&mut error] };
    
    if !can_evaluate {
        return Err("Biometric authentication not available or not enrolled.".to_string());
    }
    
    let (tx, rx) = channel();
    
    // Create an NSString for the reason
    let ns_reason = to_ns_string(reason);
    
    let reply = ConcreteBlock::new(move |success: bool, _error: *mut Object| {
        tx.send(success).expect("Failed to send result through channel");
    });
    let reply = reply.copy();
    
    unsafe {
        let _: () = msg_send![context, evaluatePolicy:policy localizedReason:ns_reason reply:reply];
    }
    
    // Wait for the result from the block
    match rx.recv() {
        Ok(true) => Ok(()),
        Ok(false) => Err("Biometric authentication failed or canceled.".to_string()),
        Err(e) => Err(format!("Internal error during biometric verification: {}", e)),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn verify_biometric(_reason: &str) -> Result<(), String> {
    Err("Biometric authentication is currently only supported on macOS.".to_string())
}

#[cfg(target_os = "macos")]
fn to_ns_string(s: &str) -> *mut Object {
    use objc::runtime::Class;
    let cls = Class::get("NSString").unwrap();
    let bytes = s.as_ptr();
    let length = s.len() as u64;
    unsafe {
        let string: *mut Object = msg_send![cls, alloc];
        let string: *mut Object = msg_send![string, initWithBytes:bytes length:length encoding:4]; // 4 = NSUTF8StringEncoding
        string
    }
}
