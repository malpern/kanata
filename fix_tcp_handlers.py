#!/usr/bin/env python3
"""
Update TCP server handlers to extract and echo request_id.
"""

import re

def main():
    filepath = 'src/tcp_server.rs'

    with open(filepath, 'r') as f:
        content = f.read()

    # Backup
    with open(f'{filepath}.backup_handlers', 'w') as f:
        f.write(content)

    # Pattern 1: Extract request_id in match arms
    replacements = [
        (r'ClientMessage::Status \{ \.\. \}', 'ClientMessage::Status { request_id, .. }'),
        (r'ClientMessage::Validate \{ config, \.\. \}', 'ClientMessage::Validate { config, request_id, .. }'),
        (r'ClientMessage::Subscribe \{ events, \.\. \}', 'ClientMessage::Subscribe { events, request_id, .. }'),
        # Reload variants - be careful with these as they appear in multiple places
        (r'ClientMessage::Reload \{ (wait,) \.\. \}', r'ClientMessage::Reload { request_id, \1 .. }'),
        (r'ClientMessage::Reload \{ \.\. \}', 'ClientMessage::Reload { request_id, .. }'),
        (r'ClientMessage::ReloadNext \{ (wait,) \.\. \}', r'ClientMessage::ReloadNext { request_id, \1 .. }'),
        (r'ClientMessage::ReloadNext \{ \.\. \}', 'ClientMessage::ReloadNext { request_id, .. }'),
        (r'ClientMessage::ReloadPrev \{ (wait,) \.\. \}', r'ClientMessage::ReloadPrev { request_id, \1 .. }'),
        (r'ClientMessage::ReloadPrev \{ \.\. \}', 'ClientMessage::ReloadPrev { request_id, .. }'),
        (r'ClientMessage::ReloadNum \{ index, \.\. \}', 'ClientMessage::ReloadNum { index, request_id, .. }'),
        (r'ClientMessage::ReloadFile \{ path, \.\. \}', 'ClientMessage::ReloadFile { path, request_id, .. }'),
    ]

    for pattern, replacement in replacements:
        content = re.sub(pattern, replacement, content)
        print(f"✅ Updated pattern: {pattern[:50]}...")

    # Pattern 2: Add request_id to ServerMessage constructions
    # Find and replace specific ServerMessage::Xyz { constructions

    # StatusInfo
    content = re.sub(
        r'(let msg = ServerMessage::StatusInfo \{)\n(\s+engine_version)',
        r'\1\n                                                    request_id,\n\2',
        content
    )

    # ReloadResult
    content = re.sub(
        r'(ServerMessage::ReloadResult \{)\n(\s+ready)',
        r'\1\n                                                        request_id,\n\2',
        content,
        flags=re.MULTILINE
    )

    # ValidationResult
    content = re.sub(
        r'(ServerMessage::ValidationResult \{)\n(\s+warnings)',
        r'\1\n                                                    request_id,\n\2',
        content
    )

    print("✅ Updated ServerMessage constructions")

    # Write back
    with open(filepath, 'w') as f:
        f.write(content)

    print(f"✅ Updated {filepath}")
    print(f"📋 Backup saved to {filepath}.backup_handlers")

if __name__ == '__main__':
    main()
