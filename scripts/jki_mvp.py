# /// script
# dependencies = [
#   "pyotp",
#   "pyperclip",
# ]
# ///
import json
import sys
import os
import pyotp
import pyperclip

def fuzzy_match(pattern, target):
    pattern = pattern.lower()
    target = target.lower()
    it = iter(target)
    return all(c in it for c in pattern)

def search_accounts(accounts, patterns):
    results = []
    for acc in accounts:
        # Match against name and issuer
        target_str = f"{acc.get('issuer') or ''} {acc.get('name')}".lower()
        # All patterns must fuzzy match the target string
        if all(fuzzy_match(p, target_str) for p in patterns):
            results.append(acc)
    return results

def main():
    # Load data
    vault_path = "vault.json"
    if not os.path.exists(vault_path):
        print(f"Error: {vault_path} not found. Run import script first.", file=sys.stderr)
        sys.exit(100)
    
    with open(vault_path, "r", encoding="utf-8") as f:
        vault = json.load(f)
    accounts = vault.get("accounts", [])

    # CLI Args Parsing (Manual for precision)
    args = sys.argv[1:]
    to_stdout = False
    patterns = []
    index_selection = None
    
    # Handle flags and --
    parsing_options = True
    pos_args = []
    
    i = 0
    while i < len(args):
        arg = args[i]
        if parsing_options and arg == "--":
            parsing_options = False
        elif parsing_options and arg == "-":
            to_stdout = True
        elif parsing_options and arg.startswith("-"):
            # Ignore other flags for now
            pass
        else:
            pos_args.append(arg)
        i += 1

    # If no patterns, list ALL accounts with OTPs
    if not pos_args:
        print("All Accounts:", file=sys.stderr)
        for idx, acc in enumerate(accounts, 1):
            totp = pyotp.TOTP(acc['secret'], digits=acc.get('digits', 6))
            issuer_str = f"[{acc['issuer']}] " if acc.get('issuer') else ""
            print(f"{idx:2}) {totp.now()} - {issuer_str}{acc['name']}", file=sys.stderr)
        sys.exit(0)

    # Check if last arg is an index
    if pos_args[-1].isdigit() and len(pos_args) > 1:
        index_selection = int(pos_args.pop())
        patterns = pos_args
    else:
        patterns = pos_args

    # Execute Search
    results = search_accounts(accounts, patterns)

    if not results:
        print("No matching accounts found.", file=sys.stderr)
        sys.exit(1)

    if len(results) == 1:
        target = results[0]
    elif index_selection is not None:
        if 1 <= index_selection <= len(results):
            target = results[index_selection - 1]
        else:
            print(f"Error: Index {index_selection} out of range (1-{len(results)}).", file=sys.stderr)
            sys.exit(2)
    else:
        # Ambiguous results
        print(f"Ambiguous results ({len(results)} matches):", file=sys.stderr)
        for idx, acc in enumerate(results, 1):
            totp = pyotp.TOTP(acc['secret'], digits=acc.get('digits', 6))
            issuer_str = f"[{acc['issuer']}] " if acc.get('issuer') else ""
            print(f"{idx:2}) {totp.now()} - {issuer_str}{acc['name']}", file=sys.stderr)
        sys.exit(2)

    # Generate OTP
    totp = pyotp.TOTP(target['secret'], digits=target.get('digits', 6))
    otp_code = totp.now()

    # Output
    if to_stdout:
        print(otp_code)
    else:
        pyperclip.copy(otp_code)
        issuer_label = f" ({target['issuer']})" if target.get('issuer') else ""
        print(f"Copied OTP for {target['name']}{issuer_label}", file=sys.stderr)
    
    sys.exit(0)

if __name__ == "__main__":
    main()
