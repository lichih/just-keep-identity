import os
import shutil
import subprocess
import time

ROOT_DIR = "/tmp/jki-verification"
REMOTE_DIR = os.path.join(ROOT_DIR, "remote")
LOCAL_A = os.path.join(ROOT_DIR, "local-a")
LOCAL_B = os.path.join(ROOT_DIR, "local-b")

def run_git(cwd, args):
    subprocess.run(["git", "-C", cwd] + args, check=True, capture_output=True)

def setup():
    if os.path.exists(ROOT_DIR):
        shutil.rmtree(ROOT_DIR)
    os.makedirs(ROOT_DIR)
    
    # Setup remote
    os.makedirs(REMOTE_DIR)
    subprocess.run(["git", "-C", REMOTE_DIR, "init", "--bare"], check=True)
    
    # Setup local-a
    run_git(ROOT_DIR, ["clone", REMOTE_DIR, "local-a"])
    run_git(LOCAL_A, ["config", "user.email", "test@example.com"])
    run_git(LOCAL_A, ["config", "user.name", "Test User"])
    with open(os.path.join(LOCAL_A, "vault.metadata.json"), "w") as f:
        f.write('{"accounts": []}')
    run_git(LOCAL_A, ["add", "."])
    run_git(LOCAL_A, ["commit", "-m", "initial"])
    run_git(LOCAL_A, ["push", "origin", "HEAD:main"])
    
    # Setup local-b
    run_git(ROOT_DIR, ["clone", REMOTE_DIR, "local-b"])
    run_git(LOCAL_B, ["config", "user.email", "test@example.com"])
    run_git(LOCAL_B, ["config", "user.name", "Test User"])

def test_sync_conflict_auto_resolve():
    print("Testing sync conflict auto-resolve...")
    
    # Update local-a and push
    with open(os.path.join(LOCAL_A, "vault.metadata.json"), "w") as f:
        f.write('{"accounts": [{"id": "a"}]}')
    run_git(LOCAL_A, ["add", "."])
    run_git(LOCAL_A, ["commit", "-m", "remote change"])
    run_git(LOCAL_A, ["push", "origin", "HEAD:main"])
    
    # Update local-b (conflict)
    with open(os.path.join(LOCAL_B, "vault.metadata.json"), "w") as f:
        f.write('{"accounts": [{"id": "b"}]}')
    # We don't commit in local-b manually, jkim sync will do it.
    
    # Run jkim sync --default in local-b
    print("Running jkim sync --default in local-b...")
    env = os.environ.copy()
    env["JKI_HOME"] = LOCAL_B
    # Compile jkim first
    subprocess.run(["cargo", "build", "-p", "jkim"], check=True)
    jkim_bin = os.path.join(os.getcwd(), "target/debug/jkim")
    
    proc = subprocess.run([jkim_bin, "sync", "--default"], env=env, capture_output=True, text=True)
    print("STDOUT:", proc.stdout)
    print("STDERR:", proc.stderr)
    
    if proc.returncode != 0:
        print("Error: jkim sync failed")
        exit(1)
        
    if "Conflicts resolved and rebase completed" not in proc.stdout:
        print("Error: Expected conflict resolution message not found")
        exit(1)
        
    # Check if backup file exists
    if not os.path.exists(os.path.join(LOCAL_B, "vault.metadata.json.conflict")):
        print("Error: Backup file vault.metadata.json.conflict not found")
        exit(1)
        
    # Check if vault.metadata.json has local content (prefer local)
    with open(os.path.join(LOCAL_B, "vault.metadata.json"), "r") as f:
        content = f.read()
        if '"id": "b"' not in content:
            print("Error: Local content not preserved in vault.metadata.json")
            exit(1)
            
    print("Success: Conflict auto-resolved and local changes preserved!")

if __name__ == "__main__":
    setup()
    test_sync_conflict_auto_resolve()
