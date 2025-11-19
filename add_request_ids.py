#!/usr/bin/env python3
"""
Add request_id: Option<u64> to all ClientMessage and ServerMessage variants.
Preserves exact formatting and only modifies protocol definitions.
"""

import re

def add_request_id_to_enum(content, enum_name):
    """Add request_id field to all variants in an enum."""

    # Find the enum block
    enum_pattern = rf'(pub enum {enum_name} \{{.*?^\}})'
    match = re.search(enum_pattern, content, re.MULTILINE | re.DOTALL)

    if not match:
        print(f"ERROR: Could not find enum {enum_name}")
        return content

    enum_block = match.group(1)
    original_enum = enum_block

    # Split into variants
    lines = enum_block.split('\n')
    result_lines = []
    in_variant = False
    variant_lines = []

    for line in lines:
        # Start of enum
        if re.match(r'^pub enum ', line):
            result_lines.append(line)
            continue

        # End of enum
        if line.strip() == '}':
            result_lines.append(line)
            continue

        # Start of a variant (uppercase letter at start after whitespace)
        if re.match(r'^\s{4}[A-Z]', line):
            # Process previous variant if any
            if variant_lines:
                result_lines.extend(process_variant(variant_lines))
                variant_lines = []

            in_variant = True
            variant_lines = [line]
            continue

        # Inside a variant
        if in_variant:
            variant_lines.append(line)

    # Process last variant
    if variant_lines:
        result_lines.extend(process_variant(variant_lines))

    new_enum = '\n'.join(result_lines)
    return content.replace(original_enum, new_enum)

def process_variant(lines):
    """Add request_id to a single variant."""
    if not lines:
        return lines

    # Check if request_id already exists
    if any('request_id' in line for line in lines):
        return lines  # Already has request_id

    # Find the closing brace
    result = []
    for i, line in enumerate(lines):
        if line.strip() in ('},', '}'):
            # Insert request_id before closing brace
            indent = '        '  # 8 spaces (variant indent + field indent)
            result.append(f'{indent}#[serde(skip_serializing_if = "Option::is_none")]')
            result.append(f'{indent}request_id: Option<u64>,')
            result.append(line)
        else:
            result.append(line)

    return result

def main():
    filepath = 'tcp_protocol/src/lib.rs'

    # Read file
    with open(filepath, 'r') as f:
        content = f.read()

    # Backup
    with open(f'{filepath}.backup', 'w') as f:
        f.write(content)

    # Add request_id to ClientMessage
    print("Adding request_id to ClientMessage variants...")
    content = add_request_id_to_enum(content, 'ClientMessage')

    # Add request_id to ServerMessage (responses only, not broadcasts)
    print("Adding request_id to ServerMessage variants...")
    content = add_request_id_to_enum(content, 'ServerMessage')

    # Write back
    with open(filepath, 'w') as f:
        f.write(content)

    print(f"✅ Updated {filepath}")
    print(f"📋 Backup saved to {filepath}.backup")

if __name__ == '__main__':
    main()
