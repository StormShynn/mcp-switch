from pathlib import Path
p = Path('.github/workflows/release.yml')
c = p.read_text(encoding='utf-8')

target = '          # Upload update.json first\n          echo \"Uploading update.json...\"\n          retry gh release upload \"\" update.json --clobber\n          sleep 3\n\n'
print('target found?', target in c)
print('first occurrence index:', c.find('Uploading update.json'))
