#!/usr/bin/env python3
import base64
import os
import sys
import json
from urllib.parse import quote

def generate_mock_totp():
    """Generates an RFC 6238 compliant 160-bit random secret."""
    # 160-bit (20 bytes) is recommended for TOTP
    raw_secret = os.urandom(20)
    # Base32 encoding
    secret_b32 = base64.b32encode(raw_secret).decode('utf-8').replace('=', '')
    
    # Simulate user-friendly format (lowercase with spaces every 4 chars)
    formatted_secret = ' '.join(secret_b32[i:i+4].lower() for i in range(0, len(secret_b32), 4))
    
    name = "alice@example.com"
    issuer = "MockService"
    
    uri = f"otpauth://totp/{issuer}:{name}?secret={secret_b32}&issuer={issuer}&digits=6&algorithm=SHA1"
    
    data = {
        "name": name,
        "issuer": issuer,
        "secret": secret_b32,
        "formatted_secret": formatted_secret,
        "uri": uri,
        "cli_command": f"jkim add '{name}' '{issuer}' --secret '{formatted_secret}'"
    }
    
    print(f"--- Mock TOTP Data ---")
    print(f"Name:   {name}")
    print(f"Issuer: {issuer}")
    print(f"Secret: {formatted_secret} (Raw: {secret_b32})")
    print(f"URI:    {uri}")
    print(f"Command: {data['cli_command']}")
    print("-" * 22)
    
    return data

if __name__ == "__main__":
    generate_mock_totp()
    # Physic verification
    sys.exit(0)
