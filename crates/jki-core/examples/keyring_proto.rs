use keyring::Entry;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let service = "jki";
    let user = "master-key";
    let test_password = "test-secret-value-1234";

    println!("--- Keyring Prototype ---");
    println!("Service: {}", service);
    println!("User: {}", user);

    // Initialize entry
    let entry = Entry::new(service, user)?;

    // 1. SET
    println!("
[1] Setting password...");
    entry.set_password(test_password)?;
    println!("Successfully set password.");

    // 2. GET
    println!("
[2] Getting password...");
    let retrieved = entry.get_password()?;
    println!("Retrieved password: {}", retrieved);

    if retrieved == test_password {
        println!("Verification SUCCESS: Retrieved password matches original.");
    } else {
        println!("Verification FAILURE: Retrieved password does NOT match original!");
        return Err("Password mismatch".into());
    }

    // 3. DELETE
    println!("
[3] Deleting credential...");
    entry.delete_credential()?;
    println!("Successfully deleted credential.");

    // Verify deletion
    println!("
[4] Verifying deletion...");
    match entry.get_password() {
        Err(e) => {
            println!("Got expected error after deletion: {:?}", e);
            println!("Deletion verified.");
        }
        Ok(_) => {
            println!("ERROR: Password still exists after deletion!");
            return Err("Deletion failed".into());
        }
    }

    println!("
--- Prototype completed successfully ---");
    Ok(())
}
