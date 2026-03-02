# /// script
# dependencies = [
#   "urllib3",
# ]
# ///
import zipfile
import json
import urllib.parse
import sys
import os

def parse_otpauth(uri):
    if not uri.startswith("otpauth://"):
        return None
    
    parsed = urllib.parse.urlparse(uri)
    path = parsed.path.strip("/")
    
    # WinAuth Label format: Issuer:Name or just Name
    if ":" in path:
        issuer, name = path.split(":", 1)
    else:
        issuer, name = None, path
    
    query = urllib.parse.parse_qs(parsed.query)
    
    # Handle WinAuth specific unescaping and URL decoding
    name = urllib.parse.unquote(name).replace("+", " ")
    
    secret = query.get("secret", [None])[0]
    if not secret:
        return None
        
    digits = int(query.get("digits", [6])[0])
    issuer_query = query.get("issuer", [None])[0]
    
    # Determine AccountType
    effective_issuer = (issuer or issuer_query or "").lower()
    account_type = "Standard"
    if "steam" in effective_issuer:
        account_type = "Steam"
    elif "battle" in effective_issuer or "blizzard" in effective_issuer:
        account_type = "Blizzard"
        
    return {
        "name": name,
        "issuer": issuer or issuer_query,
        "secret": secret,
        "digits": digits,
        "algorithm": "SHA1",
        "account_type": account_type
    }

def main():
    txt_path = "data/private/winauth-2026-02-19.txt"
    output_path = "data/private/vault.json"
    
    if not os.path.exists(txt_path):
        print(f"Error: Text file not found at {txt_path}")
        return

    accounts = []
    with open(txt_path, 'r', encoding='utf-8') as f:
        for line in f:
            line_str = line.strip()
            if line_str.startswith("otpauth://"):
                acc = parse_otpauth(line_str)
                if acc:
                    accounts.append(acc)
    
    vault = {
        "accounts": accounts,
        "version": 1
    }
    
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(vault, f, indent=4, ensure_ascii=False)
        
    print(f"Successfully imported {len(accounts)} accounts to {output_path}")

if __name__ == "__main__":
    main()
