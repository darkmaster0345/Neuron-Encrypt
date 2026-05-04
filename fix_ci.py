import re

with open('.github/workflows/release.yml', 'r') as f:
    content = f.read()

# Add verification step before "Build installer"
verification_step = """
      - name: Verify assets/icon.ico exists
        shell: bash
        run: |
          if [ ! -f neuron-encrypt/assets/icon.ico ]; then
            echo \"Error: assets/icon.ico is missing. The installer requires this icon.\"
            false
          fi
"""

# Insert it before - name: Build installer
content = content.replace('- name: Build installer', verification_step + '      - name: Build installer')

with open('.github/workflows/release.yml', 'w') as f:
    f.write(content)
