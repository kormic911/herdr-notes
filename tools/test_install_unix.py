"""Test install.sh with fake downloads; no Rust, network, or real state."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='notes-install-') as temp:
    base = Path(temp)
    (base / 'scripts').mkdir()
    shutil.copyfile(root / 'scripts/install.sh', base / 'scripts/install.sh')
    shutil.copyfile(root / 'herdr-plugin.toml', base / 'herdr-plugin.toml')
    mocks = base / 'mocks'
    mocks.mkdir()
    def mock(name, text):
        path = mocks / name
        path.write_text('#!/usr/bin/env bash\nset -eu\n' + text)
        path.chmod(0o755)
    mock('cargo', 'echo "Unexpected Cargo invocation" >&2; exit 99\n')
    mock('uname', 'if [[ "$1" == -s ]]; then echo "$TEST_OS"; else echo "$TEST_ARCH"; fi\n')
    digest = hashlib.sha256(b'fixture binary').hexdigest()
    mock('curl', '''url= out=
while (( $# )); do
  case "$1" in
    https:*) url="$1" ;;
    -o) shift; out="$1" ;;
  esac
  shift
done
[[ "$url" == https://github.com/kormic911/herdr-notes/releases/download/v*/*"$TEST_TARGET"* ]]
if [[ "$url" == *.sha256 ]]; then
  printf '%s\n' "$TEST_DIGEST" > "$out"
else
  printf 'fixture binary' > "$out"
fi
''')
    env = os.environ | {'PATH': str(mocks) + os.pathsep + os.environ['PATH'],
                        'TEST_DIGEST': digest}
    env.pop('HERDR_NOTES_BUILD_FROM_SOURCE', None)
    for system, arch, target in [
        ('Linux', 'x86_64', 'x86_64-unknown-linux-musl'),
        ('Linux', 'aarch64', 'aarch64-unknown-linux-musl'),
        ('Darwin', 'x86_64', 'x86_64-apple-darwin'),
        ('Darwin', 'arm64', 'aarch64-apple-darwin'),
    ]:
        env.update(TEST_OS=system, TEST_ARCH=arch, TEST_TARGET=target)
        subprocess.run(['bash', str(base / 'scripts/install.sh')], env=env, check=True)
        installed = base / 'target/release/herdr-notes'
        assert installed.read_bytes() == b'fixture binary'
        assert os.access(installed, os.X_OK)
    installed.write_text('keep existing binary')
    env['TEST_DIGEST'] = '0' * 64
    result = subprocess.run(['bash', str(base / 'scripts/install.sh')], env=env, capture_output=True, text=True)
    assert result.returncode != 0 and 'checksum mismatch' in result.stderr
    assert installed.read_text() == 'keep existing binary'
    env['TEST_ARCH'] = 'unsupported'
    result = subprocess.run(['bash', str(base / 'scripts/install.sh')], env=env, capture_output=True, text=True)
    assert result.returncode != 0 and 'Unsupported platform' in result.stderr
    assert not list((base / 'target/release').glob('.download-*'))
print('PASS: platform selection, verified install without Cargo, corrupt and unsupported downloads rejected')
